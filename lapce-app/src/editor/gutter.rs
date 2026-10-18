use floem::{
    Renderer, View, ViewId,
    context::PaintCx,
    peniko::kurbo::{Point, Rect, Size},
    reactive::{Memo, SignalGet, SignalWith},
    text::{Attrs, AttrsList, FamilyOwned, TextLayout},
};
use lapce_core::{buffer::rope_text::RopeText, mode::Mode};
use serde::{Deserialize, Serialize};

use super::{EditorData, view::changes_colors_screen};
use crate::config::{LapceConfig, color::LapceColor};

pub struct EditorGutterView {
    id: ViewId,
    editor: EditorData,
    width: f64,
    gutter_padding_right: Memo<f32>,
}

pub fn editor_gutter_view(
    editor: EditorData,
    gutter_padding_right: Memo<f32>,
) -> EditorGutterView {
    let id = ViewId::new();

    EditorGutterView {
        id,
        editor,
        width: 0.0,
        gutter_padding_right,
    }
}

/// Whether the gutter should show modal-relative line numbers.
/// Read-only docs (git blame, git show, history views) always show
/// absolute numbers: their gutter mirrors the underlying file's line
/// numbering, and relative numbering there rendered as misleading
/// cursor distances (the blame gutter read 1, 1, 2, 3 for a 4-line
/// view with the cursor on line 1).
fn show_relative_line_numbers(
    modal: bool,
    relative_config: bool,
    mode: Mode,
    kind_is_normal: bool,
    doc_read_only: bool,
) -> bool {
    modal
        && relative_config
        && mode != Mode::Insert
        && kind_is_normal
        && !doc_read_only
}

impl EditorGutterView {
    fn paint_head_changes(
        &self,
        cx: &mut PaintCx,
        e_data: &EditorData,
        viewport: Rect,
        is_normal: bool,
        config: &LapceConfig,
    ) {
        if !is_normal {
            return;
        }

        let changes = e_data.doc().head_changes().get_untracked();
        let line_height = config.editor.line_height() as f64;
        let gutter_padding_right = self.gutter_padding_right.get_untracked() as f64;

        let changes = changes_colors_screen(config, &e_data.editor, changes);
        for (y, height, removed, color) in changes {
            let height = if removed {
                10.0
            } else {
                height as f64 * line_height
            };
            let mut y = y - viewport.y0;
            if removed {
                y -= 5.0;
            }
            cx.fill(
                &Size::new(3.0, height).to_rect().with_origin(Point::new(
                    self.width + 5.0 - gutter_padding_right,
                    y,
                )),
                color,
                0.0,
            )
        }
    }

    fn paint_sticky_headers(
        &self,
        cx: &mut PaintCx,
        is_normal: bool,
        config: &LapceConfig,
    ) {
        if !is_normal {
            return;
        }

        if !config.editor.sticky_header {
            return;
        }
        let sticky_header_height = self.editor.sticky_header_height.get_untracked();
        if sticky_header_height == 0.0 {
            return;
        }

        let sticky_area_rect =
            Size::new(self.width + 25.0 + 30.0, sticky_header_height)
                .to_rect()
                .with_origin(Point::new(-25.0, 0.0))
                .inflate(25.0, 0.0);
        cx.fill(
            &sticky_area_rect,
            config.color(LapceColor::LAPCE_DROPDOWN_SHADOW),
            3.0,
        );
        cx.fill(
            &sticky_area_rect,
            config.color(LapceColor::EDITOR_STICKY_HEADER_BACKGROUND),
            0.0,
        );
    }
}

impl View for EditorGutterView {
    fn id(&self) -> ViewId {
        self.id
    }

    fn compute_layout(
        &mut self,
        _cx: &mut floem::context::ComputeLayoutCx,
    ) -> Option<floem::peniko::kurbo::Rect> {
        if let Some(width) = self.id.get_layout().map(|l| l.size.width as f64) {
            self.width = width;
        }
        None
    }

    fn paint(&mut self, cx: &mut floem::context::PaintCx) {
        let viewport = self.editor.viewport().get_untracked();
        let cursor = self.editor.cursor();
        let screen_lines = self.editor.screen_lines();
        let config = self.editor.common.config;

        let kind_is_normal =
            self.editor.kind.with_untracked(|kind| kind.is_normal());
        let (offset, mode) = cursor.with_untracked(|c| (c.offset(), c.get_mode()));
        let config = config.get_untracked();
        let line_height = config.editor.line_height() as f64;
        let last_line = self.editor.editor.last_line();
        let doc = self.editor.doc();
        let current_line = doc
            .buffer
            .with_untracked(|buffer| buffer.line_of_offset(offset));
        let doc_read_only =
            doc.content.with_untracked(|content| content.read_only());

        let family: Vec<FamilyOwned> =
            FamilyOwned::parse_list(&config.editor.font_family).collect();
        let attrs = Attrs::new()
            .family(&family)
            .color(config.color(LapceColor::EDITOR_DIM))
            .font_size(config.editor.font_size() as f32);
        let attrs_list = AttrsList::new(attrs.clone());
        let current_line_attrs_list = AttrsList::new(
            attrs
                .clone()
                .color(config.color(LapceColor::EDITOR_FOREGROUND)),
        );
        let show_relative = show_relative_line_numbers(
            config.core.modal,
            config.editor.modal_mode_relative_line_numbers,
            mode,
            kind_is_normal,
            doc_read_only,
        );

        screen_lines.with_untracked(|screen_lines| {
            for (line, y) in screen_lines.iter_lines_y() {
                // If it ends up outside the bounds of the file, stop trying to display line numbers
                if line > last_line {
                    break;
                }

                let text = if show_relative {
                    if line == current_line {
                        line + 1
                    } else {
                        line.abs_diff(current_line)
                    }
                } else {
                    line + 1
                }
                .to_string();

                let mut text_layout = TextLayout::new();
                if line == current_line {
                    text_layout.set_text(
                        &text,
                        current_line_attrs_list.clone(),
                        None,
                    );
                } else {
                    text_layout.set_text(&text, attrs_list.clone(), None);
                }
                let size = text_layout.size();
                let height = size.height;

                cx.draw_text(
                    &text_layout,
                    Point::new(
                        (self.width
                            - size.width
                            - self.gutter_padding_right.get_untracked() as f64)
                            .max(0.0),
                        y + (line_height - height) / 2.0 - viewport.y0,
                    ),
                );
            }
        });

        self.paint_head_changes(cx, &self.editor, viewport, kind_is_normal, &config);
        self.paint_sticky_headers(cx, kind_is_normal, &config);
    }

    fn debug_name(&self) -> std::borrow::Cow<'static, str> {
        "Editor Gutter".into()
    }
}

/// A fold marker in the gutter: the header line of a foldable block.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct FoldingDisplayItem {
    pub line: usize,
    pub folded: bool,
}

impl FoldingDisplayItem {
    /// Markers for the block headers in `lines` (buffer lines that are on screen).
    pub fn for_lines(
        state: &lapce_core::folding::FoldState,
        lines: impl Fn(usize) -> bool,
    ) -> Vec<FoldingDisplayItem> {
        state
            .ranges()
            .iter()
            .filter(|range| !state.is_hidden(range.start) && lines(range.start))
            .map(|range| FoldingDisplayItem {
                line: range.start,
                folded: state.is_folded(range.start),
            })
            .collect()
    }
}

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize, Clone, Hash, Copy)]
pub enum FoldingRangeKind {
    Comment,
    Imports,
    Region,
}

impl From<lsp_types::FoldingRangeKind> for FoldingRangeKind {
    fn from(value: lsp_types::FoldingRangeKind) -> Self {
        match value {
            lsp_types::FoldingRangeKind::Comment => FoldingRangeKind::Comment,
            lsp_types::FoldingRangeKind::Imports => FoldingRangeKind::Imports,
            lsp_types::FoldingRangeKind::Region => FoldingRangeKind::Region,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::show_relative_line_numbers;
    use lapce_core::mode::Mode;

    #[test]
    fn read_only_docs_always_show_absolute_numbers() {
        // Blame gutter regression: modal + relative numbers rendered
        // the read-only blame gutter as 1, 1, 2, 3 (cursor distances).
        assert!(!show_relative_line_numbers(
            true,
            true,
            Mode::Normal,
            true,
            true
        ));
    }

    #[test]
    fn editable_docs_follow_the_modal_relative_config() {
        assert!(show_relative_line_numbers(
            true,
            true,
            Mode::Normal,
            true,
            false
        ));
        assert!(!show_relative_line_numbers(
            true,
            true,
            Mode::Insert,
            true,
            false
        ));
        assert!(!show_relative_line_numbers(
            true,
            false,
            Mode::Normal,
            true,
            false
        ));
        assert!(!show_relative_line_numbers(
            false,
            true,
            Mode::Normal,
            true,
            false
        ));
        assert!(!show_relative_line_numbers(
            true,
            true,
            Mode::Normal,
            false,
            false
        ));
    }
}
