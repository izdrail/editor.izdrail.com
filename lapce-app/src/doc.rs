use std::{
    borrow::Cow,
    cell::RefCell,
    cmp::Ordering,
    collections::HashMap,
    ops::Range,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc,
        atomic::{self, AtomicUsize},
    },
    time::Duration,
};

use floem::{
    ViewId,
    action::exec_after,
    ext_event::create_ext_action,
    keyboard::Modifiers,
    peniko::Color,
    reactive::{
        ReadSignal, RwSignal, Scope, SignalGet, SignalUpdate, SignalWith, batch,
    },
    text::{Attrs, AttrsList, FamilyOwned, TextLayout},
    views::editor::{
        CursorInfo, Editor, EditorStyle,
        actions::CommonAction,
        command::{Command, CommandExecuted},
        id::EditorId,
        layout::{LineExtraStyle, TextLayoutLine},
        phantom_text::{PhantomText, PhantomTextKind, PhantomTextLine},
        text::{Document, DocumentPhantom, PreeditData, Styling, SystemClipboard},
        view::{ScreenLines, ScreenLinesBase},
        visual_line::HiddenLines,
    },
};
use itertools::Itertools;
use lapce_core::{
    buffer::{
        Buffer, InvalLines,
        diff::{DiffLines, rope_diff},
        rope_text::RopeText,
    },
    char_buffer::CharBuffer,
    command::EditCommand,
    cursor::{Cursor, CursorAffinity},
    editor::{Action, EditConf, EditType},
    folding::FoldState,
    indent::IndentStyle,
    language::LapceLanguage,
    line_ending::LineEnding,
    mode::MotionMode,
    register::Register,
    rope_text_pos::RopeTextPosition,
    selection::{InsertDrift, Selection},
    style::line_styles,
    syntax::{BracketParser, Syntax, edit::SyntaxEdit},
    word::{CharClassification, WordCursor, get_char_property},
};
use lapce_rpc::{
    buffer::BufferId,
    plugin::PluginId,
    proxy::ProxyResponse,
    style::{LineStyle, LineStyles, Style},
};
use lapce_xi_rope::{
    Interval, Rope, RopeDelta, Transformer,
    spans::{Spans, SpansBuilder},
};
use lsp_types::{
    CodeActionOrCommand, CodeLens, Diagnostic, DiagnosticSeverity,
    DocumentSymbolResponse, InlayHint, InlayHintLabel, TextEdit,
};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::{
    command::{CommandKind, LapceCommand},
    config::{LapceConfig, color::LapceColor},
    editor::{EditorData, EditorViewKind, compute_screen_lines},
    find::{Find, FindProgress, FindResult},
    history::DocumentHistory,
    keypress::KeyPressFocus,
    main_split::Editors,
    panel::{
        document_symbol::{SymbolData, SymbolInformationItemData},
        kind::PanelKind,
    },
    window_tab::{CommonData, Focus},
    workspace::LapceWorkspace,
};

#[derive(Clone, Debug)]
pub struct DiagnosticData {
    pub expanded: RwSignal<bool>,
    pub diagnostics: RwSignal<im::Vector<Diagnostic>>,
    pub diagnostics_span: RwSignal<Spans<Diagnostic>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorDiagnostic {
    pub range: Option<(usize, usize)>,
    pub diagnostic: Diagnostic,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct DocHistory {
    pub path: PathBuf,
    pub version: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DocContent {
    /// A file at some location. This can be a remote path.
    File { path: PathBuf, read_only: bool },
    /// A local document, which doesn't need to be sync to the disk.
    Local,
    /// A document of an old version in the source control
    History(DocHistory),
    /// A new file which doesn't exist in the file system
    Scratch {
        id: BufferId,
        name: String,
        /// Read-only scratch views (git blame) reject edits and show
        /// absolute gutter line numbers instead of modal-relative ones.
        #[serde(default)]
        read_only: bool,
    },
}

impl DocContent {
    pub fn is_local(&self) -> bool {
        matches!(self, DocContent::Local)
    }

    pub fn is_file(&self) -> bool {
        matches!(self, DocContent::File { .. })
    }

    pub fn read_only(&self) -> bool {
        match self {
            DocContent::File { read_only, .. } => *read_only,
            DocContent::Local => false,
            DocContent::History(_) => true,
            DocContent::Scratch { read_only, .. } => *read_only,
        }
    }

    pub fn path(&self) -> Option<&PathBuf> {
        match self {
            DocContent::File { path, .. } => Some(path),
            DocContent::Local => None,
            DocContent::History(_) => None,
            DocContent::Scratch { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocInfo {
    pub workspace: LapceWorkspace,
    pub path: PathBuf,
    pub scroll_offset: (f64, f64),
    pub cursor_offset: usize,
}

/// (Offset -> (Plugin the code actions are from, Code Actions))
pub type CodeActions =
    im::HashMap<usize, (PluginId, im::Vector<CodeActionOrCommand>)>;

pub type AllCodeLens = im::HashMap<usize, (PluginId, usize, im::Vector<CodeLens>)>;

#[derive(Clone, Debug)]
enum DocumentHighlightsStatus {
    Ready,
    Busy(usize),
}

pub struct DocumentHighlight {
    pub start: usize,
    pub end: usize,
    // lsp_type::DocumentHighlight has kind, but it's not used now
}

impl From<&DocumentHighlight> for lapce_core::selection::SelRegion {
    fn from(value: &DocumentHighlight) -> Self {
        Self {
            start: value.start,
            end: value.end,
            horiz: None,
        }
    }
}

impl DocumentHighlight {
    pub fn find_for_offset(
        list: &[DocumentHighlight],
        offset: usize,
    ) -> Option<usize> {
        list.binary_search_by(|highlight| {
            if highlight.start > offset {
                Ordering::Greater
            } else if highlight.end < offset {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        })
        .ok()
    }

    pub fn sort(list: &mut [DocumentHighlight]) {
        list.sort_by(|lhs, rhs| lhs.start.cmp(&rhs.start));
    }
}

#[derive(Clone)]
pub struct Doc {
    pub scope: Scope,
    pub buffer_id: BufferId,
    pub content: RwSignal<DocContent>,
    pub cache_rev: RwSignal<u64>,
    /// Whether the buffer's content has been loaded/initialized into the buffer.
    pub loaded: RwSignal<bool>,
    pub(crate) projection: RwSignal<Option<crate::multibuffer::ProjectionBinding>>,
    pub buffer: RwSignal<Buffer>,
    pub syntax: RwSignal<Syntax>,
    semantic_styles: RwSignal<Option<Spans<Style>>>,
    /// Inlay hints for the document
    pub inlay_hints: RwSignal<Option<Spans<InlayHint>>>,
    /// Current completion lens text, if any.
    /// This will be displayed even on views that are not focused.
    pub completion_lens: RwSignal<Option<String>>,
    /// (line, col)
    pub completion_pos: RwSignal<(usize, usize)>,

    /// Current inline completion text, if any.
    /// This will be displayed even on views that are not focused.
    pub inline_completion: RwSignal<Option<String>>,
    /// (line, col)
    pub inline_completion_pos: RwSignal<(usize, usize)>,

    /// (Offset -> (Plugin the code actions are from, Code Actions))
    pub code_actions: RwSignal<CodeActions>,

    pub code_lens: RwSignal<AllCodeLens>,

    /// Foldable blocks and which of them are folded. Shared by every editor of this document,
    /// except diff views, which always show every line.
    pub fold_state: RwSignal<FoldState>,

    /// Stores information about different versions of the document from source control.
    histories: RwSignal<im::HashMap<String, DocumentHistory>>,
    pub head_changes: RwSignal<im::Vector<DiffLines>>,

    line_styles: Rc<RefCell<LineStyles>>,
    pub parser: Rc<RefCell<BracketParser>>,

    /// A cache for the sticky headers which maps a line to the lines it should show in the header.
    pub sticky_headers: Rc<RefCell<HashMap<usize, Option<Vec<usize>>>>>,

    pub preedit: PreeditData,

    pub find_result: FindResult,

    /// The diagnostics for the document
    pub diagnostics: DiagnosticData,

    editors: Editors,
    pub common: Rc<CommonData>,

    pub document_symbol_data: RwSignal<Option<SymbolData>>,

    get_highlights_status: RwSignal<DocumentHighlightsStatus>,
    pub highlights: RwSignal<Option<Vec<DocumentHighlight>>>,
}

impl Doc {
    pub fn new(
        cx: Scope,
        path: PathBuf,
        diagnostics: DiagnosticData,
        editors: Editors,
        common: Rc<CommonData>,
    ) -> Self {
        let syntax = Syntax::init(&path);
        let config = common.config.get_untracked();
        Doc {
            scope: cx,
            buffer_id: BufferId::next(),
            projection: cx.create_rw_signal(None),
            buffer: cx.create_rw_signal(Buffer::new("")),
            syntax: cx.create_rw_signal(syntax),
            line_styles: Rc::new(RefCell::new(HashMap::new())),
            parser: Rc::new(RefCell::new(BracketParser::new(
                String::new(),
                config.editor.bracket_pair_colorization,
                config.editor.bracket_colorization_limit,
            ))),
            semantic_styles: cx.create_rw_signal(None),
            inlay_hints: cx.create_rw_signal(None),
            diagnostics,
            completion_lens: cx.create_rw_signal(None),
            completion_pos: cx.create_rw_signal((0, 0)),
            inline_completion: cx.create_rw_signal(None),
            inline_completion_pos: cx.create_rw_signal((0, 0)),
            cache_rev: cx.create_rw_signal(0),
            content: cx.create_rw_signal(DocContent::File {
                path,
                read_only: false,
            }),
            loaded: cx.create_rw_signal(false),
            histories: cx.create_rw_signal(im::HashMap::new()),
            head_changes: cx.create_rw_signal(im::Vector::new()),
            sticky_headers: Rc::new(RefCell::new(HashMap::new())),
            code_actions: cx.create_rw_signal(im::HashMap::new()),
            find_result: FindResult::new(cx),
            preedit: PreeditData::new(cx),
            editors,
            common,
            code_lens: cx.create_rw_signal(im::HashMap::new()),
            document_symbol_data: cx.create_rw_signal(None),
            fold_state: cx.create_rw_signal(FoldState::default()),
            get_highlights_status: cx
                .create_rw_signal(DocumentHighlightsStatus::Ready),
            highlights: cx.create_rw_signal(None),
        }
    }

    pub fn new_local(cx: Scope, editors: Editors, common: Rc<CommonData>) -> Doc {
        Self::new_content(cx, DocContent::Local, editors, common)
    }

    pub fn new_content(
        cx: Scope,
        content: DocContent,
        editors: Editors,
        common: Rc<CommonData>,
    ) -> Doc {
        let cx = cx.create_child();
        let config = common.config.get_untracked();
        Self {
            scope: cx,
            buffer_id: BufferId::next(),
            projection: cx.create_rw_signal(None),
            buffer: cx.create_rw_signal(Buffer::new("")),
            syntax: cx.create_rw_signal(Syntax::plaintext()),
            line_styles: Rc::new(RefCell::new(HashMap::new())),
            parser: Rc::new(RefCell::new(BracketParser::new(
                String::new(),
                config.editor.bracket_pair_colorization,
                config.editor.bracket_colorization_limit,
            ))),
            semantic_styles: cx.create_rw_signal(None),
            inlay_hints: cx.create_rw_signal(None),
            diagnostics: DiagnosticData {
                expanded: cx.create_rw_signal(true),
                diagnostics: cx.create_rw_signal(im::Vector::new()),
                diagnostics_span: cx.create_rw_signal(SpansBuilder::new(0).build()),
            },
            completion_lens: cx.create_rw_signal(None),
            completion_pos: cx.create_rw_signal((0, 0)),
            inline_completion: cx.create_rw_signal(None),
            inline_completion_pos: cx.create_rw_signal((0, 0)),
            cache_rev: cx.create_rw_signal(0),
            content: cx.create_rw_signal(content),
            histories: cx.create_rw_signal(im::HashMap::new()),
            head_changes: cx.create_rw_signal(im::Vector::new()),
            sticky_headers: Rc::new(RefCell::new(HashMap::new())),
            loaded: cx.create_rw_signal(true),
            find_result: FindResult::new(cx),
            code_actions: cx.create_rw_signal(im::HashMap::new()),
            preedit: PreeditData::new(cx),
            editors,
            common,
            code_lens: cx.create_rw_signal(im::HashMap::new()),
            document_symbol_data: cx.create_rw_signal(None),
            fold_state: cx.create_rw_signal(FoldState::default()),
            get_highlights_status: cx
                .create_rw_signal(DocumentHighlightsStatus::Ready),
            highlights: cx.create_rw_signal(None),
        }
    }

    pub fn new_history(
        cx: Scope,
        content: DocContent,
        editors: Editors,
        common: Rc<CommonData>,
    ) -> Doc {
        let config = common.config.get_untracked();
        let syntax = if let DocContent::History(history) = &content {
            Syntax::init(&history.path)
        } else {
            Syntax::plaintext()
        };
        Self {
            scope: cx,
            buffer_id: BufferId::next(),
            projection: cx.create_rw_signal(None),
            buffer: cx.create_rw_signal(Buffer::new("")),
            syntax: cx.create_rw_signal(syntax),
            line_styles: Rc::new(RefCell::new(HashMap::new())),
            parser: Rc::new(RefCell::new(BracketParser::new(
                String::new(),
                config.editor.bracket_pair_colorization,
                config.editor.bracket_colorization_limit,
            ))),
            semantic_styles: cx.create_rw_signal(None),
            inlay_hints: cx.create_rw_signal(None),
            diagnostics: DiagnosticData {
                expanded: cx.create_rw_signal(true),
                diagnostics: cx.create_rw_signal(im::Vector::new()),
                diagnostics_span: cx.create_rw_signal(SpansBuilder::new(0).build()),
            },
            completion_lens: cx.create_rw_signal(None),
            completion_pos: cx.create_rw_signal((0, 0)),
            inline_completion: cx.create_rw_signal(None),
            inline_completion_pos: cx.create_rw_signal((0, 0)),
            cache_rev: cx.create_rw_signal(0),
            content: cx.create_rw_signal(content),
            sticky_headers: Rc::new(RefCell::new(HashMap::new())),
            loaded: cx.create_rw_signal(true),
            histories: cx.create_rw_signal(im::HashMap::new()),
            head_changes: cx.create_rw_signal(im::Vector::new()),
            code_actions: cx.create_rw_signal(im::HashMap::new()),
            find_result: FindResult::new(cx),
            preedit: PreeditData::new(cx),
            editors,
            common,
            code_lens: cx.create_rw_signal(im::HashMap::new()),
            document_symbol_data: cx.create_rw_signal(None),
            fold_state: cx.create_rw_signal(FoldState::default()),
            get_highlights_status: cx
                .create_rw_signal(DocumentHighlightsStatus::Ready),
            highlights: cx.create_rw_signal(None),
        }
    }

    /// Create a styling instance for this doc
    pub fn styling(self: &Rc<Doc>) -> Rc<DocStyling> {
        Rc::new(DocStyling {
            config: self.common.config,
            doc: self.clone(),
        })
    }

    /// Create an [`Editor`] instance from this [`Doc`]. Note that this needs to be registered
    /// appropriately to create the [`EditorData`] and such.
    pub fn create_editor(
        self: &Rc<Doc>,
        cx: Scope,
        id: EditorId,
        is_local: bool,
    ) -> Editor {
        let common = &self.common;
        let config = common.config.get_untracked();
        let modal = config.core.modal && !is_local;

        let register = common.register;
        // TODO: we could have these Rcs created once and stored somewhere, maybe on
        // common, to avoid recreating them everytime.
        let cursor_info = CursorInfo {
            blink_interval: Rc::new(move || config.editor.blink_interval()),
            blink_timer: common.window_common.cursor_blink_timer,
            hidden: common.window_common.hide_cursor,
            should_blink: Rc::new(should_blink(common.focus, common.keyboard_focus)),
        };
        let mut editor =
            Editor::new_direct(cx, id, self.clone(), self.styling(), modal);

        editor.register = register;
        editor.cursor_info = cursor_info;
        editor.ime_allowed = common.window_common.ime_allowed;

        editor.recreate_view_effects();

        editor
    }

    fn editor_data(&self, id: EditorId) -> Option<EditorData> {
        self.editors.editor_untracked(id)
    }

    pub fn syntax(&self) -> ReadSignal<Syntax> {
        self.syntax.read_only()
    }

    pub fn set_syntax(&self, syntax: Syntax) {
        batch(|| {
            self.syntax.set(syntax);
            if self.semantic_styles.with_untracked(|s| s.is_none()) {
                self.clear_style_cache();
            }
            self.clear_sticky_headers_cache();
        });
    }

    /// Set the syntax highlighting this document should use.
    pub fn set_language(&self, language: LapceLanguage) {
        self.syntax.set(Syntax::from_language(language));
    }

    pub fn find(&self) -> &Find {
        &self.common.find
    }

    /// Whether or not the underlying buffer is loaded
    pub fn loaded(&self) -> bool {
        self.loaded.get_untracked()
    }

    //// Initialize the content with some text, this marks the document as loaded.
    pub fn init_content(&self, content: Rope) {
        let editorconfig_path = if self.common.workspace.kind.is_remote() {
            None
        } else {
            self.content.with_untracked(|c| c.path().cloned())
        };
        batch(|| {
            self.syntax.with_untracked(|syntax| {
                self.buffer.update(|buffer| {
                    buffer.init_content(content);
                    buffer.detect_indent(|| {
                        IndentStyle::from_str(syntax.language.indent_unit())
                    });
                    // `.editorconfig` beats auto-detection (local files only).
                    if let Some(path) = editorconfig_path {
                        let props = crate::editorconfig::resolve(&path);
                        let current = match buffer.indent_style() {
                            IndentStyle::Tabs => crate::editorconfig::Indent::Tabs,
                            IndentStyle::Spaces(n) => {
                                crate::editorconfig::Indent::Spaces(n)
                            }
                        };
                        match props.indent(current) {
                            Some(crate::editorconfig::Indent::Tabs) => {
                                buffer.set_indent_style(IndentStyle::Tabs)
                            }
                            Some(crate::editorconfig::Indent::Spaces(n)) => {
                                buffer.set_indent_style(IndentStyle::Spaces(n))
                            }
                            None => {}
                        }
                    }
                });
            });
            self.loaded.set(true);
            self.on_update(None);
            self.init_parser();
            self.init_diagnostics();
            self.retrieve_head();
        });
    }

    fn init_parser(&self) {
        let code = self.buffer.get_untracked().to_string();
        self.syntax.with_untracked(|syntax| {
            if syntax.styles.is_some() {
                self.parser.borrow_mut().update_code(
                    code,
                    &self.buffer.get_untracked(),
                    Some(syntax),
                );
            } else {
                self.parser.borrow_mut().update_code(
                    code,
                    &self.buffer.get_untracked(),
                    None,
                );
            }
        });
    }

    /// Reload the document's content, and is what you should typically use when you want to *set*
    /// an existing document's content.
    pub fn reload(&self, content: Rope, set_pristine: bool) {
        // self.code_actions.clear();
        // self.inlay_hints = None;
        let delta = self
            .buffer
            .try_update(|buffer| buffer.reload(content, set_pristine))
            .unwrap();
        self.apply_deltas(&[delta]);
    }

    pub fn handle_file_changed(&self, content: Rope) {
        if self.is_pristine() {
            self.reload(content, true);
        }
    }



    /// Save the canonical edited source buffers, never the projection text.
    pub(crate) fn save_projection(&self, after_action:impl FnOnce()+'static)->bool {
        let Some(binding)=self.projection.get_untracked() else {return false;};
        let current=binding.docs.get_untracked();let projection=binding.projection.borrow();
        let mut dirty=std::collections::BTreeMap::new();
        for excerpt in &projection.excerpts {
            let Some(doc)=current.get(&excerpt.source) else {binding.notice.set("Source was closed; refresh diagnostics before saving".into());return true;};
            if doc.buffer_id.0!=excerpt.buffer_id || doc.rev()!=excerpt.revision || !doc.loaded() || doc.content.with_untracked(|c|c.path()!=Some(&excerpt.source)) {
                binding.notice.set("Source changed; refresh diagnostics before saving".into());return true;
            }
            if !doc.is_pristine() {
                if doc.content.with_untracked(|c|c.read_only()) {binding.notice.set("Source is read-only".into());return true;}
                dirty.insert(excerpt.source.clone(),doc.clone());
            }
        }
        drop(projection);
        if dirty.is_empty() {self.buffer.update(|b|b.set_pristine());after_action();return true;}
        let remaining=Rc::new(std::cell::Cell::new(dirty.len()));
        let finished:Rc<RefCell<Option<Box<dyn FnOnce()>>>>=Rc::new(RefCell::new(Some(Box::new(after_action))));
        binding.notice.set("Saving source buffers".into());
        let aggregate_buffer=self.buffer;
        let pending=remaining.clone();let timeout_binding=binding.clone();
        floem::action::exec_after(std::time::Duration::from_secs(30),move |_| {
            if pending.get()>0 {timeout_binding.notice.set("Source save is not confirmed. Keep these buffers open and retry Save.".into());}
        });
        for (_,doc) in dirty {
            let remaining=remaining.clone();let finished=finished.clone();let binding=binding.clone();
            doc.save(move || {
                remaining.set(remaining.get().saturating_sub(1));
                if remaining.get()==0 {
                    let current=binding.docs.get_untracked();
                    let clean=binding.projection.borrow().excerpts.iter().all(|e|current.get(&e.source).is_some_and(|d|d.buffer_id.0==e.buffer_id && d.rev()==e.revision && d.is_pristine()));
                    if !clean {binding.notice.set("Sources changed while saving; review before closing".into());return;}
                    aggregate_buffer.update(|b|b.set_pristine());
                    binding.notice.set("Source buffers saved".into());if let Some(done)=finished.borrow_mut().take(){done();}
                }
            });
        }
        true
    }
    /// Controlled projection edits stage both synthetic and native buffers.
    /// Every unhandled path still sees a read-only scratch document.
    fn edit_projection(
        &self,
        cursor: &mut Cursor,
        action: impl FnOnce(&mut Buffer,&mut Cursor)->Vec<(Rope,RopeDelta,InvalLines)>,
    )->Option<Vec<(Rope,RopeDelta,InvalLines)>> {
        let binding=self.projection.get_untracked()?;
        let mut candidate=self.buffer.with_untracked(|b|b.detached_edit_clone());
        let mut next_cursor=cursor.clone();
        let old_cursor=cursor.offset();
        let mut deltas=action(&mut candidate,&mut next_cursor);
        if crate::multibuffer::cursor_only_projection_deltas(&deltas.iter().map(|(_,delta,_)|delta.clone()).collect::<Vec<_>>()) {
            // Native closing-pair skip can return an identity edit. Keep only
            // the cursor, never its synthetic undo entry or revision clock.
            *cursor=next_cursor;return Some(Vec::new());
        }
        let sequential:Vec<_>=deltas.iter().map(|(_,delta,_)|delta.clone()).collect();
        let composed=match crate::multibuffer::compose_projection_deltas(&binding.projection.borrow(),&sequential) {
            Ok(delta)=>delta,Err(error)=>{binding.notice.set(error.into());return Some(Vec::new());}
        };
        if deltas.len()>1 {
            let staged=self.buffer.with_untracked(|live|crate::multibuffer::stage_compound_projection(live,&composed,&candidate.to_string()));
            let (buffer,delta)=match staged {Ok(value)=>value,Err(error)=>{binding.notice.set(error.into());return Some(Vec::new());}};
            candidate=buffer;candidate.set_cursor_before(cursor.mode.clone());candidate.set_cursor_after(next_cursor.mode.clone());deltas=vec![delta];
        }
        let current:std::collections::BTreeMap<_,_>=binding.docs.get_untracked().into_iter().collect();
        let prepared=crate::multibuffer::prepare_projection_edit(&binding.projection.borrow(),&current,&composed,old_cursor,4*1024*1024);
        let prepared=match prepared {Ok(value)=>value,Err(error)=>{binding.notice.set(error);return Some(Vec::new());}};
        if candidate.to_string()!=prepared.projection.text {binding.notice.set("Projection changed during edit staging".into());return Some(Vec::new());}
        batch(|| {
            crate::buffer_transaction::commit_buffers(&current,prepared.staged);
            self.buffer.with_untracked(|live|candidate.adopt_revision_clock(live));
            self.buffer.set(candidate);
            *binding.projection.borrow_mut()=prepared.projection;
            *cursor=next_cursor;
            binding.notice.set(String::new());
            self.apply_deltas(&deltas);
        });
        Some(deltas)
    }
    pub fn do_insert(
        &self,
        cursor: &mut Cursor,
        s: &str,
        config: &LapceConfig,
    ) -> Vec<(Rope, RopeDelta, InvalLines)> {
        if let Some(binding)=self.projection.get_untracked() {
            if let lapce_core::cursor::CursorMode::Insert(selection)=&cursor.mode {
                if !binding.projection.borrow().allows_insert_selection(selection) {
                    binding.notice.set("Headers and excerpt boundaries are read-only".into());
                    return Vec::new();
                }
            }
        }
        if let Some(binding)=self.projection.get_untracked() {
            let docs=binding.docs.get_untracked();
            if let Some(deltas)=self.edit_projection(cursor,|buffer,cursor|Action::insert_with_closer_indent(cursor,buffer,s,&|_,opening,display| {
                crate::multibuffer::projection_closer_indent(&binding.projection.borrow(),display,opening,|anchor,opening| {
                    let source=docs.get(&anchor.source)?;
                    if source.buffer_id.0!=anchor.buffer_id || source.rev()!=anchor.revision || !source.loaded() {return None;}
                    source.syntax.with_untracked(|syntax|source.buffer.with_untracked(|buffer| {
                        if buffer.len()>4*1024*1024 {return None;}
                        let opener=syntax_prev_unmatched(buffer,syntax,opening,anchor.offset)?;
                        Some(buffer.indent_on_line(buffer.line_of_offset(opener)))
                    }))
                })
            },config.editor.auto_closing_matching_pairs,config.editor.auto_surround)) {return deltas;}
        }
        if self.content.with_untracked(|c| c.read_only()) {
            return Vec::new();
        }

        let old_cursor = cursor.mode.clone();
        let deltas = self.syntax.with_untracked(|syntax| {
            self.buffer
                .try_update(|buffer| {
                    Action::insert(
                        cursor,
                        buffer,
                        s,
                        &|buffer, c, offset| {
                            syntax_prev_unmatched(buffer, syntax, c, offset)
                        },
                        config.editor.auto_closing_matching_pairs,
                        config.editor.auto_surround,
                    )
                })
                .unwrap()
        });
        let mut deltas = deltas;
        if s == ">" && config.editor.auto_close_tags {
            if let Some(d) = self.auto_close_tag(cursor) {
                deltas.push(d);
            }
        }
        // Keep track of the change in the cursor mode for undo/redo
        self.buffer.update(|buffer| {
            buffer.set_cursor_before(old_cursor);
            buffer.set_cursor_after(cursor.mode.clone());
        });
        self.apply_deltas(&deltas);
        deltas
    }

    /// After a typed `>`: inserts the closing tag after the caret and keeps
    /// the caret between the tags. Returns the extra edit, to be applied by
    /// the caller together with the typed character.
    fn auto_close_tag(
        &self,
        cursor: &mut Cursor,
    ) -> Option<(Rope, RopeDelta, InvalLines)> {
        let path = self.content.with_untracked(|c| c.path().cloned())?;
        if !crate::tag_close::enabled_for(&path) {
            return None;
        }
        let offset = match &cursor.mode {
            lapce_core::cursor::CursorMode::Insert(sel) if sel.regions().len() == 1 => {
                let r = &sel.regions()[0];
                if !r.is_caret() {
                    return None;
                }
                r.end
            }
            _ => return None,
        };
        let close = self.buffer.with_untracked(|b| {
            let len = b.len();
            if offset > len || len > 4 * 1024 * 1024 {
                return None;
            }
            let from = b.prev_code_boundary(offset.saturating_sub(4096));
            let to = b.next_code_boundary((offset + 256).min(len));
            let before = b.slice_to_cow(from..offset).to_string();
            let after = b.slice_to_cow(offset..to.max(offset)).to_string();
            crate::tag_close::closing_tag(&before, &after)
        })?;
        let sel = Selection::caret(offset);
        let edit = self
            .buffer
            .try_update(|b| b.edit(&[(&sel, close.as_str())], EditType::InsertChars))?;
        cursor.mode = lapce_core::cursor::CursorMode::Insert(Selection::caret(offset));
        Some(edit)
    }

    /// Completion uses the same staged source transaction as native typing.
    /// Keep the scratch read-only for callers without an explicit cursor.
    pub(crate) fn do_completion_raw_edit(
        &self,
        cursor: &Cursor,
        edits: &[(impl AsRef<Selection>, &str)],
    ) -> Option<(Rope, RopeDelta, InvalLines)> {
        if self.projection.get_untracked().is_none() {
            return self.do_raw_edit(edits, EditType::Completion);
        }
        let mut cursor = cursor.clone();
        self.edit_projection(&mut cursor, |buffer, _| {
            vec![buffer.edit(edits, EditType::Completion)]
        })?.into_iter().next()
    }

    pub fn do_raw_edit(
        &self,
        edits: &[(impl AsRef<Selection>, &str)],
        edit_type: EditType,
    ) -> Option<(Rope, RopeDelta, InvalLines)> {
        if self.content.with_untracked(|c| c.read_only()) {
            return None;
        }

        let (text, delta, inval_lines) = self
            .buffer
            .try_update(|buffer| buffer.edit(edits, edit_type))
            .unwrap();
        self.apply_deltas(&[(text.clone(), delta.clone(), inval_lines.clone())]);
        Some((text, delta, inval_lines))
    }

    pub fn do_edit(
        &self,
        cursor: &mut Cursor,
        cmd: &EditCommand,
        modal: bool,
        register: &mut Register,
        smart_tab: bool,
    ) -> Vec<(Rope, RopeDelta, InvalLines)> {
        let mut source_indent=None;
        if matches!(cmd,EditCommand::InsertNewLine|EditCommand::InsertTab) {
            if let Some(binding)=self.projection.get_untracked() {
                let selection=self.buffer.with_untracked(|buffer|cursor.edit_selection(buffer));
                let docs=binding.docs.get_untracked();
                let style=crate::multibuffer::projection_indent_style(&binding.projection.borrow(),&selection,|excerpt| {
                    let doc=docs.get(&excerpt.source).ok_or("Source was closed")?;
                    if doc.buffer_id.0!=excerpt.buffer_id || doc.rev()!=excerpt.revision {return Err("Source changed; refresh before editing");}
                    Ok(doc.buffer.with_untracked(|buffer|buffer.indent_style()))
                });
                match style {Ok(style)=>source_indent=Some(style),Err(error)=>{binding.notice.set(error.into());return Vec::new();}}
            }
        }
        if matches!(cmd,EditCommand::InsertTab) && self.projection.get_untracked().is_some() {
            let caret=match &cursor.mode {
                lapce_core::cursor::CursorMode::Insert(selection) if selection.regions().len()==1 && selection.regions()[0].is_caret()=>Some(selection.regions()[0].end),
                _=>None,
            };
            if let Some(display)=caret {
                if let Some((source,offset))=self.projection_navigation_source(display) {
                    if source.syntax.with_untracked(|syntax|matches!(syntax.language,LapceLanguage::Php)) {
                        let expansion=source.buffer.with_untracked(|buffer| {
                            if buffer.len()>4*1024*1024 {return None;}
                            crate::php_docblock::expansion(&buffer.to_string(),offset)
                        });
                        if let Some(expansion)=expansion {
                            if let Some(deltas)=self.edit_projection(cursor,|buffer,cursor| {
                                let selection=Selection::caret(display);
                                let delta=buffer.edit(&[(&selection,expansion.insert.as_str())],EditType::Other);
                                buffer.set_cursor_before(cursor.mode.clone());
                                cursor.set_insert(Selection::caret(display+expansion.caret));
                                buffer.set_cursor_after(cursor.mode.clone());
                                vec![delta]
                            }) {return deltas;}
                        }
                    }
                }
            }
        }
        let mut source_comments=None;
        if matches!(cmd,EditCommand::ToggleLineComment) {
            if let Some(binding)=self.projection.get_untracked() {
                let selection=self.buffer.with_untracked(|buffer|cursor.edit_selection(buffer));
                if !binding.projection.borrow().allows_insert_selection(&selection) {
                    binding.notice.set("Headers and excerpt boundaries are read-only".into());return Vec::new();
                }
                let mut resolvers=std::collections::BTreeMap::new();let docs=binding.docs.get_untracked();
                for excerpt in &binding.projection.borrow().excerpts {
                    if !selection.regions().iter().any(|r|r.min()>=excerpt.display_range.start && r.max()<=excerpt.display_range.end) {continue;}
                    let Some(doc)=docs.get(&excerpt.source) else {binding.notice.set("Source was closed".into());return Vec::new();};
                    if doc.buffer_id.0!=excerpt.buffer_id || doc.rev()!=excerpt.revision {binding.notice.set("Source changed; refresh before editing".into());return Vec::new();}
                    if !resolvers.contains_key(&excerpt.source) {
                        let resolver=doc.syntax.with_untracked(|syntax|doc.buffer.with_untracked(|buffer|Self::comment_token_resolver(cmd,syntax,buffer)));
                        resolvers.insert(excerpt.source.clone(),resolver);
                    }
                }
                source_comments=Some(crate::multibuffer::projection_comment_resolver(binding.projection.borrow().clone(),resolvers));
            }
        }
        if let Some(deltas)=self.edit_projection(cursor,|buffer,cursor| {
            if let Some(style)=source_indent {buffer.set_indent_style(style);}
            let plain=Self::comment_token_resolver(cmd,&Syntax::plaintext(),buffer);
            let comments=source_comments.as_ref().map(|resolver|resolver.as_ref()).unwrap_or(plain.as_ref());
            let mut clipboard=SystemClipboard::new();
            Action::do_edit(cursor,buffer,cmd,&mut clipboard,register,EditConf {comment_tokens:comments,modal,smart_tab,keep_indent:true,auto_indent:source_indent.is_some()})
        }) {return deltas;}
        if self.content.with_untracked(|c| c.read_only())
            && !cmd.not_changing_buffer()
        {
            return Vec::new();
        }

        let mut clipboard = SystemClipboard::new();
        let old_cursor = cursor.mode.clone();
        let docblock = if *cmd == EditCommand::InsertTab {
            self.php_docblock_tab(cursor)
        } else { None };
        let deltas = if let Some(delta) = docblock { vec![delta] } else { self.syntax.with_untracked(|syntax| {
            self.buffer
                .try_update(|buffer| {
                    let comment_tokens = Self::comment_token_resolver(cmd, syntax, buffer);
                    Action::do_edit(
                        cursor,
                        buffer,
                        cmd,
                        &mut clipboard,
                        register,
                        EditConf {
                            comment_tokens: &comment_tokens,
                            modal,
                            smart_tab,
                            keep_indent: true,
                            auto_indent: true,
                        },
                    )
                })
                .unwrap()
        }) };

        let mut deltas = deltas;
        if matches!(cmd, EditCommand::InsertNewLine)
            && !deltas.is_empty()
            && self.common.config.get_untracked().editor.auto_close_tags
        {
            if let Some(d) = self.auto_end_directive(cursor) {
                deltas.push(d);
            }
        }

        if !deltas.is_empty() {
            self.buffer.update(|buffer| {
                buffer.set_cursor_before(old_cursor);
                buffer.set_cursor_after(cursor.mode.clone());
            });
            self.apply_deltas(&deltas);
        }

        deltas
    }

    /// Tab after a standalone PHP docblock opener closes it in one undo step.
    fn php_docblock_tab(&self, cursor: &mut Cursor) -> Option<(Rope, RopeDelta, InvalLines)> {
        if !self.syntax.with_untracked(|syntax| matches!(syntax.language, LapceLanguage::Php)) {
            return None;
        }
        let offset = match &cursor.mode {
            lapce_core::cursor::CursorMode::Insert(selection) if selection.regions().len() == 1 => {
                let region = &selection.regions()[0];
                if !region.is_caret() { return None; }
                region.end
            }
            _ => return None,
        };
        let expansion = self.buffer.with_untracked(|buffer| {
            if buffer.len() > 4 * 1024 * 1024 { return None; }
            crate::php_docblock::expansion(&buffer.text().to_string(), offset)
        })?;
        let selection = Selection::caret(offset);
        let delta = self.buffer.try_update(|buffer| buffer.edit(
            &[(&selection, expansion.insert.as_str())], EditType::InsertChars,
        ))?;
        cursor.mode = lapce_core::cursor::CursorMode::Insert(Selection::caret(offset + expansion.caret));
        Some(delta)
    }

    /// After Enter on a Blade block directive line: adds the matching
    /// `@end...` below the new (indented) line and keeps the caret on it.
    fn auto_end_directive(
        &self,
        cursor: &mut Cursor,
    ) -> Option<(Rope, RopeDelta, InvalLines)> {
        let path = self.content.with_untracked(|c| c.path().cloned())?;
        if !path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.to_lowercase().ends_with(".blade.php"))
        {
            return None;
        }
        let offset = match &cursor.mode {
            lapce_core::cursor::CursorMode::Insert(sel) if sel.regions().len() == 1 => {
                let r = &sel.regions()[0];
                if !r.is_caret() {
                    return None;
                }
                r.end
            }
            _ => return None,
        };
        let insert = self.buffer.with_untracked(|b| {
            let len = b.len();
            if len > 4 * 1024 * 1024 {
                return None;
            }
            let line = b.line_of_offset(offset);
            if line == 0 {
                return None;
            }
            let line_end = b.offset_of_line(line + 1).min(len);
            let line_text = b.slice_to_cow(offset..line_end).to_string();
            // Only when nothing but the line break follows the caret.
            if !line_text.trim_matches(['\r', '\n']).is_empty() {
                return None;
            }
            let prev_start = b.offset_of_line(line - 1);
            let prev_end = b.offset_of_line(line);
            let prev = b.slice_to_cow(prev_start..prev_end).to_string();
            let prev = prev.trim_end_matches(['\r', '\n']);
            let to = b.next_code_boundary((line_end + 200_000).min(len));
            let following = b.slice_to_cow(line_end..to.max(line_end)).to_string();
            let end = crate::blade_end::end_directive(prev, &following)?;
            let indent: String =
                prev.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
            Some(format!("{}{indent}{end}", b.line_ending().get_chars()))
        })?;
        let sel = Selection::caret(offset);
        let edit = self
            .buffer
            .try_update(|b| b.edit(&[(&sel, insert.as_str())], EditType::InsertChars))?;
        cursor.mode = lapce_core::cursor::CursorMode::Insert(Selection::caret(offset));
        Some(edit)
    }

    pub fn apply_deltas(&self, deltas: &[(Rope, RopeDelta, InvalLines)]) {
        let rev = self.rev() - deltas.len() as u64;
        batch(|| {
            for (i, (_, delta, inval)) in deltas.iter().enumerate() {
                self.apply_fold_edit(inval);
                self.update_styles(delta);
                self.update_inlay_hints(delta);
                self.update_diagnostics(delta);
                self.update_completion_lens(delta);
                self.update_find_result(delta);
                if let DocContent::File { path, .. } = self.content.get_untracked() {
                    self.update_breakpoints(delta, &path, &inval.old_text);
                    self.common.proxy.update(
                        path,
                        delta.clone(),
                        rev + i as u64 + 1,
                    );
                }
            }
        });

        // TODO(minor): We could avoid this potential allocation since most apply_delta callers are actually using a Vec
        // which we could reuse.
        // We use a smallvec because there is unlikely to be more than a couple of deltas
        let edits: SmallVec<[SyntaxEdit; 3]> = deltas
            .iter()
            .map(|(before_text, delta, _)| {
                SyntaxEdit::from_delta(before_text, delta.clone())
            })
            .collect();
        self.on_update(Some(edits));
    }

    pub fn is_pristine(&self) -> bool {
        if let Some(binding)=self.projection.get() {
            let current=binding.docs.get();
            return binding.projection.borrow().excerpts.iter().all(|e|current.get(&e.source).is_some_and(|doc|doc.buffer_id.0==e.buffer_id && doc.buffer.with(|b|b.is_pristine())));
        }
        self.buffer.with_untracked(|b| b.is_pristine())
    }

    /// Get the buffer's current revision. This is used to track whether the buffer has changed.
    pub fn rev(&self) -> u64 {
        self.buffer.with_untracked(|b| b.rev())
    }

    /// Get the buffer's line-ending.
    /// Note: this may not be the same as what the actual line endings in the file are, rather this
    /// is what the line-ending is set to (and what it will be saved as).
    pub fn line_ending(&self) -> LineEnding {
        self.buffer.with_untracked(|b| b.line_ending())
    }

    fn on_update(&self, edits: Option<SmallVec<[SyntaxEdit; 3]>>) {
        batch(|| {
            self.trigger_syntax_change(edits);
            self.trigger_head_change();
            self.check_auto_save();
            self.get_inlay_hints();
            self.find_result.reset();
            self.get_semantic_styles();
            self.do_bracket_colorization();
            self.clear_code_actions();
            self.clear_style_cache();
            self.get_code_lens();
            self.get_document_symbol();
            self.get_folding_range();
        });
    }

    fn do_bracket_colorization(&self) {
        if self.parser.borrow().active {
            self.syntax.with_untracked(|syntax| {
                if syntax.rev == self.rev() && syntax.styles.is_some() {
                    self.parser.borrow_mut().update_code(
                        self.buffer.get_untracked().to_string(),
                        &self.buffer.get_untracked(),
                        Some(syntax),
                    );
                } else {
                    self.parser.borrow_mut().update_code(
                        self.buffer.get_untracked().to_string(),
                        &self.buffer.get_untracked(),
                        None,
                    );
                }
            })
        }
    }

    pub fn do_text_edit(&self, edits: &[TextEdit]) {
        let edits = self.buffer.with_untracked(|buffer| {
            edits
                .iter()
                .map(|edit| {
                    let selection = lapce_core::selection::Selection::region(
                        buffer.offset_of_position(&edit.range.start),
                        buffer.offset_of_position(&edit.range.end),
                    );
                    (selection, edit.new_text.as_str())
                })
                .collect::<Vec<_>>()
        });
        self.do_raw_edit(&edits, EditType::Completion);
    }

    fn check_auto_save(&self) {
        let config = self.common.config.get_untracked();
        if config.editor.autosave_interval > 0 {
            let Some(path) = self.content.with_untracked(|c| c.path().cloned())
            else {
                return;
            };
            let rev = self.rev();
            let doc = self.clone();
            let scope = self.scope;
            let proxy = self.common.proxy.clone();
            let format = config.editor.format_on_save;
            exec_after(
                Duration::from_millis(config.editor.autosave_interval),
                move |_| {
                    let current_rev = match doc
                        .buffer
                        .try_with_untracked(|b| b.as_ref().map(|b| b.rev()))
                    {
                        Some(rev) => rev,
                        None => return,
                    };

                    if current_rev != rev || doc.is_pristine() {
                        return;
                    }

                    if format {
                        let send = create_ext_action(scope, move |result| {
                            let current_rev = doc.rev();
                            if current_rev != rev {
                                return;
                            }
                            if let Ok(ProxyResponse::GetDocumentFormatting {
                                edits,
                            }) = result
                            {
                                doc.do_text_edit(&edits);
                            }
                            doc.save(|| {});
                        });
                        proxy.get_document_formatting(path, move |result| {
                            send(result);
                        });
                    } else {
                        doc.save(|| {});
                    }
                },
            );
        }
    }

    /// Update the styles after an edit, so the highlights are at the correct positions.
    /// This does not do a reparse of the document itself.
    fn update_styles(&self, delta: &RopeDelta) {
        batch(|| {
            self.semantic_styles.update(|styles| {
                if let Some(styles) = styles.as_mut() {
                    styles.apply_shape(delta);
                }
            });
            self.syntax.update(|syntax| {
                if let Some(styles) = syntax.styles.as_mut() {
                    styles.apply_shape(delta);
                }
                syntax.lens.apply_delta(delta);
            });
        });
    }

    /// Update the inlay hints so their positions are correct after an edit.
    fn update_inlay_hints(&self, delta: &RopeDelta) {
        self.inlay_hints.update(|inlay_hints| {
            if let Some(hints) = inlay_hints.as_mut() {
                hints.apply_shape(delta);
            }
        });
    }

    pub fn trigger_syntax_change(&self, edits: Option<SmallVec<[SyntaxEdit; 3]>>) {
        let (rev, text) =
            self.buffer.with_untracked(|b| (b.rev(), b.text().clone()));

        let doc = self.clone();
        let send = create_ext_action(self.scope, move |syntax| {
            if doc.buffer.with_untracked(|b| b.rev()) == rev {
                doc.syntax.set(syntax);
                doc.get_folding_range();
                doc.do_bracket_colorization();
                doc.clear_style_cache();
                doc.clear_sticky_headers_cache();
            }
        });

        self.syntax.update(|syntax| {
            syntax.cancel_flag.store(1, atomic::Ordering::Relaxed);
            syntax.cancel_flag = Arc::new(AtomicUsize::new(0));
        });
        let mut syntax = self.syntax.get_untracked();
        rayon::spawn(move || {
            syntax.parse(rev, text, edits.as_deref());
            send(syntax);
        });
    }

    fn clear_style_cache(&self) {
        self.line_styles.borrow_mut().clear();
        self.clear_text_cache();
    }

    fn clear_code_actions(&self) {
        self.code_actions.update(|c| {
            c.clear();
        });
    }

    /// Inform any dependents on this document that they should clear any cached text.
    pub fn clear_text_cache(&self) {
        self.cache_rev.try_update(|cache_rev| {
            *cache_rev += 1;

            // TODO: ???
            // Update the text layouts within the callback so that those alerted to cache rev
            // will see the now empty layouts.
            // self.text_layouts.borrow_mut().clear(*cache_rev, None);
        });
    }

    fn clear_sticky_headers_cache(&self) {
        self.sticky_headers.borrow_mut().clear();
    }

    /// Get the active style information, either the semantic styles or the
    /// tree-sitter syntax styles.
    fn styles(&self) -> Option<Spans<Style>> {
        if let Some(semantic_styles) = self.semantic_styles.get_untracked() {
            Some(semantic_styles)
        } else {
            self.syntax.with_untracked(|syntax| syntax.styles.clone())
        }
    }

    /// Get the style information for the particular line from semantic/syntax highlighting.
    /// This caches the result if possible.
    /// Paint each excerpt with its canonical source's current lexer snapshot.
    /// Never reuse a style from a different source identity/revision.
    fn projection_line_style(&self,line:usize)->Option<Arc<Vec<LineStyle>>> {
        let binding=self.projection.get_untracked()?;
        let current=binding.docs.get_untracked();
        let projection=binding.projection.borrow();
        let (from,to)=self.buffer.with_untracked(|b|(b.offset_of_line(line),b.offset_of_line(line+1).min(b.len())));
        let mut painted=Vec::new();
        for excerpt in &projection.excerpts {
            let begin=from.max(excerpt.display_range.start);let end=to.min(excerpt.display_range.end);
            if begin>=end {continue;}
            let Some(source)=current.get(&excerpt.source) else {continue;};
            if source.buffer_id.0!=excerpt.buffer_id || source.rev()!=excerpt.revision {continue;}
            let source_start=excerpt.source_range.start+begin-excerpt.display_range.start;
            let source_end=source_start+end-begin;
            let pieces=source.buffer.with_untracked(|b| {
                let mut pieces=Vec::new();let mut offset=source_start;
                while offset<source_end {
                    let source_line=b.line_of_offset(offset);let line_start=b.offset_of_line(source_line);
                    let stop=b.offset_of_line(source_line+1).min(source_end);
                    if stop<=offset {break;}
                    pieces.push((source_line,offset-line_start..stop-line_start,begin-from+offset-source_start));offset=stop;
                }
                pieces
            });
            for (source_line,columns,display_column) in pieces {
                painted.extend(crate::multibuffer::projected_line_styles(&source.line_style(source_line),columns,display_column));
            }
        }
        Some(Arc::new(painted))
    }

    pub(crate) fn projection_ime_stamp(&self,cursor:&Cursor)->Result<Option<crate::multibuffer::ProjectionImeStamp>,&'static str> {
        let Some(binding)=self.projection.get_untracked() else {return Ok(None);};
        let lapce_core::cursor::CursorMode::Insert(selection)=&cursor.mode else {return Err("IME composition requires insert mode");};
        let source=crate::multibuffer::projection_ime_anchor(&binding.projection.borrow(),selection)?;
        let docs=binding.docs.get_untracked();let doc=docs.get(&source.source).ok_or("Source was closed")?;
        if !doc.loaded() || doc.buffer_id.0!=source.buffer_id || doc.rev()!=source.revision {return Err("Source changed; restart composition after refreshing");}
        if doc.content.with_untracked(|content|content.read_only()) {return Err("Source is read-only");}
        let region=selection.regions()[0];
        Ok(Some(crate::multibuffer::ProjectionImeStamp{origin_id:self.buffer_id.0,origin_revision:self.rev(),source,selection:region.start..region.end}))
    }
    pub(crate) fn refuse_projection_ime(&self) {
        if let Some(binding)=self.projection.get_untracked() {binding.notice.set("IME composition moved or source changed; restart inside one source excerpt".into());}
    }
    fn line_diagnostic_ranges(&self, line: usize) -> Vec<(std::ops::Range<usize>, Diagnostic)> {
        let (display, display_end) = self.buffer.with_untracked(|buffer| {
            (buffer.offset_of_line(line), buffer.offset_of_line(line + 1))
        });
        let projected = self.projection.get_untracked().is_some();
        let mapped = if projected {
            self.projection_navigation_source(display)
        } else { None };
        if projected && mapped.is_none() { return Vec::new(); }
        let (source, start, end) = if let Some((source, offset)) = mapped {
            let Some(binding) = self.projection.get_untracked() else { return Vec::new(); };
            let projection = binding.projection.borrow();
            let Some(excerpt) = projection.excerpts.iter().find(|excerpt| display >= excerpt.display_range.start && display < excerpt.display_range.end) else { return Vec::new(); };
            let visible = (display_end - display).min(excerpt.display_range.end - display);
            (Some(source), offset, offset + visible)
        } else { (None, display, display_end) };
        let diagnostics = source.as_deref().unwrap_or(self).diagnostics.diagnostics_span;
        diagnostics.with_untracked(|spans| spans.iter_chunks(start..end)
            .filter_map(|(range, diagnostic)| clip_diagnostic_line(start..end, range.start()..range.end()).map(|range| (range, diagnostic.clone())))
            .collect())
    }

    pub(crate) fn projection_navigation_source(&self,display:usize)->Option<(Rc<Doc>,usize)> {
        let binding=self.projection.get_untracked()?;
        let anchor=binding.projection.borrow().navigation_anchor(display)?;
        let current=binding.docs.get_untracked();let source=current.get(&anchor.source)?;
        if !source.loaded() || source.buffer_id.0!=anchor.buffer_id || source.rev()!=anchor.revision
            || source.content.with_untracked(|c|c.path()!=Some(&anchor.source)) {return None;}
        Some((source.clone(),anchor.offset))
    }

    /// Outer None means an ordinary document; inner None hides a projection
    /// header, separator or stale source number instead of inventing a row.
    pub(crate) fn projection_source_line(&self,line:usize)->Option<Option<usize>> {
        let binding=self.projection.get_untracked()?;
        let projection=binding.projection.borrow();
        let display=self.buffer.with_untracked(|b|b.offset_of_line(line));
        let number=projection.source_line_anchor(display).and_then(|anchor| {
            let current=binding.docs.get_untracked();let source=current.get(&anchor.source)?;
            if source.buffer_id.0!=anchor.buffer_id || source.rev()!=anchor.revision {return None;}
            Some(source.buffer.with_untracked(|b|b.line_of_offset(anchor.offset)))
        });
        Some(number)
    }

    /// Reserve enough gutter space for source numbers, which can be far larger
    /// than the short excerpt document's own line count. Reads are reactive.
    pub(crate) fn gutter_line_number_max(&self)->usize {
        let aggregate=self.buffer.with(|b|b.last_line()+1);
        let Some(binding)=self.projection.get() else {return aggregate;};
        let current=binding.docs.get();let projection=binding.projection.borrow();
        projection.excerpts.iter().filter_map(|excerpt| {
            let source=current.get(&excerpt.source)?;
            source.buffer.with(|b| {
                (source.buffer_id.0==excerpt.buffer_id && b.rev()==excerpt.revision)
                    .then(||b.line_of_offset(excerpt.source_range.end)+1)
            })
        }).max().unwrap_or(1)
    }

    pub(crate) fn track_projection_styles(&self) {
        let doc=self.clone();
        self.scope.create_effect(move |_| {
            let Some(binding)=doc.projection.get() else {return;};
            let current=binding.docs.get();
            // Aggregate edits can move excerpt anchors without changing the
            // binding signal. Track the aggregate buffer revision as well.
            let _=doc.buffer.with(|buffer|buffer.rev());
            let paths:Vec<_>=binding.projection.borrow().excerpts.iter().map(|e|e.source.clone()).collect();
            for path in paths {if let Some(source)=current.get(&path) {let _=source.cache_rev.get();let _=source.buffer.with(|buffer|buffer.rev());}}
            doc.clear_style_cache();
        });
    }

    pub fn line_style(&self, line: usize) -> Arc<Vec<LineStyle>> {
        if let Some(styles)=self.projection_line_style(line) {return styles;}
        if let Some(styles) = self.line_styles.borrow().get(&line).cloned() {
            return styles;
        }
        let styles = self.syntax.with_untracked(|syntax| {
            // Edits invalidate the old snapshot immediately. Never paint styles
            // from a different revision while the worker is still lexing.
            if syntax.rev != self.rev() {
                return Vec::new();
            }
            match &syntax.lexilla {
                Some(snapshot) => {
                    lapce_core::lexilla::line_styles(snapshot, &syntax.text, line)
                }
                None => match &syntax.textmate {
                    Some(snapshot) => lapce_core::textmate::line_styles(
                        snapshot,
                        &syntax.text,
                        line,
                    ),
                    None => Vec::new(),
                },
            }
        });
        let styles = Arc::new(styles);
        self.line_styles.borrow_mut().insert(line, styles.clone());
        styles
    }

    /// Request semantic styles for the buffer from the LSP through the proxy.
    pub fn get_semantic_styles(&self) {
        if !self.loaded() {
            return;
        }

        let path =
            if let DocContent::File { path, .. } = self.content.get_untracked() {
                path
            } else {
                return;
            };

        let (atomic_rev, rev, len) = self
            .buffer
            .with_untracked(|b| (b.atomic_rev(), b.rev(), b.len()));

        let doc = self.clone();
        let send = create_ext_action(self.scope, move |styles| {
            if let Some(styles) = styles {
                if doc.buffer.with_untracked(|b| b.rev()) == rev {
                    doc.semantic_styles.set(Some(styles));
                    doc.clear_style_cache();
                }
            }
        });

        self.common.proxy.get_semantic_tokens(path, move |result| {
            if let Ok(ProxyResponse::GetSemanticTokens { styles }) = result {
                if styles.styles.is_empty() {
                    send(None);
                    return;
                }
                if atomic_rev.load(atomic::Ordering::Acquire) != rev {
                    send(None);
                    return;
                }
                std::thread::spawn(move || {
                    let mut styles_span = SpansBuilder::new(len);
                    for style in styles.styles {
                        if atomic_rev.load(atomic::Ordering::Acquire) != rev {
                            send(None);
                            return;
                        }
                        styles_span.add_span(
                            Interval::new(style.start, style.end),
                            style.style,
                        );
                    }

                    let styles = styles_span.build();
                    send(Some(styles));
                });
            } else {
                send(None);
            }
        });
    }

    pub fn get_code_lens(&self) {
        let cx = self.scope;
        let doc = self.clone();
        self.code_lens.update(|code_lens| {
            code_lens.clear();
        });
        let rev = self.rev();
        if let DocContent::File { path, .. } = doc.content.get_untracked() {
            let send = create_ext_action(cx, move |result| {
                if rev != doc.rev() {
                    return;
                }
                if let Ok(ProxyResponse::GetCodeLensResponse { plugin_id, resp }) =
                    result
                {
                    let Some(codelens) = resp else {
                        return;
                    };
                    doc.code_lens.update(|code_lens| {
                        for codelens in codelens {
                            if codelens.command.is_none() {
                                continue;
                            }
                            let entry = code_lens
                                .entry(codelens.range.start.line as usize)
                                .or_insert_with(|| {
                                    (
                                        plugin_id,
                                        doc.buffer.with_untracked(|b| {
                                            b.offset_of_line(
                                                codelens.range.start.line as usize,
                                            )
                                        }),
                                        im::Vector::new(),
                                    )
                                });
                            entry.2.push_back(codelens);
                        }
                    });
                }
            });
            self.common.proxy.get_code_lens(path, move |result| {
                send(result);
            });
        }
    }

    pub fn get_document_symbol(&self) {
        let cx = self.scope;
        let doc = self.clone();
        let rev = self.rev();
        if let DocContent::File { path, .. } = doc.content.get_untracked() {
            let send = create_ext_action(cx, {
                let path = path.clone();
                move |result: Result<ProxyResponse, lapce_rpc::RpcError>| {
                    if rev != doc.rev() {
                        return;
                    }
                    let from_lsp: Vec<RwSignal<SymbolInformationItemData>> =
                        match result {
                            Ok(ProxyResponse::GetDocumentSymbols {
                                resp: DocumentSymbolResponse::Nested(symbols),
                            }) => symbols
                                .into_iter()
                                .map(|x| {
                                    cx.create_rw_signal(
                                        SymbolInformationItemData::from((x, cx)),
                                    )
                                })
                                .collect(),
                            Ok(ProxyResponse::GetDocumentSymbols {
                                resp: DocumentSymbolResponse::Flat(infos),
                            }) => {
                                lsp_types::Url::from_file_path(&path)
                                    .map(|uri| {
                                        crate::panel::document_symbol::flat_to_nested(
                                            infos, &uri,
                                        )
                                    })
                                    .unwrap_or_default()
                                .into_iter()
                                .map(|x| {
                                    cx.create_rw_signal(
                                        SymbolInformationItemData::from((x, cx)),
                                    )
                                })
                                .collect()
                            }
                            _ => Vec::new(),
                        };
                    // No language server answer: Blade views get a local
                    // outline so the panel is not blank.
                    let items = if from_lsp.is_empty()
                        && crate::blade_outline::is_blade_path(&path)
                    {
                        let text = doc
                            .buffer
                            .with_untracked(|b| b.text().to_string());
                        crate::blade_outline::outline(&text)
                            .into_iter()
                            .map(|x| {
                                cx.create_rw_signal(
                                    SymbolInformationItemData::from((x, cx)),
                                )
                            })
                            .collect()
                    } else {
                        from_lsp
                    };
                    {
                        let symbol_new = Some(SymbolData::new(items, path, cx));
                        doc.document_symbol_data.update(|symbol| {
                            *symbol = symbol_new;
                        });
                    }
                }
            });

            self.common.proxy.get_document_symbols(path, move |result| {
                send(result);
            });
        }
    }

    /// Request inlay hints for the buffer from the LSP through the proxy.
    pub fn get_inlay_hints(&self) {
        if !self.loaded() {
            return;
        }

        let path =
            if let DocContent::File { path, .. } = self.content.get_untracked() {
                path
            } else {
                return;
            };

        let (buffer, rev, len) = self
            .buffer
            .with_untracked(|b| (b.clone(), b.rev(), b.len()));

        let doc = self.clone();
        let send = create_ext_action(self.scope, move |hints| {
            if doc.buffer.with_untracked(|b| b.rev()) == rev {
                doc.inlay_hints.set(Some(hints));
                doc.clear_text_cache();
            }
        });

        self.common.proxy.get_inlay_hints(path, move |result| {
            if let Ok(ProxyResponse::GetInlayHints { mut hints }) = result {
                // Sort the inlay hints by their position, as the LSP does not guarantee that it will
                // provide them in the order that they are in within the file
                // as well, Spans does not iterate in the order that they appear
                hints.sort_by(|left, right| left.position.cmp(&right.position));

                let mut hints_span = SpansBuilder::new(len);
                for hint in hints {
                    let offset = buffer.offset_of_position(&hint.position).min(len);
                    hints_span.add_span(
                        Interval::new(offset, (offset + 1).min(len)),
                        hint,
                    );
                }
                let hints = hints_span.build();
                send(hints);
            }
        });
    }

    pub fn diagnostics(&self) -> &DiagnosticData {
        &self.diagnostics
    }

    /// Update the diagnostics' positions after an edit so that they appear in the correct place.
    fn update_diagnostics(&self, delta: &RopeDelta) {
        if self
            .diagnostics
            .diagnostics
            .with_untracked(|d| d.is_empty())
        {
            return;
        }

        self.diagnostics.diagnostics_span.update(|diagnostics| {
            diagnostics.apply_shape(delta);
        });
    }

    /// init diagnostics offset ranges from lsp positions
    pub fn init_diagnostics(&self) {
        let len = self.buffer.with_untracked(|b| b.len());
        let diagnostics = self.diagnostics.diagnostics.get_untracked();

        let span = self.buffer.with_untracked(|buffer| {
            let mut span = SpansBuilder::new(len);
            for diag in diagnostics.iter() {
                let start = buffer.offset_of_position(&diag.range.start);
                let end = buffer.offset_of_position(&diag.range.end);
                span.add_span(Interval::new(start, end), diag.to_owned());
            }
            span.build()
        });
        self.diagnostics.diagnostics_span.set(span);

        self.clear_text_cache();
        self.clear_code_actions();
    }

    /// Publish local ranges only after the syntax worker has accepted this revision.
    /// Until then the fold state keeps following edits through [`Self::apply_fold_edit`].
    pub fn get_folding_range(&self) {
        let ranges = self.syntax.with_untracked(|syntax| {
            (syntax.rev == self.rev()).then(|| syntax.folding_ranges.clone())
        });
        if let Some(ranges) = ranges {
            self.fold_state.update(|state| state.set_ranges(ranges));
        }
    }

    /// Keep folds attached to their blocks across an edit. Blocks the edit touches unfold.
    fn apply_fold_edit(&self, inval: &InvalLines) {
        if self.fold_state.with_untracked(|state| state.is_empty()) {
            return;
        }
        self.fold_state.update(|state| {
            state.apply_edit(inval.start_line, inval.inval_count, inval.new_count)
        });
    }

    /// Fold or unfold. `f` returns whether anything changed.
    pub fn update_folds(&self, f: impl FnOnce(&mut FoldState) -> bool) -> bool {
        // Work on a copy so nothing is notified when nothing changed.
        let mut state = self.fold_state.get_untracked();
        let changed = f(&mut state);
        if changed {
            self.fold_state.set(state);
        }
        changed
    }

    /// Get the current completion lens text
    pub fn completion_lens(&self) -> Option<String> {
        self.completion_lens.get_untracked()
    }

    pub fn set_completion_lens(
        &self,
        completion_lens: String,
        line: usize,
        col: usize,
    ) {
        // TODO: more granular invalidation
        self.completion_lens.set(Some(completion_lens));
        self.completion_pos.set((line, col));
        self.clear_text_cache();
    }

    pub fn clear_completion_lens(&self) {
        // TODO: more granular invalidation
        if self.completion_lens.get_untracked().is_some() {
            self.completion_lens.set(None);
            self.clear_text_cache();
        }
    }

    fn update_breakpoints(&self, delta: &RopeDelta, path: &Path, old_text: &Rope) {
        if self
            .common
            .breakpoints
            .with_untracked(|breakpoints| breakpoints.contains_key(path))
        {
            self.common.breakpoints.update(|breakpoints| {
                if let Some(path_breakpoints) = breakpoints.get_mut(path) {
                    let mut transformer = Transformer::new(delta);
                    self.buffer.with_untracked(|buffer| {
                        *path_breakpoints = path_breakpoints
                            .clone()
                            .into_values()
                            .map(|mut b| {
                                let offset = old_text.offset_of_line(b.line);
                                let offset = transformer.transform(offset, false);
                                let line = buffer.line_of_offset(offset);
                                b.line = line;
                                b.offset = offset;
                                (b.line, b)
                            })
                            .collect();
                    });
                }
            });
        }
    }

    /// Update the completion lens position after an edit so that it appears in the correct place.
    pub fn update_completion_lens(&self, delta: &RopeDelta) {
        let Some(completion) = self.completion_lens.get_untracked() else {
            return;
        };

        let (line, col) = self.completion_pos.get_untracked();
        let offset = self
            .buffer
            .with_untracked(|b| b.offset_of_line_col(line, col));

        // If the edit is easily checkable + updateable from, then we alter the lens' text.
        // In normal typing, if we didn't do this, then the text would jitter forward and then
        // backwards as the completion lens is updated.
        // TODO: this could also handle simple deletion, but we don't currently keep track of
        // the past completion lens string content in the field.
        if delta.as_simple_insert().is_some() {
            let (iv, new_len) = delta.summary();
            if iv.start() == iv.end()
                && iv.start() == offset
                && new_len <= completion.len()
            {
                // Remove the # of newly inserted characters
                // These aren't necessarily the same as the characters literally in the
                // text, but the completion will be updated when the completion widget
                // receives the update event, and it will fix this if needed.
                // TODO: this could be smarter and use the insert's content
                self.completion_lens
                    .set(Some(completion[new_len..].to_string()));
            }
        }

        // Shift the position by the rope delta
        let mut transformer = Transformer::new(delta);

        let new_offset = transformer.transform(offset, true);
        let new_pos = self
            .buffer
            .with_untracked(|b| b.offset_to_line_col(new_offset));

        self.completion_pos.set(new_pos);
    }

    fn update_find_result(&self, delta: &RopeDelta) {
        self.find_result.occurrences.update(|s| {
            *s = s.apply_delta(delta, true, InsertDrift::Default);
        })
    }

    pub fn update_find(&self) {
        let find_rev = self.common.find.rev.get_untracked();
        if self.find_result.find_rev.get_untracked() != find_rev {
            if self
                .common
                .find
                .search_string
                .with_untracked(|search_string| {
                    search_string
                        .as_ref()
                        .map(|s| s.content.is_empty())
                        .unwrap_or(true)
                })
            {
                self.find_result.occurrences.set(Selection::new());
            }
            self.find_result.reset();
            self.find_result.find_rev.set(find_rev);
        }

        if self.find_result.progress.get_untracked() != FindProgress::Started {
            return;
        }

        let search = self.common.find.search_string.get_untracked();
        let search = match search {
            Some(search) => search,
            None => return,
        };
        if search.content.is_empty() {
            return;
        }

        self.find_result
            .progress
            .set(FindProgress::InProgress(Selection::new()));

        let find_result = self.find_result.clone();
        let send = create_ext_action(self.scope, move |occurrences: Selection| {
            find_result.occurrences.set(occurrences);
            find_result.progress.set(FindProgress::Ready);
        });

        let text = self.buffer.with_untracked(|b| b.text().clone());
        let case_matching = self.common.find.case_matching.get_untracked();
        let whole_words = self.common.find.whole_words.get_untracked();
        rayon::spawn(move || {
            let mut occurrences = Selection::new();
            Find::find(
                &text,
                &search,
                0,
                text.len(),
                case_matching,
                whole_words,
                true,
                &mut occurrences,
            );
            send(occurrences);
        });
    }

    /// Get the sticky headers for a particular line, creating them if necessary.
    pub fn sticky_headers(&self, line: usize) -> Option<Vec<usize>> {
        if let Some(lines) = self.sticky_headers.borrow().get(&line) {
            return lines.clone();
        }
        let lines = self.buffer.with_untracked(|buffer| {
            let offset = buffer.offset_of_line(line + 1);
            self.syntax.with_untracked(|syntax| {
                syntax.sticky_headers(offset).map(|offsets| {
                    offsets
                        .iter()
                        .filter_map(|offset| {
                            let l = buffer.line_of_offset(*offset);
                            if l <= line { Some(l) } else { None }
                        })
                        .dedup()
                        .sorted()
                        .collect()
                })
            })
        });
        self.sticky_headers.borrow_mut().insert(line, lines.clone());
        lines
    }

    pub fn head_changes(&self) -> RwSignal<im::Vector<DiffLines>> {
        self.head_changes
    }

    /// Retrieve the `head` version of the buffer
    pub fn retrieve_head(&self) {
        if let DocContent::File { path, .. } = self.content.get_untracked() {
            let histories = self.histories;

            let send = {
                let path = path.clone();
                let doc = self.clone();
                create_ext_action(self.scope, move |result| {
                    if let Ok(ProxyResponse::BufferHeadResponse {
                        content, ..
                    }) = result
                    {
                        let history = DocumentHistory::new(
                            path.clone(),
                            "head".to_string(),
                            &content,
                        );
                        histories.update(|histories| {
                            histories.insert("head".to_string(), history);
                        });

                        doc.trigger_head_change();
                    }
                })
            };

            let path = path.clone();
            let proxy = self.common.proxy.clone();
            std::thread::spawn(move || {
                proxy.get_buffer_head(path, move |result| {
                    send(result);
                });
            });
        }
    }

    pub fn trigger_head_change(&self) {
        let history = if let Some(text) =
            self.histories.with_untracked(|histories| {
                histories
                    .get("head")
                    .map(|history| history.buffer.text().clone())
            }) {
            text
        } else {
            return;
        };

        let rev = self.rev();
        let left_rope = history;
        let (atomic_rev, right_rope) = self
            .buffer
            .with_untracked(|b| (b.atomic_rev(), b.text().clone()));

        let send = {
            let atomic_rev = atomic_rev.clone();
            let head_changes = self.head_changes;
            create_ext_action(self.scope, move |changes| {
                let changes = if let Some(changes) = changes {
                    changes
                } else {
                    return;
                };

                if atomic_rev.load(atomic::Ordering::Acquire) != rev {
                    return;
                }

                head_changes.set(changes);
            })
        };

        rayon::spawn(move || {
            let changes =
                rope_diff(left_rope, right_rope, rev, atomic_rev.clone(), None);
            send(changes.map(im::Vector::from));
        });
    }

    pub fn save(&self, after_action: impl FnOnce() + 'static) {
        let content = self.content.get_untracked();
        if let DocContent::File { path, .. } = content {
            let rev = self.rev();
            let buffer = self.buffer;
            let send = create_ext_action(self.scope, move |result| {
                if let Ok(ProxyResponse::SaveResponse {}) = result {
                    let current_rev = buffer.with_untracked(|buffer| buffer.rev());
                    if current_rev == rev {
                        buffer.update(|buffer| {
                            buffer.set_pristine();
                        });
                        after_action();
                    }
                }
            });

            self.common.proxy.save(rev, path, true, move |result| {
                send(result);
            })
        }
    }

    pub fn set_inline_completion(
        &self,
        inline_completion: String,
        line: usize,
        col: usize,
    ) {
        // TODO: more granular invalidation
        batch(|| {
            self.inline_completion.set(Some(inline_completion));
            self.inline_completion_pos.set((line, col));
            self.clear_text_cache();
        });
    }

    pub fn clear_inline_completion(&self) {
        if self.inline_completion.with_untracked(Option::is_some) {
            self.inline_completion.set(None);
            self.clear_text_cache();
        }
    }

    pub fn update_inline_completion(&self, delta: &RopeDelta) {
        let Some(completion) = self.inline_completion.get_untracked() else {
            return;
        };

        let (line, col) = self.completion_pos.get_untracked();
        let offset = self
            .buffer
            .with_untracked(|b| b.offset_of_line_col(line, col));

        // If the edit is easily checkable + updateable from, then we alter the text.
        // In normal typing, if we didn't do this, then the text would jitter forward and then
        // backwards as the completion is updated.
        // TODO: this could also handle simple deletion, but we don't currently keep track of
        // the past completion string content in the field.
        if delta.as_simple_insert().is_some() {
            let (iv, new_len) = delta.summary();
            if iv.start() == iv.end()
                && iv.start() == offset
                && new_len <= completion.len()
            {
                // Remove the # of newly inserted characters
                // These aren't necessarily the same as the characters literally in the
                // text, but the completion will be updated when the completion widget
                // receives the update event, and it will fix this if needed.
                self.inline_completion
                    .set(Some(completion[new_len..].to_string()));
            }
        }

        // Shift the position by the rope delta
        let mut transformer = Transformer::new(delta);

        let new_offset = transformer.transform(offset, true);
        let new_pos = self
            .buffer
            .with_untracked(|b| b.offset_to_line_col(new_offset));

        self.inline_completion_pos.set(new_pos);
    }

    pub fn code_actions(&self) -> RwSignal<CodeActions> {
        self.code_actions
    }

    /// Returns the offsets of the brackets enclosing the given offset.
    /// Uses a language aware algorithm if syntax support is available for the current language,
    /// else falls back to a language unaware algorithm.
    pub fn find_enclosing_brackets(&self, offset: usize) -> Option<(usize, usize)> {
        let rev = self.rev();
        self.syntax
            .with_untracked(|syntax| {
                (!syntax.text.is_empty() && syntax.rev == rev)
                    .then(|| syntax.find_enclosing_pair(offset))
            })
            // If syntax.text is empty, either the buffer is empty or we don't have syntax support
            // for the current language.
            // Try a language unaware search for enclosing brackets in case it is the latter.
            .unwrap_or_else(|| {
                self.buffer.with_untracked(|buffer| {
                    WordCursor::new(buffer.text(), offset).find_enclosing_pair()
                })
            })
    }

    pub fn update_highlights(&self, offset: usize) {
        if !self.loaded() {
            return;
        }

        let path =
            if let DocContent::File { path, .. } = self.content.get_untracked() {
                path
            } else {
                return;
            };

        let cursor_in_highlight = self.highlights.with_untracked(|highlights| {
            if let Some(highlights) = highlights {
                DocumentHighlight::find_for_offset(highlights, offset).is_some()
            } else {
                false
            }
        });

        if cursor_in_highlight {
            // Cursor is in the region of same highlight, skip querying LSP.
            return;
        }

        let need_start = match self.get_highlights_status.get_untracked() {
            DocumentHighlightsStatus::Ready => true,
            DocumentHighlightsStatus::Busy(_) => false,
        };

        self.get_highlights_status
            .update(|status| *status = DocumentHighlightsStatus::Busy(offset));

        if !need_start {
            return;
        }

        let (position, rev) = self
            .buffer
            .with_untracked(|b| (b.offset_to_position(offset), b.rev()));

        let doc = self.clone();

        let handle_response = create_ext_action(self.scope, move |response| {
            let mut new_highlights: Option<Vec<DocumentHighlight>> = None;

            if doc.buffer.with_untracked(|b| b.rev()) == rev {
                if let DocumentHighlightsStatus::Busy(offset) =
                    doc.get_highlights_status.get_untracked()
                {
                    if let Ok(ProxyResponse::GetDocumentHighlights {
                        highlights: Some(highlights),
                    }) = response
                    {
                        new_highlights = doc.buffer.with_untracked(move |buffer| {
                            let mut result = highlights
                                .iter()
                                .map(|highlight| DocumentHighlight {
                                    start: buffer
                                        .offset_of_position(&highlight.range.start),
                                    end: buffer
                                        .offset_of_position(&highlight.range.end),
                                })
                                .collect::<Vec<_>>();
                            DocumentHighlight::sort(&mut result);

                            // This is a kind of debouncing - if cursor moves before we got response from LSP,
                            // check if cursor is still in the region.
                            // In case cursor moved too far we may want to issue LSP request again, for new offset
                            // but it is easily recoverable by the user - just move cursor a bit to update highlights.
                            DocumentHighlight::find_for_offset(&result, offset)
                                .map(|_| result)
                        });
                    }
                }
            }

            doc.highlights.update(|value| *value = new_highlights);
            doc.get_highlights_status
                .update(|status| *status = DocumentHighlightsStatus::Ready);
        });

        self.common
            .proxy
            .get_document_highlights(path, position, handle_response);
    }
}

impl Document for Doc {
    fn text(&self) -> Rope {
        self.buffer.with_untracked(|buffer| buffer.text().clone())
    }

    fn cache_rev(&self) -> RwSignal<u64> {
        self.cache_rev
    }

    fn find_unmatched(&self, offset: usize, previous: bool, ch: char) -> usize {
        self.syntax().with_untracked(|syntax| {
            if syntax.layers.is_some() {
                syntax
                    .find_tag(offset, previous, &CharBuffer::from(ch))
                    .unwrap_or(offset)
            } else {
                let text = self.text();
                let mut cursor = WordCursor::new(&text, offset);
                let new_offset = if previous {
                    cursor.previous_unmatched(ch)
                } else {
                    cursor.next_unmatched(ch)
                };

                new_offset.unwrap_or(offset)
            }
        })
    }

    fn find_matching_pair(&self, offset: usize) -> usize {
        self.syntax().with_untracked(|syntax| {
            if syntax.layers.is_some() {
                syntax.find_matching_pair(offset).unwrap_or(offset)
            } else {
                let text = self.text();
                WordCursor::new(&text, offset)
                    .match_pairs()
                    .unwrap_or(offset)
            }
        })
    }

    fn preedit(&self) -> PreeditData {
        self.preedit.clone()
    }

    fn compute_screen_lines(
        &self,
        editor: &Editor,
        base: RwSignal<ScreenLinesBase>,
    ) -> ScreenLines {
        let Some(editor_data) = self.editor_data(editor.id()) else {
            return ScreenLines {
                lines: Default::default(),
                info: Default::default(),
                diff_sections: Default::default(),
                base,
            };
        };

        // Folded lines are hidden in this editor's lines before anything is measured. Reading
        // the state here also reruns the screen line calculation whenever a fold changes.
        let hidden =
            if matches!(editor_data.kind.get_untracked(), EditorViewKind::Normal) {
                self.fold_state.with(|state| state.hidden_ranges())
            } else {
                Vec::new()
            };
        editor.lines().set_hidden_lines(HiddenLines::new(hidden));

        compute_screen_lines(
            self.common.config,
            base,
            editor_data.kind.read_only(),
            &editor_data.doc_signal().get(),
            editor.lines(),
            editor.text_prov(),
            editor.config_id(),
        )
    }

    fn run_command(
        &self,
        ed: &Editor,
        cmd: &Command,
        count: Option<usize>,
        modifiers: Modifiers,
    ) -> CommandExecuted {
        let Some(editor_data) = self.editor_data(ed.id()) else {
            return CommandExecuted::No;
        };

        let cmd = CommandKind::from(cmd.clone());
        let cmd = LapceCommand {
            kind: cmd,
            data: None,
        };
        editor_data.run_command(&cmd, count, modifiers)
    }

    fn receive_char(&self, ed: &Editor, c: &str) {
        let Some(editor_data) = self.editor_data(ed.id()) else {
            return;
        };

        editor_data.receive_char(c);
    }

    fn edit(
        &self,
        iter: &mut dyn Iterator<Item = (Selection, &str)>,
        edit_type: EditType,
    ) {
        if self.content.with_untracked(|c|c.read_only()) {return;}
        let delta = self
            .buffer
            .try_update(|buffer| buffer.edit(iter, edit_type))
            .unwrap();
        self.apply_deltas(&[delta]);
    }
}

impl DocumentPhantom for Doc {
    fn phantom_text(
        &self,
        editor_id: EditorId,
        editor_style: &EditorStyle,
        line: usize,
    ) -> PhantomTextLine {
        // Reuse only the source's diagnostic lens, never its inlays, completion
        // text or IME overlay. The projection owns its local composition overlay.
        if self.projection.get_untracked().is_some() {
            let display=self.buffer.with_untracked(|b|b.offset_of_line(line));
            let mut text:Vec<PhantomText>=self.projection_navigation_source(display).map(|(source,offset)| {
                let (source_line,source_col)=source.buffer.with_untracked(|b|b.offset_to_line_col(offset));
                let display_len=self.buffer.with_untracked(|b|b.offset_of_line(line+1)-display);
                source.phantom_text(editor_id,editor_style,source_line).text.into_iter()
                    .filter_map(|mut text| {
                        if text.kind!=PhantomTextKind::Diagnostic || text.col<source_col {return None;}
                        text.col-=source_col;
                        (text.col<=display_len).then_some(text)
                    }).collect()
            }).unwrap_or_default();
            if let Some(preedit)=self.preedit_phantom(Some(self.common.config.get_untracked().color(LapceColor::EDITOR_FOREGROUND)),line) {
                let offset=self.preedit.preedit.with_untracked(|value|value.as_ref().map(|value|value.offset));
                if offset.is_some_and(|offset|self.projection_ime_stamp(&Cursor::new(lapce_core::cursor::CursorMode::Insert(Selection::caret(offset)),None,None)).is_ok()) {text.push(preedit);}
            }
            text.sort_by_key(|text|text.col);
            return PhantomTextLine{text:text.into()};
        }
        let config = &self.common.config.get_untracked();

        let (start_offset, end_offset) = self.buffer.with_untracked(|buffer| {
            (buffer.offset_of_line(line), buffer.offset_of_line(line + 1))
        });

        let inlay_hints = self.inlay_hints.get_untracked();
        // If hints are enabled, and the hints field is filled, then get the hints for this line
        // and convert them into PhantomText instances
        let hints = config
            .editor
            .enable_inlay_hints
            .then_some(())
            .and(inlay_hints.as_ref())
            .map(|hints| hints.iter_chunks(start_offset..end_offset))
            .into_iter()
            .flatten()
            .filter(|(interval, _)| {
                interval.start >= start_offset && interval.start < end_offset
            })
            .map(|(interval, inlay_hint)| {
                let (col, affinity) = self.buffer.with_untracked(|b| {
                    let mut cursor =
                        lapce_xi_rope::Cursor::new(b.text(), interval.start);

                    let next_char = cursor.peek_next_codepoint();
                    let prev_char = cursor.prev_codepoint();

                    let mut affinity = None;
                    if let Some(prev_char) = prev_char {
                        let c = get_char_property(prev_char);
                        if c == CharClassification::Other {
                            affinity = Some(CursorAffinity::Backward)
                        } else if matches!(
                            c,
                            CharClassification::Lf
                                | CharClassification::Cr
                                | CharClassification::Space
                        ) {
                            affinity = Some(CursorAffinity::Forward)
                        }
                    };
                    if affinity.is_none() {
                        if let Some(next_char) = next_char {
                            let c = get_char_property(next_char);
                            if c == CharClassification::Other {
                                affinity = Some(CursorAffinity::Forward)
                            } else if matches!(
                                c,
                                CharClassification::Lf
                                    | CharClassification::Cr
                                    | CharClassification::Space
                            ) {
                                affinity = Some(CursorAffinity::Backward)
                            }
                        }
                    }

                    let (_, col) = b.offset_to_line_col(interval.start);
                    (col, affinity)
                });
                let text = match &inlay_hint.label {
                    InlayHintLabel::String(label) => label.to_string(),
                    InlayHintLabel::LabelParts(parts) => {
                        parts.iter().map(|p| &p.value).join("")
                    }
                };
                PhantomText {
                    kind: PhantomTextKind::InlayHint,
                    col,
                    text,
                    affinity,
                    fg: Some(config.color(LapceColor::INLAY_HINT_FOREGROUND)),
                    // font_family: Some(config.editor.inlay_hint_font_family()),
                    font_size: Some(config.editor.inlay_hint_font_size()),
                    bg: Some(config.color(LapceColor::INLAY_HINT_BACKGROUND)),
                    under_line: None,
                }
            });
        // You're quite unlikely to have more than six hints on a single line
        // this later has the diagnostics added onto it, but that's still likely to be below six
        // overall.
        let mut text: SmallVec<[PhantomText; 6]> = hints.collect();

        // If error lens is enabled, and the diagnostics field is filled, then get the diagnostics
        // that end on this line which have a severity worse than HINT and convert them into
        // PhantomText instances

        let mut diag_text: SmallVec<[PhantomText; 6]> =
            self.buffer.with_untracked(|buffer| {
                config
                    .editor
                    .enable_error_lens
                    .then_some(())
                    .map(|_| self.diagnostics.diagnostics_span.get_untracked())
                    .map(|diags| {
                        diags
                            .iter_chunks(start_offset..end_offset)
                            .filter_map(|(iv, diag)| {
                                let end = iv.end();
                                let end_line = buffer.line_of_offset(end);
                                if end_line == line
                                    && diag.severity < Some(DiagnosticSeverity::HINT)
                                {
                                    let fg = {
                                        let severity = diag
                                            .severity
                                            .unwrap_or(DiagnosticSeverity::WARNING);
                                        let theme_prop = if severity
                                            == DiagnosticSeverity::ERROR
                                        {
                                            LapceColor::ERROR_LENS_ERROR_FOREGROUND
                                        } else if severity
                                            == DiagnosticSeverity::WARNING
                                        {
                                            LapceColor::ERROR_LENS_WARNING_FOREGROUND
                                        } else {
                                            // information + hint (if we keep that) + things without a severity
                                            LapceColor::ERROR_LENS_OTHER_FOREGROUND
                                        };

                                        config.color(theme_prop)
                                    };

                                    let text =
                                        if config.editor.only_render_error_styling {
                                            "".to_string()
                                        } else if config.editor.error_lens_multiline
                                        {
                                            format!("    {}", diag.message)
                                        } else {
                                            format!(
                                                "    {}",
                                                diag.message.lines().join(" ")
                                            )
                                        };
                                    Some(PhantomText {
                                        kind: PhantomTextKind::Diagnostic,
                                        col: end_offset - start_offset,
                                        affinity: Some(CursorAffinity::Backward),
                                        text,
                                        fg: Some(fg),
                                        font_size: Some(
                                            config.editor.error_lens_font_size(),
                                        ),
                                        bg: None,
                                        under_line: None,
                                    })
                                } else {
                                    None
                                }
                            })
                            .collect::<SmallVec<[PhantomText; 6]>>()
                    })
                    .unwrap_or_default()
            });

        text.append(&mut diag_text);

        let (completion_line, completion_col) = self.completion_pos.get_untracked();
        let completion_text = config
            .editor
            .enable_completion_lens
            .then_some(())
            .and(self.completion_lens.get_untracked())
            // TODO: We're probably missing on various useful completion things to include here!
            .filter(|_| line == completion_line)
            .map(|completion| PhantomText {
                kind: PhantomTextKind::Completion,
                col: completion_col,
                text: completion.clone(),
                fg: Some(config.color(LapceColor::COMPLETION_LENS_FOREGROUND)),
                font_size: Some(config.editor.completion_lens_font_size()),
                affinity: Some(CursorAffinity::Backward),
                // font_family: Some(config.editor.completion_lens_font_family()),
                bg: None,
                under_line: None,
                // TODO: italics?
            });
        if let Some(completion_text) = completion_text {
            text.push(completion_text);
        }

        // TODO: don't display completion lens and inline completion at the same time
        // and/or merge them so that they can be shifted between like multiple inline completions
        // can
        let (inline_completion_line, inline_completion_col) =
            self.inline_completion_pos.get_untracked();
        let inline_completion_text = (config.editor.enable_inline_completion
            || config.ai_autocomplete.enabled)
            .then_some(())
            .and(self.inline_completion.get_untracked())
            .filter(|_| line == inline_completion_line)
            .map(|completion| PhantomText {
                kind: PhantomTextKind::Completion,
                col: inline_completion_col,
                text: completion.clone(),
                affinity: Some(CursorAffinity::Backward),
                fg: Some(config.color(LapceColor::COMPLETION_LENS_FOREGROUND)),
                font_size: Some(config.editor.completion_lens_font_size()),
                // font_family: Some(config.editor.completion_lens_font_family()),
                bg: None,
                under_line: None,
                // TODO: italics?
            });
        if let Some(inline_completion_text) = inline_completion_text {
            text.push(inline_completion_text);
        }

        if let Some(preedit) = self
            .preedit_phantom(Some(config.color(LapceColor::EDITOR_FOREGROUND)), line)
        {
            text.push(preedit)
        }

        // Colour swatches (see color_swatch.rs). Skipped for very long lines.
        if config.editor.enable_color_swatches
            && end_offset - start_offset <= 20_000
            && matches!(
                self.content.get_untracked(),
                DocContent::File { ref path, .. } if crate::color_swatch::enabled_for(path)
            )
        {
            let line_text = self
                .buffer
                .with_untracked(|b| b.slice_to_cow(start_offset..end_offset).to_string());
            for sw in crate::color_swatch::find(&line_text) {
                let [r, g, b, a] = sw.rgba;
                text.push(PhantomText {
                    kind: PhantomTextKind::InlayHint,
                    col: sw.col,
                    text: "\u{25A0} ".to_string(),
                    affinity: Some(CursorAffinity::Backward),
                    fg: Some(floem::peniko::Color::from_rgba8(r, g, b, a)),
                    font_size: None,
                    bg: None,
                    under_line: None,
                });
            }
        }

        text.sort_by(|a, b| {
            if a.col == b.col {
                a.kind.cmp(&b.kind)
            } else {
                a.col.cmp(&b.col)
            }
        });

        PhantomTextLine { text }
    }

    fn has_multiline_phantom(&self, _: EditorId, _: &EditorStyle) -> bool {
        // TODO: actually check
        true
    }
}
impl CommonAction for Doc {
    fn exec_motion_mode(
        &self,
        _ed: &Editor,
        cursor: &mut Cursor,
        motion_mode: MotionMode,
        range: Range<usize>,
        is_vertical: bool,
        register: &mut Register,
    ) {
        if let Some(binding)=self.projection.get_untracked() {
            let selected=self.buffer.with_untracked(|buffer|crate::multibuffer::projection_motion_selection(&binding.projection.borrow(),buffer,range.clone(),is_vertical,&motion_mode));
            let selected=match selected {Ok(value)=>value,Err(error)=>{binding.notice.set(error.into());return;}};
            let docs=binding.docs.get_untracked();
            let style=crate::multibuffer::projection_indent_style(&binding.projection.borrow(),&selected,|excerpt| {
                let doc=docs.get(&excerpt.source).ok_or("Source was closed")?;
                if !doc.loaded() {return Err("Source is still loading");}
                if doc.buffer_id.0!=excerpt.buffer_id || doc.rev()!=excerpt.revision {return Err("Source changed; refresh before editing");}
                if doc.content.with_untracked(|content|content.read_only()) {return Err("Source is read-only");}
                Ok(doc.buffer.with_untracked(|buffer|buffer.indent_style()))
            });
            let style=match style {Ok(value)=>value,Err(error)=>{binding.notice.set(error.into());return;}};
            let staged=self.buffer.with_untracked(|buffer|crate::multibuffer::stage_projection_motion(&binding.projection.borrow(),buffer,cursor,motion_mode.clone(),range,is_vertical,register,style));
            let (candidate,next,next_register,deltas)=match staged {Ok(value)=>value,Err(error)=>{binding.notice.set(error.into());return;}};
            if matches!(motion_mode,MotionMode::Yank{..}) {
                *register=next_register;binding.notice.set(String::new());return;
            }
            let applied=self.edit_projection(cursor,|buffer,cursor|{*buffer=candidate;*cursor=next;deltas}).unwrap_or_default();
            if !applied.is_empty() {*register=next_register;}
            return;
        }
        let deltas = self
            .buffer
            .try_update(move |buffer| {
                Action::execute_motion_mode(
                    cursor,
                    buffer,
                    motion_mode,
                    range,
                    is_vertical,
                    register,
                )
            })
            .unwrap();
        self.apply_deltas(&deltas);
    }

    fn do_edit(
        &self,
        _ed: &Editor,
        cursor: &mut Cursor,
        cmd: &EditCommand,
        modal: bool,
        register: &mut Register,
        smart_tab: bool,
    ) -> bool {
        let deltas = Doc::do_edit(self, cursor, cmd, modal, register, smart_tab);
        !deltas.is_empty()
    }
}

#[derive(Clone)]
pub struct DocStyling {
    config: ReadSignal<Arc<LapceConfig>>,
    doc: Rc<Doc>,
}
impl DocStyling {
    fn apply_colorization(
        &self,
        edid: EditorId,
        style: &EditorStyle,
        line: usize,
        attrs: &Attrs,
        attrs_list: &mut AttrsList,
    ) {
        let config = self.config.get_untracked();
        let phantom_text = self.doc.phantom_text(edid, style, line);
        if let Some(bracket_styles) = self.doc.parser.borrow().bracket_pos.get(&line)
        {
            for bracket_style in bracket_styles.iter() {
                if let Some(fg_color) = bracket_style.style.fg_color.as_ref() {
                    if let Some(fg_color) = config.style_color(fg_color) {
                        let start = phantom_text.col_at(bracket_style.start);
                        let end = phantom_text.col_at(bracket_style.end);
                        attrs_list
                            .add_span(start..end, attrs.clone().color(fg_color));
                    }
                }
            }
        }
    }
}
impl Styling for DocStyling {
    fn id(&self) -> u64 {
        self.config.with_untracked(|config| config.id)
    }

    fn font_size(&self, _: EditorId, _line: usize) -> usize {
        self.config
            .with_untracked(|config| config.editor.font_size())
    }

    fn line_height(&self, _: EditorId, _line: usize) -> f32 {
        self.config
            .with_untracked(|config| config.editor.line_height()) as f32
    }

    fn font_family(
        &self,
        _: EditorId,
        _line: usize,
    ) -> std::borrow::Cow<'_, [floem::text::FamilyOwned]> {
        // TODO: cache this
        Cow::Owned(self.config.with_untracked(|config| {
            FamilyOwned::parse_list(&config.editor.font_family).collect()
        }))
    }

    fn weight(&self, _: EditorId, _line: usize) -> floem::text::Weight {
        floem::text::Weight::NORMAL
    }

    fn italic_style(&self, _: EditorId, _line: usize) -> floem::text::Style {
        floem::text::Style::Normal
    }

    fn stretch(&self, _: EditorId, _line: usize) -> floem::text::Stretch {
        floem::text::Stretch::Normal
    }

    fn indent_line(&self, _: EditorId, line: usize, line_content: &str) -> usize {
        if line_content.trim().is_empty() {
            let text = self.doc.rope_text();
            let offset = text.offset_of_line(line);
            if let Some(offset) =
                self.doc.syntax.with_untracked(|s| s.parent_offset(offset))
            {
                return text.line_of_offset(offset);
            }
        }

        line
    }

    fn tab_width(&self, _: EditorId, _line: usize) -> usize {
        self.config.with_untracked(|config| config.editor.tab_width)
    }

    fn atomic_soft_tabs(&self, _: EditorId, _line: usize) -> bool {
        self.config
            .with_untracked(|config| config.editor.atomic_soft_tabs)
    }

    fn apply_attr_styles(
        &self,
        edid: EditorId,
        style: &EditorStyle,
        line: usize,
        default: Attrs,
        attrs_list: &mut AttrsList,
    ) {
        let config = self.doc.common.config.get_untracked();

        self.apply_colorization(edid, style, line, &default, attrs_list);

        let phantom_text = self.doc.phantom_text(edid, style, line);
        for line_style in self.doc.line_style(line).iter() {
            if let Some(fg_color) = line_style.style.fg_color.as_ref() {
                if let Some(fg_color) = config.style_color(fg_color) {
                    let start = phantom_text.col_at(line_style.start);
                    let end = phantom_text.col_at(line_style.end);
                    attrs_list.add_span(start..end, default.clone().color(fg_color));
                }
            }
        }
    }

    fn apply_layout_styles(
        &self,
        edid: EditorId,
        style: &EditorStyle,
        line: usize,
        layout_line: &mut TextLayoutLine,
    ) {
        let doc = &self.doc;
        let config = doc.common.config.get_untracked();

        layout_line.extra_style.clear();
        let layout = &layout_line.text;

        let phantom_text = doc.phantom_text(edid, style, line);

        let phantom_styles = phantom_text
            .offset_size_iter()
            .filter(move |(_, _, _, p)| p.bg.is_some() || p.under_line.is_some())
            .flat_map(move |(col_shift, size, col, phantom)| {
                let start = col + col_shift;
                let end = start + size;

                extra_styles_for_range(
                    layout,
                    start,
                    end,
                    phantom.bg,
                    phantom.under_line,
                    None,
                )
            });
        layout_line.extra_style.extend(phantom_styles);

        let mut max_severity: Option<DiagnosticSeverity> = None;
        // Diagnostic coordinates belong to the source, never the aggregate.
        // Clip to this visible source line before accounting for phantom text.
        for (range, diag) in doc.line_diagnostic_ranges(line) {
            if diag.severity >= Some(DiagnosticSeverity::HINT) { continue; }
            let start = phantom_text.col_after(range.start, true);
            let end = phantom_text.col_after(range.end, false);
            if let Some(severity) = diag.severity {
                max_severity = Some(max_severity.map_or(severity, |max| max.min(severity)));
            }
            let color_name = if diag.severity == Some(DiagnosticSeverity::ERROR) {
                LapceColor::LAPCE_ERROR
            } else {
                LapceColor::LAPCE_WARN
            };
            layout_line.extra_style.extend(extra_styles_for_range(
                layout, start, end, None, None, Some(config.color(color_name)),
            ));
        }

        // Add the styling for the diagnostic severity, if applicable
        if let Some(max_severity) = max_severity {
            let theme_prop = if max_severity == DiagnosticSeverity::ERROR {
                LapceColor::ERROR_LENS_ERROR_BACKGROUND
            } else if max_severity == DiagnosticSeverity::WARNING {
                LapceColor::ERROR_LENS_WARNING_BACKGROUND
            } else {
                LapceColor::ERROR_LENS_OTHER_BACKGROUND
            };

            let size = layout.size();
            let x1 = if !config.editor.error_lens_end_of_line {
                let error_end_x = size.width;
                Some(error_end_x.max(size.width))
            } else {
                None
            };

            // TODO(minor): Should we show the background only on wrapped lines that have the
            // diagnostic actually on that line?
            // That would make it more obvious where it is from and matches other editors.
            layout_line.extra_style.push(LineExtraStyle {
                x: 0.0,
                y: 0.0,
                width: x1,
                height: size.height,
                bg_color: Some(config.color(theme_prop)),
                under_line: None,
                wave_line: None,
            });
        }
    }

    fn paint_caret(&self, edid: EditorId, _line: usize) -> bool {
        let Some(e_data) = self.doc.editor_data(edid) else {
            return true;
        };

        // If the find is active, then we don't want to paint the caret
        !e_data.find_focus.get_untracked()
    }
}

impl Doc {
    /// Largest buffer the edit path will lex synchronously for a comment
    /// toggle when the async snapshot is stale. Past this, the toggle vetoes
    /// (a no-op) rather than block the UI thread on an unbounded full-file
    /// lex.
    const MAX_SYNC_COMMENT_LEX_BYTES: usize = 1 << 20;

    /// Builds the comment-delimiter resolver handed to the edit layer.
    ///
    /// Cost is matched to the command: every command except
    /// ToggleLineComment - and toggles on single-syntax languages - gets a
    /// zero-cost closure over the language-wide token, preserving the cost
    /// profile ordinary edits (every keystroke) had before per-region
    /// resolution existed. Only a toggle on a regional language
    /// ([`LapceLanguage::comment_tokens_are_regional`]) pays for regions:
    ///
    /// * Reuse: the asynchronously maintained snapshot is valid for the
    ///   current buffer revision (the common case - painting caught up), so
    ///   it is reused with no new lexing;
    /// * FreshLex: otherwise the CURRENT buffer is highlighted once,
    ///   synchronously, bounded to `MAX_SYNC_COMMENT_LEX_BYTES`;
    /// * Veto: a stale snapshot past that bound makes the toggle a no-op
    ///   rather than risk stale region spans or an unbounded lex.
    fn comment_token_resolver(
        cmd: &EditCommand,
        syntax: &Syntax,
        buffer: &Buffer,
    ) -> Box<dyn Fn(usize) -> (&'static str, &'static str)> {
        let language = syntax.language;
        let snapshot_current = syntax.rev == buffer.rev() && syntax.lexilla.is_some();
        match Self::comment_token_source(cmd, language, snapshot_current, buffer.text().len())
        {
            CommentTokenSource::Global => {
                let global = language.comment_tokens();
                Box::new(move |_| global)
            }
            CommentTokenSource::ReuseSnapshot => {
                let snapshot = syntax.lexilla.clone().expect("current implies present");
                Box::new(move |offset| {
                    language.comment_tokens_at(Some(snapshot.as_ref()), offset)
                })
            }
            CommentTokenSource::FreshLex => {
                Self::fresh_comment_token_resolver(language, buffer.text(), &syntax.project)
            }
            CommentTokenSource::Veto => Box::new(|_| ("", "")),
        }
    }

    /// Picks how `comment_token_resolver` obtains region information. Kept
    /// separate so the cost gating is directly testable.
    fn comment_token_source(
        cmd: &EditCommand,
        language: LapceLanguage,
        snapshot_current: bool,
        text_len: usize,
    ) -> CommentTokenSource {
        if !matches!(cmd, EditCommand::ToggleLineComment)
            || !language.comment_tokens_are_regional()
        {
            return CommentTokenSource::Global;
        }
        if snapshot_current {
            return CommentTokenSource::ReuseSnapshot;
        }
        if text_len <= Self::MAX_SYNC_COMMENT_LEX_BYTES {
            return CommentTokenSource::FreshLex;
        }
        CommentTokenSource::Veto
    }

    /// Comment-delimiter resolver from a FRESH highlight of the given text.
    /// Region spans and style bytes are only valid for the exact text they
    /// were lexed from, so this is the safe answer when the asynchronously
    /// maintained `syntax.lexilla` snapshot may lag an edit or a language
    /// switch.
    fn fresh_comment_token_resolver(
        language: LapceLanguage,
        text: &Rope,
        project: &lapce_core::project_profile::ProjectProfile,
    ) -> Box<dyn Fn(usize) -> (&'static str, &'static str)> {
        let snapshot = lapce_core::lexilla::highlight_profile(language, text, project);
        Box::new(move |offset| {
            language.comment_tokens_at(snapshot.as_deref(), offset)
        })
    }
}

/// How the comment-toggle resolver obtains region information; see
/// `Doc::comment_token_source`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommentTokenSource {
    /// Language-wide token; no snapshot consulted. Zero cost.
    Global,
    /// The async snapshot is valid for the current buffer revision: reuse it.
    ReuseSnapshot,
    /// Snapshot missing or stale, buffer within the synchronous-lex bound.
    FreshLex,
    /// Region resolution required but unavailable within the cost bound:
    /// every line reports unresolvable, so the whole toggle is a no-op.
    Veto,
}

impl std::fmt::Debug for Doc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("Document {:?}", self.buffer_id))
    }
}

/// Get the previous unmatched character `c` from the `offset` using `syntax` if applicable
fn syntax_prev_unmatched(
    buffer: &Buffer,
    syntax: &Syntax,
    c: char,
    offset: usize,
) -> Option<usize> {
    if syntax.layers.is_some() {
        syntax.find_tag(offset, true, &CharBuffer::new(c))
    } else {
        WordCursor::new(buffer.text(), offset).previous_unmatched(c)
    }
}

fn should_blink(
    focus: RwSignal<Focus>,
    keyboard_focus: RwSignal<Option<ViewId>>,
) -> impl Fn() -> bool {
    move || {
        let Some(focus) = focus.try_get_untracked() else {
            return false;
        };
        if matches!(
            focus,
            Focus::Workbench
                | Focus::Palette
                | Focus::Panel(PanelKind::Plugin)
                | Focus::Panel(PanelKind::Search)
                | Focus::Panel(PanelKind::SourceControl)
        ) {
            return true;
        }

        if keyboard_focus.get_untracked().is_some() {
            return true;
        }
        false
    }
}

fn extra_styles_for_range(
    text_layout: &TextLayout,
    start: usize,
    end: usize,
    bg_color: Option<Color>,
    under_line: Option<Color>,
    wave_line: Option<Color>,
) -> impl Iterator<Item = LineExtraStyle> + '_ {
    let start_hit = text_layout.hit_position(start);
    let end_hit = text_layout.hit_position(end);

    text_layout
        .layout_runs()
        .enumerate()
        .filter_map(move |(current_line, run)| {
            if current_line < start_hit.line || current_line > end_hit.line {
                return None;
            }

            let x = if current_line == start_hit.line {
                start_hit.point.x
            } else {
                run.glyphs.first().map(|g| g.x).unwrap_or(0.0) as f64
            };
            let end_x = if current_line == end_hit.line {
                end_hit.point.x
            } else {
                run.glyphs.last().map(|g| g.x + g.w).unwrap_or(0.0) as f64
            };
            let width = end_x - x;

            if width == 0.0 {
                return None;
            }

            let height = (run.max_ascent + run.max_descent) as f64;
            let y = run.line_y as f64 - run.max_ascent as f64;

            Some(LineExtraStyle {
                x,
                y,
                width: Some(width),
                height,
                bg_color,
                under_line,
                wave_line,
            })
        })
}

#[cfg(test)]
mod toggle_comment_tests {
    use lapce_core::{
        buffer::{Buffer, rope_text::RopeText},
        command::EditCommand,
        cursor::{Cursor, CursorMode},
        editor::{Action, EditConf},
        language::LapceLanguage,
        register::{Clipboard, Register},
        selection::Selection,
        syntax::Syntax,
    };

    struct NoClipboard;
    impl Clipboard for NoClipboard {
        fn get_string(&mut self) -> Option<String> {
            None
        }
        fn put_string(&mut self, _s: impl AsRef<str>) {}
    }

    fn toggle_range(
        text: &str,
        sel: (usize, usize),
        comment_tokens: &dyn Fn(usize) -> (&'static str, &'static str),
    ) -> String {
        let mut buffer = Buffer::new(text);
        let mut cursor = Cursor::new(
            CursorMode::Insert(Selection::region(sel.0, sel.1)),
            None,
            None,
        );
        let mut register = Register::default();
        Action::do_edit(
            &mut cursor,
            &mut buffer,
            &EditCommand::ToggleLineComment,
            &mut NoClipboard,
            &mut register,
            EditConf {
                comment_tokens,
                modal: false,
                smart_tab: false,
                keep_indent: true,
                auto_indent: true,
            },
        );
        buffer.text().to_string()
    }

    fn toggle(text: &str, start: &'static str, end: &'static str) -> String {
        toggle_range(text, (0, text.len()), &move |_: usize| (start, end))
    }

    #[test]
    fn toggle_leaves_region_crossing_selection_unchanged() {
        // A selection covering template AND script lines resolves two token
        // pairs. Wrapping each line with its region's delimiters cannot
        // round-trip (commenting the boundary tags destroys the region on
        // the fresh snapshot and strands the inner lines), so the whole
        // toggle is a no-op: the user toggles one region at a time.
        let vue = "<template>\n  <p>x</p>\n</template>\n<script>\nlet a = 1\n</script>\n";
        let snapshot = lapce_core::lexilla::highlight(LapceLanguage::Vue, &vue.into()).unwrap();
        let result = toggle_range(vue, (0, vue.len()), &|offset| {
            LapceLanguage::Vue.comment_tokens_at(Some(snapshot.as_ref()), offset)
        });
        assert_eq!(result, vue);
    }

    #[test]
    fn toggle_round_trips_homogeneous_region_selections() {
        let vue = "<template>\n  <p>x</p>\n</template>\n<script>\nlet a = 1\n</script>\n";
        let toggle = |text: &str, sel: (usize, usize)| -> String {
            let snapshot =
                lapce_core::lexilla::highlight(LapceLanguage::Vue, &text.into()).unwrap();
            toggle_range(text, sel, &|offset| {
                LapceLanguage::Vue.comment_tokens_at(Some(snapshot.as_ref()), offset)
            })
        };
        // Template region only: markup pair, restored exactly by a second
        // toggle (commented markup repaints SCE_H_COMMENT, still markup).
        let template_end = vue.find("</template>").unwrap() + "</template>".len();
        let commented = toggle(vue, (0, template_end));
        assert_eq!(
            commented,
            "<!-- <template> -->\n<!--   <p>x</p> -->\n<!-- </template> -->\n<script>\nlet a = 1\n</script>\n"
        );
        assert_eq!(toggle(&commented, (0, template_end + 14)), vue);
        // Script body only: "//", restored exactly on the fresh snapshot
        // where the line paints SCE_HJ_COMMENTLINE (43), still the script
        // region because the boundary tags were never touched.
        let js_start = vue.find("let a").unwrap();
        let js_end = js_start + "let a = 1".len();
        let commented = toggle(vue, (js_start, js_end));
        assert_eq!(
            commented,
            "<template>\n  <p>x</p>\n</template>\n<script>\n// let a = 1\n</script>\n"
        );
        assert_eq!(toggle(&commented, (js_start, js_end + 3)), vue);
    }

    #[test]
    fn toggle_leaves_style_block_and_text_selections_unchanged() {
        // A whole <style> block crosses regions (markup tags + CSS body) and
        // plain text has no region: both selections are full no-ops, never
        // partial edits.
        let toggle = |text: &str| -> String {
            let snapshot =
                lapce_core::lexilla::highlight(LapceLanguage::Html, &text.into()).unwrap();
            toggle_range(text, (0, text.len()), &|offset| {
                LapceLanguage::Html.comment_tokens_at(Some(snapshot.as_ref()), offset)
            })
        };
        let style_block = "<style>\np { color: red }\n</style>\n";
        assert_eq!(toggle(style_block), style_block);
        let with_text = "<p>x</p>\nplain text\n<p>y</p>\n";
        assert_eq!(toggle(with_text), with_text);
    }

    #[test]
    fn toggle_round_trips_css_body_with_css_block_comment() {
        // The embed_web span is a reliable CSS signal: a CSS-body-only
        // selection toggles with the CSS block comment and the second toggle
        // (fresh snapshot, span re-detected from the tag structure) restores
        // byte-for-byte.
        let doc = "<style>\np { color: red }\n</style>\n";
        let body_start = doc.find("p { color").unwrap();
        let body_end = body_start + "p { color: red }".len();
        let toggle = |text: &str| -> String {
            let snapshot =
                lapce_core::lexilla::highlight(LapceLanguage::Html, &text.into()).unwrap();
            toggle_range(text, (body_start, body_end), &|offset| {
                LapceLanguage::Html.comment_tokens_at(Some(snapshot.as_ref()), offset)
            })
        };
        let commented = toggle(doc);
        assert_eq!(commented, "<style>\n/* p { color: red } */\n</style>\n");
        assert_eq!(toggle(&commented), doc);
    }

    #[test]
    fn resolver_uses_fresh_text_when_buffer_edited_before_relex() {
        // The asynchronously maintained syntax snapshot lags edits; the
        // production resolver is rebuilt from the CURRENT buffer text at
        // toggle time, so an edit-before-relex cannot strand stale regions.
        let before = lapce_xi_rope::Rope::from("<p>x</p>\n");
        let resolver =
            super::Doc::fresh_comment_token_resolver(LapceLanguage::Html, &before, &Default::default());
        assert_eq!(resolver(0), ("<!--", "-->"));
        let after = lapce_xi_rope::Rope::from("<script>\nlet a = 1\n</script>\n");
        let resolver =
            super::Doc::fresh_comment_token_resolver(LapceLanguage::Html, &after, &Default::default());
        let off = after.to_string().find("let a").unwrap();
        assert_eq!(resolver(off), ("//", ""));
    }

    #[test]
    fn resolver_follows_language_switch() {
        // After a language switch the resolver uses the NEW language with a
        // fresh snapshot: no stale old-language regions or lexer numbering.
        let text = lapce_xi_rope::Rope::from("<?php echo 1; ?>\n");
        let resolver =
            super::Doc::fresh_comment_token_resolver(LapceLanguage::Php, &text, &Default::default());
        assert_eq!(resolver(0), ("//", ""));
        let resolver =
            super::Doc::fresh_comment_token_resolver(LapceLanguage::Html, &text, &Default::default());
        assert_eq!(resolver(0), ("", ""));
    }

    #[test]
    fn comment_token_source_gates_region_resolution_cost() {
        use super::CommentTokenSource::*;
        // Ordinary edits (every keystroke) never lex, even on a regional
        // language with a stale snapshot: only ToggleLineComment consults
        // the resolver, so the keystroke path keeps its previous cost.
        assert_eq!(
            super::Doc::comment_token_source(
                &EditCommand::InsertNewLine,
                LapceLanguage::Html,
                false,
                10
            ),
            Global
        );
        // Toggles on single-syntax languages never need regions.
        assert_eq!(
            super::Doc::comment_token_source(
                &EditCommand::ToggleLineComment,
                LapceLanguage::Rust,
                false,
                10
            ),
            Global
        );
        // A revision-valid snapshot is reused with no new lexing, at any
        // size.
        assert_eq!(
            super::Doc::comment_token_source(
                &EditCommand::ToggleLineComment,
                LapceLanguage::Html,
                true,
                usize::MAX
            ),
            ReuseSnapshot
        );
        // Stale snapshot: a bounded fresh lex for ordinary files...
        assert_eq!(
            super::Doc::comment_token_source(
                &EditCommand::ToggleLineComment,
                LapceLanguage::Html,
                false,
                10
            ),
            FreshLex
        );
        // ...and a bounded no-op veto past the size cap, never an unbounded
        // synchronous lex.
        assert_eq!(
            super::Doc::comment_token_source(
                &EditCommand::ToggleLineComment,
                LapceLanguage::Html,
                false,
                (1 << 20) + 1
            ),
            Veto
        );
    }

    #[test]
    fn ordinary_edit_resolver_never_lexes() {
        // Behavioral control for the keystroke path: with no current
        // snapshot, a non-toggle command resolves with the language-wide
        // token even where a fresh lex would say otherwise - proof that no
        // full-file lex ran. The same buffer under ToggleLineComment lexes
        // fresh and sees the script region.
        let text = "<script>\nlet a = 1\n</script>\n";
        let buffer = Buffer::new(text);
        let mut syntax = Syntax::from_language(LapceLanguage::Html);
        syntax.rev = buffer.rev().wrapping_add(1); // no snapshot is current
        let off = text.find("let a").unwrap();
        let resolver =
            super::Doc::comment_token_resolver(&EditCommand::InsertNewLine, &syntax, &buffer);
        assert_eq!(resolver(off), LapceLanguage::Html.comment_tokens());
        let resolver = super::Doc::comment_token_resolver(
            &EditCommand::ToggleLineComment,
            &syntax,
            &buffer,
        );
        assert_eq!(resolver(off), ("//", ""));
    }

    #[test]
    fn toggle_reuses_revision_valid_snapshot_without_lexing() {
        // A snapshot valid for the current buffer revision is reused as-is.
        // The test deliberately pairs the buffer with a snapshot lexed from
        // DIFFERENT text so a fresh lex is detectable: reused, offset 10 is
        // markup (the `<span>` tag in `other`); freshly lexed, it would be
        // the JavaScript body of the buffer and resolve to `//`.
        let buffer = Buffer::new("<script>\nlet a = 1\n</script>\n");
        let other = "<p>xx</p>\n<span>y</span>\n";
        let mut syntax = Syntax::from_language(LapceLanguage::Html);
        syntax.rev = buffer.rev();
        syntax.lexilla = Some(
            lapce_core::lexilla::highlight(LapceLanguage::Html, &other.into()).unwrap(),
        );
        let resolver = super::Doc::comment_token_resolver(
            &EditCommand::ToggleLineComment,
            &syntax,
            &buffer,
        );
        assert_eq!(resolver(10), ("<!--", "-->"));
    }

    #[test]
    fn toggle_resolves_unlexable_offset_to_veto_not_global() {
        // A required region lookup that cannot be answered - here an offset
        // past the highlighted text - must veto the toggle, never fall back
        // to the language-wide token on mixed content.
        let text = "<script>\nlet a = 1\n</script>\n";
        let resolver = super::Doc::fresh_comment_token_resolver(
            LapceLanguage::Html,
            &text.into(),
            &Default::default(),
        );
        assert_eq!(resolver(text.len() + 100), ("", ""));
        assert_ne!(LapceLanguage::Html.comment_tokens().0, "");
    }

    #[test]
    fn toggle_wraps_and_unwraps_block_delimiters() {
        // Full delimiter pair through the real consumer: insert writes both
        // `<!--` and `-->`, toggle-back strips both and restores the input.
        let text = "<div>hi</div>\n  <p>x</p>\n";
        let commented = toggle(text, "<!--", "-->");
        assert_eq!(commented, "<!-- <div>hi</div> -->\n<!--   <p>x</p> -->\n");
        assert_eq!(toggle(&commented, "<!--", "-->"), text);
    }

    #[test]
    fn toggle_keeps_prefix_only_languages_unchanged() {
        let text = "let a = 1;\nlet b = 2;\n";
        let commented = toggle(text, "//", "");
        assert_eq!(commented, "// let a = 1;\n// let b = 2;\n");
        assert_eq!(toggle(&commented, "//", ""), text);
    }

    #[test]
    fn toggle_is_noop_for_tokenless_languages() {
        let text = "{\"a\": 1}\n";
        assert_eq!(toggle(text, "", ""), text);
    }

    #[test]
    fn toggle_blade_pair_round_trips() {
        let text = "<p>{{ $x }}</p>\n";
        let commented = toggle(text, "{{--", "--}}");
        assert_eq!(commented, "{{-- <p>{{ $x }}</p> --}}\n");
        assert_eq!(toggle(&commented, "{{--", "--}}"), text);
    }

    #[test]
    fn toggle_strips_degenerate_empty_comment() {
        let text = "<!-- -->\n";
        assert_eq!(toggle(text, "<!--", "-->"), "\n");
    }

    #[test]
    fn toggle_block_delimiters_on_multiline_after_first_line() {
        // Regression: the closing delimiter used to land one byte early on
        // every line after the first because line starts were summed from
        // newline-stripped content lengths.
        let text = "a\nb\nc\n";
        let commented = toggle(text, "<!--", "-->");
        assert_eq!(commented, "<!-- a -->\n<!-- b -->\n<!-- c -->\n");
        assert_eq!(toggle(&commented, "<!--", "-->"), text);
    }

    #[test]
    fn toggle_block_delimiters_with_multibyte_text() {
        // Offsets are bytes: the closing delimiter must land after multi-byte
        // characters, not inside them.
        let text = "\u{3bb} = 1\n\u{1f642}\n";
        let commented = toggle(text, "<!--", "-->");
        assert_eq!(commented, "<!-- \u{3bb} = 1 -->\n<!-- \u{1f642} -->\n");
        assert_eq!(toggle(&commented, "<!--", "-->"), text);
    }

    #[test]
    fn toggle_block_delimiters_with_crlf() {
        // The closing delimiter belongs before the line ending, not after the
        // `\r` and not one byte into the next line.
        let text = "a\r\nb\r\n";
        let commented = toggle(text, "<!--", "-->");
        assert_eq!(commented, "<!-- a -->\r\n<!-- b -->\r\n");
        assert_eq!(toggle(&commented, "<!--", "-->"), text);
    }

    #[test]
    fn toggle_block_delimiters_at_eof_without_newline() {
        let text = "a\nb";
        let commented = toggle(text, "<!--", "-->");
        assert_eq!(commented, "<!-- a -->\n<!-- b -->");
        assert_eq!(toggle(&commented, "<!--", "-->"), text);
    }
}

/// Return columns for the visible intersection only. A touching range does not
/// decorate a protected header or spill into another source's first line.
fn clip_diagnostic_line(line: std::ops::Range<usize>, diagnostic: std::ops::Range<usize>) -> Option<std::ops::Range<usize>> {
    if diagnostic.start == diagnostic.end && line.start <= diagnostic.start && diagnostic.start < line.end {
        let col = diagnostic.start - line.start;
        return Some(col..col);
    }
    let start = line.start.max(diagnostic.start);
    let end = line.end.min(diagnostic.end);
    (start < end).then(|| start - line.start..end - line.start)
}

#[cfg(test)]
mod projection_diagnostic_style_tests {
    use super::clip_diagnostic_line;
    #[test] fn canonical_offset_becomes_display_columns() {
        assert_eq!(clip_diagnostic_line(270000..270020, 270009..270015), Some(9..15));
    }
    #[test] fn multiline_diagnostic_is_clipped_at_both_ends() {
        assert_eq!(clip_diagnostic_line(100..120, 90..130), Some(0..20));
        assert_eq!(clip_diagnostic_line(100..120, 90..108), Some(0..8));
    }
    #[test] fn adjacent_source_and_header_ranges_do_not_bleed() {
        assert_eq!(clip_diagnostic_line(100..120, 90..100), None);
        assert_eq!(clip_diagnostic_line(100..120, 120..125), None);
        assert_eq!(clip_diagnostic_line(100..100, 90..130), None);
    }
    #[test] fn zero_width_diagnostic_keeps_its_severity_on_its_line() {
        assert_eq!(clip_diagnostic_line(100..120, 109..109), Some(9..9));
        assert_eq!(clip_diagnostic_line(100..120, 120..120), None);
    }
    #[test] fn utf8_columns_remain_byte_based() {
        let text = "日本語 BROKEN";
        let start = text.find("BROKEN").unwrap();
        assert_eq!(clip_diagnostic_line(100..100+text.len(), 100+start..100+text.len()), Some(start..text.len()));
        assert!(text.is_char_boundary(start));
    }
}
