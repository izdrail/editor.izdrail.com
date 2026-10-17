use crate::chat_session::{self, RunGate, RunState, SavedTurn, Session};
use crate::{
    chat::{self, Context, Update},
    command::InternalCommand,
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
        Arc, Mutex,
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
    /// ACP mode: route Send to the configured external ACP agent
    /// (opencode, a Claude Code/Codex adapter) instead of chat/Ask/Plan.
    pub acp: RwSignal<bool>,
    /// ACP resume: the next ACP send resumes the most recent stored
    /// session for this agent+project instead of starting fresh.
    pub acp_resume: RwSignal<bool>,
    pub trust_reads: RwSignal<bool>,
    pub trusted_endpoint: RwSignal<String>,
    pub review: crate::agent_review_ui::ReviewData,
    gate: RunGate,
    cancellation: RwSignal<Arc<AtomicBool>>,
    storage: Option<PathBuf>,
    worker: Rc<ChatWorker>,
    /// Pid of the live ACP agent subprocess, so Stop can signal it while
    /// the worker thread is parked in a blocking read.
    acp_pid: Arc<Mutex<Option<u32>>>,
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
            acp: cx.create_rw_signal(false),
            acp_resume: cx.create_rw_signal(false),
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
            acp_pid: Arc::new(Mutex::new(None)),
        }
    }
}
impl ChatData {
    /// Write the current session as a walkthrough markdown document under
    /// <workspace>/.lapce/walkthroughs/ and return the file path. The
    /// caller opens it; no screenshots or recordings yet.
    pub fn export_walkthrough(
        &self,
        root: &std::path::Path,
    ) -> Result<PathBuf, String> {
        let turns: Vec<(String, String)> = self
            .turns
            .get_untracked()
            .iter()
            .map(|t| (t.role.to_string(), t.text.get_untracked()))
            .collect();
        if turns.is_empty() {
            return Err("Nothing to walk through yet".into());
        }
        let markdown = walkthrough_markdown(&turns);
        let dir = root.join(".lapce").join("walkthroughs");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let name = format!("walkthrough-{stamp}.md");
        let path = dir.join(&name);
        // Never overwrite a same-second export.
        let path = if path.exists() {
            dir.join(format!("walkthrough-{stamp}-{}.md", std::process::id()))
        } else {
            path
        };
        std::fs::write(&path, markdown).map_err(|e| e.to_string())?;
        Ok(path)
    }

    fn stop(&self) {
        self.gate.cancel();
        if let Some(pid) = self.acp_pid.lock().unwrap().take() {
            #[cfg(target_os = "linux")]
            {
                let _ = std::process::Command::new("kill")
                    .arg(pid.to_string())
                    .status();
            }
            #[cfg(not(target_os = "linux"))]
            let _ = pid;
        }
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
fn schedule_file_path(tab: &WindowTabData) -> Option<PathBuf> {
    tab.workspace
        .path
        .as_ref()
        .map(|p| p.join(".lapce").join("scheduled-prompts.json"))
}

/// Re-arm every minute. One due prompt fires per tick, so a backlog of
/// missed prompts drains one at a time instead of bursting.
fn start_prompt_scheduler(tab: Rc<WindowTabData>) {
    floem::action::exec_after(std::time::Duration::from_secs(60), move |_| {
        run_due_prompts(&tab);
        start_prompt_scheduler(tab.clone());
    });
}

fn run_due_prompts(tab: &Rc<WindowTabData>) {
    let Some(path) = schedule_file_path(tab) else {
        return;
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return;
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return;
    };
    let Ok(mut file) =
        serde_json::from_str::<crate::chat_schedule::ScheduleFile>(&text)
    else {
        return;
    };
    crate::chat_schedule::normalize(&mut file);
    let data = tab.chat.clone();
    if data.busy.get_untracked() {
        return;
    }
    if !tab.common.config.get_untracked().ai_chat.enabled {
        return;
    }
    let now = chrono::Local::now();
    let Some(idx) = file
        .prompts
        .iter()
        .position(|p| crate::chat_schedule::is_due(p, &now))
    else {
        return;
    };
    let prompt = file.prompts[idx].prompt.clone();
    file.prompts[idx].last_fired_unix = now.timestamp().max(0) as u64;
    // Persist the fire before sending, so a crash cannot double-fire.
    if let Ok(bytes) = serde_json::to_vec_pretty(&file) {
        let _ = std::fs::write(&path, bytes);
    }
    data.input.set(prompt);
    send(tab);
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
    // Slash commands override the mode buttons: they are explicit.
    let (prompt, record) = match chat::parse_slash(&prompt) {
        Some(Ok(chat::SlashCommand::Plan(text))) => {
            send_agent(tab, crate::agent_loop::Mode::Plan, &text);
            return;
        }
        Some(Ok(chat::SlashCommand::Ask(text))) => {
            send_agent(tab, crate::agent_loop::Mode::Ask, &text);
            return;
        }
        Some(Ok(chat::SlashCommand::Btw(text))) => (text, false),
        Some(Err(message)) => {
            data.error.set(message);
            return;
        }
        None => (prompt, true),
    };
    if data.acp.get_untracked() {
        send_acp(tab, &prompt);
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
            context: {
                let base = ctx
                    .as_ref()
                    .map(Context::indicator)
                    .unwrap_or_else(|| "No editor context".into());
                if record {
                    base
                } else {
                    format!("Side question - not added to history | {base}")
                }
            },
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
    data.persist(settings.save_history && record);
    let save_history = settings.save_history && record;
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
                    if record {
                        let mut history = request_messages[1..].to_vec();
                        history.push(Message {
                            role: "assistant".into(),
                            content: reply.get_untracked(),
                        });
                        done_data.history.set(history);
                    }
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
                // Local chat lane: no remote parameters.
                remote: None,
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
    let artifact_root = root.clone();
    let artifact_prompt = prompt.to_string();
    let main_split = tab.main_split.clone();
    create_effect(move |_| {
        if let Some(update) = progress.get() {
            if !done.gate.accepts(run) {
                return;
            }
            match update {
                Update::Delta(text) => reply.set(text),
                Update::Done(text) => {
                    if mode == crate::agent_loop::Mode::Plan {
                        write_plan_artifact(
                            &done,
                            &main_split,
                            &artifact_root,
                            &artifact_prompt,
                            &text,
                        );
                    }
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
/// Plan-mode artifact: persist the plan as Markdown under .lapce/plans
/// and open it as a normal editor buffer, so review and feedback happen
/// in a real document (edit the file, then ask the agent to proceed).
#[cfg(target_os = "linux")]
fn write_plan_artifact(
    data: &ChatData,
    main_split: &crate::main_split::MainSplitData,
    root: &std::path::Path,
    prompt: &str,
    plan: &str,
) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dir = root.join(".lapce").join("plans");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        data.error.set(format!("Could not create the plans directory: {e}"));
        return;
    }
    let path = dir.join(format!("plan-{stamp}.md"));
    if let Err(e) =
        std::fs::write(&path, crate::agent_loop::plan_artifact(prompt, plan))
    {
        data.error.set(format!("Could not write the plan artifact: {e}"));
        return;
    }
    main_split.jump_to_location(
        crate::editor::location::EditorLocation {
            path,
            position: None,
            scroll_offset: None,
            ignore_unconfirmed: false,
            same_editor_tab: false,
        },
        None,
    );
}

#[cfg(not(target_os = "linux"))]
fn send_agent(tab: &WindowTabData, _: crate::agent_loop::Mode, _: &str) {
    tab.chat
        .error
        .set("Secure Ask/Plan project tools are Linux-only in this version.".into());
}

/// ACP mode: one Send spawns the configured agent (fresh session per
/// turn for now; session/load continuity is a later milestone), runs the
/// handshake on a worker thread, forwards the prompt and streams
/// rendered TurnEvents into the reply. Stop kills the subprocess by pid.
#[cfg(target_os = "linux")]
fn send_acp(tab: &WindowTabData, prompt: &str) {
    let data = tab.chat.clone();
    let settings = tab.common.config.get_untracked().ai_chat.clone();
    let spec = settings.acp_command.trim().to_string();
    let Some((program, args)) = crate::agent_acp_session::split_command(&spec) else {
        data.error.set(
            "Set an ACP agent command in Settings > AI Chat first (for example `opencode acp`)."
                .into(),
        );
        return;
    };
    let Some(root) = tab.workspace.path.clone() else {
        data.error.set("Open a local project folder first.".into());
        return;
    };
    let root = match root.canonicalize() {
        Ok(r) => r,
        Err(e) => {
            data.error.set(format!("Cannot resolve the project folder: {e}"));
            return;
        }
    };
    let ctx = context(tab);
    let composed = crate::agent_acp_session::compose_prompt(ctx.as_ref(), prompt);
    let resume = if data.acp_resume.get_untracked() {
        let cwd = root.to_string_lossy().into_owned();
        crate::agent_acp_store::load_sessions(&root)
            .into_iter()
            .find(|s| s.agent == spec && s.cwd == cwd)
            .map(|s| s.session_id)
    } else {
        None
    };
    if data.acp_resume.get_untracked() && resume.is_none() {
        data.error.set(
            "No stored ACP session for this agent and project yet.".into(),
        );
        return;
    }
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
                "ACP agent: {spec} | {}",
                ctx.as_ref()
                    .map(Context::indicator)
                    .unwrap_or_else(|| "No editor context".into())
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
    let save = settings.save_history;
    let (tx, rx) = std::sync::mpsc::channel();
    let progress = floem::ext_event::create_signal_from_channel(rx);
    let done = data.clone();
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
    let acp_pid = data.acp_pid.clone();
    let title: String = prompt.lines().next().unwrap_or("").chars().take(80).collect();
    std::thread::spawn(move || {
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let result = run_acp_turn(
            &program, &arg_refs, &root, &composed, resume, &cancelled, &acp_pid, &tx,
        );
        let _ = tx.send(match result {
            Ok((text, resumable_id)) => {
                if let Some(session_id) = resumable_id {
                    let entry = crate::agent_acp_store::StoredSession {
                        session_id,
                        agent: spec,
                        cwd: root.to_string_lossy().into_owned(),
                        title,
                        updated_unix: crate::agent_acp_store::now_unix(),
                    };
                    let _ = crate::agent_acp_store::record_session(&root, entry);
                }
                Update::Done(text)
            }
            Err(error) => Update::Error(error),
        });
    });
}
#[cfg(not(target_os = "linux"))]
fn send_acp(tab: &WindowTabData, _: &str) {
    tab.chat
        .error
        .set("ACP agents are Linux-only in this version.".into());
}

/// Render one session event as a short status line for the reply body.
#[cfg(target_os = "linux")]
fn render_acp_event(event: &crate::agent_acp_session::TurnEvent) -> String {
    use crate::agent_acp_session::TurnEvent as E;
    match event {
        E::Connected { agent, version, .. } => format!("Connected to {agent} {version}."),
        E::NeedsAuth { methods } => format!(
            "The agent wants authentication ({}). Authenticate it in a terminal once, then retry.",
            methods.join(", ")
        ),
        E::SessionStarted { .. } => "Session started.".into(),
        E::Text { text } => text.clone(),
        E::Plan { entries } => format!(
            "Plan:\n{}",
            entries.iter().map(|e| format!("- {e}")).collect::<Vec<_>>().join("\n")
        ),
        E::ToolCall { title, status, .. } => format!("Tool: {title} ({status})"),
        E::Usage { used, size } => format!("Context: {used}/{size} tokens."),
        E::FsRead { path, ok } => {
            format!("Agent read {path}{}.", if *ok { "" } else { " (refused)" })
        }
        E::Notice { text } => text.clone(),
        E::Error { message } => format!("Agent error: {message}"),
        E::TurnDone { .. } | E::Closed => String::new(),
    }
}

/// Own the subprocess for one turn; always reaps it on the way out.
#[cfg(target_os = "linux")]
fn run_acp_turn(
    program: &str,
    args: &[&str],
    root: &std::path::Path,
    prompt: &str,
    resume: Option<String>,
    cancelled: &Arc<AtomicBool>,
    pid_slot: &Arc<Mutex<Option<u32>>>,
    tx: &std::sync::mpsc::Sender<Update>,
) -> Result<(String, Option<String>), String> {
    let mut process = crate::agent_acp_process::AcpProcess::spawn(program, args, root)
        .map_err(|e| format!("Could not launch `{program}`: {e}"))?;
    *pid_slot.lock().unwrap() = Some(process.id());
    let result = drive_acp_turn(&mut process, root, prompt, resume, cancelled, tx);
    *pid_slot.lock().unwrap() = None;
    process.kill();
    result
}

/// Handshake, one prompt, stream until the turn ends. Returns the full
/// reply body accumulated from chunks and status lines.
#[cfg(target_os = "linux")]
fn drive_acp_turn(
    process: &mut crate::agent_acp_process::AcpProcess,
    root: &std::path::Path,
    prompt: &str,
    resume: Option<String>,
    cancelled: &Arc<AtomicBool>,
    tx: &std::sync::mpsc::Sender<Update>,
) -> Result<(String, Option<String>), String> {
    use crate::agent_acp_session::{AcpSession, Phase, TurnEvent};
    let (mut session, hello) = AcpSession::start_resumable(
        root.to_path_buf(),
        "lapce",
        env!("CARGO_PKG_VERSION"),
        resume,
    );
    process.send(&hello).map_err(|e| e.message)?;
    let mut guard = 0;
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        guard += 1;
        if guard > 64 {
            return Err("ACP setup did not finish after 64 messages".into());
        }
        let Some(message) = process.recv().map_err(|e| e.message)? else {
            let logs = process.stderr_log();
            return Err(format!("Agent exited during setup. {}", logs.join(" | ")));
        };
        let (events, out) = session.handle(&message);
        for event in &events {
            let line = render_acp_event(event);
            if !line.is_empty() {
                let _ = tx.send(Update::Delta(line));
            }
        }
        for value in out {
            process.send(&value).map_err(|e| e.message)?;
        }
        if events.iter().any(|e| matches!(e, TurnEvent::Closed)) {
            return Err("Agent closed the session during setup".into());
        }
        if events.iter().any(|e| matches!(e, TurnEvent::NeedsAuth { .. })) {
            return Err("Agent needs terminal authentication first".into());
        }
        if session.phase() == Phase::Ready {
            break;
        }
    }
    let prompt_message = session.begin_prompt(prompt).map_err(|e| e.message)?;
    process.send(&prompt_message).map_err(|e| e.message)?;
    let mut text = String::new();
    loop {
        if cancelled.load(Ordering::Relaxed) {
            if let Some(cancel) = session.cancel() {
                let _ = process.send(&cancel);
            }
            return Err("Cancelled".into());
        }
        let Some(message) = process.recv().map_err(|e| e.message)? else {
            return Err("Agent exited mid-turn".into());
        };
        let (events, out) = session.handle(&message);
        for value in out {
            process.send(&value).map_err(|e| e.message)?;
        }
        let mut finished = false;
        for event in events {
            match event {
                TurnEvent::TurnDone { .. } => finished = true,
                TurnEvent::Error { message } => return Err(message),
                TurnEvent::Closed => return Err("Agent closed the session".into()),
                other => {
                    let line = render_acp_event(&other);
                    if !line.is_empty() {
                        if !text.is_empty() {
                            text.push_str("\n\n");
                        }
                        text.push_str(&line);
                        let _ = tx.send(Update::Delta(text.clone()));
                    }
                }
            }
        }
        if finished {
            let resumable = if session.can_resume() {
                session.session_id().map(str::to_string)
            } else {
                None
            };
            return Ok((text, resumable));
        }
    }
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
    let acp_mode = data.clone();
    let acp_resume = data.clone();
    let resume_tab = tab.clone();
    let walkthrough_tab = tab.clone();
    let walkthrough = data.clone();
    let schedules_tab = tab.clone();
    let schedules_data = data.clone();
    start_prompt_scheduler(tab.clone());
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
            label(|| "Walkthrough".to_string())
                .on_click_stop(move |_| {
                    if walkthrough.busy.get_untracked() {
                        return;
                    }
                    let Some(root) = walkthrough_tab.workspace.path.clone()
                    else {
                        walkthrough
                            .error
                            .set("Open a folder to export a walkthrough".into());
                        return;
                    };
                    match walkthrough.export_walkthrough(&root) {
                        Ok(path) => {
                            walkthrough_tab
                                .common
                                .internal_command
                                .send(InternalCommand::OpenFile { path });
                        }
                        Err(e) => walkthrough.error.set(e),
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
            label(|| "Schedules".to_string())
                .on_click_stop(move |_| {
                    let Some(root) = schedules_tab.workspace.path.clone()
                    else {
                        schedules_data
                            .error
                            .set("Open a folder to schedule prompts".into());
                        return;
                    };
                    let dir = root.join(".lapce");
                    let path = dir.join("scheduled-prompts.json");
                    if !path.exists() {
                        let _ = std::fs::create_dir_all(&dir);
                        let _ = std::fs::write(
                            &path,
                            crate::chat_schedule::EXAMPLE_FILE,
                        );
                    }
                    schedules_tab
                        .common
                        .internal_command
                        .send(InternalCommand::OpenFile { path });
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
        h_stack((
            label(|| "Chat".to_string()).on_click_stop(move |_| { if !chat_mode.busy.get_untracked() { chat_mode.mode.set(None); chat_mode.acp.set(false); chat_mode.acp_resume.set(false); } }),
            label(|| "Ask".to_string()).on_click_stop(move |_| { if !ask_mode.busy.get_untracked() { ask_mode.mode.set(Some(crate::agent_loop::Mode::Ask)); ask_mode.acp.set(false); ask_mode.acp_resume.set(false); } }),
            label(|| "Plan".to_string()).on_click_stop(move |_| { if !plan_mode.busy.get_untracked() { plan_mode.mode.set(Some(crate::agent_loop::Mode::Plan)); plan_mode.acp.set(false); plan_mode.acp_resume.set(false); } }),
            label(|| "ACP".to_string()).on_click_stop(move |_| { if !acp_mode.busy.get_untracked() { acp_mode.mode.set(None); acp_mode.acp.set(true); acp_mode.acp_resume.set(false); } }),
            label(move || {
                let settings = resume_tab.common.config.get().ai_chat.clone();
                let count = resume_tab
                    .workspace
                    .path
                    .as_ref()
                    .and_then(|p| p.canonicalize().ok())
                    .map(|root| {
                        let cwd = root.to_string_lossy().into_owned();
                        crate::agent_acp_store::load_sessions(&root)
                            .into_iter()
                            .filter(|s| s.agent == settings.acp_command.trim() && s.cwd == cwd)
                            .count()
                    })
                    .unwrap_or(0);
                format!("Resume ({count})")
            })
            .on_click_stop(move |_| {
                if !acp_resume.busy.get_untracked() {
                    acp_resume.mode.set(None);
                    acp_resume.acp.set(true);
                    acp_resume.acp_resume.set(true);
                }
            }),
        )).style(|s| s.gap(12.0).padding(6.0).cursor(CursorStyle::Pointer)),
        label(move || format!("Mode: {}. Act is unavailable; Ask/Plan never write files.", if data.acp.get() { "ACP agent" } else { match data.mode.get() { None => "Chat", Some(crate::agent_loop::Mode::Ask) => "Ask", Some(crate::agent_loop::Mode::Plan) => "Plan", Some(crate::agent_loop::Mode::Act) => "Act" } }))
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

/// Cap on the rendered walkthrough, so a pathological session can never
/// produce an unbounded file.
pub const WALKTHROUGH_CAP: usize = 512 * 1024;

/// Render a chat session as a walkthrough: the goal (first user turn's
/// first line), then each turn in order. Fences and non-ASCII content in
/// turn text pass through unchanged; the output is clipped on a char
/// boundary with an explicit marker.
pub fn walkthrough_markdown(turns: &[(String, String)]) -> String {
    let mut out = String::from("# Walkthrough\n\n");
    if let Some((_, first)) = turns.iter().find(|(role, _)| role == "You") {
        let goal = first.lines().next().unwrap_or("").trim();
        if !goal.is_empty() {
            out.push_str("**Goal:** ");
            out.push_str(goal);
            out.push_str("\n\n");
        }
    }
    for (role, text) in turns {
        out.push_str("## ");
        out.push_str(role);
        out.push_str("\n\n");
        out.push_str(text.trim());
        out.push_str("\n\n");
    }
    if out.len() > WALKTHROUGH_CAP {
        let mut end = WALKTHROUGH_CAP;
        while !out.is_char_boundary(end) {
            end -= 1;
        }
        out.truncate(end);
        out.push_str("\n[walkthrough truncated to fit the file cap]\n");
    }
    out
}

#[cfg(test)]
mod walkthrough_tests {
    use super::walkthrough_markdown;

    fn turns(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter().map(|(r, t)| (r.to_string(), t.to_string())).collect()
    }

    #[test]
    fn renders_goal_then_turns_in_order() {
        let md = walkthrough_markdown(&turns(&[
            ("You", "add a health endpoint\nmore detail here"),
            ("Assistant", "Done:\n```rust\nfn health() {}\n```"),
            ("You", "thanks"),
        ]));
        assert!(md.starts_with("# Walkthrough\n\n**Goal:** add a health endpoint\n"));
        let you = md.find("## You").unwrap();
        let assistant = md.find("## Assistant").unwrap();
        let you2 = md.rfind("## You").unwrap();
        assert!(you < assistant && assistant < you2);
        // Fences pass through untouched.
        assert!(md.contains("```rust\nfn health() {}\n```"));
    }

    #[test]
    fn skips_empty_goal_and_keeps_non_ascii() {
        let md = walkthrough_markdown(&turns(&[
            ("Assistant", "héllo 日本語 🦀"),
            ("You", "\n"),
        ]));
        assert!(!md.contains("**Goal:**"));
        assert!(md.contains("héllo 日本語 🦀"));
    }

    #[test]
    fn cap_clips_on_a_char_boundary_with_a_marker() {
        // '日' is 3 bytes; a cap mid-codepoint must pull back, not split.
        let big = "日".repeat(super::WALKTHROUGH_CAP);
        let md = walkthrough_markdown(&turns(&[("You", &big)]));
        assert!(md.ends_with("[walkthrough truncated to fit the file cap]\n"));
        assert!(std::str::from_utf8(md.as_bytes()).is_ok());
        assert!(md.len() <= super::WALKTHROUGH_CAP + 64);
    }
}
