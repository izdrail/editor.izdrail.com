//! Window-wide registry of live agent runs: the manager surface for
//! parallel agents (Antigravity parity, first slice). Every chat send
//! path (chat, Ask/Plan/Act and ACP) registers its run; the chat
//! panel's Runs section lists the window's live runs, newest first,
//! each with a Stop control wired to the run's own cancellation flag.
//! Runs in different workspace window tabs already execute in
//! parallel; this registry is what makes them visible and stoppable
//! from one place.
//!
//! Removal is explicit: every run-termination path in the chat panel
//! routes through ChatData::end_active_run, which finishes the
//! registry entry and clears busy together, so the section cannot
//! list a finished run. A workspace tab closed mid-run leaves its
//! entry until the window closes (its Stop flag simply has no
//! listener left); per-session background runs are the next slice.
use floem::reactive::{RwSignal, Scope, SignalGet, SignalUpdate};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// One live agent run anywhere in the window's workspace tabs.
#[derive(Clone)]
pub struct RunEntry {
    pub id: u64,
    /// First line of the owner's prompt, capped for the row label.
    pub title: String,
    /// Which send path started the run: Chat, Ask, Plan, Act or ACP.
    pub mode: &'static str,
    /// Local HH:MM start label.
    pub started: String,
    /// When the run registered: drives the live elapsed label on the
    /// Running row (item 230).
    pub started_at: std::time::Instant,
    /// The run's own cancellation flag: Stop stores true and the
    /// run's worker observes it exactly like the panel's Stop button.
    pub cancellation: Arc<AtomicBool>,
}

/// The window's live runs, newest first. Clone-cheap: the entries
/// signal is Copy and the id counter is shared.
#[derive(Clone)]
pub struct RunRegistry {
    next: Arc<AtomicU64>,
    runs: RwSignal<Vec<RunEntry>>,
}

impl RunRegistry {
    pub fn new(cx: Scope) -> Self {
        Self {
            next: Arc::new(AtomicU64::new(1)),
            runs: cx.create_rw_signal(Vec::new()),
        }
    }
    /// Register a starting run and return its id for later finish.
    /// The list is capped so a leak grows bounded, never unbounded.
    pub fn start(
        &self,
        title: &str,
        mode: &'static str,
        cancellation: Arc<AtomicBool>,
    ) -> u64 {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let entry = RunEntry {
            id,
            title: run_title(title),
            mode,
            started: chrono::Local::now().format("%H:%M").to_string(),
            started_at: std::time::Instant::now(),
            cancellation,
        };
        self.runs.update(|runs| {
            runs.insert(0, entry);
            runs.truncate(32);
        });
        id
    }
    /// Drop the entry for a finished run. Unknown ids are ignored so
    /// a double-finish (Stop racing Error, for example) is harmless.
    pub fn finish(&self, id: u64) {
        self.runs.update(|runs| runs.retain(|e| e.id != id));
    }
    /// The live entries, newest first, for the Runs section.
    pub fn entries(&self) -> Vec<RunEntry> {
        self.runs.get()
    }
}

/// The row title for a run: the prompt's first line, capped at 40
/// chars with an ellipsis, so long prompts stay one line.
pub fn run_title(prompt: &str) -> String {
    let first = prompt.lines().next().unwrap_or("").trim();
    if first.is_empty() {
        return "(untitled run)".to_string();
    }
    let mut title: String = first.chars().take(40).collect();
    if first.chars().count() > 40 {
        title.push('…');
    }
    title
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_lists_newest_first_and_finish_removes() {
        let cx = Scope::new();
        let registry = RunRegistry::new(cx);
        let flag = Arc::new(AtomicBool::new(false));
        let first = registry.start("add a health route", "Ask", flag.clone());
        let second = registry.start("refactor the parser", "Act", flag.clone());
        assert_ne!(first, second);
        let entries = registry.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, second);
        assert_eq!(entries[0].mode, "Act");
        assert_eq!(entries[1].title, "add a health route");
        registry.finish(first);
        let entries = registry.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, second);
        // Finishing an unknown id is a no-op, not a panic.
        registry.finish(first);
        assert_eq!(registry.entries().len(), 1);
    }

    #[test]
    fn run_title_uses_first_line_capped() {
        assert_eq!(run_title("hello world"), "hello world");
        assert_eq!(run_title("first\nsecond"), "first");
        assert_eq!(run_title("   \n"), "(untitled run)");
        let long = "a".repeat(60);
        let title = run_title(&long);
        assert_eq!(title.chars().count(), 41);
        assert!(title.ends_with('…'));
    }

    #[test]
    fn stop_flag_is_shared_with_the_run() {
        let cx = Scope::new();
        let registry = RunRegistry::new(cx);
        let flag = Arc::new(AtomicBool::new(false));
        registry.start("work", "Chat", flag.clone());
        registry.entries()[0]
            .cancellation
            .store(true, Ordering::Release);
        assert!(flag.load(Ordering::Acquire));
    }
}
