use lapce_vscode_experiment::*;
use std::path::Path;
fn worker() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_lapce-vscode-experiment"))
}
const MANIFEST: &str = include_str!("../examples/hello/package.json");
fn run(source: &str) -> Result<Outcome, String> {
    Extension::from_source(MANIFEST, source)?.execute(
        worker(),
        "sandbox.hello",
        serde_json::json!([]),
    )
}
#[test]
fn commonjs_command_and_config_and_message() {
    let result = demo(worker()).unwrap();
    assert_eq!(result.value, "Hello from the sandbox");
    assert_eq!(result.messages.len(), 1);
    assert_eq!(result.messages[0].kind, "info");
}
#[test]
fn load_real_extension_directory() {
    let ext = Extension::load(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/hello")
            .as_path(),
    )
    .unwrap();
    assert_eq!(ext.commands[0].title, "Sandbox: Hello");
    assert!(
        ext.execute(worker(), "sandbox.hello", serde_json::json!([]))
            .is_ok()
    );
}
#[test]
fn no_node_or_host_capabilities() {
    for module in ["fs", "child_process", "net", "http", "os", "./helper"] {
        assert!(run(&format!("require({module:?});")).is_err());
    }
    let result = run("exports.activate = () => require('vscode').commands.registerCommand('sandbox.hello', () => [typeof process, typeof fetch, typeof WebAssembly, typeof std, typeof os]);").unwrap();
    assert_eq!(
        result.value,
        serde_json::json!([
            "undefined",
            "undefined",
            "undefined",
            "undefined",
            "undefined"
        ])
    );
}
#[test]
fn cannot_call_editor_builtin_or_register_undeclared() {
    assert!(run("exports.activate = () => require('vscode').commands.registerCommand('workbench.action.terminal.new', () => {});").is_err());
    assert!(run("exports.activate = () => require('vscode').commands.registerCommand('sandbox.hello', () => require('vscode').commands.executeCommand('workbench.action.terminal.new'));").is_err());
}
#[test]
fn infinite_loops_terminated_in_activation_command_and_microtask() {
    for source in [
        "while(true) {}",
        "exports.activate = () => {while(true) {}};",
        "exports.activate = () => require('vscode').commands.registerCommand('sandbox.hello', () => {while(true) {}});",
        "exports.activate = () => require('vscode').commands.registerCommand('sandbox.hello', () => Promise.resolve().then(() => {while(true) {}}));",
    ] {
        assert!(run(source).is_err());
    }
}
#[test]
fn heap_bomb_and_message_flood_rejected() {
    assert!(run("const a=[]; while(true) a.push('x'.repeat(100000));").is_err());
    assert!(run("exports.activate = () => require('vscode').commands.registerCommand('sandbox.hello', () => { for(let i=0;i<100;i++) require('vscode').window.showInformationMessage('x'); });").is_err());
}
#[test]
fn pending_forever_and_unknown_command_rejected() {
    assert!(run("exports.activate = () => new Promise(() => {});").is_err());
    assert!(
        Extension::from_source(MANIFEST, "")
            .unwrap()
            .execute(worker(), "other", serde_json::json!([]))
            .is_err()
    );
}
#[test]
fn async_commands_and_isolation() {
    let source = "exports.activate = () => require('vscode').commands.registerCommand('sandbox.hello', async () => {globalThis.count=(globalThis.count||0)+1; return globalThis.count;});";
    assert_eq!(run(source).unwrap().value, 1);
    assert_eq!(run(source).unwrap().value, 1);
}
#[test]
fn input_and_output_limits() {
    assert!(
        Extension::from_source(MANIFEST, &"x".repeat((1024 * 1024) + 1)).is_err()
    );
    assert!(run("exports.activate = () => require('vscode').commands.registerCommand('sandbox.hello', () => 'x'.repeat(100000));").is_err());
}
