use lapce_core::diagnostics::install_panic_hook_at;

#[test]
fn panic_leaves_crash_report_with_backtrace() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crash.log");
    install_panic_hook_at(path.clone(), "test-process");
    let result = std::thread::Builder::new()
        .name("worker-1".into())
        .spawn(|| panic!("kaboom {}", 42))
        .unwrap()
        .join();
    assert!(result.is_err());
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("message: kaboom 42"), "{text}");
    assert!(text.contains("thread: worker-1"), "{text}");
    assert!(text.contains("process: test-process"), "{text}");
    assert!(text.contains("panic_hook.rs"), "{text}");
    assert!(text.contains("backtrace:"), "{text}");
}
