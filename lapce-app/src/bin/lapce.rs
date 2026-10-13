#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use lapce_app::app;

pub fn main() {
    if std::env::args().nth(1).as_deref() == Some("--vscode-experiment-worker") {
        std::process::exit(lapce_app::vscode_experiment::worker_main());
    }
    app::launch();
}
