use std::{cell::RefCell, collections::HashMap, rc::Rc};

use floem::{
    View, ViewId,
    action::show_context_menu,
    event::{Event, EventListener, EventPropagation},
    kurbo::Size,
    menu::{Menu, MenuItem},
    reactive::{SignalGet, SignalUpdate, SignalWith, create_rw_signal},
    style::CursorStyle,
    views::{
        Decorators, container, dyn_stack, empty, label,
        scroll::{Thickness, VerticalScrollAsHorizontal, scroll},
        stack, svg, tab,
    },
};
use lapce_rpc::terminal::TermId;

use super::kind::PanelKind;
use crate::{
    app::{clickable_icon_base, tooltip_label},
    command::{InternalCommand, LapceWorkbenchCommand},
    config::{color::LapceColor, icon::LapceIcons},
    debug::RunDebugMode,
    listener::Listener,
    terminal::{
        panel::TerminalPanelData, tab::TerminalTabData, view::terminal_view,
    },
    window_tab::{Focus, WindowTabData},
};

pub fn terminal_panel(window_tab_data: Rc<WindowTabData>) -> impl View {
    let focus = window_tab_data.common.focus;
    let config = window_tab_data.common.config;
    stack((
        terminal_tab_header(window_tab_data.clone()),
        terminal_tab_content(window_tab_data.clone()),
    ))
    .on_event_cont(EventListener::PointerDown, move |_| {
        if focus.get_untracked() != Focus::Panel(PanelKind::Terminal) {
            focus.set(Focus::Panel(PanelKind::Terminal));
        }
    })
    .style(move |s| {
        // 1px ring marks the focused panel; the panel-background color
        // keeps it invisible otherwise, without layout shift.
        let config = config.get();
        let ring = if focus.get() == Focus::Panel(PanelKind::Terminal) {
            config.color(LapceColor::EDITOR_FOCUS)
        } else {
            config.color(LapceColor::PANEL_BACKGROUND)
        };
        s.absolute()
            .size_pct(100.0, 100.0)
            .flex_col()
            .border(1.0)
            .border_color(ring)
    })
    .debug_name("Terminal Panel")
}

fn terminal_tab_header(window_tab_data: Rc<WindowTabData>) -> impl View {
    let terminal = window_tab_data.terminal.clone();
    let config = window_tab_data.common.config;
    let focus = window_tab_data.common.focus;
    let keyboard_focus = window_tab_data.common.keyboard_focus;
    let header_ids = Rc::new(RefCell::new(HashMap::new()));
    let active_index = move || terminal.tab_info.with(|info| info.active);
    let tab_info = terminal.tab_info;
    let header_width = create_rw_signal(0.0);
    let header_height = create_rw_signal(0.0);
    let icon_width = create_rw_signal(0.0);
    let scroll_size = create_rw_signal(Size::ZERO);
    let workbench_command = window_tab_data.common.workbench_command;
    let keyboard_last_close = terminal.keyboard_last_close;
    let new_focus_terminal = terminal.clone();
    let new_focus_panel = window_tab_data.panel.clone();
    let new_terminal = clickable_icon_base(
        || LapceIcons::ADD,
        None::<fn()>,
        || false,
        || false,
        config,
    );
    let new_terminal_id = new_terminal.id();
    let new_terminal = new_terminal
        .keyboard_navigable()
        .style(move |s| {
            s.focus_visible(|s| {
                s.border_color(config.get().color(LapceColor::EDITOR_FOCUS))
            })
        })
        .on_event_stop(EventListener::FocusGained, move |_| {
            keyboard_focus.set(Some(new_terminal_id));
            terminal_ui_trace(&format!("focus:new:{new_terminal_id:?}"));
        })
        .on_event_stop(EventListener::FocusLost, move |_| {
            keyboard_last_close.set(false);
            if keyboard_focus.get_untracked() == Some(new_terminal_id) {
                keyboard_focus.set(None);
            }
            if new_focus_terminal.tab_info.with_untracked(|info| info.tabs.is_empty()) {
                new_focus_panel.hide_panel(&PanelKind::Terminal);
                // A global shortcut may already have moved application
                // focus to Palette. Do not overwrite that destination.
                if focus.get_untracked() == Focus::Panel(PanelKind::Terminal) {
                    focus.set(Focus::Workbench);
                }
            }
        })
        .on_cleanup(move || {
            if keyboard_focus.get_untracked() == Some(new_terminal_id) {
                keyboard_focus.set(None);
            }
        })
        .on_click_stop(move |event| {
            if let Event::KeyDown(key) = event {
                if !terminal_tab_activation(&key.key.logical_key, key.modifiers) {
                    return;
                }
            }
            keyboard_last_close.set(false);
            terminal_ui_trace("activate:new");
            workbench_command.send(LapceWorkbenchCommand::NewTerminalTab);
        })
        .on_event(EventListener::KeyDown, |event| {
            if let Event::KeyDown(key) = event {
                if terminal_tab_activation(
                    &key.key.logical_key,
                    floem::keyboard::Modifiers::empty(),
                ) || (matches!(key.key.logical_key, floem::keyboard::Key::Character(_))
                    && (key.modifiers.is_empty()
                        || key.modifiers == floem::keyboard::Modifiers::SHIFT))
                {
                    // A button is not a text receiver. Do not let workbench
                    // receive_char report a no-op as handled and move focus.
                    return EventPropagation::Stop;
                }
            }
            EventPropagation::Continue
        });

    stack((
        scroll(dyn_stack(
            move || {
                let tabs = terminal.tab_info.with(|info| info.tabs.clone());
                for (i, (index, _)) in tabs.iter().enumerate() {
                    if index.get_untracked() != i {
                        index.set(i);
                    }
                }
                tabs
            },
            |(_, tab)| tab.terminal_tab_id,
            move |(index, tab)| {
                let header_ids = header_ids.clone();
                let close_header_ids = header_ids.clone();
                let terminal = terminal.clone();
                let local_terminal = terminal.clone();
                let terminal_tab_id = tab.terminal_tab_id;

                let title = {
                    let tab = tab.clone();
                    move || {
                        let terminal = tab.active_terminal(true);
                        let run_debug = terminal.as_ref().map(|t| t.run_debug);
                        if let Some(run_debug) = run_debug {
                            if let Some(name) = run_debug.with(|run_debug| {
                                run_debug.as_ref().map(|r| r.config.name.clone())
                            }) {
                                return name;
                            }
                        }

                        let title = terminal.map(|t| t.title);
                        let title = title.map(|t| t.get());
                        title.unwrap_or_default()
                    }
                };

                let svg_string = move || {
                    let terminal = tab.active_terminal(true);
                    let run_debug = terminal.as_ref().map(|t| t.run_debug);
                    if let Some(run_debug) = run_debug {
                        if let Some((mode, stopped)) = run_debug.with(|run_debug| {
                            run_debug.as_ref().map(|r| (r.mode, r.stopped))
                        }) {
                            let svg = match (mode, stopped) {
                                (RunDebugMode::Run, false) => LapceIcons::START,
                                (RunDebugMode::Run, true) => LapceIcons::RUN_ERRORS,
                                (RunDebugMode::Debug, false) => LapceIcons::DEBUG,
                                (RunDebugMode::Debug, true) => {
                                    LapceIcons::DEBUG_DISCONNECT
                                }
                            };
                            return svg;
                        }
                    }
                    LapceIcons::TERMINAL
                };
                let row = stack((
                    container({
                        stack((
                            container(
                                svg(move || config.get().ui_svg(svg_string()))
                                    .style(move |s| {
                                        let config = config.get();
                                        let size = config.ui.icon_size() as f32;
                                        s.size(size, size).color(
                                            config.color(
                                                LapceColor::LAPCE_ICON_ACTIVE,
                                            ),
                                        )
                                    }),
                            )
                            .style(|s| s.padding_horiz(10.0).padding_vert(12.0)),
                            label(title).style(|s| {
                                s.min_width(0.0)
                                    .flex_basis(0.0)
                                    .flex_grow(1.0f32)
                                    .text_ellipsis()
                                    .selectable(false)
                            }),
                            {
                                let close = clickable_icon_base(
                                    || LapceIcons::CLOSE,
                                    None::<fn()>,
                                    || false,
                                    || false,
                                    config,
                                )
                                .keyboard_navigable()
                                .style(move |s| {
                                    s.focus_visible(|s| {
                                        s.border_color(config.get().color(
                                            LapceColor::EDITOR_FOCUS,
                                        ))
                                    })
                                })
                                .on_click_stop(move |event| {
                                    if let Event::KeyDown(key) = event {
                                        if !terminal_tab_activation(
                                            &key.key.logical_key,
                                            key.modifiers,
                                        ) {
                                            return;
                                        }
                                    }
                                    let keyboard = matches!(event, Event::KeyDown(_));
                                    keyboard_last_close.set(keyboard && terminal.tab_info
                                        .with_untracked(|info| info.tabs.len() == 1));
                                    terminal_ui_trace("activate:close");
                                    terminal.close_tab(Some(terminal_tab_id));
                                    // Keep native traversal owned by a live header
                                    // after removing the focused close control.
                                    if !keyboard {
                                        return;
                                    }
                                    if let Some(tab) = terminal.active_tab(false) {
                                        if let Some(id) = close_header_ids
                                            .borrow()
                                            .get(&tab.terminal_tab_id)
                                            .copied()
                                        {
                                            keyboard_focus.set(Some(id));
                                            id.request_focus();
                                        }
                                    } else if keyboard {
                                        keyboard_focus.set(Some(new_terminal_id));
                                        new_terminal_id.request_focus();
                                    }
                                })
                                // Do not select a non-active tab while closing it.
                                .on_event_stop(EventListener::PointerDown, |_| {});
                                let close_id = close.id();
                                tooltip_label(
                                    config,
                                    close
                                        // Floem's keyboard Click return is not used
                                        // to stop KeyDown propagation. Consume the
                                        // activation here before it reaches PTY.
                                        .on_event(EventListener::KeyDown, |event| {
                                            if let Event::KeyDown(key) = event {
                                                if terminal_tab_activation(
                                                    &key.key.logical_key,
                                                    floem::keyboard::Modifiers::empty(),
                                                ) {
                                                    return EventPropagation::Stop;
                                                }
                                            }
                                            EventPropagation::Continue
                                        })
                                        .on_cleanup(move || {
                                            if keyboard_focus.get_untracked()
                                                == Some(close_id)
                                            {
                                                keyboard_focus.set(None);
                                            }
                                        })
                                        .on_event_stop(
                                            EventListener::FocusGained,
                                            move |_| {
                                                keyboard_focus.set(Some(close_id));
                                                terminal_ui_trace(&format!(
                                                    "focus:close:{close_id:?}"
                                                ));
                                            },
                                        )
                                        .on_event_stop(
                                            EventListener::FocusLost,
                                            move |_| {
                                                if keyboard_focus.get_untracked()
                                                    == Some(close_id)
                                                {
                                                    keyboard_focus.set(None);
                                                }
                                            },
                                        ),
                                    || "Close",
                                )
                                .style(|s| s.margin_horiz(6.0))
                            },
                            empty().style(move |s| {
                                s.absolute()
                                    .width_full()
                                    .height(header_height.get() - 15.0)
                                    .border_right(1.0)
                                    .border_color(
                                        config.get().color(LapceColor::LAPCE_BORDER),
                                    )
                                    .pointer_events_none()
                            }),
                        ))
                        .style(move |s| {
                            s.items_center().width(200.0).border_color(
                                config.get().color(LapceColor::LAPCE_BORDER),
                            )
                        })
                    })
                    .style(|s| s.items_center()),
                    container({
                        label(|| "".to_string()).style(move |s| {
                            s.size_pct(100.0, 100.0)
                                .border_bottom(if active_index() == index.get() {
                                    2.0
                                } else {
                                    0.0
                                })
                                .border_color(config.get().color(
                                    if focus.get()
                                        == Focus::Panel(PanelKind::Terminal)
                                    {
                                        LapceColor::LAPCE_TAB_ACTIVE_UNDERLINE
                                    } else {
                                        LapceColor::LAPCE_TAB_INACTIVE_UNDERLINE
                                    },
                                ))
                        })
                    })
                    .style(|s| {
                        s.absolute()
                            .padding_horiz(3.0)
                            .size_pct(100.0, 100.0)
                            .pointer_events_none()
                    }),
                ))
                .style(move |s| {
                    s.cursor(CursorStyle::Pointer).focus_visible(|s| {
                        s.background(
                            config.get().color(LapceColor::PANEL_CURRENT_BACKGROUND),
                        )
                    })
                });
                let row_id = row.id();
                header_ids.borrow_mut().insert(terminal_tab_id, row_id);
                let activate = move || {
                    if tab_info.with_untracked(|tab| tab.active)
                        != index.get_untracked()
                    {
                        tab_info.update(|tab| tab.active = index.get_untracked());
                        local_terminal.update_debug_active_term();
                    }
                };
                let pointer_activate = activate.clone();
                row.keyboard_navigable()
                    .on_cleanup(move || {
                        header_ids.borrow_mut().retain(|k, v| {
                            !(*k == terminal_tab_id && *v == row_id)
                        });
                        if keyboard_focus.get_untracked() == Some(row_id) {
                            keyboard_focus.set(None);
                        }
                    })
                    .on_event_stop(EventListener::FocusGained, move |_| {
                        keyboard_focus.set(Some(row_id));
                        terminal_ui_trace(&format!("focus:header:{row_id:?}"));
                    })
                    .on_event_stop(EventListener::FocusLost, move |_| {
                        if keyboard_focus.get_untracked() == Some(row_id) {
                            keyboard_focus.set(None);
                        }
                    })
                    .on_event(EventListener::KeyDown, move |event| {
                        if let Event::KeyDown(key) = event {
                            if terminal_tab_activation(
                                &key.key.logical_key,
                                key.modifiers,
                            ) {
                                return EventPropagation::Stop;
                            }
                        }
                        EventPropagation::Continue
                    })
                    .on_click_stop(move |event| {
                        if let Event::KeyDown(key) = event {
                            if terminal_tab_activation(
                                &key.key.logical_key,
                                key.modifiers,
                            ) {
                                terminal_ui_trace("activate:header");
                                activate();
                            }
                        }
                    })
                    .on_event_cont(EventListener::PointerDown, move |_| {
                        pointer_activate()
                    })
            },
        ))
        .on_resize(move |rect| {
            if rect.size() != scroll_size.get_untracked() {
                scroll_size.set(rect.size());
            }
        })
        .style(move |s| {
            let header_width = header_width.get();
            let icon_width = icon_width.get();
            s.set(VerticalScrollAsHorizontal, true)
                .absolute()
                .max_width(header_width - icon_width)
                .set(Thickness, 3)
        }),
        empty().style(move |s| {
            let size = scroll_size.get();
            s.size(size.width, size.height).pointer_events_none()
        }),
        container(tooltip_label(config, new_terminal, || "New Terminal"))
        .on_resize(move |rect| {
            let width = rect.size().width;
            if icon_width.get_untracked() != width {
                icon_width.set(width);
            }
        })
        .style(|s| s.padding_horiz(10)),
    ))
    .on_resize(move |rect| {
        let size = rect.size();
        if header_width.get_untracked() != size.width {
            header_width.set(size.width);
        }
        if header_height.get_untracked() != size.height {
            header_height.set(size.height);
        }
    })
    .on_double_click(move |_| {
        window_tab_data
            .panel
            .toggle_maximize(&crate::panel::kind::PanelKind::Terminal);
        EventPropagation::Stop
    })
    .style(move |s| {
        let config = config.get();
        s.width_pct(100.0)
            .items_center()
            .border_bottom(1.0)
            .border_color(config.color(LapceColor::LAPCE_BORDER))
    })
}

fn terminal_tab_split(
    terminal_panel_data: TerminalPanelData,
    terminal_tab_data: TerminalTabData,
    tab_index: usize,
) -> impl View {
    let config = terminal_panel_data.common.config;
    let internal_command = terminal_panel_data.common.internal_command;
    let workspace = terminal_panel_data.workspace.clone();
    let active = terminal_tab_data.active;
    let terminal_tab_scope = terminal_tab_data.scope;
    dyn_stack(
        move || {
            let terminals = terminal_tab_data.terminals.get();
            for (i, (index, _)) in terminals.iter().enumerate() {
                if index.get_untracked() != i {
                    index.set(i);
                }
            }
            terminals
        },
        |(_, terminal)| terminal.term_id,
        move |(index, terminal)| {
            let terminal_panel_data = terminal_panel_data.clone();
            let terminal_scope = terminal.scope;
            container({
                let terminal_view = terminal_view(
                    terminal.term_id,
                    terminal.raw.read_only(),
                    terminal.mode.read_only(),
                    terminal.run_debug.read_only(),
                    terminal_panel_data,
                    terminal.launch_error,
                    internal_command,
                    workspace.clone(),
                );
                let view_id = terminal_view.id();
                let have_task = terminal.run_debug.get_untracked().is_some();
                terminal_view
                    .on_event_cont(EventListener::PointerDown, move |_| {
                        active.set(index.get_untracked());
                    })
                    .on_secondary_click_stop(move |_| {
                        if have_task {
                            tab_secondary_click(
                                internal_command,
                                view_id,
                                tab_index,
                                index.get_untracked(),
                                terminal.term_id,
                            );
                        }
                    })
                    .on_event(EventListener::PointerWheel, move |event| {
                        if let Event::PointerWheel(pointer_event) = event {
                            terminal.clone().wheel_scroll(pointer_event.delta.y);
                            EventPropagation::Stop
                        } else {
                            EventPropagation::Continue
                        }
                    })
                    .on_cleanup(move || {
                        terminal_scope.dispose();
                    })
                    .style(|s| s.size_pct(100.0, 100.0))
            })
            .style(move |s| {
                s.size_pct(100.0, 100.0).padding_horiz(10.0).apply_if(
                    index.get() > 0,
                    |s| {
                        s.border_left(1.0).border_color(
                            config.get().color(LapceColor::LAPCE_BORDER),
                        )
                    },
                )
            })
        },
    )
    .on_cleanup(move || {
        terminal_tab_scope.dispose();
    })
    .style(|s| s.size_pct(100.0, 100.0))
}

fn terminal_tab_content(window_tab_data: Rc<WindowTabData>) -> impl View {
    let terminal = window_tab_data.terminal.clone();
    tab(
        move || terminal.tab_info.with(|info| info.active),
        move || terminal.tab_info.with(|info| info.tabs.clone()),
        |(_, tab)| tab.terminal_tab_id,
        move |(tab_index, tab)| {
            terminal_tab_split(terminal.clone(), tab, tab_index.get_untracked())
        },
    )
    .style(|s| s.size_pct(100.0, 100.0))
}

fn tab_secondary_click(
    internal_command: Listener<InternalCommand>,
    view_id: ViewId,
    tab_index: usize,
    terminal_index: usize,
    term_id: TermId,
) {
    let mut menu = Menu::new("");
    menu = menu
        .entry(MenuItem::new("Stop").action(move || {
            internal_command.send(InternalCommand::StopTerminal { term_id });
        }))
        .entry(MenuItem::new("Restart").action(move || {
            internal_command.send(InternalCommand::RestartTerminal { term_id });
        }))
        .entry(MenuItem::new("Clear All").action(move || {
            internal_command.send(InternalCommand::ClearTerminalBuffer {
                view_id,
                tab_index,
                terminal_index,
            });
        }));
    show_context_menu(menu, None);
}

// Header activation must never inject a newline/space into the shell.
fn terminal_tab_activation(
    key: &floem::keyboard::Key,
    modifiers: floem::keyboard::Modifiers,
) -> bool {
    modifiers.is_empty()
        && matches!(
            key,
            floem::keyboard::Key::Named(
                floem::keyboard::NamedKey::Enter | floem::keyboard::NamedKey::Space
            )
        )
}
#[cfg(test)]
mod terminal_tab_keyboard_tests {
    use super::terminal_tab_activation;
    use floem::keyboard::{Key, Modifiers, NamedKey};
    #[test]
    fn rebuilt_header_cleanup_preserves_new_identity() {
        let tab = crate::id::TerminalTabId::next();
        let old = floem::ViewId::new();
        let new = floem::ViewId::new();
        let mut ids = std::collections::HashMap::from([(tab, new)]);
        ids.retain(|k, v| !(*k == tab && *v == old));
        assert_eq!(ids.get(&tab), Some(&new));
        ids.retain(|k, v| !(*k == tab && *v == new));
        assert!(ids.is_empty());
    }

    #[test]
    fn plain_activation_isolated_from_shell_input() {
        for key in [NamedKey::Enter, NamedKey::Space] {
            assert!(terminal_tab_activation(
                &Key::Named(key.clone()),
                Modifiers::empty()
            ));
            assert!(!terminal_tab_activation(
                &Key::Named(key),
                Modifiers::CONTROL
            ));
        }
        assert!(!terminal_tab_activation(
            &Key::Named(NamedKey::Tab),
            Modifiers::empty()
        ));
    }
}

/// Opt-in diagnostics record only control identity/action, never shell data.
fn terminal_ui_trace(event: &str) {
    if std::env::var("LAPCE_TERMINAL_UI_TRACE").ok().as_deref() == Some("1") {
        eprintln!("TERMINAL_UI event={event}");
    }
}
