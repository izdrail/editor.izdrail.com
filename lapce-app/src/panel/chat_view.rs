use crate::chat_session::{self, RunGate, RunState, SavedTurn, Session};
use crate::{
    chat::{self, Context, Update},
    config::color::LapceColor,
    doc::DocContent,
    markdown::{MarkdownContent, parse_markdown},
    window_tab::WindowTabData,
};
use floem::{
    IntoView, View,
    reactive::{
        RwSignal, Scope, SignalGet, SignalUpdate, SignalWith, create_effect,
    },
    style::CursorStyle,
    views::{
        Decorators, container, dyn_stack, empty, h_stack, label, rich_text,
        scroll::scroll, text_editor, v_stack,
    },
};
use lapce_ai::chat::Message;
use lapce_core::buffer::rope_text::RopeText;
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
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
    pub session: RwSignal<Session>,
    pub state: RwSignal<RunState>,
    pub mode: RwSignal<Option<crate::agent_loop::Mode>>,
    pub trust_reads: RwSignal<bool>,
    pub trusted_endpoint: RwSignal<String>,
    pub review: crate::agent_review_ui::ReviewData,
    gate: RunGate,
    cancellation: RwSignal<Arc<AtomicBool>>,
    storage: Option<PathBuf>,
    worker: Rc<ChatWorker>,
}
struct ChatWorker {
    local: lapce_ai::process::Worker,
    network: RefCell<Arc<AtomicBool>>,
}
impl Drop for ChatWorker {
    fn drop(&mut self) {
        self.network.borrow().store(true, Ordering::Release);
        self.local.cancel();
    }
}
impl ChatData {
    pub fn new(cx: Scope, storage: Option<PathBuf>, restore: bool) -> Self {
        let worker = lapce_ai::process::Worker::new(
            std::env::current_exe().unwrap_or_default(),
        );

        let review = crate::agent_review_ui::ReviewData::new(
            cx,
            storage.as_ref().map(|p| p.join("checkpoints")),
        );
        let (session, error) = if restore {
            match storage
                .as_ref()
                .map(|p| chat_session::latest(p))
                .transpose()
            {
                Ok(Some(Some(s))) => (s, String::new()),
                Ok(_) => (Session::new(), String::new()),
                Err(e) => (Session::new(), e),
            }
        } else {
            (Session::new(), String::new())
        };
        let turns = session
            .turns
            .iter()
            .map(|t| Turn {
                role: if t.role == "user" { "You" } else { "Assistant" },
                text: cx.create_rw_signal(t.text.clone()),
                context: t.context.clone(),
            })
            .collect();
        Self {
            session: cx.create_rw_signal(session.clone()),
            state: cx.create_rw_signal(session.state.clone()),
            mode: cx.create_rw_signal(None),
            trust_reads: cx.create_rw_signal(false),
            trusted_endpoint: cx.create_rw_signal(String::new()),
            gate: RunGate::default(),
            cancellation: cx.create_rw_signal(Arc::new(AtomicBool::new(false))),
            storage,
            review,
            turns: cx.create_rw_signal(turns),
            input: cx.create_rw_signal(String::new()),
            busy: cx.create_rw_signal(false),
            error: cx.create_rw_signal(error),
            history: cx.create_rw_signal({
                // Restarted runs cannot promote partial assistant output into history.
                let count =
                    if matches!(session.state, RunState::Done | RunState::Idle) {
                        session.turns.len()
                    } else {
                        session.turns.len().saturating_sub(2)
                    };
                session.turns[..count]
                    .iter()
                    .map(|t| Message {
                        role: t.role.clone(),
                        content: t.text.clone(),
                    })
                    .collect()
            }),
            worker: Rc::new(ChatWorker {
                local: worker,
                network: RefCell::new(Arc::new(AtomicBool::new(false))),
            }),
        }
    }
}
impl ChatData {
    fn stop(&self) {
        self.gate.cancel();
        self.cancellation
            .get_untracked()
            .store(true, Ordering::Release);
        self.worker.local.cancel();
        self.busy.set(false);
        self.state.set(RunState::Cancelled);
    }
    fn persist(&self, enabled: bool) {
        if !enabled {
            return;
        }
        let mut session = self.session.get_untracked();
        session.state = self.state.get_untracked();
        session.turns = self
            .turns
            .get_untracked()
            .iter()
            .map(|t| SavedTurn {
                role: if t.role == "You" { "user" } else { "assistant" }.into(),
                text: t.text.get_untracked(),
                context: t.context.clone(),
            })
            .collect();
        self.session.set(session.clone());
        if let Some(path) = &self.storage {
            if let Err(e) = chat_session::save(path, &session) {
                self.error.set(format!("Chat history was not saved: {e}"));
            }
        } else {
            self.error.set("No local chat storage directory".into());
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
    if let Some(mode) = data.mode.get_untracked() {
        send_agent(tab, mode, &prompt);
        return;
    }
    let ctx = context(tab);
    let messages =
        chat::assemble(&data.history.get_untracked(), &prompt, ctx.as_ref());
    let run = data.gate.begin();
    let cancelled = Arc::new(AtomicBool::new(false));
    data.cancellation.set(cancelled.clone());
    *data.worker.network.borrow_mut() = cancelled.clone();
    data.state.set(RunState::Running);
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
    data.persist(settings.save_history);
    let save_history = settings.save_history;
    let (tx, rx) = std::sync::mpsc::channel();
    let progress = floem::ext_event::create_signal_from_channel(rx);
    let done_data = data.clone();
    let request_messages = messages.clone();
    create_effect(move |_| {
        if let Some(update) = progress.get() {
            if !done_data.gate.accepts(run) {
                return;
            }
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
                    done_data.state.set(RunState::Done);
                    done_data.persist(save_history);
                }
                Update::Error(error) => {
                    done_data.error.set(error);
                    done_data.busy.set(false);
                    done_data.state.set(RunState::Error);
                    done_data.persist(save_history);
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
                data.worker
                    .local
                    .complete(request, move |result| match result {
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
            if let Err(e) = chat::ollama(
                &settings.endpoint,
                &settings.model,
                &messages,
                cancelled,
                |u| {
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
                },
            ) {
                let _ = tx.send(Update::Error(e));
            }
        });
    } else {
        let _ = tx.send(Update::Error("Backend must be local or ollama".into()));
    }
}
#[cfg(target_os = "linux")]
fn send_agent(tab: &WindowTabData, mode: crate::agent_loop::Mode, prompt: &str) {
    let data = tab.chat.clone();
    let settings = tab.common.config.get_untracked().ai_chat.clone();
    if settings.backend != "ollama" {
        data.error.set("Ask/Plan requires a tool-capable Ollama model. Simple Chat still works locally.".into());
        return;
    }
    if !data.trust_reads.get_untracked()
        || data.trusted_endpoint.get_untracked() != settings.endpoint
        || !tab.workspace.kind.is_local()
    {
        data.error.set(
            "Approve project reads in this panel for a local workspace first."
                .into(),
        );
        return;
    }
    let Some(root) = tab.workspace.path.clone() else {
        data.error.set("Open a local project folder first.".into());
        return;
    };
    let run = data.gate.begin();
    let cancelled = Arc::new(AtomicBool::new(false));
    data.cancellation.set(cancelled.clone());
    *data.worker.network.borrow_mut() = cancelled.clone();
    data.state.set(RunState::Running);
    data.busy.set(true);
    data.error.set(String::new());
    let reply = tab.scope.create_rw_signal(String::new());
    data.turns.update(|turns| {
        turns.push(Turn {
            role: "You",
            text: tab.scope.create_rw_signal(prompt.into()),
            context: format!(
                "{:?}: approved project reads from {}",
                mode,
                root.display()
            ),
        });
        turns.push(Turn {
            role: "Assistant",
            text: reply,
            context: String::new(),
        });
    });
    data.input.set(String::new());
    data.persist(settings.save_history);
    let (tx, rx) = std::sync::mpsc::channel();
    let progress = floem::ext_event::create_signal_from_channel(rx);
    let done = data.clone();
    let save = settings.save_history;
    create_effect(move |_| {
        if let Some(update) = progress.get() {
            if !done.gate.accepts(run) {
                return;
            }
            match update {
                Update::Delta(text) => reply.set(text),
                Update::Done(text) => {
                    reply.set(text);
                    done.busy.set(false);
                    done.state.set(RunState::Done);
                    done.persist(save);
                }
                Update::Error(error) => {
                    done.error.set(error);
                    done.busy.set(false);
                    done.state.set(RunState::Error);
                    done.persist(save);
                }
            }
        }
    });
    let prompt = prompt.to_string();
    std::thread::spawn(move || {
        let result = (|| {
            let reader = crate::agent_read::ReadRoot::new(&root, true)?;
            crate::agent_loop::run(
                mode,
                &prompt,
                crate::agent_loop::Limits::default(),
                cancelled.clone(),
                |messages, tools| {
                    crate::agent_transport::ollama(
                        &settings.endpoint,
                        &settings.model,
                        messages,
                        tools,
                        1024,
                        cancelled.clone(),
                    )
                },
                |call| match call.name.as_str() {
                    "read_file" => reader.read(std::path::Path::new(
                        call.arguments["path"].as_str().ok_or("Missing path")?,
                    )),
                    "search" => {
                        let result = reader.search(
                            call.arguments["literal"]
                                .as_str()
                                .ok_or("Missing literal")?,
                        )?;
                        Ok(serde_json::json!({"matches":result.matches,"truncated":result.truncated,"skipped_files":result.skipped_files}).to_string())
                    }
                    _ => Err("Unregistered tool".into()),
                },
                |event| {
                    let text = match event {
                        crate::agent_loop::Event::Step(n) => format!(
                            "Agent step {n}: reading and reasoning. No files will be changed."
                        ),
                        crate::agent_loop::Event::ProposedCalls(calls) => format!(
                            "Proposed read tools: {}",
                            calls
                                .iter()
                                .map(|c| c.name.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                        crate::agent_loop::Event::ToolResult { id, error } => {
                            format!(
                                "Tool {id}: {}",
                                if error {
                                    "denied or failed"
                                } else {
                                    "read completed"
                                }
                            )
                        }
                        crate::agent_loop::Event::Finished(_) => return,
                    };
                    let _ = tx.send(Update::Delta(text));
                },
            )
        })();
        let _ = tx.send(match result {
            Ok(text) => Update::Done(text),
            Err(error) => Update::Error(error),
        });
    });
}
#[cfg(not(target_os = "linux"))]
fn send_agent(tab: &WindowTabData, _: crate::agent_loop::Mode, _: &str) {
    tab.chat
        .error
        .set("Secure Ask/Plan project tools are Linux-only in this version.".into());
}

pub fn chat_panel(tab: Rc<WindowTabData>) -> impl View {
    let data = tab.chat.clone();
    let config = tab.common.config;

    let click_tab = tab.clone();
    let indicator_tab = tab.clone();
    let clear = data.clone();
    let stop = data.clone();
    let chat_mode = data.clone();
    let ask_mode = data.clone();
    let plan_mode = data.clone();
    let trust = data.clone();
    let revoke = data.clone();
    let input_signal = data.input;
    let composer = text_editor(data.input.get_untracked())
        .placeholder("Ask about this file or selection...")
        .editor_style(|s| s.hide_gutter(true).scroll_beyond_last_line(false))
        .update(move |update| {
            if let Some(editor) = update.editor {
                input_signal.set(editor.doc().text().to_string());
            }
        });
    let composer_doc = composer.doc();
    create_effect(move |_| {
        let input = input_signal.get();
        if input.is_empty() && !composer_doc.text().is_empty() {
            use floem::views::editor::core::{
                editor::EditType, selection::Selection,
            };
            composer_doc.edit_single(
                Selection::region(0, composer_doc.text().len()),
                "",
                EditType::Other,
            );
        }
    });
    v_stack((
        h_stack((
            label(|| "AI CHAT".to_string()),
            label(|| "New chat".to_string())
                .on_click_stop(move |_| {
                    if !clear.busy.get_untracked() {
                        clear.turns.set(Vec::new());
                        clear.history.set(Vec::new());
                        clear.input.set(String::new());
                        clear.error.set(String::new());
                        clear.session.set(Session::new());
                        clear.state.set(RunState::Idle);
                        clear.persist(config.get_untracked().ai_chat.save_history);
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
            label(|| "Stop".to_string())
                .on_click_stop(move |_| { if stop.busy.get_untracked() { stop.stop(); stop.persist(config.get_untracked().ai_chat.save_history); } })
                .style(move |s| s.padding(6.0).cursor(CursorStyle::Pointer).selectable(false)),
        )),
        h_stack((
            label(|| "Chat".to_string()).on_click_stop(move |_| { if !chat_mode.busy.get_untracked() { chat_mode.mode.set(None); } }),
            label(|| "Ask".to_string()).on_click_stop(move |_| { if !ask_mode.busy.get_untracked() { ask_mode.mode.set(Some(crate::agent_loop::Mode::Ask)); } }),
            label(|| "Plan".to_string()).on_click_stop(move |_| { if !plan_mode.busy.get_untracked() { plan_mode.mode.set(Some(crate::agent_loop::Mode::Plan)); } }),
        )).style(|s| s.gap(12.0).padding(6.0).cursor(CursorStyle::Pointer)),
        label(move || format!("Mode: {}. Act is unavailable; Ask/Plan never write files.", match data.mode.get() { None => "Chat", Some(crate::agent_loop::Mode::Ask) => "Ask", Some(crate::agent_loop::Mode::Plan) => "Plan", Some(crate::agent_loop::Mode::Act) => "Act" }))
            .style(|s| s.font_size(11.0).max_width_full()),
        label(move || format!("Ask/Plan reads project files and sends results to {}. Approve only this project and server if you trust them.", config.get().ai_chat.endpoint))
            .style(|s| s.font_size(11.0).max_width_full()),
        h_stack((
            label(|| "Approve project reads for this window".to_string()).on_click_stop(move |_| { if !trust.busy.get_untracked() { trust.trusted_endpoint.set(config.get_untracked().ai_chat.endpoint.clone());
                    trust.trust_reads.set(true); } }),
            label(|| "Revoke".to_string()).on_click_stop(move |_| { revoke.trust_reads.set(false); if revoke.busy.get_untracked() { revoke.stop(); revoke.persist(config.get_untracked().ai_chat.save_history); } }),
        )).style(|s| s.gap(12.0).padding(6.0).cursor(CursorStyle::Pointer)),
        label(move || format!("Project read permission: {}", if data.trust_reads.get() { "approved for this window" } else { "not approved" }))
            .style(|s| s.font_size(11.0).max_width_full()),
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
        label(move || format!("Session: {:?}", data.state.get()))
            .style(|s| s.font_size(11.0).max_width_full()),
        crate::agent_review_ui::panel(tab.clone()),
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
                "Enter inserts a new line. Click Send. History is opt-in in Settings."
                    .to_string()
            }
        })
        .style(|s| s.font_size(11.0).max_width_full()),
        h_stack((
            composer.style(|s| s.flex_grow(1.0).min_width(0.0).height(100.0)),
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
