fn main() {
    if std::env::args().nth(1).as_deref() == Some("--vscode-experiment-worker") {
        std::process::exit(lapce_vscode_experiment::worker_main());
    }
    let args: Vec<_> = std::env::args().collect();
    let result = if args.len() == 3 {
        lapce_vscode_experiment::Extension::load(std::path::Path::new(&args[1]))
            .and_then(|e| {
                e.execute(
                    &std::env::current_exe().unwrap(),
                    &args[2],
                    serde_json::json!([]),
                )
            })
    } else {
        lapce_vscode_experiment::demo(&std::env::current_exe().unwrap())
    };
    match result {
        Ok(result) => println!("{}", serde_json::to_string(&result).unwrap()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
