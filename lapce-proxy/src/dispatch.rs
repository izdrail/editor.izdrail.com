use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

use alacritty_terminal::{event::WindowSize, event_loop::Msg};
use anyhow::{Context, Result, anyhow};
use crossbeam_channel::Sender;
use git2::{
    DiffOptions, ErrorCode::NotFound, Oid, Repository, build::CheckoutBuilder,
};
use grep_matcher::Matcher;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{SearcherBuilder, sinks::UTF8};
use indexmap::IndexMap;
use lapce_rpc::{
    RequestId, RpcError,
    buffer::BufferId,
    core::{CoreNotification, CoreRpcHandler, FileChanged},
    file::FileNodeItem,
    file_line::FileLine,
    proxy::{
        ProxyHandler, ProxyNotification, ProxyRequest, ProxyResponse,
        ProxyRpcHandler, SearchMatch,
    },
    source_control::{DiffInfo, FileDiff},
    terminal::TermId,
};
use lapce_xi_rope::Rope;
use lsp_types::{MessageType, ShowMessageParams, TextDocumentItem, Url};
use parking_lot::Mutex;

use crate::{
    buffer::{Buffer, get_mod_time, load_file},
    plugin::{PluginCatalog, PluginCatalogRpcHandler, remove_volt},
    terminal::{Terminal, TerminalSender},
    watcher::{FileWatcher, Notify, WatchToken},
};

const OPEN_FILE_EVENT_TOKEN: WatchToken = WatchToken(1);
const WORKSPACE_EVENT_TOKEN: WatchToken = WatchToken(2);

pub struct Dispatcher {
    workspace: Option<PathBuf>,
    pub proxy_rpc: ProxyRpcHandler,
    core_rpc: CoreRpcHandler,
    catalog_rpc: PluginCatalogRpcHandler,
    buffers: HashMap<PathBuf, Buffer>,
    terminals: HashMap<TermId, TerminalSender>,
    file_watcher: FileWatcher,
    window_id: usize,
    tab_id: usize,
    lsp: Option<crate::lsp::Manager>,
    dap: crate::dap::Manager,
    ai: lapce_ai::process::Worker,
}

impl ProxyHandler for Dispatcher {
    fn handle_notification(&mut self, rpc: ProxyNotification) {
        use ProxyNotification::*;
        match rpc {
            CancelInlineCompletion {} => self.ai.soft_cancel(),
            Initialize {
                workspace,
                window_id,
                tab_id,
                disabled_volts,
                extra_plugin_paths,
                plugin_configurations,
            } => {
                self.window_id = window_id;
                self.tab_id = tab_id;
                self.dap.shutdown_all();
                self.workspace = workspace;
                // Workspace is known here, not in `new`: install the
                // PHP-aware adapter resolver over a fresh manager.
                self.dap = crate::dap::Manager::with_resolver(
                    self.core_rpc.clone(),
                    std::sync::Arc::new(crate::php_debug::php_adapter_resolver(
                        self.workspace.clone(),
                        true,
                    )),
                );
                if let Some(old) = self.lsp.take() {
                    old.send(crate::lsp::Job::Shutdown);
                }
                self.lsp = Some(crate::lsp::Manager::new(
                    self.workspace.clone(),
                    self.core_rpc.clone(),
                ));
                self.file_watcher.notify(FileWatchNotifier::new(
                    self.workspace.clone(),
                    self.core_rpc.clone(),
                    self.proxy_rpc.clone(),
                ));
                if let Some(workspace) = self.workspace.as_ref() {
                    self.file_watcher
                        .watch(workspace, true, WORKSPACE_EVENT_TOKEN);
                }

                let plugin_rpc = self.catalog_rpc.clone();
                let workspace = self.workspace.clone();
                thread::spawn(move || {
                    let mut plugin = PluginCatalog::new(
                        workspace,
                        disabled_volts,
                        extra_plugin_paths,
                        plugin_configurations,
                        plugin_rpc.clone(),
                    );
                    plugin_rpc.mainloop(&mut plugin);
                });

                self.core_rpc.notification(CoreNotification::ProxyStatus {
                    status: lapce_rpc::proxy::ProxyStatus::Connected,
                });

                // send home directory for initinal filepicker dir
                let dirs = directories::UserDirs::new();

                if let Some(dirs) = dirs {
                    self.core_rpc.home_dir(dirs.home_dir().into());
                }
            }
            CloseLanguageDocument { path } => {
                if let Some(lsp) = &self.lsp {
                    lsp.send(crate::lsp::Job::Close(path));
                }
            }
            OpenPaths { paths } => {
                self.core_rpc
                    .notification(CoreNotification::OpenPaths { paths });
            }
            OpenFileChanged { path } => {
                if path.exists() {
                    if let Some(buffer) = self.buffers.get(&path) {
                        if get_mod_time(&buffer.path) == buffer.mod_time {
                            return;
                        }
                        match load_file(&buffer.path) {
                            Ok(content) => {
                                self.core_rpc.open_file_changed(
                                    path,
                                    FileChanged::Change(content),
                                );
                            }
                            Err(err) => {
                                tracing::event!(
                                    tracing::Level::ERROR,
                                    "Failed to re-read file after change notification: {err}"
                                );
                            }
                        }
                    }
                } else {
                    self.buffers.remove(&path);
                    if let Some(lsp) = &self.lsp {
                        lsp.send(crate::lsp::Job::Close(path.clone()));
                    }
                    self.core_rpc.open_file_changed(path, FileChanged::Delete);
                }
            }
            Shutdown {} => {
                self.dap.shutdown_all();
                self.ai.cancel();
                self.catalog_rpc.shutdown();
                if let Some(lsp) = self.lsp.take() {
                    lsp.send(crate::lsp::Job::Shutdown);
                }
                for (_, sender) in self.terminals.iter() {
                    sender.send(Msg::Shutdown);
                }
                self.proxy_rpc.shutdown();
            }
            Update { path, delta, rev } => {
                // Every keystroke cancels the in-flight suggestion, but
                // cooperatively: the worker keeps its model warm.
                self.ai.soft_cancel();
                let buffer = self.buffers.get_mut(&path).unwrap();
                let old_text = buffer.rope.clone();
                buffer.update(&delta, rev);
                if let Some(lsp) = &self.lsp {
                    lsp.send(crate::lsp::Job::Sync(buffer.clone()));
                }
                self.catalog_rpc.did_change_text_document(
                    &path,
                    rev,
                    delta,
                    old_text,
                    buffer.rope.clone(),
                );
            }
            NewTerminal { term_id, profile } => {
                let mut terminal = match Terminal::new(term_id, profile, 50, 10) {
                    Ok(terminal) => terminal,
                    Err(e) => {
                        self.core_rpc.terminal_launch_failed(term_id, e.to_string());
                        return;
                    }
                };

                #[allow(unused)]
                let mut child_id = None;

                #[cfg(target_os = "windows")]
                {
                    child_id = terminal.pty.child_watcher().pid().map(|x| x.get());
                }
                #[cfg(not(target_os = "windows"))]
                {
                    child_id = Some(terminal.pty.child().id());
                }

                self.core_rpc.terminal_process_id(term_id, child_id);
                let tx = terminal.tx.clone();
                let poller = terminal.poller.clone();
                let sender = TerminalSender::new(tx, poller);
                self.terminals.insert(term_id, sender);
                let rpc = self.core_rpc.clone();
                thread::spawn(move || {
                    terminal.run(rpc);
                });
            }
            TerminalWrite { term_id, content } => {
                if let Some(tx) = self.terminals.get(&term_id) {
                    tx.send(Msg::Input(content.into_bytes().into()));
                }
            }
            TerminalResize {
                term_id,
                width,
                height,
            } => {
                if let Some(tx) = self.terminals.get(&term_id) {
                    let size = WindowSize {
                        num_lines: height as u16,
                        num_cols: width as u16,
                        cell_width: 1,
                        cell_height: 1,
                    };

                    tx.send(Msg::Resize(size));
                }
            }
            TerminalClose { term_id } => {
                if let Some(tx) = self.terminals.remove(&term_id) {
                    tx.send(Msg::Shutdown);
                }
            }
            GitCommit { message, diffs } => {
                if let Some(workspace) = self.workspace.as_ref() {
                    match git_commit(workspace, &message, diffs) {
                        Ok(()) => (),
                        Err(e) => {
                            self.core_rpc.show_message(
                                "Git Commit failure".to_owned(),
                                ShowMessageParams {
                                    typ: MessageType::ERROR,
                                    message: e.to_string(),
                                },
                            );
                        }
                    }
                }
            }
            GitCheckout { reference } => {
                if let Some(workspace) = self.workspace.as_ref() {
                    match git_checkout(workspace, &reference) {
                        Ok(()) => (),
                        Err(e) => eprintln!("{e:?}"),
                    }
                }
            }
            GitCreateBranch { name } => {
                if let Some(workspace) = self.workspace.as_ref() {
                    match git_create_branch(workspace, &name) {
                        Ok(()) => (),
                        Err(e) => eprintln!("{e:?}"),
                    }
                }
            }
            GitDiscardFilesChanges { files } => {
                if let Some(workspace) = self.workspace.as_ref() {
                    match git_discard_files_changes(
                        workspace,
                        files.iter().map(AsRef::as_ref),
                    ) {
                        Ok(()) => (),
                        Err(e) => eprintln!("{e:?}"),
                    }
                }
            }
            GitDiscardWorkspaceChanges {} => {
                if let Some(workspace) = self.workspace.as_ref() {
                    match git_discard_workspace_changes(workspace) {
                        Ok(()) => (),
                        Err(e) => eprintln!("{e:?}"),
                    }
                }
            }
            GitInit {} => {
                if let Some(workspace) = self.workspace.as_ref() {
                    match git_init(workspace) {
                        Ok(()) => (),
                        Err(e) => eprintln!("{e:?}"),
                    }
                }
            }
            // Compatibility notifications cannot start plugins or external servers.
            Completion {
                request_id,
                path,
                input,
                position,
            } => {
                let core = self.core_rpc.clone();
                self.catalog_rpc
                    .completion(request_id, &path, input.clone(), position);
                self.lsp_request(
                    path,
                    "textDocument/completion",
                    serde_json::json!({"position":position}),
                    move |result| {
                        if let Ok((plugin_id, value)) = result {
                            if let Ok(resp) = serde_json::from_value::<
                                Option<lsp_types::CompletionResponse>,
                            >(value)
                            {
                                core.completion_response(
                                    request_id,
                                    input,
                                    resp.unwrap_or_else(|| {
                                        lsp_types::CompletionResponse::Array(
                                            Vec::new(),
                                        )
                                    }),
                                    plugin_id,
                                );
                            }
                        }
                    },
                );
            }
            SignatureHelp {
                request_id,
                path,
                position,
            } => {
                let core = self.core_rpc.clone();
                self.catalog_rpc
                    .signature_help(request_id, &path, position);
                self.lsp_request(
                    path,
                    "textDocument/signatureHelp",
                    serde_json::json!({"position": position}),
                    move |result| {
                        if let Ok((plugin_id, value)) = result {
                            if let Ok(Some(resp)) = serde_json::from_value::<
                                Option<lsp_types::SignatureHelp>,
                            >(value)
                            {
                                core.signature_help_response(
                                    request_id,
                                    resp,
                                    plugin_id,
                                );
                            }
                        }
                    },
                );
            }
            UpdatePluginConfigs { configs } => {
                if let Err(err) = self.catalog_rpc.update_plugin_configs(configs)
                {
                    tracing::error!("{:?}", err);
                }
            }
            InstallVolt { volt } => {
                if let Err(err) = self.catalog_rpc.install_volt(volt) {
                    tracing::error!("{:?}", err);
                }
            }
            ReloadVolt { volt } => {
                if let Err(err) = self.catalog_rpc.reload_volt(volt) {
                    tracing::error!("{:?}", err);
                }
            }
            RemoveVolt { volt } => {
                if let Err(err) = remove_volt(self.catalog_rpc.clone(), volt) {
                    tracing::error!("{:?}", err);
                }
            }
            DisableVolt { volt } => {
                self.catalog_rpc.stop_volt(volt);
            }
            EnableVolt { volt } => {
                if let Err(err) = self.catalog_rpc.enable_volt(volt) {
                    tracing::error!("{:?}", err);
                }
            }
            DapStart { config, breakpoints } => {
                self.dap.start(config, breakpoints, self.workspace.clone());
            }
            DapRestart { dap_id, breakpoints } => {
                self.dap.restart(dap_id, breakpoints, self.workspace.clone());
            }
            DapContinue { dap_id, thread_id } => self.dap.continue_(dap_id, thread_id),
            DapPause { dap_id, thread_id } => self.dap.pause(dap_id, thread_id),
            DapStepOver { dap_id, thread_id } => self.dap.step_over(dap_id, thread_id),
            DapStepInto { dap_id, thread_id } => self.dap.step_into(dap_id, thread_id),
            DapStepOut { dap_id, thread_id } => self.dap.step_out(dap_id, thread_id),
            DapStop { dap_id } => self.dap.stop(dap_id),
            DapDisconnect { dap_id } => self.dap.disconnect(dap_id),
            DapSetBreakpoints {
                dap_id,
                path,
                breakpoints,
            } => self.dap.set_breakpoints(dap_id, path, breakpoints),
            // Process ids only matter for run-in-terminal, which we do not
            // advertise to adapters.
            DapProcessId { .. }
            | LspCancel { .. } => {}
        }
    }

    fn handle_request(&mut self, id: RequestId, rpc: ProxyRequest) {
        use ProxyRequest::*;
        match rpc {
            GitDiffForCommit { diffs } => {
                let proxy = self.proxy_rpc.clone();
                let workspace = self.workspace.clone();
                std::thread::spawn(move || {
                    let result = match workspace {
                        Some(workspace) => git_diff_for_commit(&workspace, &diffs)
                            .map(|diff| ProxyResponse::GitDiffForCommit { diff })
                            .map_err(|e| RpcError {
                                code: -32603,
                                message: e.to_string(),
                            }),
                        None => Err(RpcError {
                            code: -32603,
                            message: "No workspace open".into(),
                        }),
                    };
                    proxy.handle_response(id, result);
                });
            }
            NewBuffer { buffer_id, path } => {
                let buffer = Buffer::new(buffer_id, path.clone());
                let content = buffer.rope.to_string();
                let read_only = buffer.read_only;
                self.file_watcher.watch(&path, false, OPEN_FILE_EVENT_TOKEN);
                if let Some(lsp) = &self.lsp {
                    lsp.send(crate::lsp::Job::Sync(buffer.clone()));
                }
                self.catalog_rpc.did_open_document(
                    &path,
                    buffer.language_id.to_string(),
                    buffer.rev as i32,
                    content.clone(),
                );
                self.buffers.insert(path, buffer);
                self.respond_rpc(
                    id,
                    Ok(ProxyResponse::NewBufferResponse { content, read_only }),
                );
            }
            BufferHead { path } => {
                let result = if let Some(workspace) = self.workspace.as_ref() {
                    let result = file_get_head(workspace, &path);
                    if let Ok((_blob_id, content)) = result {
                        Ok(ProxyResponse::BufferHeadResponse {
                            version: "head".to_string(),
                            content,
                        })
                    } else {
                        Err(RpcError {
                            code: 0,
                            message: "can't get file head".to_string(),
                        })
                    }
                } else {
                    Err(RpcError {
                        code: 0,
                        message: "no workspace set".to_string(),
                    })
                };
                self.respond_rpc(id, result);
            }
            GlobalSearch {
                pattern,
                case_sensitive,
                whole_word,
                is_regex,
            } => {
                static WORKER_ID: AtomicU64 = AtomicU64::new(0);
                let our_id = WORKER_ID.fetch_add(1, Ordering::SeqCst) + 1;

                let workspace = self.workspace.clone();
                let buffers = self
                    .buffers
                    .iter()
                    .map(|p| p.0)
                    .cloned()
                    .collect::<Vec<PathBuf>>();
                let proxy_rpc = self.proxy_rpc.clone();

                // Perform the search on another thread to avoid blocking the proxy thread
                thread::spawn(move || {
                    proxy_rpc.handle_response(
                        id,
                        search_in_path(
                            our_id,
                            &WORKER_ID,
                            workspace
                                .iter()
                                .flat_map(|w| ignore::Walk::new(w).flatten())
                                .chain(
                                    buffers.iter().flat_map(|p| {
                                        ignore::Walk::new(p).flatten()
                                    }),
                                )
                                .map(|p| p.into_path()),
                            &pattern,
                            case_sensitive,
                            whole_word,
                            is_regex,
                        ),
                    );
                });
            }
            GitGetRemoteFileUrl { file } => {
                if let Some(workspace) = self.workspace.as_ref() {
                    match git_get_remote_file_url(workspace, &file) {
                        Ok(s) => self.proxy_rpc.handle_response(
                            id,
                            Ok(ProxyResponse::GitGetRemoteFileUrl { file_url: s }),
                        ),
                        Err(e) => eprintln!("{e:?}"),
                    }
                }
            }
            GetFiles { .. } => {
                let workspace = self.workspace.clone();
                let proxy_rpc = self.proxy_rpc.clone();
                thread::spawn(move || {
                    let result = if let Some(workspace) = workspace {
                        let git_folder =
                            ignore::overrides::OverrideBuilder::new(&workspace)
                                .add("!.git/")
                                .map(|git_folder| git_folder.build());

                        let walker = match git_folder {
                            Ok(Ok(git_folder)) => {
                                ignore::WalkBuilder::new(&workspace)
                                    .hidden(false)
                                    .parents(false)
                                    .require_git(false)
                                    .overrides(git_folder)
                                    .build()
                            }
                            _ => ignore::WalkBuilder::new(&workspace)
                                .parents(false)
                                .require_git(false)
                                .build(),
                        };

                        let mut items = Vec::new();
                        for path in walker.flatten() {
                            if let Some(file_type) = path.file_type() {
                                if file_type.is_file() {
                                    items.push(path.into_path());
                                }
                            }
                        }
                        Ok(ProxyResponse::GetFilesResponse { items })
                    } else {
                        Ok(ProxyResponse::GetFilesResponse { items: Vec::new() })
                    };
                    proxy_rpc.handle_response(id, result);
                });
            }
            GetOpenFilesContent {} => {
                let items = self
                    .buffers
                    .iter()
                    .map(|(path, buffer)| TextDocumentItem {
                        uri: Url::from_file_path(path).unwrap(),
                        language_id: buffer.language_id.to_string(),
                        version: buffer.rev as i32,
                        text: buffer.get_document(),
                    })
                    .collect();
                let resp = ProxyResponse::GetOpenFilesContentResponse { items };
                self.proxy_rpc.handle_response(id, Ok(resp));
            }
            ReadDir { path } => {
                let proxy_rpc = self.proxy_rpc.clone();
                thread::spawn(move || {
                    let path = match path {
                        lapce_rpc::proxy::PathRequest::Home => {
                            std::env::home_dir().unwrap_or(PathBuf::from("/"))
                        }
                        lapce_rpc::proxy::PathRequest::Path(path_buf) => path_buf,
                    };
                    let result = fs::read_dir(&path)
                        .map(|entries| {
                            let mut items = entries
                                .into_iter()
                                .filter_map(|entry| {
                                    entry
                                        .map(|e| FileNodeItem {
                                            path: e.path(),
                                            is_dir: e.path().is_dir(),
                                            open: false,
                                            read: false,
                                            children: HashMap::new(),
                                            children_open_count: 0,
                                        })
                                        .ok()
                                })
                                .collect::<Vec<FileNodeItem>>();

                            items.sort();

                            ProxyResponse::ReadDirResponse {
                                items,
                                dir: FileNodeItem {
                                    path: path.clone(),
                                    is_dir: path.is_dir(),
                                    read: false,
                                    open: false,
                                    children: HashMap::new(),
                                    children_open_count: 0,
                                },
                            }
                        })
                        .map_err(|e| RpcError {
                            code: 0,
                            message: format!(
                                "Error reading directory {:?}: {}",
                                &path,
                                e.to_string()
                            ),
                        });
                    proxy_rpc.handle_response(id, result);
                });
            }
            Save {
                rev,
                path,
                create_parents,
            } => {
                let buffer = self.buffers.get_mut(&path).unwrap();
                let result = buffer
                    .save(rev, create_parents)
                    .map(|_r| {
                        if let Some(lsp) = &self.lsp {
                            lsp.send(crate::lsp::Job::Save(path.clone()));
                        }
                        self.catalog_rpc
                            .did_save_text_document(&path, buffer.rope.clone());
                        ProxyResponse::SaveResponse {}
                    })
                    .map_err(|e| RpcError {
                        code: 0,
                        message: e.to_string(),
                    });
                self.respond_rpc(id, result);
            }
            SaveBufferAs {
                buffer_id,
                path,
                rev,
                content,
                create_parents,
            } => {
                let mut buffer = Buffer::new(buffer_id, path.clone());
                buffer.rope = Rope::from(content);
                buffer.rev = rev;
                let result = buffer
                    .save(rev, create_parents)
                    .map(|_| ProxyResponse::Success {})
                    .map_err(|e| RpcError {
                        code: 0,
                        message: e.to_string(),
                    });
                self.buffers.insert(path, buffer);
                self.respond_rpc(id, result);
            }
            CreateFile { path } => {
                let result = path
                    .parent()
                    .map_or(Ok(()), std::fs::create_dir_all)
                    .and_then(|()| {
                        std::fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(path)
                    })
                    .map(|_| ProxyResponse::Success {})
                    .map_err(|e| RpcError {
                        code: 0,
                        message: e.to_string(),
                    });
                self.respond_rpc(id, result);
            }
            CreateDirectory { path } => {
                let result = std::fs::create_dir_all(path)
                    .map(|_| ProxyResponse::Success {})
                    .map_err(|e| RpcError {
                        code: 0,
                        message: e.to_string(),
                    });
                self.respond_rpc(id, result);
            }
            TrashPath { path } => {
                let result = trash::delete(path)
                    .map(|_| ProxyResponse::Success {})
                    .map_err(|e| RpcError {
                        code: 0,
                        message: e.to_string(),
                    });
                self.respond_rpc(id, result);
            }
            DuplicatePath {
                existing_path,
                new_path,
            } => {
                // We first check if the destination already exists, because copy can overwrite it
                // and that's not the default behavior we want for when a user duplicates a document.
                let result = if new_path.exists() {
                    Err(RpcError {
                        code: 0,
                        message: format!("{new_path:?} already exists"),
                    })
                } else {
                    if let Some(parent) = new_path.parent() {
                        if let Err(error) = std::fs::create_dir_all(parent) {
                            let result = Err(RpcError {
                                code: 0,
                                message: error.to_string(),
                            });
                            self.respond_rpc(id, result);
                            return;
                        }
                    }
                    std::fs::copy(existing_path, new_path)
                        .map(|_| ProxyResponse::Success {})
                        .map_err(|e| RpcError {
                            code: 0,
                            message: e.to_string(),
                        })
                };
                self.respond_rpc(id, result);
            }
            RenamePath { from, to } => {
                // We first check if the destination already exists, because rename can overwrite it
                // and that's not the default behavior we want for when a user renames a document.
                let result = if to.exists() {
                    Err(format!("{} already exists", to.display()))
                } else {
                    Ok(())
                };

                let result = result.and_then(|_| {
                    if let Some(parent) = to.parent() {
                        fs::create_dir_all(parent).map_err(|e| {
                            if let io::ErrorKind::AlreadyExists = e.kind() {
                                format!(
                                    "{} has a parent that is not a directory",
                                    to.display()
                                )
                            } else {
                                e.to_string()
                            }
                        })
                    } else {
                        Ok(())
                    }
                });

                let result = result
                    .and_then(|_| fs::rename(&from, &to).map_err(|e| e.to_string()));

                let result = result
                    .map(|_| {
                        let to = to.canonicalize().unwrap_or(to);

                        let (is_dir, is_file) = to
                            .metadata()
                            .map(|metadata| (metadata.is_dir(), metadata.is_file()))
                            .unwrap_or((false, false));

                        if is_dir {
                            // Update all buffers in which a file the renamed directory is an
                            // ancestor of is open to use the file's new path.
                            // This could be written more nicely if `HashMap::extract_if` were
                            // stable.
                            let child_buffers: Vec<_> = self
                                .buffers
                                .keys()
                                .filter_map(|path| {
                                    path.strip_prefix(&from).ok().map(|suffix| {
                                        (path.clone(), suffix.to_owned())
                                    })
                                })
                                .collect();

                            for (path, suffix) in child_buffers {
                                if let Some(mut buffer) = self.buffers.remove(&path)
                                {
                                    let new_path = to.join(suffix);
                                    buffer.path = new_path;

                                    self.buffers.insert(buffer.path.clone(), buffer);
                                }
                            }
                        } else if is_file {
                            // If the renamed file is open in a buffer, update it to use the new
                            // path.
                            let buffer = self.buffers.remove(&from);

                            if let Some(mut buffer) = buffer {
                                buffer.path.clone_from(&to);
                                self.buffers.insert(to.clone(), buffer);
                            }
                        }

                        ProxyResponse::CreatePathResponse { path: to }
                    })
                    .map_err(|message| RpcError { code: 0, message });

                self.respond_rpc(id, result);
            }
            TestCreateAtPath { path } => {
                // This performs a best effort test to see if an attempt to create an item at
                // `path` or rename an item to `path` will succeed.
                // Currently the only conditions that are tested are that `path` doesn't already
                // exist and that `path` doesn't have a parent that exists and is not a directory.
                let result = if path.exists() {
                    Err(format!("{} already exists", path.display()))
                } else {
                    Ok(path)
                };

                let result = result
                    .and_then(|path| {
                        let parent_is_dir = path
                            .ancestors()
                            .skip(1)
                            .find(|parent| parent.exists())
                            .is_none_or(|parent| parent.is_dir());

                        if parent_is_dir {
                            Ok(ProxyResponse::Success {})
                        } else {
                            Err(format!(
                                "{} has a parent that is not a directory",
                                path.display()
                            ))
                        }
                    })
                    .map_err(|message| RpcError { code: 0, message });

                self.respond_rpc(id, result);
            }
            ReferencesResolve { items } => {
                let items: Vec<FileLine> = items
                    .into_iter()
                    .filter_map(|location| {
                        let Ok(path) = location.uri.to_file_path() else {
                            tracing::error!(
                                "get file path fail: {:?}",
                                location.uri
                            );
                            return None;
                        };
                        let buffer = self.get_buffer_or_insert(path.clone());
                        let line_num = location.range.start.line as usize;
                        let content = buffer.line_to_cow(line_num).to_string();
                        Some(FileLine {
                            path,
                            position: location.range.start,
                            content,
                        })
                    })
                    .collect();
                let resp = ProxyResponse::ReferencesResolveResponse { items };
                self.proxy_rpc.handle_response(id, Ok(resp));
            }
            CompletionResolve {
                plugin_id,
                completion_item,
            } => {
                let proxy = self.proxy_rpc.clone();
                let catalog_rpc = self.catalog_rpc.clone();
                let completion_item = *completion_item;
                let item_value =
                    serde_json::to_value(&completion_item).unwrap_or_default();
                let fallback = move || {
                    catalog_rpc.completion_resolve(
                        plugin_id,
                        completion_item,
                        move |result| {
                            proxy.handle_response(
                                id,
                                result.map(|item| {
                                    ProxyResponse::CompletionResolveResponse {
                                        item: Box::new(item),
                                    }
                                }),
                            );
                        },
                    );
                };
                if let Some(lsp) = &self.lsp {
                    let proxy = self.proxy_rpc.clone();
                    let mut fallback = Some(fallback);
                    lsp.send(crate::lsp::Job::Resolve {
                        id: plugin_id,
                        method: "completionItem/resolve".to_string(),
                        item: item_value,
                        callback: Box::new(move |result| {
                            match result {
                                Ok(value) => {
                                    proxy.handle_response(
                                        id,
                                        serde_json::from_value(value)
                                            .map(|item| {
                                                ProxyResponse::CompletionResolveResponse {
                                                    item: Box::new(item),
                                                }
                                            })
                                            .map_err(|error| RpcError {
                                                code: -32603,
                                                message: error.to_string(),
                                            }),
                                    );
                                }
                                Err(_) => {
                                    if let Some(fallback) = fallback.take() {
                                        fallback();
                                    }
                                }
                            }
                        }),
                    });
                } else {
                    fallback();
                }
            }
            GetHover {
                request_id,
                path,
                position,
            } => {
                let merge = FirstOk::new(
                    self.proxy_rpc.clone(),
                    id,
                    move |hover| ProxyResponse::HoverResponse { request_id, hover },
                );
                let merged = merge.clone();
                self.catalog_rpc.hover(&path, position, move |_, result| {
                    merged.deliver(result)
                });
                self.lsp_request(
                    path,
                    "textDocument/hover",
                    serde_json::json!({"position":position}),
                    move |result| {
                        merge.deliver(result.and_then(|(_, value)| {
                            let hover = serde_json::from_value::<
                                Option<lsp_types::Hover>,
                            >(value)
                            .map_err(|error| RpcError {
                                code: -32603,
                                message: error.to_string(),
                            })?;
                            Ok(hover.unwrap_or(lsp_types::Hover {
                                contents: lsp_types::HoverContents::Array(
                                    Vec::new(),
                                ),
                                range: None,
                            }))
                        }));
                    },
                );
            }
            GetDefinition {
                request_id,
                path,
                position,
            } => {
                let merge = FirstOk::new(
                    self.proxy_rpc.clone(),
                    id,
                    move |definition| ProxyResponse::GetDefinitionResponse {
                        request_id,
                        definition,
                    },
                );
                let merged = merge.clone();
                self.catalog_rpc.get_definition(&path, position, move |_, result| {
                    merged.deliver(result)
                });
                self.lsp_request(
                    path,
                    "textDocument/definition",
                    serde_json::json!({"position":position}),
                    move |result| {
                        merge.deliver(result.and_then(|(_, value)| {
                            let definition = serde_json::from_value::<
                                Option<lsp_types::GotoDefinitionResponse>,
                            >(value)
                            .map_err(|error| RpcError {
                                code: -32603,
                                message: error.to_string(),
                            })?;
                            Ok(definition.unwrap_or(
                                lsp_types::GotoDefinitionResponse::Array(
                                    Vec::new(),
                                ),
                            ))
                        }));
                    },
                );
            }
            GetInlineCompletions {
                position,
                trigger_kind,
                ai_request,
                ..
            } => {
                if !matches!(
                    trigger_kind,
                    lsp_types::InlineCompletionTriggerKind::Invoked
                        | lsp_types::InlineCompletionTriggerKind::Automatic
                ) {
                    self.respond_rpc(
                        id,
                        Ok(ProxyResponse::GetInlineCompletions {
                            completions: lsp_types::InlineCompletionResponse::Array(
                                vec![],
                            ),
                        }),
                    );
                    return;
                }
                let request =
                    match serde_json::from_str::<lapce_ai::Request>(&ai_request) {
                        Ok(request) => request,
                        Err(e) => {
                            self.respond_rpc(
                                id,
                                Err(RpcError {
                                    code: -32602,
                                    message: e.to_string(),
                                }),
                            );
                            return;
                        }
                    };
                let proxy = self.proxy_rpc.clone();
                self.ai.complete(request, move |result| {
                    proxy.handle_response(
                        id,
                        result
                            .map(|completion| {
                                let items = if completion.text.is_empty() {
                                    vec![]
                                } else {
                                    vec![lsp_types::InlineCompletionItem {
                                        insert_text: completion.text,
                                        filter_text: None,
                                        range: Some(lsp_types::Range::new(
                                            position, position,
                                        )),
                                        command: None,
                                        insert_text_format: Some(
                                            lsp_types::InsertTextFormat::PLAIN_TEXT,
                                        ),
                                    }]
                                };
                                ProxyResponse::GetInlineCompletions {
                                    completions:
                                        lsp_types::InlineCompletionResponse::Array(
                                            items,
                                        ),
                                }
                            })
                            .map_err(|message| RpcError {
                                code: -32603,
                                message,
                            }),
                    );
                });
            }
            GetReferences { path, position } => {
                let merge = FirstOk::new(self.proxy_rpc.clone(), id, move |references| {
                    ProxyResponse::GetReferencesResponse { references }
                });
                let merged = merge.clone();
                self.catalog_rpc.get_references(&path, position, move |_, result| {
                    merged.deliver(result)
                });
                self.lsp_request(
                    path,
                    "textDocument/references",
                    serde_json::json!({"position": position, "context": {"includeDeclaration": true}}),
                    move |result| {
                        merge.deliver(result.and_then(|(_, value)| {
                            serde_json::from_value::<Option<Vec<lsp_types::Location>>>(value)
                                .map(|r| r.unwrap_or_default())
                                .map_err(|error| RpcError { code: -32603, message: error.to_string() })
                        }));
                    },
                );
            }
            GetTypeDefinition {
                request_id,
                path,
                position,
            } => {
                let merge = FirstOk::new(self.proxy_rpc.clone(), id, move |definition| {
                    ProxyResponse::GetTypeDefinition { request_id, definition }
                });
                let merged = merge.clone();
                self.catalog_rpc.get_type_definition(&path, position, move |_, result| {
                    merged.deliver(result)
                });
                self.lsp_request(
                    path,
                    "textDocument/typeDefinition",
                    serde_json::json!({"position": position}),
                    move |result| {
                        merge.deliver(result.and_then(|(_, value)| {
                            serde_json::from_value::<Option<lsp_types::request::GotoTypeDefinitionResponse>>(value)
                                .map_err(|error| RpcError { code: -32603, message: error.to_string() })?
                                .ok_or_else(|| RpcError { code: -32603, message: "no type definition".into() })
                        }));
                    },
                );
            }
            ShowCallHierarchy { path, position } => {
                let merge = FirstOk::new(self.proxy_rpc.clone(), id, move |items| {
                    ProxyResponse::ShowCallHierarchyResponse { items }
                });
                let merged = merge.clone();
                self.catalog_rpc.show_call_hierarchy(&path, position, move |_, result| {
                    merged.deliver(result)
                });
                self.lsp_request(
                    path,
                    "callHierarchy/prepare",
                    serde_json::json!({"position": position}),
                    move |result| {
                        merge.deliver(result.and_then(|(_, value)| {
                            serde_json::from_value::<Option<Vec<lsp_types::CallHierarchyItem>>>(value)
                                .map_err(|error| RpcError { code: -32603, message: error.to_string() })
                        }));
                    },
                );
            }
            CallHierarchyIncoming {
                path,
                call_hierarchy_item,
            } => {
                let merge = FirstOk::new(self.proxy_rpc.clone(), id, move |items| {
                    ProxyResponse::CallHierarchyIncomingResponse { items }
                });
                let merged = merge.clone();
                self.catalog_rpc.call_hierarchy_incoming(
                    &path,
                    call_hierarchy_item.clone(),
                    move |_, result| merged.deliver(result),
                );
                self.lsp_request(
                    path,
                    "callHierarchy/incomingCalls",
                    serde_json::json!({"item": call_hierarchy_item}),
                    move |result| {
                        merge.deliver(result.and_then(|(_, value)| {
                            serde_json::from_value::<Option<Vec<lsp_types::CallHierarchyIncomingCall>>>(value)
                                .map_err(|error| RpcError { code: -32603, message: error.to_string() })
                        }));
                    },
                );
            }
            GetInlayHints { path } => {
                let range = {
                    let buffer = self.buffers.get(&path).unwrap();
                    lsp_types::Range {
                        start: lsp_types::Position::new(0, 0),
                        end: buffer.offset_to_position(buffer.len()),
                    }
                };
                let merge = FirstOk::new(self.proxy_rpc.clone(), id, move |hints| {
                    ProxyResponse::GetInlayHints { hints }
                });
                let merged = merge.clone();
                self.catalog_rpc.get_inlay_hints(&path, range, move |_, result| {
                    merged.deliver(result)
                });
                self.lsp_request(
                    path,
                    "textDocument/inlayHint",
                    serde_json::json!({"range": range}),
                    move |result| {
                        merge.deliver(result.and_then(|(_, value)| {
                            serde_json::from_value::<Option<Vec<lsp_types::InlayHint>>>(value)
                                .map(|r| r.unwrap_or_default())
                                .map_err(|error| RpcError { code: -32603, message: error.to_string() })
                        }));
                    },
                );
            }
            GetSemanticTokens { path } => {
                let Some(buffer) = self.buffers.get(&path) else {
                    self.respond_rpc(id, Err(language_services_disabled()));
                    return;
                };
                let text = buffer.rope.clone();
                let rev = buffer.rev;
                let len = buffer.len();
                let local_path = path.clone();
                let proxy_rpc = self.proxy_rpc.clone();
                let catalog_rpc = self.catalog_rpc.clone();

                let handle_tokens =
                    move |result: Result<Vec<lapce_rpc::style::LineStyle>, RpcError>| match result {
                        Ok(styles) => {
                            proxy_rpc.handle_response(
                                id,
                                Ok(ProxyResponse::GetSemanticTokens {
                                    styles: lapce_rpc::style::SemanticStyles {
                                        rev,
                                        path: local_path,
                                        styles,
                                        len,
                                    },
                                }),
                            );
                        }
                        Err(e) => {
                            proxy_rpc.handle_response(id, Err(e));
                        }
                    };

                let proxy_rpc = self.proxy_rpc.clone();
                self.catalog_rpc.get_semantic_tokens(
                    &path,
                    move |plugin_id, result| match result {
                        Ok(result) => {
                            catalog_rpc.format_semantic_tokens(
                                plugin_id,
                                result,
                                text,
                                Box::new(handle_tokens),
                            );
                        }
                        Err(e) => {
                            proxy_rpc.handle_response(id, Err(e));
                        }
                    },
                );
            }
            GetCodeActions {
                path,
                position,
                diagnostics,
            } => {
                let merge = FirstOk::new(self.proxy_rpc.clone(), id, move |(plugin_id, resp)| {
                    ProxyResponse::GetCodeActionsResponse { plugin_id, resp }
                });
                let merged = merge.clone();
                self.catalog_rpc.get_code_actions(
                    &path,
                    position,
                    diagnostics.clone(),
                    move |plugin_id, result| {
                        merged.deliver(result.map(|resp| (plugin_id, resp)))
                    },
                );
                self.lsp_request(
                    path,
                    "textDocument/codeAction",
                    serde_json::json!({
                        "range": {"start": position, "end": position},
                        "context": {"diagnostics": diagnostics},
                    }),
                    move |result| {
                        merge.deliver(result.and_then(|(plugin_id, value)| {
                            serde_json::from_value::<Option<lsp_types::CodeActionResponse>>(value)
                                .map(|resp| (plugin_id, resp.unwrap_or_default()))
                                .map_err(|error| RpcError { code: -32603, message: error.to_string() })
                        }));
                    },
                );
            }
            GetDocumentSymbols { path } => {
                let merge = FirstOk::new(self.proxy_rpc.clone(), id, move |resp| {
                    ProxyResponse::GetDocumentSymbols { resp }
                });
                let merged = merge.clone();
                self.catalog_rpc.get_document_symbols(&path, move |_, result| {
                    merged.deliver(result)
                });
                self.lsp_request(
                    path,
                    "textDocument/documentSymbol",
                    serde_json::json!({}),
                    move |result| {
                        merge.deliver(result.and_then(|(_, value)| {
                            serde_json::from_value::<Option<lsp_types::DocumentSymbolResponse>>(value)
                                .map(|resp| resp.unwrap_or(lsp_types::DocumentSymbolResponse::Nested(Vec::new())))
                                .map_err(|error| RpcError { code: -32603, message: error.to_string() })
                        }));
                    },
                );
            }
            GetWorkspaceSymbols { query } => {
                let merge = FirstOk::new(self.proxy_rpc.clone(), id, move |symbols| {
                    ProxyResponse::GetWorkspaceSymbols { symbols }
                });
                let merged = merge.clone();
                self.catalog_rpc.get_workspace_symbols(query.clone(), move |_, result| {
                    merged.deliver(result)
                });
                let path = match self.workspace.clone() {
                    Some(workspace) => workspace,
                    None => {
                        merge.deliver(Err(RpcError {
                            code: -32602,
                            message: "no workspace open".into(),
                        }));
                        return;
                    }
                };
                self.lsp_request(
                    path,
                    "workspace/symbol",
                    serde_json::json!({"query": query}),
                    move |result| {
                        merge.deliver(result.and_then(|(_, value)| {
                            serde_json::from_value::<Option<Vec<lsp_types::SymbolInformation>>>(value)
                                .map(|r| r.unwrap_or_default())
                                .map_err(|error| RpcError { code: -32603, message: error.to_string() })
                        }));
                    },
                );
            }
            GetDocumentFormatting { path } => {
                // A project that configures Prettier (config file plus a binary)
                // formats through it; language servers and plugins are skipped.
                // Failure means no edits, never a fallback to a second formatter.
                if let Some(formatter) = crate::external_format::resolve(&path) {
                    let snapshot = self.buffers.get(&path).map(|b| {
                        (b.rope.to_string(), b.offset_to_position(b.len()))
                    });
                    let proxy_rpc = self.proxy_rpc.clone();
                    thread::spawn(move || {
                        let result = snapshot
                            .and_then(|(old, end)| {
                                crate::external_format::format(&formatter, &path, &old)
                                    .map(|new| {
                                        crate::external_format::whole_document_edit(&old, new, end)
                                    })
                            })
                            .ok_or_else(|| RpcError {
                                code: -32603,
                                message: "external formatter unavailable or failed".into(),
                            });
                        proxy_rpc.handle_response(
                            id,
                            result.map(|edits| ProxyResponse::GetDocumentFormatting { edits }),
                        );
                    });
                    return;
                }
                let merge = FirstOk::new(self.proxy_rpc.clone(), id, move |edits| {
                    ProxyResponse::GetDocumentFormatting { edits }
                });
                let merged = merge.clone();
                self.catalog_rpc.get_document_formatting(&path, move |_, result| {
                    merged.deliver(result)
                });
                self.lsp_request(
                    path,
                    "textDocument/formatting",
                    serde_json::json!({"options": {"tabSize": 4, "insertSpaces": true}}),
                    move |result| {
                        merge.deliver(result.and_then(|(_, value)| {
                            serde_json::from_value::<Option<Vec<lsp_types::TextEdit>>>(value)
                                .map(|r| r.unwrap_or_default())
                                .map_err(|error| RpcError { code: -32603, message: error.to_string() })
                        }));
                    },
                );
            }
            PrepareRename { path, position } => {
                let merge = FirstOk::new(self.proxy_rpc.clone(), id, move |resp| {
                    ProxyResponse::PrepareRename { resp }
                });
                let merged = merge.clone();
                self.catalog_rpc.prepare_rename(&path, position, move |_, result| {
                    merged.deliver(result)
                });
                self.lsp_request(
                    path,
                    "textDocument/prepareRename",
                    serde_json::json!({"position": position}),
                    move |result| {
                        merge.deliver(result.and_then(|(_, value)| {
                            serde_json::from_value::<Option<lsp_types::PrepareRenameResponse>>(value)
                                .map_err(|error| RpcError { code: -32603, message: error.to_string() })?
                                .ok_or_else(|| RpcError { code: -32603, message: "nothing to rename".into() })
                        }));
                    },
                );
            }
            Rename {
                path,
                position,
                new_name,
            } => {
                let merge = FirstOk::new(self.proxy_rpc.clone(), id, move |edit| {
                    ProxyResponse::Rename { edit }
                });
                let merged = merge.clone();
                self.catalog_rpc.rename(&path, position, new_name.clone(), move |_, result| {
                    merged.deliver(result)
                });
                self.lsp_request(
                    path,
                    "textDocument/rename",
                    serde_json::json!({"position": position, "newName": new_name}),
                    move |result| {
                        merge.deliver(result.and_then(|(_, value)| {
                            serde_json::from_value::<Option<lsp_types::WorkspaceEdit>>(value)
                                .map_err(|error| RpcError { code: -32603, message: error.to_string() })?
                                .ok_or_else(|| RpcError { code: -32603, message: "rename not possible".into() })
                        }));
                    },
                );
            }
            GetSelectionRange { positions, path } => {
                let merge = FirstOk::new(self.proxy_rpc.clone(), id, move |ranges| {
                    ProxyResponse::GetSelectionRange { ranges }
                });
                let merged = merge.clone();
                self.catalog_rpc.get_selection_range(
                    path.as_path(),
                    positions.clone(),
                    move |_, result| merged.deliver(result),
                );
                self.lsp_request(
                    path,
                    "textDocument/selectionRange",
                    serde_json::json!({"positions": positions}),
                    move |result| {
                        merge.deliver(result.and_then(|(_, value)| {
                            serde_json::from_value::<Option<Vec<lsp_types::SelectionRange>>>(value)
                                .map(|r| r.unwrap_or_default())
                                .map_err(|error| RpcError { code: -32603, message: error.to_string() })
                        }));
                    },
                );
            }
            CodeActionResolve {
                action_item,
                plugin_id,
            } => {
                let merge = FirstOk::new(self.proxy_rpc.clone(), id, move |item| {
                    ProxyResponse::CodeActionResolveResponse {
                        item: Box::new(item),
                    }
                });
                let merged = merge.clone();
                self.catalog_rpc.action_resolve(
                    *action_item.clone(),
                    plugin_id,
                    move |result| merged.deliver(result),
                );
                if let Some(lsp) = &self.lsp {
                    lsp.send(crate::lsp::Job::Resolve {
                        id: plugin_id,
                        method: "codeAction/resolve".to_string(),
                        item: serde_json::to_value(&action_item).unwrap_or_default(),
                        callback: Box::new(move |result| {
                            merge.deliver(result.and_then(|value| {
                                serde_json::from_value::<lsp_types::CodeAction>(value)
                                    .map_err(|error| RpcError { code: -32603, message: error.to_string() })
                            }));
                        }),
                    });
                } else {
                    merge.deliver(Err(language_services_disabled()));
                }
            }
            GetCodeLens { path } => {
                let merge = FirstOk::new(self.proxy_rpc.clone(), id, move |(plugin_id, resp)| {
                    ProxyResponse::GetCodeLensResponse { plugin_id, resp }
                });
                let merged = merge.clone();
                self.catalog_rpc.get_code_lens(&path, move |plugin_id, result| {
                    merged.deliver(result.map(|resp| (plugin_id, resp)))
                });
                self.lsp_request(
                    path,
                    "textDocument/codeLens",
                    serde_json::json!({}),
                    move |result| {
                        merge.deliver(result.and_then(|(plugin_id, value)| {
                            serde_json::from_value::<Option<Vec<lsp_types::CodeLens>>>(value)
                                .map(|resp| (plugin_id, resp))
                                .map_err(|error| RpcError { code: -32603, message: error.to_string() })
                        }));
                    },
                );
            }
            LspFoldingRange { path } => {
                let merge = FirstOk::new(self.proxy_rpc.clone(), id, move |(plugin_id, resp)| {
                    ProxyResponse::LspFoldingRangeResponse { plugin_id, resp }
                });
                let merged = merge.clone();
                self.catalog_rpc.get_lsp_folding_range(&path, move |plugin_id, result| {
                    merged.deliver(result.map(|resp| (plugin_id, resp)))
                });
                self.lsp_request(
                    path,
                    "textDocument/foldingRange",
                    serde_json::json!({}),
                    move |result| {
                        merge.deliver(result.and_then(|(plugin_id, value)| {
                            serde_json::from_value::<Option<Vec<lsp_types::FoldingRange>>>(value)
                                .map(|resp| (plugin_id, resp))
                                .map_err(|error| RpcError { code: -32603, message: error.to_string() })
                        }));
                    },
                );
            }
            GetCodeLensResolve { code_lens, path } => {
                let merge = FirstOk::new(self.proxy_rpc.clone(), id, move |(plugin_id, resp)| {
                    ProxyResponse::GetCodeLensResolveResponse { plugin_id, resp }
                });
                let merged = merge.clone();
                self.catalog_rpc.get_code_lens_resolve(
                    &path,
                    &code_lens,
                    move |plugin_id, result| {
                        merged.deliver(result.map(|resp| (plugin_id, resp)))
                    },
                );
                self.lsp_request(
                    path,
                    "codeLens/resolve",
                    serde_json::to_value(&code_lens).unwrap_or_default(),
                    move |result| {
                        merge.deliver(result.and_then(|(plugin_id, value)| {
                            serde_json::from_value::<lsp_types::CodeLens>(value)
                                .map(|resp| (plugin_id, resp))
                                .map_err(|error| RpcError { code: -32603, message: error.to_string() })
                        }));
                    },
                );
            }
            GotoImplementation { path, position } => {
                let merge = FirstOk::new(self.proxy_rpc.clone(), id, move |(plugin_id, resp)| {
                    ProxyResponse::GotoImplementationResponse { plugin_id, resp }
                });
                let merged = merge.clone();
                self.catalog_rpc.go_to_implementation(&path, position, move |plugin_id, result| {
                    merged.deliver(result.map(|resp| (plugin_id, resp)))
                });
                self.lsp_request(
                    path,
                    "textDocument/implementation",
                    serde_json::json!({"position": position}),
                    move |result| {
                        merge.deliver(result.and_then(|(plugin_id, value)| {
                            serde_json::from_value::<Option<lsp_types::request::GotoImplementationResponse>>(value)
                                .map(|resp| (plugin_id, resp))
                                .map_err(|error| RpcError { code: -32603, message: error.to_string() })
                        }));
                    },
                );
            }
            DapVariable { dap_id, reference } => {
                let session = self.dap.session(dap_id);
                let rpc = self.proxy_rpc.clone();
                thread::spawn(move || {
                    let result = session.context("no such debug session")
                        .and_then(|session| session.variables(reference))
                        .map(|variables| ProxyResponse::DapVariableResponse { variables })
                        .map_err(dap_error);
                    rpc.handle_response(id, result);
                });
            }
            DapGetScopes { dap_id, frame_id } => {
                let session = self.dap.session(dap_id);
                let rpc = self.proxy_rpc.clone();
                thread::spawn(move || {
                    let result = session.context("no such debug session")
                        .and_then(|session| session.scopes_with_variables(frame_id))
                        .map(|scopes| ProxyResponse::DapGetScopesResponse { scopes })
                        .map_err(dap_error);
                    rpc.handle_response(id, result);
                });
            }
            GetSignature { .. } | GetDocumentHighlights { .. } => {
                self.respond_rpc(id, Err(language_services_disabled()));
            }
        }
    }
}

/// Stable, immediate rejection for the services this editor does not have.
fn language_services_disabled() -> RpcError {
    RpcError {
        code: -32601,
        message: "Debugging and signature help are not available in this editor"
            .to_owned(),
    }
}

/// Responds to a proxy request with the first successful result from two
/// async sources - the built-in language server manager and the plugin
/// catalog. If both fail, the first error is returned.
struct FirstOk<T> {
    proxy_rpc: ProxyRpcHandler,
    id: RequestId,
    wrap: Box<dyn Fn(T) -> ProxyResponse + Send + Sync>,
    state: Mutex<(bool, Option<RpcError>)>,
}

impl<T: Send + 'static> FirstOk<T> {
    fn new(
        proxy_rpc: ProxyRpcHandler,
        id: RequestId,
        wrap: impl Fn(T) -> ProxyResponse + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            proxy_rpc,
            id,
            wrap: Box::new(wrap),
            state: Mutex::new((false, None)),
        })
    }

    fn deliver(&self, result: Result<T, RpcError>) {
        let mut state = self.state.lock();
        let (responded, first_err) = &mut *state;
        if *responded {
            return;
        }
        match result {
            Ok(value) => {
                *responded = true;
                drop(state);
                self.proxy_rpc.handle_response(self.id, Ok((self.wrap)(value)));
            }
            Err(err) => match first_err.take() {
                Some(first) => {
                    *responded = true;
                    drop(state);
                    self.proxy_rpc.handle_response(self.id, Err(first));
                }
                None => {
                    *first_err = Some(err);
                }
            },
        }
    }
}

fn dap_error(err: anyhow::Error) -> RpcError {
    RpcError {
        code: -32603,
        message: format!("{err:#}"),
    }
}

impl Dispatcher {
    pub fn new(core_rpc: CoreRpcHandler, proxy_rpc: ProxyRpcHandler) -> Self {
        let file_watcher = FileWatcher::new();
        let dap = crate::dap::Manager::new(core_rpc.clone());
        let catalog_rpc =
            PluginCatalogRpcHandler::new(core_rpc.clone(), proxy_rpc.clone());

        Self {
            workspace: None,
            catalog_rpc,
            proxy_rpc,
            core_rpc,
            buffers: HashMap::new(),
            terminals: HashMap::new(),
            file_watcher,
            window_id: 1,
            tab_id: 1,
            lsp: None,
            dap,
            ai: lapce_ai::process::Worker::new(
                std::env::current_exe().expect("proxy executable"),
            ),
        }
    }

    #[cfg(test)]
    pub(crate) fn test_lsp(
        &mut self,
        root: PathBuf,
        registry: crate::lsp::registry::Registry,
    ) {
        self.lsp = Some(crate::lsp::Manager::with_registry(
            Some(root),
            self.core_rpc.clone(),
            Ok(registry),
        ));
    }
    fn lsp_request(
        &mut self,
        path: PathBuf,
        method: &str,
        mut params: serde_json::Value,
        callback: impl FnOnce(
            Result<(lapce_rpc::plugin::PluginId, serde_json::Value), RpcError>,
        ) + Send
        + 'static,
    ) {
        let buffer = self.get_buffer_or_insert(path.clone()).clone();
        let Some(lsp) = &self.lsp else {
            callback(Err(RpcError {
                code: -32601,
                message: "language server manager not running".into(),
            }));
            return;
        };
        let Ok(uri) = Url::from_file_path(&path) else {
            callback(Err(RpcError {
                code: -32602,
                message: "Invalid file URI".into(),
            }));
            return;
        };
        params["textDocument"] = serde_json::json!({"uri":uri});
        lsp.send(crate::lsp::Job::Request {
            buffer,
            method: method.to_string(),
            params,
            callback: Box::new(callback),
        });
    }

    fn respond_rpc(&self, id: RequestId, result: Result<ProxyResponse, RpcError>) {
        self.proxy_rpc.handle_response(id, result);
    }

    fn get_buffer_or_insert(&mut self, path: PathBuf) -> &mut Buffer {
        self.buffers
            .entry(path.clone())
            .or_insert(Buffer::new(BufferId::next(), path))
    }
}

struct FileWatchNotifier {
    core_rpc: CoreRpcHandler,
    proxy_rpc: ProxyRpcHandler,
    workspace: Option<PathBuf>,
    workspace_fs_change_handler: Arc<Mutex<Option<Sender<bool>>>>,
    last_diff: Arc<Mutex<DiffInfo>>,
}

impl Notify for FileWatchNotifier {
    fn notify(&self, events: Vec<(WatchToken, notify::Event)>) {
        self.handle_fs_events(events);
    }
}

impl FileWatchNotifier {
    fn new(
        workspace: Option<PathBuf>,
        core_rpc: CoreRpcHandler,
        proxy_rpc: ProxyRpcHandler,
    ) -> Self {
        let notifier = Self {
            workspace,
            core_rpc,
            proxy_rpc,
            workspace_fs_change_handler: Arc::new(Mutex::new(None)),
            last_diff: Arc::new(Mutex::new(DiffInfo::default())),
        };

        if let Some(workspace) = notifier.workspace.clone() {
            let core_rpc = notifier.core_rpc.clone();
            let last_diff = notifier.last_diff.clone();
            thread::spawn(move || {
                if let Some(diff) = git_diff_new(&workspace) {
                    core_rpc.diff_info(diff.clone());
                    *last_diff.lock() = diff;
                }
            });
        }

        notifier
    }

    fn handle_fs_events(&self, events: Vec<(WatchToken, notify::Event)>) {
        for (token, event) in events {
            match token {
                OPEN_FILE_EVENT_TOKEN => self.handle_open_file_fs_event(event),
                WORKSPACE_EVENT_TOKEN => self.handle_workspace_fs_event(event),
                _ => {}
            }
        }
    }

    fn handle_open_file_fs_event(&self, event: notify::Event) {
        if event.kind.is_modify() || event.kind.is_remove() {
            for path in event.paths {
                #[cfg(windows)]
                if let Some(path_str) = path.to_str() {
                    const PREFIX: &str = r"\\?\";
                    if let Some(path_str) = path_str.strip_prefix(PREFIX) {
                        let path = PathBuf::from(&path_str);
                        self.proxy_rpc.notification(
                            ProxyNotification::OpenFileChanged { path },
                        );
                        continue;
                    }
                }
                self.proxy_rpc
                    .notification(ProxyNotification::OpenFileChanged { path });
            }
        }
    }

    fn handle_workspace_fs_event(&self, event: notify::Event) {
        let explorer_change = match &event.kind {
            notify::EventKind::Create(_)
            | notify::EventKind::Remove(_)
            | notify::EventKind::Modify(notify::event::ModifyKind::Name(_)) => true,
            notify::EventKind::Modify(_) => false,
            _ => return,
        };

        let mut handler = self.workspace_fs_change_handler.lock();
        if let Some(sender) = handler.as_mut() {
            if explorer_change {
                // only send the value if we need to update file explorer as well
                if let Err(err) = sender.send(explorer_change) {
                    tracing::error!("{:?}", err);
                }
            }
            return;
        }
        let (sender, receiver) = crossbeam_channel::unbounded();
        if explorer_change {
            // only send the value if we need to update file explorer as well
            if let Err(err) = sender.send(explorer_change) {
                tracing::error!("{:?}", err);
            }
        }

        let local_handler = self.workspace_fs_change_handler.clone();
        let core_rpc = self.core_rpc.clone();
        let workspace = self.workspace.clone().unwrap();
        let last_diff = self.last_diff.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(500));

            {
                local_handler.lock().take();
            }

            let mut explorer_change = false;
            for e in receiver {
                if e {
                    explorer_change = true;
                    break;
                }
            }
            if explorer_change {
                core_rpc.workspace_file_change();
            }
            if let Some(diff) = git_diff_new(&workspace) {
                let mut last_diff = last_diff.lock();
                if diff != *last_diff {
                    core_rpc.diff_info(diff.clone());
                    *last_diff = diff;
                }
            }
        });
        *handler = Some(sender);
    }
}

#[derive(Clone, Debug)]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub header: String,
}

fn git_init(workspace_path: &Path) -> Result<()> {
    if Repository::discover(workspace_path).is_err() {
        Repository::init(workspace_path)?;
    };
    Ok(())
}

fn git_commit(
    workspace_path: &Path,
    message: &str,
    diffs: Vec<FileDiff>,
) -> Result<()> {
    let repo = Repository::discover(workspace_path)?;
    let mut index = repo.index()?;
    for diff in diffs {
        match diff {
            FileDiff::Modified(p) | FileDiff::Added(p) => {
                index.add_path(p.strip_prefix(workspace_path)?)?;
            }
            FileDiff::Renamed(a, d) => {
                index.add_path(a.strip_prefix(workspace_path)?)?;
                index.remove_path(d.strip_prefix(workspace_path)?)?;
            }
            FileDiff::Deleted(p) => {
                index.remove_path(p.strip_prefix(workspace_path)?)?;
            }
            FileDiff::Conflicted(p) => {
                // Never `git add` a conflicted path: that would mark
                // it resolved with the conflict markers included.
                return Err(anyhow!(
                    "{} has unresolved merge conflicts - resolve them in the editor first.",
                    p.display()
                ));
            }
        }
    }
    index.write()?;
    let tree = index.write_tree()?;
    let tree = repo.find_tree(tree)?;

    match repo.signature() {
        Ok(signature) => {
            let parents = repo
                .head()
                .and_then(|head| Ok(vec![head.peel_to_commit()?]))
                .unwrap_or(vec![]);
            let parents_refs = parents.iter().collect::<Vec<_>>();

            repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parents_refs,
            )?;
            Ok(())
        }
        Err(e) => match e.code() {
            NotFound => Err(anyhow!(
                "No user.name and/or user.email configured for this git repository."
            )),
            _ => Err(anyhow!(
                "Error while creating commit's signature: {}",
                e.message()
            )),
        },
    }
}

/// Unified HEAD-to-worktree+index patch limited to the given paths, for
/// the AI commit-message suggestion. Hard-capped so a huge changeset can
/// never flood the wire or the model prompt; the cap marker is explicit.
fn git_diff_for_commit(
    workspace_path: &Path,
    diffs: &[FileDiff],
) -> Result<String> {
    const MAX_DIFF_BYTES: usize = 256 * 1024;
    let repo = Repository::discover(workspace_path)?;
    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let mut opts = git2::DiffOptions::new();
    for diff in diffs {
        match diff {
            FileDiff::Renamed(old, new) => {
                if let Ok(path) = old.strip_prefix(workspace_path) {
                    opts.pathspec(path);
                }
                if let Ok(path) = new.strip_prefix(workspace_path) {
                    opts.pathspec(path);
                }
            }
            other => {
                if let Ok(path) = other.path().strip_prefix(workspace_path) {
                    opts.pathspec(path);
                }
            }
        }
    }
    // Untracked files are commit candidates too; include their content.
    opts.include_untracked(true).recurse_untracked_dirs(true);
    let mut diff =
        repo.diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut opts))?;
    let mut text = String::new();
    let mut truncated = false;
    diff.print(git2::DiffFormat::Patch, |_, _, line| {
        if text.len() >= MAX_DIFF_BYTES {
            truncated = true;
            return false;
        }
        if matches!(line.origin(), '+' | '-' | ' ') {
            text.push(line.origin());
        }
        text.push_str(&String::from_utf8_lossy(line.content()));
        true
    })?;
    if truncated {
        text.push_str("\n[diff truncated at 256 KB by the editor]\n");
    }
    Ok(text)
}

fn git_checkout(workspace_path: &Path, reference: &str) -> Result<()> {
    let repo = Repository::discover(workspace_path)?;
    let (object, reference) = repo.revparse_ext(reference)?;
    repo.checkout_tree(&object, None)?;
    repo.set_head(reference.unwrap().name().unwrap())?;
    Ok(())
}

/// Create a new branch at HEAD and switch to it. The working tree is
/// untouched (the new branch points at the same commit), so unlike
/// checkout this can never refuse over uncommitted changes. Existing
/// names and invalid refnames error instead of overwriting.
fn git_create_branch(workspace_path: &Path, name: &str) -> Result<()> {
    let repo = Repository::discover(workspace_path)?;
    let commit = repo.head()?.peel_to_commit()?;
    repo.branch(name, &commit, false)?;
    repo.set_head(&format!("refs/heads/{name}"))?;
    Ok(())
}

fn git_discard_files_changes<'a>(
    workspace_path: &Path,
    files: impl Iterator<Item = &'a Path>,
) -> Result<()> {
    let repo = Repository::discover(workspace_path)?;

    let mut checkout_b = CheckoutBuilder::new();
    checkout_b.update_only(false).force();

    let mut had_path = false;
    for path in files {
        // Remove the workspace path so it is relative to the folder
        if let Ok(path) = path.strip_prefix(workspace_path) {
            had_path = true;
            checkout_b.path(path);
        }
    }

    if !had_path {
        // If there we no paths then we do nothing
        // because the default behavior of checkout builder is to select all files
        // if it is not given a path
        return Ok(());
    }

    repo.checkout_index(None, Some(&mut checkout_b))?;

    Ok(())
}

fn git_discard_workspace_changes(workspace_path: &Path) -> Result<()> {
    let repo = Repository::discover(workspace_path)?;
    let mut checkout_b = CheckoutBuilder::new();
    checkout_b.force();

    repo.checkout_index(None, Some(&mut checkout_b))?;

    Ok(())
}

fn git_delta_format(
    workspace_path: &Path,
    delta: &git2::DiffDelta,
) -> Option<(git2::Delta, git2::Oid, PathBuf)> {
    match delta.status() {
        git2::Delta::Added | git2::Delta::Untracked => Some((
            git2::Delta::Added,
            delta.new_file().id(),
            delta.new_file().path().map(|p| workspace_path.join(p))?,
        )),
        git2::Delta::Deleted => Some((
            git2::Delta::Deleted,
            delta.old_file().id(),
            delta.old_file().path().map(|p| workspace_path.join(p))?,
        )),
        git2::Delta::Modified => Some((
            git2::Delta::Modified,
            delta.new_file().id(),
            delta.new_file().path().map(|p| workspace_path.join(p))?,
        )),
        git2::Delta::Conflicted => Some((
            git2::Delta::Conflicted,
            delta.new_file().id(),
            delta.new_file().path().map(|p| workspace_path.join(p))?,
        )),
        _ => None,
    }
}

/// Commits the checked-out branch is ahead of / behind its configured
/// upstream, as `(ahead, behind)`. `(0, 0)` for detached HEADs, unborn
/// branches, and branches without an upstream - the status bar hides
/// zero counts, so every failure mode collapses to "show nothing".
fn git_ahead_behind(repo: &Repository, head_name: &str) -> (usize, usize) {
    (|| {
        let upstream_ref = repo
            .branch_upstream_name(&format!("refs/heads/{head_name}"))
            .ok()?;
        let upstream_ref = upstream_ref.as_str().ok()?;
        let upstream_oid = repo
            .revparse_single(upstream_ref)
            .and_then(|obj| obj.peel_to_commit())
            .ok()?
            .id();
        let local_oid = repo
            .revparse_single("HEAD")
            .and_then(|obj| obj.peel_to_commit())
            .ok()?
            .id();
        repo.graph_ahead_behind(local_oid, upstream_oid).ok()
    })()
    .unwrap_or((0, 0))
}

/// Rename-pair and convert raw git deltas into the `FileDiff` list
/// (added / deleted / modified / renamed / conflicted), sorted by
/// path. Shared by the workdir and staged-diff computations so both
/// sections behave identically.
fn deltas_to_file_diffs(
    deltas: &[(git2::Delta, Oid, PathBuf)],
) -> Vec<FileDiff> {
    let mut renames = Vec::new();
    let mut renamed_deltas = HashSet::new();

    for (added_index, delta) in deltas.iter().enumerate() {
        if delta.0 == git2::Delta::Added {
            for (deleted_index, d) in deltas.iter().enumerate() {
                if d.0 == git2::Delta::Deleted && d.1 == delta.1 {
                    renames.push((added_index, deleted_index));
                    renamed_deltas.insert(added_index);
                    renamed_deltas.insert(deleted_index);
                    break;
                }
            }
        }
    }

    let mut file_diffs = Vec::new();
    for (added_index, deleted_index) in renames.iter() {
        file_diffs.push(FileDiff::Renamed(
            deltas[*added_index].2.clone(),
            deltas[*deleted_index].2.clone(),
        ));
    }
    for (i, delta) in deltas.iter().enumerate() {
        if renamed_deltas.contains(&i) {
            continue;
        }
        let diff = match delta.0 {
            git2::Delta::Added => FileDiff::Added(delta.2.clone()),
            git2::Delta::Deleted => FileDiff::Deleted(delta.2.clone()),
            git2::Delta::Modified => FileDiff::Modified(delta.2.clone()),
            git2::Delta::Conflicted => FileDiff::Conflicted(delta.2.clone()),
            _ => continue,
        };
        file_diffs.push(diff);
    }
    file_diffs.sort_by_key(|d| match d {
        FileDiff::Modified(p)
        | FileDiff::Added(p)
        | FileDiff::Renamed(p, _)
        | FileDiff::Conflicted(p)
        | FileDiff::Deleted(p) => p.clone(),
    });
    file_diffs
}

fn git_diff_new(workspace_path: &Path) -> Option<DiffInfo> {
    let repo = Repository::discover(workspace_path).ok()?;
    let name = match repo.head() {
        Ok(head) => head.shorthand().ok()?.to_string(),
        _ => "(No branch)".to_owned(),
    };

    let mut branches = Vec::new();
    for branch in repo.branches(None).ok()? {
        branches.push(branch.ok()?.0.name().ok()??.to_string());
    }

    let mut tags = Vec::new();
    if let Ok(git_tags) = repo.tag_names(None) {
        for tag in git_tags.into_iter().flatten() {
            let Some(tag) = tag else {
                continue;
            };
            tags.push(tag.to_owned());
        }
    }

    let mut deltas = Vec::new();
    let mut diff_options = DiffOptions::new();
    let diff = repo
        .diff_index_to_workdir(
            None,
            Some(
                diff_options
                    .include_untracked(true)
                    .recurse_untracked_dirs(true),
            ),
        )
        .ok()?;
    for delta in diff.deltas() {
        if let Some(delta) = git_delta_format(workspace_path, &delta) {
            deltas.push(delta);
        }
    }
    let file_diffs = deltas_to_file_diffs(&deltas);

    let oid = match repo.revparse_single("HEAD^{tree}") {
        Ok(obj) => obj.id(),
        _ => Oid::ZERO_SHA1,
    };

    // Staged (index) changes: HEAD tree -> index, kept in their own
    // list so the panel can show a separate "Staged Changes" section
    // instead of mixing them into the workdir changes.
    let cached_diff = repo
        .diff_tree_to_index(repo.find_tree(oid).ok().as_ref(), None, None)
        .ok();

    let mut staged_deltas = Vec::new();
    if let Some(cached_diff) = cached_diff {
        for delta in cached_diff.deltas() {
            if let Some(delta) = git_delta_format(workspace_path, &delta) {
                staged_deltas.push(delta);
            }
        }
    }
    let staged_diffs = deltas_to_file_diffs(&staged_deltas);

    let (ahead, behind) = git_ahead_behind(&repo, &name);
    Some(DiffInfo {
        head: name,
        branches,
        tags,
        diffs: file_diffs,
        staged_diffs,
        ahead,
        behind,
    })
}

fn file_get_head(workspace_path: &Path, path: &Path) -> Result<(String, String)> {
    let repo = Repository::discover(workspace_path)?;
    let head = repo.head()?;
    let tree = head.peel_to_tree()?;
    let tree_entry = tree.get_path(path.strip_prefix(workspace_path)?)?;
    let blob = repo.find_blob(tree_entry.id())?;
    let id = blob.id().to_string();
    let content = std::str::from_utf8(blob.content())
        .with_context(|| "content bytes to string")?
        .to_string();
    Ok((id, content))
}

fn git_get_remote_file_url(workspace_path: &Path, file: &Path) -> Result<String> {
    let repo = Repository::discover(workspace_path)?;
    let head = repo.head()?;
    let target_remote = repo.find_remote(
        repo.branch_upstream_remote(head.name().unwrap())?
            .as_str()
            .unwrap(),
    )?;

    // Grab URL part of remote
    let remote = target_remote
        .url()
        .map_err(|e| anyhow!("Failed to convert remote to str: {e}"))?;

    let remote_url = match Url::parse(remote) {
        Ok(url) => url,
        Err(_) => {
            // Parse URL as ssh
            Url::parse(&format!("ssh://{}", remote.replacen(':', "/", 1)))?
        }
    };

    // Get host part
    let host = remote_url
        .host_str()
        .ok_or(anyhow!("Couldn't find remote host"))?;
    // Get namespace (e.g. organisation/project in case of GitHub, org/team/team/team/../project on GitLab)
    let namespace = if let Some(stripped) = remote_url.path().strip_suffix(".git") {
        stripped
    } else {
        remote_url.path()
    };

    let commit = head.peel_to_commit()?.id();

    let file_path = file
        .strip_prefix(workspace_path)?
        .to_str()
        .ok_or(anyhow!("Couldn't convert file path to str"))?;

    let url = format!("https://{host}{namespace}/blob/{commit}/{file_path}",);

    Ok(url)
}

fn search_in_path(
    id: u64,
    current_id: &AtomicU64,
    paths: impl Iterator<Item = PathBuf>,
    pattern: &str,
    case_sensitive: bool,
    whole_word: bool,
    is_regex: bool,
) -> Result<ProxyResponse, RpcError> {
    let mut matches = IndexMap::new();
    let mut matcher = RegexMatcherBuilder::new();
    let matcher = matcher.case_insensitive(!case_sensitive).word(whole_word);
    let matcher = if is_regex {
        matcher.build(pattern)
    } else {
        matcher.build_literals(&[&regex::escape(pattern)])
    };
    let matcher = matcher.map_err(|_| RpcError {
        code: 0,
        message: "can't build matcher".to_string(),
    })?;
    let mut searcher = SearcherBuilder::new().build();

    for path in paths {
        if current_id.load(Ordering::SeqCst) != id {
            return Err(RpcError {
                code: 0,
                message: "expired search job".to_string(),
            });
        }

        if path.is_file() {
            let mut line_matches = Vec::new();
            if let Err(err) = searcher.search_path(
                &matcher,
                path.clone(),
                UTF8(|lnum, line| {
                    if current_id.load(Ordering::SeqCst) != id {
                        return Ok(false);
                    }

                    let mymatch = matcher.find(line.as_bytes())?.unwrap();
                    let line = if line.len() > 200 {
                        // Shorten the line to avoid sending over absurdly long-lines
                        // (such as in minified javascript)
                        // Note that the start/end are column based, not absolute from the
                        // start of the file.
                        let left_keep = line[..mymatch.start()]
                            .chars()
                            .rev()
                            .take(100)
                            .map(|c| c.len_utf8())
                            .sum::<usize>();
                        let right_keep = line[mymatch.end()..]
                            .chars()
                            .take(100)
                            .map(|c| c.len_utf8())
                            .sum::<usize>();
                        let display_range =
                            mymatch.start() - left_keep..mymatch.end() + right_keep;
                        line[display_range].to_string()
                    } else {
                        line.to_string()
                    };
                    line_matches.push(SearchMatch {
                        line: lnum as usize,
                        start: mymatch.start(),
                        end: mymatch.end(),
                        line_content: line,
                    });
                    Ok(true)
                }),
            ) {
                {
                    tracing::error!("{:?}", err);
                }
            }
            if !line_matches.is_empty() {
                matches.insert(path.clone(), line_matches);
            }
        }
    }

    Ok(ProxyResponse::GlobalSearchResponse { matches })
}

#[cfg(test)]
mod no_language_server_tests {
    use super::*;
    use lapce_rpc::proxy::ProxyRpc;

    #[test]
    fn unsupported_requests_finish_immediately() {
        let rpc = ProxyRpcHandler::new();
        let mut dispatcher = Dispatcher::new(CoreRpcHandler::new(), rpc.clone());
        let (tx, rx) = crossbeam_channel::bounded(1);
        rpc.request_async(
            ProxyRequest::GetSemanticTokens {
                path: PathBuf::from("does-not-exist.rs"),
            },
            move |result| {
                tx.send(result).unwrap();
            },
        );
        let ProxyRpc::Request(id, request) = rpc.rx().recv().unwrap() else {
            panic!("not a request")
        };
        dispatcher.handle_request(id, request);
        let error = rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .unwrap_err();
        assert_eq!(error.code, -32601);
    }

    #[test]
    fn opening_missing_file_needs_no_plugin_catalog() {
        let rpc = ProxyRpcHandler::new();
        let mut dispatcher = Dispatcher::new(CoreRpcHandler::new(), rpc.clone());
        let (tx, rx) = crossbeam_channel::bounded(1);
        rpc.request_async(
            ProxyRequest::NewBuffer {
                buffer_id: BufferId::next(),
                path: PathBuf::from("missing-no-lsp-test.txt"),
            },
            move |result| {
                tx.send(result).unwrap();
            },
        );
        let ProxyRpc::Request(id, request) = rpc.rx().recv().unwrap() else {
            panic!("not a request")
        };
        dispatcher.handle_request(id, request);
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            Ok(ProxyResponse::NewBufferResponse { .. })
        ));
    }
}

#[cfg(test)]
mod git_ahead_behind_tests {
    use super::*;

    /// Scratch repo: c1 (root) -> c2 (HEAD) -> c3 (dangling, used as the
    /// upstream tip). Moves refs/remotes/origin/<branch> and checks the
    /// ahead/behind math, including the no-upstream case.
    #[test]
    fn ahead_behind_tracks_upstream_movement() {
        let dir = std::env::temp_dir().join(format!(
            "lapce-git-ahead-behind-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let repo = Repository::init(&dir).expect("init scratch repo");

        let sig = git2::Signature::now("lapce-test", "lapce-test@example.invalid")
            .expect("signature");
        let tree_id = repo
            .treebuilder(None)
            .and_then(|mut tb| tb.write())
            .expect("empty tree");
        let tree = repo.find_tree(tree_id).expect("find tree");

        let c1 = repo
            .commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[])
            .expect("root commit");
        let c1_commit = repo.find_commit(c1).expect("find c1");
        let c2 = repo
            .commit(Some("HEAD"), &sig, &sig, "c2", &tree, &[&c1_commit])
            .expect("second commit");
        let c2_commit = repo.find_commit(c2).expect("find c2");
        let c3 = repo
            .commit(None, &sig, &sig, "c3", &tree, &[&c2_commit])
            .expect("dangling commit");

        let head_name = repo
            .head()
            .expect("head")
            .shorthand()
            .expect("head shorthand")
            .to_string();
        // No upstream configured yet: zeroes, not an error.
        assert_eq!(git_ahead_behind(&repo, &head_name), (0, 0));

        repo.remote("origin", "https://example.invalid/scratch.git")
            .expect("add origin");
        let mut branch = repo
            .find_branch(&head_name, git2::BranchType::Local)
            .expect("local branch");
        branch
            .set_upstream(Some(&format!("origin/{head_name}")))
            .expect("set upstream");

        // Upstream at the root: local is one ahead.
        repo.reference(
            &format!("refs/remotes/origin/{head_name}"),
            c1,
            true,
            "test: upstream at c1",
        )
        .expect("upstream ref");
        assert_eq!(git_ahead_behind(&repo, &head_name), (1, 0));

        // Upstream past HEAD: local is one behind.
        repo.reference(
            &format!("refs/remotes/origin/{head_name}"),
            c3,
            true,
            "test: upstream at c3",
        )
        .expect("move upstream ref");
        assert_eq!(git_ahead_behind(&repo, &head_name), (0, 1));

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod git_create_branch_tests {
    use super::*;

    /// Creating a branch at HEAD switches HEAD to it and keeps the same
    /// commit; duplicate names and invalid refnames error instead of
    /// silently overwriting.
    #[test]
    fn create_branch_switches_head_without_overwrite() {
        let dir = std::env::temp_dir().join(format!(
            "lapce-git-create-branch-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let repo = Repository::init(&dir).expect("init scratch repo");
        let sig = git2::Signature::now("lapce-test", "lapce-test@example.invalid")
            .expect("signature");
        let tree_id = repo
            .treebuilder(None)
            .and_then(|mut tb| tb.write())
            .expect("empty tree");
        let tree = repo.find_tree(tree_id).expect("find tree");
        let head_commit = repo
            .commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[])
            .expect("root commit");

        git_create_branch(&dir, "feature/test").expect("create branch");

        let head = repo.head().expect("head");
        assert_eq!(head.shorthand().ok(), Some("feature/test"));
        assert_eq!(
            repo.head()
                .and_then(|h| h.peel_to_commit())
                .map(|c| c.id())
                .expect("head commit"),
            head_commit
        );
        assert!(git_create_branch(&dir, "feature/test").is_err());
        assert!(git_create_branch(&dir, "bad..name").is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod git_staged_diff_tests {
    use git2::Repository;

    use super::git_diff_new;

    fn init_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "test").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        dir
    }

    fn commit_all(dir: &tempfile::TempDir, message: &str) {
        let repo = Repository::open(dir.path()).unwrap();
        let mut index = repo.index().unwrap();
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let sig = repo.signature().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parents: Vec<git2::Commit> = repo
            .head()
            .ok()
            .and_then(|h| h.peel_to_commit().ok())
            .into_iter()
            .collect();
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
            .unwrap();
    }

    #[test]
    fn staged_and_unstaged_are_separate() {
        let dir = init_repo();
        std::fs::write(dir.path().join("a.txt"), "v1").unwrap();
        commit_all(&dir, "init");

        // Stage a modification to a.txt; leave b.txt as an unstaged
        // new file.
        std::fs::write(dir.path().join("a.txt"), "v2").unwrap();
        std::fs::write(dir.path().join("b.txt"), "new").unwrap();
        {
            let repo = Repository::open(dir.path()).unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(std::path::Path::new("a.txt")).unwrap();
            index.write().unwrap();
        }

        let info =
            git_diff_new(dir.path()).expect("git_diff_new returned None");

        let staged: Vec<_> =
            info.staged_diffs.iter().map(|d| d.path().clone()).collect();
        let unstaged: Vec<_> =
            info.diffs.iter().map(|d| d.path().clone()).collect();

        // FileDiff paths are workspace-absolute.
        assert!(staged.contains(&dir.path().join("a.txt")));
        assert!(unstaged.contains(&dir.path().join("b.txt")));
        assert!(!unstaged.contains(&dir.path().join("a.txt")));
        assert!(!staged.contains(&dir.path().join("b.txt")));
    }

    #[test]
    fn file_staged_then_modified_again_appears_in_both() {
        let dir = init_repo();
        std::fs::write(dir.path().join("a.txt"), "v1").unwrap();
        commit_all(&dir, "init");

        std::fs::write(dir.path().join("a.txt"), "v2").unwrap();
        {
            let repo = Repository::open(dir.path()).unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(std::path::Path::new("a.txt")).unwrap();
            index.write().unwrap();
        }
        std::fs::write(dir.path().join("a.txt"), "v3").unwrap();

        let info =
            git_diff_new(dir.path()).expect("git_diff_new returned None");

        let staged: Vec<_> =
            info.staged_diffs.iter().map(|d| d.path().clone()).collect();
        let unstaged: Vec<_> =
            info.diffs.iter().map(|d| d.path().clone()).collect();

        assert!(staged.contains(&dir.path().join("a.txt")));
        assert!(unstaged.contains(&dir.path().join("a.txt")));
    }
}
