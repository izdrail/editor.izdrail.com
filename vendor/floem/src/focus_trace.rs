//! Opt-in debug-only focus diagnostics. Never formats logical character text.
use std::cell::Cell;
use crate::{event::Event, keyboard::{Key, PhysicalKey}, id::ViewId};
thread_local! {
    static SEQUENCE: Cell<u64> = const { Cell::new(0) };
    static CURRENT: Cell<u64> = const { Cell::new(0) };
}
pub(crate) fn enabled() -> bool {
    std::env::var("LAPCE_FOCUS_TRACE").ok().as_deref() == Some("1")
}
pub(crate) fn begin(event: &Event, focus: Option<ViewId>) -> bool {
    if !enabled() { return false; }
    let Event::KeyDown(event) = event else { return false; };
    let id = SEQUENCE.with(|n| { let id = n.get() + 1; n.set(id); id });
    CURRENT.with(|n| n.set(id));
    let logical = match &event.key.logical_key {
        Key::Character(_) => "character(redacted)".to_string(),
        Key::Named(name) => format!("named:{name:?}"),
        _ => "other(redacted)".to_string(),
    };
    let physical = match event.key.physical_key {
        PhysicalKey::Code(code) => format!("code:{code:?}"),
        _ => "native(redacted)".to_string(),
    };
    eprintln!("FOCUS_TRACE seq={id} begin logical={logical} physical={physical} modifiers={:?} before={focus:?}", event.modifiers);
    true
}
pub(crate) fn note(stage: &str, focus: Option<ViewId>, paint: bool) {
    if !enabled() { return; }
    let id = CURRENT.with(Cell::get);
    eprintln!("FOCUS_TRACE seq={id} stage={stage} focus={focus:?} paint_requested={paint}");
}
pub(crate) fn transition(old: Option<ViewId>, new: Option<ViewId>, paint: bool) {
    if !enabled() { return; }
    let id = CURRENT.with(Cell::get);
    eprintln!("FOCUS_TRACE seq={id} transition old={old:?} new={new:?} paint_requested={paint}");
}
