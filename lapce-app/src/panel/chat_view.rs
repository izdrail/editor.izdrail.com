use crate::{
    chat::{self, Context, Update},
    config::color::LapceColor,
    doc::DocContent,
    markdown::{MarkdownContent, parse_markdown},
    window_tab::WindowTabData,
};
use floem::{
    IntoView, View,
    style::CursorStyle,
    reactive::{
        RwSignal, Scope, SignalGet, SignalUpdate, SignalWith, create_effect,
    },
    views::{
        Decorators, container, dyn_stack, empty, h_stack, label, rich_text,
        scroll::scroll, text_input, v_stack,
    },
};
use lapce_ai::chat::Message;
use lapce_core::buffer::rope_text::RopeText;
use std::rc::Rc;
#[derive(Clone)]
pub struct Turn {
    pub role: &'static str,
    pub text: RwSignal<String>,
    pub context: String,
}
#[derive(Clone)]
pub struct ChatData {
    pub turns: RwSignal<Vec<Turn>>,
    pub input: RwSignal<String>,
    pub busy: RwSignal<bool>,
    pub error: RwSignal<String>,
    pub history: RwSignal<Vec<Message>>,
    worker: Rc<ChatWorker>,
}
struct ChatWorker(lapce_ai::process::Worker);
impl Drop for ChatWorker {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
impl ChatData {
    pub fn new(cx: Scope) -> Self {
        let worker = lapce_ai::process::Worker::new(
            std::env::current_exe().unwrap_or_default(),
        );

        Self {
            turns: cx.create_rw_signal(Vec::new()),
            input: cx.create_rw_signal(String::new()),
            busy: cx.create_rw_signal(false),
            error: cx.create_rw_signal(String::new()),
            history: cx.create_rw_signal(Vec::new()),
            worker: Rc::new(ChatWorker(worker)),
        }
    }
}
fn context(tab: &WindowTabData) -> Option<Context> {
    let editor = tab.main_split.active_editor.get()?;
    let doc = editor.doc_signal().get();
    let content = doc.content.get();
    let name = match content {
        DocContent::File { path, .. } => path.display().to_string(),
        DocContent::Scratch { name, .. } => name,
        _ => return None,
    };
    doc.buffer.with(|b| {
        let selection = editor.cursor().with(|c| c.edit_selection(b));
        let regions: Vec<_> = selection
            .regions()
            .iter()
            .filter(|r| r.min() != r.max())
            .collect();
        if regions.is_empty() {
            Some(Context::new(name, &b.to_string(), false))
        } else {
            let selected = regions
                .iter()
                .map(|r| b.text().slice_to_cow(r.min()..r.max()).into_owned())
                .collect::<Vec<_>>()
                .join("\n");
            Some(Context::new(name, &selected, true))
        }
    })
}
fn send(tab: &WindowTabData) {
    let data = tab.chat.clone();
    if data.busy.get_untracked() {
        return;
    }
    let settings = tab.common.config.get_untracked().ai_chat.clone();
    if !settings.enabled {
        data.error
            .set("AI Chat disabled. Enable it in Settings > AI Chat.".into());
        return;
    }
    let prompt = data.input.get_untracked().trim().to_string();
    if prompt.is_empty() {
        data.error.set("Type a message first.".into());
        return;
    }
    let ctx = context(tab);
    let messages =
        chat::assemble(&data.history.get_untracked(), &prompt, ctx.as_ref());
    let reply = tab.scope.create_rw_signal(String::new());
    data.turns.update(|v| {
        v.push(Turn {
            role: "You",
            text: tab.scope.create_rw_signal(prompt),
            context: ctx
                .as_ref()
                .map(Context::indicator)
                .unwrap_or_else(|| "No editor context".into()),
        });
        v.push(Turn {
            role: "Assistant",
            text: reply,
            context: String::new(),
        });
    });
    data.input.set(String::new());
    data.busy.set(true);
    data.error.set(String::new());
    let (tx, rx) = std::sync::mpsc::channel();
    let progress = floem::ext_event::create_signal_from_channel(rx);
    let done_data = data.clone();
    let request_messages = messages.clone();
    create_effect(move |_| {
        if let Some(update) = progress.get() {
            match update {
                Update::Delta(text) => reply.set(text),
                Update::Done(text) => {
                    reply.set(text);
                    let mut history = request_messages[1..].to_vec();
                    history.push(Message {
                        role: "assistant".into(),
                        content: reply.get_untracked(),
                    });
                    done_data.history.set(history);
                    done_data.busy.set(false);
                }
                Update::Error(error) => {
                    done_data.error.set(error);
                    done_data.busy.set(false);
                }
            }
        }
    });
    if settings.backend == "local" {
        let run = (|| -> Result<lapce_ai::Request, String> {
            let entry = lapce_ai::catalog::find(&settings.local_model)
                .filter(|m| m.chat)
                .ok_or("Choose a curated instruct chat model in Settings")?;
            let root = lapce_core::directory::Directory::data_local_directory()
                .ok_or("No model directory")?
                .join("ai-models");
            let installed: lapce_ai::Installed = serde_json::from_slice(&std::fs::read(root.join(format!("installed-{}.json", entry.id))).map_err(|_| "Download your selected Chat model in Settings first (about 1 GB for the default). No automatic download.".to_string())?).map_err(|e| e.to_string())?;
            Ok(lapce_ai::Request {
                model: installed.model,
                tokenizer: installed.tokenizer,
                prefix: String::new(),
                suffix: String::new(),
                prefix_tokens: 0,
                suffix_tokens: 0,
                max_tokens: 512,
                messages: Some(messages.clone()),
                request_id: 0,
            })
        })();
        match run {
            Ok(request) => {
                data.worker.0.complete(request, move |result| match result {
                    Ok(c) if c.cancelled => {
                        let _ = tx.send(Update::Error("Cancelled".into()));
                    }
                    Ok(c) => {
                        let _ = tx.send(Update::Delta(c.text.clone()));
                        let _ = tx.send(Update::Done(c.text.clone()));
                    }
                    Err(e) => {
                        let _ = tx.send(Update::Error(e));
                    }
                })
            }
            Err(e) => {
                let _ = tx.send(Update::Error(e));
            }
        }
    } else if settings.backend == "ollama" {
        std::thread::spawn(move || {
            let accumulated = std::cell::RefCell::new(String::new());
            if let Err(e) =
                chat::ollama(&settings.endpoint, &settings.model, &messages, |u| {
                    let u = match u {
                        Update::Delta(s) => {
                            accumulated.borrow_mut().push_str(&s);
                            Update::Delta(accumulated.borrow().clone())
                        }
                        Update::Done(_) => {
                            Update::Done(accumulated.borrow().clone())
                        }
                        u => u,
                    };
                    let _ = tx.send(u);
                })
            {
                let _ = tx.send(Update::Error(e));
            }
        });
    } else {
        let _ = tx.send(Update::Error("Backend must be local or ollama".into()));
    }
}
pub fn chat_panel(tab: Rc<WindowTabData>) -> impl View {
    let data = tab.chat.clone();
    let config = tab.common.config;
    let send_tab = tab.clone();
    let click_tab = tab.clone();
    let indicator_tab = tab.clone();
    let clear = data.clone();
    v_stack((
        h_stack((
            label(|| "AI CHAT".to_string()),
            label(|| "New chat".to_string())
                .on_click_stop(move |_| {
                    if !clear.busy.get_untracked() {
                        clear.turns.set(Vec::new());
                        clear.history.set(Vec::new());
                        clear.error.set(String::new());
                    }
                })
                .style(move |s| {
                    s.padding(6.0)
                        .margin_left(8.0)
                        .border(1.0)
                        .border_radius(4.0)
                        .border_color(config.get().color(LapceColor::LAPCE_BORDER))
                        .cursor(CursorStyle::Pointer)
                        .selectable(false)
                }),
        )),
        label(move || {
            let c = config.get();
            format!(
                "{} | {}",
                c.ai_chat.backend,
                if c.ai_chat.backend == "local" {
                    &c.ai_chat.local_model
                } else {
                    &c.ai_chat.model
                }
            )
        })
        .style(|s| s.font_size(11.0)),
        container(
            scroll(
                dyn_stack(
                    move || data.turns.get().into_iter().enumerate(),
                    |(i, _)| *i,
                    move |(_, turn)| {
                        let body = turn.text;
                        let role = turn.role;
                        let render_id = std::sync::atomic::AtomicU64::new(0);
                        v_stack((
                            label(move || role.to_string()),
                            label(move || turn.context.clone())
                                .style(|s| s.font_size(10.0).max_width_full()),
                            dyn_stack(
                                move || {
                                    parse_markdown(&body.get(), 1.5, &config.get())
                                        .into_iter()
                                        .enumerate()
                                },
                                move |_| {
                                    render_id.fetch_add(
                                        1,
                                        std::sync::atomic::Ordering::Relaxed,
                                    )
                                },
                                move |(_, content)| match content {
                                    MarkdownContent::Text(layout) => {
                                        rich_text(move || layout.clone())
                                            .style(|s| s.max_width_full())
                                            .into_any()
                                    }
                                    _ => empty().into_any(),
                                },
                            )
                            .style(|s| s.flex_col().max_width_full()),
                        ))
                        .style(move |s| {
                            s.width_full()
                                .padding(10.0)
                                .gap(5.0)
                                .border_bottom(1.0)
                                .border_color(
                                    config.get().color(LapceColor::LAPCE_BORDER),
                                )
                        })
                    },
                )
                .style(|s| s.flex_col().width_full()),
            )
            .scroll_to_percent(move || {
                let turns = data.turns.get();
                if let Some(turn) = turns.last() {
                    let _ = turn.text.get();
                }
                100.0
            })
            .style(|s| s.absolute().size_full()),
        )
        .style(|s| {
            s.width_full()
                .flex_grow(1.0)
                .flex_basis(0.0)
                .min_height(0.0)
        }),
        label(move || data.error.get()).style(move |s| {
            s.max_width_full()
                .color(config.get().color(LapceColor::LAPCE_ERROR))
        }),
        label(move || {
            context(&indicator_tab)
                .map(|c| format!("On Send: {}", c.indicator()))
                .unwrap_or_else(|| "On Send: no editor context".into())
        })
        .style(|s| s.font_size(10.0).max_width_full()),
        label(move || {
            if data.busy.get() {
                "Thinking... (local replies arrive as a full turn)".to_string()
            } else {
                "Enter to send. Code stays local unless Ollama is selected."
                    .to_string()
            }
        })
        .style(|s| s.font_size(11.0).max_width_full()),
        h_stack((
            text_input(data.input)
                .placeholder("Ask about this file or selection...")
                .on_enter(move || send(&send_tab))
                .style(|s| s.flex_grow(1.0).min_width(0.0)),
            label(|| "Send".to_string())
                .on_click_stop(move |_| send(&click_tab))
                .style(move |s| {
                    s.padding(8.0)
                        .margin_left(6.0)
                        .border(1.0)
                        .border_radius(4.0)
                        .border_color(config.get().color(LapceColor::LAPCE_BORDER))
                        .cursor(CursorStyle::Pointer)
                        .selectable(false)
                }),
        ))
        .style(|s| s.width_full().padding_bottom(8.0)),
    ))
    .style(move |s| {
        // Absolute and sized to the panel, like the other panels, so the
        // chat can neither overflow its slot nor sit under a sibling.
        s.absolute()
            .size_pct(100.0, 100.0)
            .padding(8.0)
            .gap(8.0)
            .background(config.get().color(LapceColor::PANEL_BACKGROUND))
    })
}
