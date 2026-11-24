use std::{path::PathBuf, rc::Rc, sync::Arc};

use floem::{
    View, ViewId,
    event::{Event, EventListener, EventPropagation},
    peniko::Color,
    reactive::{
        ReadSignal, RwSignal, SignalGet, SignalUpdate, SignalWith, create_effect, create_memo,
        create_rw_signal,
    },
    style::{CursorStyle, Style},
    views::{Decorators, container, dyn_stack, label, scroll, stack, svg},
};
use lsp_types::{DiagnosticRelatedInformation, DiagnosticSeverity};

use super::{data::PanelSection, position::PanelPosition, view::PanelBuilder};
use crate::{
    command::InternalCommand,
    config::{LapceConfig, color::LapceColor, icon::LapceIcons},
    doc::{DiagnosticData, EditorDiagnostic},
    editor::location::{EditorLocation, EditorPosition},
    listener::Listener,
    lsp::path_from_url,
    window_tab::WindowTabData,
    workspace::LapceWorkspace,
};

pub fn problem_panel(
    window_tab_data: Rc<WindowTabData>,
    position: PanelPosition,
) -> impl View {
    let config = window_tab_data.common.config;
    let is_bottom = position.is_bottom();
    let available_width = create_rw_signal(800.0);
    let stacked = move || !is_bottom || available_width.get() < 480.0;
    PanelBuilder::new(config, position)
        .add_style(
            "Errors",
            problem_section(window_tab_data.clone(), DiagnosticSeverity::ERROR),
            window_tab_data.panel.section_open(PanelSection::Error),
            move |s| {
                s.border_color(config.get().color(LapceColor::LAPCE_BORDER))
                    .apply_if(!stacked(), |s| s.border_right(1.0))
                    .apply_if(stacked(), |s| s.border_bottom(1.0))
            },
        )
        .add(
            "Warnings",
            problem_section(window_tab_data.clone(), DiagnosticSeverity::WARNING),
            window_tab_data.panel.section_open(PanelSection::Warn),
        )
        .build()
        .on_resize(move |rect| available_width.set(rect.width()))
        .style(move |s| s.min_width(0.0).apply_if(stacked(), |s| s.flex_col()))
        .debug_name("Problem Panel")
}

fn problem_section(
    window_tab_data: Rc<WindowTabData>,
    severity: DiagnosticSeverity,
) -> impl View {
    let config = window_tab_data.common.config;
    let main_split = window_tab_data.main_split.clone();
    let diagnostic_split = main_split.clone();
    let has_diagnostics = create_memo(move |_| {
        diagnostic_split.diagnostics.get().iter().any(|(_, data)| {
            data.diagnostics.get().iter().any(|diagnostic| diagnostic.severity == Some(severity))
        })
    });
    let internal_command = window_tab_data.common.internal_command;
    let keyboard_focus = window_tab_data.common.keyboard_focus;
    container(stack((
        scroll(
            dyn_stack(
                move || main_split.diagnostics.get(),
                |(p, _)| p.clone(),
                move |(path, diagnostic_data)| {
                    file_view(
                        main_split.common.workspace.clone(),
                        path,
                        diagnostic_data,
                        severity,
                        internal_command,
                        config,
                        keyboard_focus,
                    )
                },
            )
            .style(|s| s.flex_col().width_pct(100.0).line_height(1.8)),
        )
        .style(|s| s.absolute().size_pct(100.0, 100.0)),
        label(move || problem_empty_message(severity))
            .style(move |s| {
                s.width_pct(100.0).max_width_pct(100.0).min_width(0.0).padding(12.0)
                    .text_overflow(floem::style::TextOverflow::Wrap)
                    .color(config.get().color(LapceColor::PANEL_FOREGROUND))
                    .apply_if(has_diagnostics.get(), |s| s.hide())
                    .selectable(false)
            }),
    )).style(|s| s.absolute().size_pct(100.0, 100.0).min_width(0.0)))
    .style(|s| s.size_pct(100.0, 100.0).min_width(0.0))
}

fn problem_empty_message(severity: DiagnosticSeverity) -> &'static str {
    if severity == DiagnosticSeverity::ERROR {
        "No errors reported. Diagnostics appear here when a language service reports them."
    } else {
        "No warnings reported. Diagnostics appear here when a language service reports them."
    }
}

#[cfg(test)]
mod problem_empty_tests {
    use super::*;

    #[test]
    fn empty_message_describes_reports_not_a_clean_code_check() {
        assert!(problem_empty_message(DiagnosticSeverity::ERROR).starts_with("No errors reported."));
        assert!(problem_empty_message(DiagnosticSeverity::WARNING).starts_with("No warnings reported."));
        for severity in [DiagnosticSeverity::ERROR, DiagnosticSeverity::WARNING] {
            assert!(problem_empty_message(severity).contains("when a language service reports them"));
        }
    }
}

fn file_view(
    workspace: Arc<LapceWorkspace>,
    path: PathBuf,
    diagnostic_data: DiagnosticData,
    severity: DiagnosticSeverity,
    internal_command: Listener<InternalCommand>,
    config: ReadSignal<Arc<LapceConfig>>,
    keyboard_focus: RwSignal<Option<ViewId>>,
) -> impl View {
    let collapsed = create_rw_signal(false);

    let diagnostics = create_rw_signal(im::Vector::new());
    create_effect(move |_| {
        let span = diagnostic_data.diagnostics_span.get();
        let d = if !span.is_empty() {
            span.iter()
                .filter_map(|(iv, diag)| {
                    if diag.severity == Some(severity) {
                        Some(EditorDiagnostic {
                            range: Some((iv.start, iv.end)),
                            diagnostic: diag.to_owned(),
                        })
                    } else {
                        None
                    }
                })
                .collect::<im::Vector<EditorDiagnostic>>()
        } else {
            let diagnostics = diagnostic_data.diagnostics.get();
            let diagnostics: im::Vector<EditorDiagnostic> = diagnostics
                .into_iter()
                .filter_map(|d| {
                    if d.severity == Some(severity) {
                        Some(EditorDiagnostic {
                            range: None,
                            diagnostic: d,
                        })
                    } else {
                        None
                    }
                })
                .collect();
            diagnostics
        };
        diagnostics.set(d);
    });

    let full_path = path.clone();
    let path = if let Some(workspace_path) = workspace.path.as_ref() {
        path.strip_prefix(workspace_path)
            .unwrap_or(&full_path)
            .to_path_buf()
    } else {
        path
    };
    let style_path = path.clone();

    let icon = match severity {
        DiagnosticSeverity::ERROR => LapceIcons::ERROR,
        _ => LapceIcons::WARNING,
    };
    let icon_color = move || {
        let config = config.get();
        match severity {
            DiagnosticSeverity::ERROR => config.color(LapceColor::LAPCE_ERROR),
            _ => config.color(LapceColor::LAPCE_WARN),
        }
    };

    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    let folder = path
        .parent()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    stack((
        stack((
            keyboard_problem_action(container(
                stack((
                    label(move || file_name.clone()).style(|s| {
                        s.margin_right(6.0)
                            .max_width_pct(100.0)
                            .text_ellipsis()
                            .selectable(false)
                    }),
                    label(move || folder.clone()).style(move |s| {
                        s.color(config.get().color(LapceColor::EDITOR_DIM))
                            .min_width(0.0)
                            .text_ellipsis()
                            .selectable(false)
                    }),
                ))
                .style(move |s| s.width_pct(100.0).min_width(0.0)),
            ), keyboard_focus, config)
            .on_click_stop(move |_| {
                collapsed.update(|collapsed| *collapsed = !*collapsed);
            })
            .style(move |s| {
                let config = config.get();
                s.width_pct(100.0)
                    .min_width(0.0)
                    .padding_left(10.0 + (config.ui.icon_size() as f32 + 6.0) * 2.0)
                    .padding_right(10.0)
                    .hover(|s| {
                        s.cursor(CursorStyle::Pointer).background(
                            config.color(LapceColor::PANEL_HOVERED_BACKGROUND),
                        )
                    })
            }),
            stack((
                svg(move || {
                    config.get().ui_svg(if collapsed.get() {
                        LapceIcons::ITEM_CLOSED
                    } else {
                        LapceIcons::ITEM_OPENED
                    })
                })
                .style(move |s| {
                    let config = config.get();
                    let size = config.ui.icon_size() as f32;
                    s.margin_right(6.0)
                        .size(size, size)
                        .color(config.color(LapceColor::LAPCE_ICON_ACTIVE))
                }),
                svg(move || config.get().file_svg(&path).0).style(move |s| {
                    let config = config.get();
                    let size = config.ui.icon_size() as f32;
                    let color = config.file_svg(&style_path).1;
                    s.min_width(size)
                        .size(size, size)
                        .apply_opt(color, Style::color)
                }),
                label(|| " ".to_string()).style(move |s| s.selectable(false)),
            ))
            .style(|s| s.absolute().items_center().margin_left(10.0)),
        ))
        .style(move |s| s.width_pct(100.0).min_width(0.0)),
        dyn_stack(
            move || {
                if collapsed.get() {
                    im::Vector::new()
                } else {
                    diagnostics.get()
                }
            },
            |d| (d.range, d.diagnostic.range),
            move |d| {
                item_view(
                    full_path.clone(),
                    d,
                    icon,
                    icon_color,
                    internal_command,
                    config,
                    keyboard_focus,
                )
            },
        )
        .style(|s| s.flex_col().width_pct(100.0).min_width_pct(0.0)),
    ))
    .style(move |s| {
        s.width_pct(100.0)
            .items_start()
            .flex_col()
            .apply_if(diagnostics.with(|d| d.is_empty()), |s| s.hide())
    })
}

fn item_view(
    path: PathBuf,
    d: EditorDiagnostic,
    icon: &'static str,
    icon_color: impl Fn() -> Color + 'static,
    internal_command: Listener<InternalCommand>,
    config: ReadSignal<Arc<LapceConfig>>,
    keyboard_focus: RwSignal<Option<ViewId>>,
) -> impl View {
    let related = d.diagnostic.related_information.unwrap_or_default();
    let position = if let Some((start, _)) = d.range {
        EditorPosition::Offset(start)
    } else {
        EditorPosition::Position(d.diagnostic.range.start)
    };
    let location = EditorLocation {
        path,
        position: Some(position),
        scroll_offset: None,
        ignore_unconfirmed: false,
        same_editor_tab: false,
    };
    stack((
        keyboard_problem_action(container({
            stack((
                label(move || d.diagnostic.message.clone()).style(move |s| {
                    s.width_pct(100.0)
                        .min_width(0.0)
                        .padding_left(
                            10.0 + (config.get().ui.icon_size() as f32 + 6.0) * 3.0,
                        )
                        .padding_right(10.0)
                }),
                stack((
                    svg(move || config.get().ui_svg(icon)).style(move |s| {
                        let config = config.get();
                        let size = config.ui.icon_size() as f32;
                        s.size(size, size).color(icon_color())
                    }),
                    label(|| " ".to_string()).style(move |s| s.selectable(false)),
                ))
                .style(move |s| {
                    s.absolute().items_center().margin_left(
                        10.0 + (config.get().ui.icon_size() as f32 + 6.0) * 2.0,
                    )
                }),
            ))
            .style(move |s| {
                s.width_pct(100.0).min_width(0.0).hover(|s| {
                    s.cursor(CursorStyle::Pointer).background(
                        config.get().color(LapceColor::PANEL_HOVERED_BACKGROUND),
                    )
                })
            })
        }), keyboard_focus, config)
        .on_click_stop(move |_| {
            internal_command.send(InternalCommand::JumpToLocation {
                location: location.clone(),
            });
        })
        .style(|s| s.width_pct(100.0).min_width_pct(0.0)),
        related_view(related, internal_command, config, keyboard_focus),
    ))
    .style(|s| s.width_pct(100.0).min_width_pct(0.0).flex_col())
}

fn related_view(
    related: Vec<DiagnosticRelatedInformation>,
    internal_command: Listener<InternalCommand>,
    config: ReadSignal<Arc<LapceConfig>>,
    keyboard_focus: RwSignal<Option<ViewId>>,
) -> impl View {
    let is_empty = related.is_empty();
    stack((
        dyn_stack(
            move || related.clone(),
            |related| (related.location.uri.clone(), related.location.range, related.message.clone()),
            move |related| {
                let full_path = path_from_url(&related.location.uri);
                let path = full_path
                    .file_name()
                    .and_then(|f| f.to_str())
                    .map(|f| {
                        format!(
                            "{f} [{}, {}]: ",
                            related.location.range.start.line + 1,
                            related.location.range.start.character + 1
                        )
                    })
                    .unwrap_or_default();
                let location = EditorLocation {
                    path: full_path,
                    position: Some(EditorPosition::Position(
                        related.location.range.start,
                    )),
                    scroll_offset: None,
                    ignore_unconfirmed: false,
                    same_editor_tab: false,
                };
                let message = format!("{path}{}", related.message);
                keyboard_problem_action(container(
                    label(move || message.clone())
                        .style(move |s| s.width_pct(100.0).min_width(0.0)),
                ), keyboard_focus, config)
                .on_click_stop(move |_| {
                    internal_command.send(InternalCommand::JumpToLocation {
                        location: location.clone(),
                    });
                })
                .style(move |s| {
                    let config = config.get();
                    s.padding_left(10.0 + (config.ui.icon_size() as f32 + 6.0) * 4.0)
                        .padding_right(10.0)
                        .width_pct(100.0)
                        .min_width(0.0)
                        .hover(|s| {
                            s.cursor(CursorStyle::Pointer).background(
                                config.color(LapceColor::PANEL_HOVERED_BACKGROUND),
                            )
                        })
                })
            },
        )
        .style(|s| s.width_pct(100.0).min_width(0.0).flex_col()),
        stack((
            svg(move || config.get().ui_svg(LapceIcons::LINK)).style(move |s| {
                let config = config.get();
                let size = config.ui.icon_size() as f32;
                s.size(size, size)
                    .color(config.color(LapceColor::EDITOR_DIM))
            }),
            label(|| " ".to_string()).style(move |s| s.selectable(false)),
        ))
        .style(move |s| {
            s.absolute()
                .items_center()
                .margin_left(10.0 + (config.get().ui.icon_size() as f32 + 6.0) * 3.0)
        }),
    ))
    .style(move |s| {
        s.width_pct(100.0)
            .min_width(0.0)
            .items_start()
            .color(config.get().color(LapceColor::PANEL_FOREGROUND))
            .apply_if(is_empty, |s| s.hide())
    })
}

// Native traversal must not fall through to the document's indentation binding.
fn keyboard_problem_action(
    view: impl View + 'static,
    keyboard_focus: RwSignal<Option<ViewId>>,
    config: ReadSignal<Arc<LapceConfig>>,
) -> impl View {
    let id = view.id();
    view.keyboard_navigable()
        .on_event_stop(EventListener::FocusGained, move |_| keyboard_focus.set(Some(id)))
        .on_event_stop(EventListener::FocusLost, move |_| {
            if keyboard_focus.get_untracked() == Some(id) {
                keyboard_focus.set(None);
            }
        })
        .on_event(EventListener::KeyDown, |event| {
            if let Event::KeyDown(key) = event {
                if matches!(key.key.logical_key, floem::keyboard::Key::Named(
                    floem::keyboard::NamedKey::Enter | floem::keyboard::NamedKey::Space
                )) {
                    return EventPropagation::Stop;
                }
            }
            EventPropagation::Continue
        })
        .style(move |s| s.focus_visible(|s| s.background(config.get().color(LapceColor::PANEL_CURRENT_BACKGROUND))))
}
