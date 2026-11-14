use std::{ops::AddAssign, path::PathBuf, rc::Rc};

use floem::{
    View,
    peniko::Color,
    reactive::{RwSignal, Scope, SignalGet, SignalUpdate, SignalWith},
    style::CursorStyle,
    views::{
        Decorators, VirtualVector, container, editor::id::Id, label, scroll, stack,
        svg, virtual_stack,
    },
};
use lsp_types::{DocumentSymbol, SymbolKind};

use super::position::PanelPosition;
use crate::{
    command::InternalCommand,
    config::{color::LapceColor, icon::LapceIcons},
    editor::location::EditorLocation,
    window_tab::WindowTabData,
};

#[derive(Clone, Debug)]
pub struct SymbolData {
    pub path: PathBuf,
    pub file: RwSignal<SymbolInformationItemData>,
}

impl SymbolData {
    pub fn new(
        items: Vec<RwSignal<SymbolInformationItemData>>,
        path: PathBuf,
        cx: Scope,
    ) -> Self {
        let name = path
            .file_name()
            .and_then(|x| x.to_str())
            .map(|x| x.to_string())
            .unwrap_or_default();
        #[allow(deprecated)]
        let file_ds = DocumentSymbol {
            name: name.clone(),
            detail: None,
            kind: SymbolKind::FILE,
            tags: None,
            deprecated: None,
            range: Default::default(),
            selection_range: Default::default(),
            children: None,
        };
        let file = cx.create_rw_signal(SymbolInformationItemData {
            id: Id::next(),
            name,
            detail: None,
            item: file_ds,
            open: cx.create_rw_signal(true),
            children: items,
        });
        Self { path, file }
    }
    fn get_children(
        &self,
        min: usize,
        max: usize,
    ) -> Vec<(
        usize,
        usize,
        Rc<PathBuf>,
        RwSignal<SymbolInformationItemData>,
    )> {
        let path = Rc::new(self.path.clone());
        let level: usize = 0;
        let mut next = 0;
        get_children(self.file, &mut next, min, max, level, path.clone())
    }
}

#[derive(Debug, Clone)]
pub struct SymbolInformationItemData {
    pub id: Id,
    pub name: String,
    pub detail: Option<String>,
    pub item: DocumentSymbol,
    pub open: RwSignal<bool>,
    pub children: Vec<RwSignal<SymbolInformationItemData>>,
}

impl From<(DocumentSymbol, Scope)> for SymbolInformationItemData {
    fn from((mut item, cx): (DocumentSymbol, Scope)) -> Self {
        let children = if let Some(children) = item.children.take() {
            children
                .into_iter()
                .map(|x| cx.create_rw_signal(Self::from((x, cx))))
                .collect()
        } else {
            Vec::with_capacity(0)
        };
        Self {
            id: Id::next(),
            name: item.name.clone(),
            detail: item.detail.clone(),
            item,
            open: cx.create_rw_signal(true),
            children,
        }
    }
}

impl SymbolInformationItemData {
    pub fn child_count(&self) -> usize {
        let mut count = 1;
        if self.open.get() {
            for child in &self.children {
                count += child.with(|x| x.child_count())
            }
        }
        count
    }
}

fn get_children(
    data: RwSignal<SymbolInformationItemData>,
    next: &mut usize,
    min: usize,
    max: usize,
    level: usize,
    path: Rc<PathBuf>,
) -> Vec<(
    usize,
    usize,
    Rc<PathBuf>,
    RwSignal<SymbolInformationItemData>,
)> {
    let mut children = Vec::new();
    if *next >= min && *next < max {
        children.push((*next, level, path.clone(), data));
    } else if *next >= max {
        return children;
    }
    next.add_assign(1);
    if data.get_untracked().open.get() {
        for child in data.get().children {
            let child_children =
                get_children(child, next, min, max, level + 1, path.clone());
            children.extend(child_children);
            if *next > max {
                break;
            }
        }
    }
    children
}

pub struct VirtualList {
    root: Option<RwSignal<Option<SymbolData>>>,
}

impl VirtualList {
    pub fn new(root: Option<RwSignal<Option<SymbolData>>>) -> Self {
        Self { root }
    }
}

impl
    VirtualVector<(
        usize,
        usize,
        Rc<PathBuf>,
        RwSignal<SymbolInformationItemData>,
    )> for VirtualList
{
    fn total_len(&self) -> usize {
        if let Some(root) = self.root.as_ref().and_then(|x| x.get()) {
            root.file.get_untracked().child_count()
        } else {
            0
        }
    }

    fn slice(
        &mut self,
        range: std::ops::Range<usize>,
    ) -> impl Iterator<
        Item = (
            usize,
            usize,
            Rc<PathBuf>,
            RwSignal<SymbolInformationItemData>,
        ),
    > {
        if let Some(root) = self.root.as_ref().and_then(|x| x.get()) {
            let min = range.start;
            let max = range.end;
            let children = root.get_children(min, max);
            children.into_iter()
        } else {
            Vec::new().into_iter()
        }
    }
}

pub fn symbol_panel(
    window_tab_data: Rc<WindowTabData>,
    _position: PanelPosition,
) -> impl View {
    let config = window_tab_data.common.config;
    let ui_line_height = window_tab_data.common.ui_line_height;
    scroll(
        virtual_stack(
            {
                let window_tab_data = window_tab_data.clone();
                move || {
                    let editor = window_tab_data.main_split.get_active_editor();
                    VirtualList::new(editor.map(|x| x.doc().document_symbol_data))
                }
            },
            move |(_, _, _, item)| item.get_untracked().id,
            move |(_, level, path,  rw_data)| {
                let data = rw_data.get_untracked();
                let open = data.open;
                let has_child = !data.children.is_empty();
                let kind = data.item.kind;
                stack((
                    container(
                        svg(move || {
                            let config = config.get();
                            let svg_str = match open.get() {
                                true => LapceIcons::ITEM_OPENED,
                                false => LapceIcons::ITEM_CLOSED,
                            };
                            config.ui_svg(svg_str)
                        })
                        .style(move |s| {
                            let config = config.get();
                            let color = if has_child {
                                config.color(LapceColor::LAPCE_ICON_ACTIVE)
                            } else {
                                Color::TRANSPARENT
                            };
                            let size = config.ui.icon_size() as f32;
                            s.size(size, size)
                                 .color(color)
                        })
                    ).style(|s| s.padding(4.0).margin_left(6.0).margin_right(2.0))
                    .on_click_stop({
                        move |_x| {
                            if has_child {
                                open.update(|x| {
                                    *x = !*x;
                                });
                            }
                        }
                    }),
                    svg(move || {
                        let config = config.get();
                        config
                            .symbol_svg(&kind)
                            .unwrap_or_else(|| config.ui_svg(LapceIcons::FILE))
                    }).style(move |s| {
                            let config = config.get();
                            let size = config.ui.icon_size() as f32;
                            s.min_width(size)
                                .size(size, size)
                                .margin_right(5.0)
                                .color(config.symbol_color(&kind).unwrap_or_else(|| {
                                    config.color(LapceColor::LAPCE_ICON_ACTIVE)
                                }))
                        }),
                    label(move || {
                            data.name.replace('\n', "↵")
                    })
                    .style(move |s| {
                        s.selectable(false)
                    }),
                    label(move || {
                        data.detail.clone().unwrap_or_default()
                    }).style(move |s| s.margin_left(6.0)
                                              .color(config.get().color(LapceColor::EDITOR_DIM))
                                              .selectable(false)
                                              .apply_if(
                                                data.item.detail.clone().is_none(),
                                                 |s| s.hide())
                    ),
                ))
                .style(move |s| {
                    s.padding_right(5.0)
                        .padding_left((level * 10) as f32)
                        .items_center()
                        .height(ui_line_height.get())
                        .hover(|s| {
                            s.background(
                                config
                                    .get()
                                    .color(LapceColor::PANEL_HOVERED_BACKGROUND),
                            )
                            .cursor(CursorStyle::Pointer)
                        })
                })
                .on_click_stop({
                    let window_tab_data = window_tab_data.clone();
                    let data = rw_data;
                    move |_| {
                        let data = data.get_untracked();
                            window_tab_data
                                .common
                                .internal_command
                                .send(InternalCommand::JumpToLocation { location: EditorLocation {
                                    path: path.to_path_buf(),
                                    position: Some(crate::editor::location::EditorPosition::Position(data.item.selection_range.start)),
                                    scroll_offset: None,
                                    ignore_unconfirmed: false,
                                    same_editor_tab: false,
                                } });
                    }
                })
            },
        ).item_size_fixed(move || ui_line_height.get())
        .style(|s| s.flex_col().absolute().min_width_full()),
    )
    .style(|s| s.absolute().size_full())
}

/// Chain of symbol names containing the given position, outermost first.
/// Used by the editor breadcrumbs bar to show the syntax context at the
/// cursor. `line` and `character` are zero-based; a position on a symbol
/// boundary counts as inside the symbol.
pub fn symbol_path_at(
    items: &[RwSignal<SymbolInformationItemData>],
    line: u32,
    character: u32,
) -> Vec<String> {
    for item in items {
        let hit = item.with_untracked(|data| {
            if !range_contains(&data.item.range, line, character) {
                return None;
            }
            let mut path = vec![data.name.clone()];
            path.extend(symbol_path_at(&data.children, line, character));
            Some(path)
        });
        if let Some(path) = hit {
            return path;
        }
    }
    Vec::new()
}

fn range_contains(
    range: &lsp_types::Range,
    line: u32,
    character: u32,
) -> bool {
    let start = range.start;
    let end = range.end;
    (line > start.line || (line == start.line && character >= start.character))
        && (line < end.line || (line == end.line && character <= end.character))
}

/// Convert a flat `SymbolInformation` list (returned by language servers
/// when the client does not advertise hierarchical document symbol support)
/// into a nested `DocumentSymbol` tree, keeping only symbols for `uri`.
/// Children are symbols strictly contained in another symbol's range.
pub fn flat_to_nested(
    infos: Vec<lsp_types::SymbolInformation>,
    uri: &lsp_types::Url,
) -> Vec<DocumentSymbol> {
    let mut flat: Vec<DocumentSymbol> = infos
        .into_iter()
        .filter(|info| &info.location.uri == uri)
        .map(|info| {
            #[allow(deprecated)]
            DocumentSymbol {
                name: info.name,
                detail: info.container_name,
                kind: info.kind,
                tags: info.tags,
                deprecated: info.deprecated,
                range: info.location.range,
                selection_range: info.location.range,
                children: Some(Vec::new()),
            }
        })
        .collect();
    // Outer symbols first: earlier start wins; on ties the wider range wins.
    flat.sort_by(|a, b| {
        (a.range.start.line, a.range.start.character)
            .cmp(&(b.range.start.line, b.range.start.character))
            .then(
                (b.range.end.line, b.range.end.character)
                    .cmp(&(a.range.end.line, a.range.end.character)),
            )
    });
    let mut roots: Vec<DocumentSymbol> = Vec::new();
    for sym in flat {
        insert_nested(&mut roots, sym);
    }
    roots
}

fn range_strictly_contains(
    outer: &lsp_types::Range,
    inner: &lsp_types::Range,
) -> bool {
    let start_inside = inner.start.line > outer.start.line
        || (inner.start.line == outer.start.line
            && inner.start.character >= outer.start.character);
    let end_inside = inner.end.line < outer.end.line
        || (inner.end.line == outer.end.line
            && inner.end.character <= outer.end.character);
    let strictly = inner.start != outer.start || inner.end != outer.end;
    start_inside && end_inside && strictly
}

fn insert_nested(into: &mut Vec<DocumentSymbol>, sym: DocumentSymbol) {
    // Descend into the last (deepest) chain whose range contains `sym`.
    if let Some(last) = into.last_mut() {
        if range_strictly_contains(&last.range, &sym.range) {
            insert_nested(
                last.children.get_or_insert_with(Vec::new),
                sym,
            );
            return;
        }
    }
    into.push(sym);
}

#[cfg(test)]
mod tests {
    use super::*;
    use floem::reactive::Scope;
    use lsp_types::Position;

    fn sym(
        name: &str,
        start: (u32, u32),
        end: (u32, u32),
        children: Vec<DocumentSymbol>,
    ) -> DocumentSymbol {
        #[allow(deprecated)]
        DocumentSymbol {
            name: name.to_string(),
            detail: None,
            kind: SymbolKind::FUNCTION,
            tags: None,
            deprecated: None,
            range: lsp_types::Range::new(
                Position::new(start.0, start.1),
                Position::new(end.0, end.1),
            ),
            selection_range: lsp_types::Range::new(
                Position::new(start.0, start.1),
                Position::new(end.0, end.1),
            ),
            children: if children.is_empty() {
                None
            } else {
                Some(children)
            },
        }
    }

    fn items(
        cx: Scope,
        syms: Vec<DocumentSymbol>,
    ) -> Vec<RwSignal<SymbolInformationItemData>> {
        syms.into_iter()
            .map(|s| cx.create_rw_signal(SymbolInformationItemData::from((s, cx))))
            .collect()
    }

    #[test]
    fn symbol_path_nested() {
        let cx = Scope::new();
        let tree = vec![sym(
            "outer",
            (0, 0),
            (10, 0),
            vec![sym("inner", (2, 0), (5, 0), vec![])],
        )];
        let items = items(cx, tree);
        assert_eq!(
            symbol_path_at(&items, 3, 4),
            vec!["outer".to_string(), "inner".to_string()]
        );
        assert_eq!(symbol_path_at(&items, 7, 0), vec!["outer".to_string()]);
        assert!(symbol_path_at(&items, 12, 0).is_empty());
    }

    #[test]
    fn symbol_path_boundaries() {
        let cx = Scope::new();
        let tree = vec![sym("a", (1, 2), (3, 4), vec![])];
        let items = items(cx, tree);
        // Inclusive on both ends.
        assert_eq!(symbol_path_at(&items, 1, 2), vec!["a".to_string()]);
        assert_eq!(symbol_path_at(&items, 3, 4), vec!["a".to_string()]);
        assert!(symbol_path_at(&items, 1, 1).is_empty());
        assert!(symbol_path_at(&items, 3, 5).is_empty());
    }

    #[test]
    fn symbol_path_siblings() {
        let cx = Scope::new();
        let tree = vec![
            sym("a", (0, 0), (5, 0), vec![]),
            sym("b", (6, 0), (9, 0), vec![sym("c", (7, 0), (8, 0), vec![])]),
        ];
        let items = items(cx, tree);
        assert_eq!(symbol_path_at(&items, 4, 0), vec!["a".to_string()]);
        assert_eq!(
            symbol_path_at(&items, 7, 5),
            vec!["b".to_string(), "c".to_string()]
        );
        assert!(symbol_path_at(&items, 5, 1).is_empty());
    }

    #[test]
    fn symbol_path_empty() {
        let items: Vec<RwSignal<SymbolInformationItemData>> = Vec::new();
        assert!(symbol_path_at(&items, 0, 0).is_empty());
    }

    fn flat_info(
        name: &str,
        start: (u32, u32),
        end: (u32, u32),
        uri: &lsp_types::Url,
    ) -> lsp_types::SymbolInformation {
        #[allow(deprecated)]
        lsp_types::SymbolInformation {
            name: name.to_string(),
            kind: SymbolKind::FUNCTION,
            tags: None,
            deprecated: None,
            location: lsp_types::Location {
                uri: uri.clone(),
                range: lsp_types::Range::new(
                    Position::new(start.0, start.1),
                    Position::new(end.0, end.1),
                ),
            },
            container_name: None,
        }
    }

    #[test]
    fn flat_to_nested_nests_by_containment() {
        let uri = lsp_types::Url::parse("file:///proj/src/main.rs").unwrap();
        let infos = vec![
            flat_info("outer", (0, 0), (10, 0), &uri),
            flat_info("inner", (2, 0), (5, 0), &uri),
            flat_info("other", (12, 0), (14, 0), &uri),
        ];
        let tree = flat_to_nested(infos, &uri);
        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].name, "outer");
        let children = tree[0].children.as_ref().unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].name, "inner");
        assert_eq!(tree[1].name, "other");
    }

    #[test]
    fn flat_to_nested_filters_other_files_and_empty() {
        let uri = lsp_types::Url::parse("file:///proj/src/main.rs").unwrap();
        let other = lsp_types::Url::parse("file:///proj/src/lib.rs").unwrap();
        let infos = vec![flat_info("alien", (0, 0), (1, 0), &other)];
        assert!(flat_to_nested(infos, &uri).is_empty());
        assert!(flat_to_nested(Vec::new(), &uri).is_empty());
    }

    #[test]
    fn flat_to_nested_handles_unsorted_input() {
        let uri = lsp_types::Url::parse("file:///proj/src/main.rs").unwrap();
        // Inner arrives before outer.
        let infos = vec![
            flat_info("inner", (2, 0), (5, 0), &uri),
            flat_info("outer", (0, 0), (10, 0), &uri),
        ];
        let tree = flat_to_nested(infos, &uri);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "outer");
        assert_eq!(
            tree[0].children.as_ref().unwrap()[0].name,
            "inner".to_string()
        );
    }
}
