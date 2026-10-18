use std::rc::Rc;

use floem::{
    IntoView, View,
    event::EventListener,
    keyboard::Modifiers,
    reactive::{RwSignal, Scope, SignalGet, SignalUpdate, create_rw_signal},
    style::{CursorStyle, Display, Position},
    views::{Decorators, container, label, scroll, stack, svg},
};
use floem::views::editor::text::SystemClipboard;
use lapce_core::{
    command::FocusCommand, meta::VERSION, mode::Mode, register::Clipboard,
};

use crate::{
    command::{CommandExecuted, CommandKind},
    config::color::LapceColor,
    keypress::KeyPressFocus,
    web_link::web_link,
    window_tab::{Focus, WindowTabData},
};

struct AboutUri {}

impl AboutUri {
    const LAPCE: &'static str = "https://lapce.dev";
    const GITHUB: &'static str = "https://github.com/lapce/lapce";
    const MATRIX: &'static str = "https://matrix.to/#/#lapce-editor:matrix.org";
    const DISCORD: &'static str = "https://discord.gg/n8tGJ6Rn6D";
    const CODICONS: &'static str = "https://github.com/microsoft/vscode-codicons";
}

#[derive(Clone, Debug)]
pub struct AboutData {
    pub visible: RwSignal<bool>,
    pub focus: RwSignal<Focus>,
}

impl AboutData {
    pub fn new(cx: Scope, focus: RwSignal<Focus>) -> Self {
        let visible = cx.create_rw_signal(false);

        Self { visible, focus }
    }

    pub fn open(&self) {
        self.visible.set(true);
        self.focus.set(Focus::AboutPopup);
    }

    pub fn close(&self) {
        self.visible.set(false);
        self.focus.set(Focus::Workbench);
    }
}

impl KeyPressFocus for AboutData {
    fn get_mode(&self) -> Mode {
        Mode::Insert
    }

    fn check_condition(
        &self,
        _condition: crate::keypress::condition::Condition,
    ) -> bool {
        self.visible.get_untracked()
    }

    fn run_command(
        &self,
        command: &crate::command::LapceCommand,
        _count: Option<usize>,
        _mods: Modifiers,
    ) -> crate::command::CommandExecuted {
        match &command.kind {
            CommandKind::Workbench(_) => {}
            CommandKind::Edit(_) => {}
            CommandKind::Move(_) => {}
            CommandKind::Scroll(_) => {}
            CommandKind::Focus(cmd) => {
                if cmd == &FocusCommand::ModalClose {
                    self.close();
                }
            }
            CommandKind::MotionMode(_) => {}
            CommandKind::MultiSelection(_) => {}
        }
        CommandExecuted::Yes
    }

    fn receive_char(&self, _c: &str) {}

    fn focus_only(&self) -> bool {
        true
    }
}

pub fn about_popup(window_tab_data: Rc<WindowTabData>) -> impl View {
    let about_data = window_tab_data.about_data.clone();
    let config = window_tab_data.common.config;
    let internal_command = window_tab_data.common.internal_command;
    let logo_size = 100.0;

    exclusive_popup(window_tab_data, about_data.visible, move || {
        stack((
            svg(move || (config.get()).logo_svg()).style(move |s| {
                s.size(logo_size, logo_size)
                    .color(config.get().color(LapceColor::EDITOR_FOREGROUND))
            }),
            label(|| "Lapce".to_string()).style(move |s| {
                s.font_bold()
                    .margin_top(10.0)
                    .color(config.get().color(LapceColor::EDITOR_FOREGROUND))
            }),
            label(|| format!("Version: {}", VERSION)).style(move |s| {
                s.margin_top(10.0)
                    .color(config.get().color(LapceColor::EDITOR_DIM))
            }),
            {
                let report = {
                    let c = config.get_untracked();
                    let ai = &c.ai_autocomplete;
                    let chat = &c.ai_chat;
                    let chat_model = if chat.backend == "local" {
                        chat.local_model.clone()
                    } else {
                        chat.model.clone()
                    };
                    let ai_model = if ai.backend == "local" {
                        ai.local_model.clone()
                    } else {
                        ai.remote_model.clone()
                    };
                    lapce_core::diagnostics::diagnostics_report(
                        VERSION,
                        (&ai.backend, &ai.endpoint, &ai_model),
                        (&chat.backend, &chat.endpoint, &chat_model),
                        lapce_core::diagnostics::crash_log_tail(20),
                    )
                };
                let copied = create_rw_signal(false);
                let report_view = report.clone();
                stack((
                    label(|| "Diagnostics".to_string()).style(move |s| {
                        s.font_bold()
                            .margin_top(20.0)
                            .color(config.get().color(LapceColor::EDITOR_DIM))
                    }),
                    container(
                        scroll(label(move || report_view.clone()).style(
                            move |s| {
                                s.font_size(11.0)
                                    .color(
                                        config
                                            .get()
                                            .color(LapceColor::EDITOR_DIM),
                                    )
                                    .selectable(true)
                            },
                        ))
                        .style(|s| s.width_full().height_full()),
                    )
                    .style(move |s| {
                        s.width(420.0)
                            .max_height(220.0)
                            .margin_top(6.0)
                            .padding(8.0)
                            .border(1.0)
                            .border_radius(6.0)
                            .border_color(
                                config.get().color(LapceColor::LAPCE_BORDER),
                            )
                    }),
                    label(move || {
                        if copied.get() {
                            "Copied to clipboard".to_string()
                        } else {
                            "Copy diagnostics".to_string()
                        }
                    })
                    .on_click_stop(move |_| {
                        let mut clipboard = SystemClipboard::new();
                        clipboard.put_string(report.clone());
                        copied.set(true);
                    })
                    .style(move |s| {
                        s.margin_top(8.0)
                            .padding_horiz(12.0)
                            .padding_vert(4.0)
                            .border(1.0)
                            .border_radius(6.0)
                            .border_color(
                                config.get().color(LapceColor::LAPCE_BORDER),
                            )
                            .hover(|s| {
                                s.cursor(CursorStyle::Pointer).background(
                                    config.get().color(
                                        LapceColor::PANEL_HOVERED_BACKGROUND,
                                    ),
                                )
                            })
                    }),
                ))
                .style(|s| s.flex_col().items_center())
                .into_any()
            },
            web_link(
                || "Website".to_string(),
                || AboutUri::LAPCE.to_string(),
                move || config.get().color(LapceColor::EDITOR_LINK),
                internal_command,
            )
            .style(|s| s.margin_top(20.0)),
            web_link(
                || "GitHub".to_string(),
                || AboutUri::GITHUB.to_string(),
                move || config.get().color(LapceColor::EDITOR_LINK),
                internal_command,
            )
            .style(|s| s.margin_top(10.0)),
            web_link(
                || "Discord".to_string(),
                || AboutUri::DISCORD.to_string(),
                move || config.get().color(LapceColor::EDITOR_LINK),
                internal_command,
            )
            .style(|s| s.margin_top(10.0)),
            web_link(
                || "Matrix".to_string(),
                || AboutUri::MATRIX.to_string(),
                move || config.get().color(LapceColor::EDITOR_LINK),
                internal_command,
            )
            .style(|s| s.margin_top(10.0)),
            label(|| "Attributions".to_string()).style(move |s| {
                s.font_bold()
                    .color(config.get().color(LapceColor::EDITOR_DIM))
                    .margin_top(40.0)
            }),
            web_link(
                || "Codicons (CC-BY-4.0)".to_string(),
                || AboutUri::CODICONS.to_string(),
                move || config.get().color(LapceColor::EDITOR_LINK),
                internal_command,
            )
            .style(|s| s.margin_top(10.0)),
        ))
        .style(|s| s.flex_col().items_center())
    })
    .debug_name("About Popup")
}

fn exclusive_popup<V: View + 'static>(
    window_tab_data: Rc<WindowTabData>,
    visibility: RwSignal<bool>,
    content: impl FnOnce() -> V,
) -> impl View {
    let config = window_tab_data.common.config;

    container(
        container(
            container(content())
                .style(move |s| {
                    let config = config.get();
                    s.padding_vert(25.0)
                        .padding_horiz(100.0)
                        .border(1.0)
                        .border_radius(6.0)
                        .border_color(config.color(LapceColor::LAPCE_BORDER))
                        .background(config.color(LapceColor::PANEL_BACKGROUND))
                })
                .on_event_stop(EventListener::PointerDown, move |_| {}),
        )
        .style(move |s| {
            s.flex_grow(1.0f32)
                .flex_row()
                .items_center()
                .hover(move |s| s.cursor(CursorStyle::Default))
        }),
    )
    .on_event_stop(EventListener::PointerDown, move |_| {
        window_tab_data.about_data.close();
    })
    // Prevent things behind the grayed out area from being hovered.
    .on_event_stop(EventListener::PointerMove, move |_| {})
    .style(move |s| {
        s.display(if visibility.get() {
            Display::Flex
        } else {
            Display::None
        })
        .position(Position::Absolute)
        .size_pct(100.0, 100.0)
        .flex_col()
        .items_center()
        .background(
            config
                .get()
                .color(LapceColor::LAPCE_DROPDOWN_SHADOW)
                .multiply_alpha(0.5),
        )
    })
}
