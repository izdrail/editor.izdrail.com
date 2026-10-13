//! Explicit demo only. Never discovers or executes installed marketplace code.
use crate::window_tab::WindowTabData;
use floem::ext_event::create_ext_action;

static RUNNING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub fn run_demo(tab: &WindowTabData) {
    use std::sync::atomic::Ordering;
    if RUNNING.swap(true, Ordering::AcqRel) {
        return;
    }
    let messages = tab.messages;
    let send = create_ext_action(
        tab.common.scope,
        move |result: Result<lapce_vscode_experiment::Outcome, String>| {
            use floem::reactive::SignalUpdate;
            let output = match result {
                Ok(result) => result
                    .messages
                    .into_iter()
                    .map(|message| lsp_types::ShowMessageParams {
                        typ: match message.kind.as_str() {
                            "warning" => lsp_types::MessageType::WARNING,
                            "error" => lsp_types::MessageType::ERROR,
                            _ => lsp_types::MessageType::INFO,
                        },
                        message: message.text,
                    })
                    .collect::<Vec<_>>(),
                Err(error) => vec![lsp_types::ShowMessageParams {
                    typ: lsp_types::MessageType::ERROR,
                    message: error,
                }],
            };
            messages.update(|messages| {
                messages.extend(
                    output
                        .into_iter()
                        .map(|m| ("VS Code sandbox experiment".into(), m)),
                )
            });
        },
    );
    std::thread::spawn(move || {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                RUNNING.store(false, Ordering::Release);
            }
        }
        let _reset = Reset;
        send(
            std::env::current_exe()
                .map_err(|e| e.to_string())
                .and_then(|worker| lapce_vscode_experiment::demo(&worker)),
        );
    });
}

pub use lapce_vscode_experiment::worker_main;
