//! Window-wide registry of live agent runs: the manager surface for
//! parallel agents (Antigravity parity, first slice). Every chat send
//! path (chat, Ask/Plan/Act and ACP) registers its run; the chat
//! panel's Runs section lists the window's live runs, newest first,
//! each with a Stop control wired to the run's own cancellation flag.
//! Runs in different workspace window tabs already execute in
//! parallel; this registry is what makes them visible and stoppable
//! from one place.
//!
//! Removal is doubly owned: the panel routes its terminations
//! through ChatData::end_active_run, and the worker thread's
//! FinishGuard marks its entry worker-done on every exit - normal,
//! error, cancel or panic - so a workspace tab closed mid-run no
//! longer strands a "Running" row: the one-second runs tick sweeps
//! it. Floem signals stay on the UI thread everywhere in this
//! codebase, so the guard carries only a plain Arc<AtomicBool>,
//! never the registry. Journal/reattach are the next slices (see
//! docs/background-runs.md).
use floem::reactive::{RwSignal, Scope, SignalGet, SignalUpdate};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

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
    /// Set by the worker's FinishGuard when the run's thread ends,
    /// however it ends (item 236 / parity 7 slice 1). The UI sweeps
    /// flagged entries on the one-second runs tick - floem signals
    /// stay on the UI thread everywhere in this codebase, so the
    /// guard carries only this plain flag, never the registry.
    pub worker_done: Arc<AtomicBool>,
    /// The replayable record of the run's progress and outcome
    /// (item 240 / parity 7 slice 2): the worker dual-writes every
    /// step here as it emits; a panel attaching later replays from
    /// here. Plain data behind a Mutex - never a signal.
    pub journal: Arc<Mutex<RunJournal>>,
}

/// Worker-side finish guarantee (item 236 / parity 7 slice 1): the
/// worker thread owns the end of its registry entry - normal,
/// error, cancel, even panic - instead of relying on a panel
/// effect that may already be gone (a closed tab). Drop marks the
/// entry worker-done; the UI sweep removes it. A panel that
/// finishes the entry first makes the later drop a no-op.
pub struct FinishGuard {
    done: Arc<AtomicBool>,
}
impl Drop for FinishGuard {
    fn drop(&mut self) {
        self.done.store(true, Ordering::SeqCst);
    }
}

/// One journaled step row: the replayable record of a run's
/// progress (item 238 / parity 7 slice 2). Plain owned data - the
/// worker thread writes it without touching the panel's types or
/// any floem signal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalStep {
    pub title: String,
    pub detail: String,
    pub error: bool,
}

/// How a run ended, replayed by a panel that (re)attaches after
/// the fact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JournalOutcome {
    /// Final reply or proposal text.
    Done(String),
    Error(String),
    Cancelled,
}

/// Journal bounds (docs/background-runs.md): overflow drops the
/// oldest steps and counts them, never blocks the run.
pub const JOURNAL_MAX_STEPS: usize = 200;
pub const JOURNAL_MAX_BYTES: usize = 256 * 1024;

/// The bounded, thread-safe record of what a run did. The worker
/// appends every step and, at the end, the outcome; a panel
/// attaching later replays from here (slice 3).
#[derive(Clone, Debug, Default)]
pub struct RunJournal {
    steps: VecDeque<JournalStep>,
    bytes: usize,
    dropped: usize,
    outcome: Option<JournalOutcome>,
}
impl RunJournal {
    pub fn push_step(&mut self, step: JournalStep) {
        self.bytes += step.title.len() + step.detail.len();
        self.steps.push_back(step);
        while self.steps.len() > JOURNAL_MAX_STEPS
            || (self.bytes > JOURNAL_MAX_BYTES && self.steps.len() > 1)
        {
            // The `len > 1` guard keeps the newest step even when
            // it alone exceeds the byte cap: a run always shows
            // its latest state.
            if let Some(old) = self.steps.pop_front() {
                self.bytes -= old.title.len() + old.detail.len();
                self.dropped += 1;
            }
        }
    }
    /// The first terminal state wins; a run ends once.
    pub fn finish(&mut self, outcome: JournalOutcome) {
        if self.outcome.is_none() {
            self.outcome = Some(outcome);
        }
    }
    pub fn steps(&self) -> impl Iterator<Item = &JournalStep> {
        self.steps.iter()
    }
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }
    /// Steps dropped to stay inside the caps; the reattach view
    /// renders this as a marker row.
    pub fn dropped(&self) -> usize {
        self.dropped
    }
    pub fn outcome(&self) -> Option<&JournalOutcome> {
        self.outcome.as_ref()
    }
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
            worker_done: Arc::new(AtomicBool::new(false)),
            journal: Arc::new(Mutex::new(RunJournal::default())),
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
    /// The worker-side end-of-run guarantee for this entry: drop
    /// marks it worker-done for the UI sweep. An unknown id yields
    /// a guard on a detached flag (a harmless no-op).
    pub fn finish_guard(&self, id: u64) -> FinishGuard {
        let done = self
            .runs
            .get_untracked()
            .into_iter()
            .find(|e| e.id == id)
            .map(|e| e.worker_done.clone())
            .unwrap_or_else(|| Arc::new(AtomicBool::new(true)));
        FinishGuard { done }
    }
    /// The run's journal for the worker's dual-writes (item 240).
    /// An unknown id yields None; the worker then skips journaling.
    pub fn journal(&self, id: u64) -> Option<Arc<Mutex<RunJournal>>> {
        self.runs
            .get_untracked()
            .into_iter()
            .find(|e| e.id == id)
            .map(|e| e.journal.clone())
    }
    /// Remove entries whose worker thread has ended without a
    /// panel-side finish (closed tab). Returns the removed count.
    /// Idempotent; runs on the UI thread from the runs tick.
    pub fn sweep_finished_workers(&self) -> usize {
        let done = self
            .runs
            .get_untracked()
            .iter()
            .filter(|e| e.worker_done.load(Ordering::SeqCst))
            .count();
        if done > 0 {
            self.runs.update(|v| {
                v.retain(|e| !e.worker_done.load(Ordering::SeqCst))
            });
        }
        done
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
    fn finish_guard_marks_done_and_sweep_removes() {
        let cx = Scope::new();
        let registry = RunRegistry::new(cx);
        let flag = Arc::new(AtomicBool::new(false));
        let first = registry.start("one", "Ask", flag.clone());
        let second = registry.start("two", "Act", flag);
        let guard = registry.finish_guard(first);
        // Still listed while the worker runs.
        assert_eq!(registry.sweep_finished_workers(), 0);
        assert_eq!(registry.entries().len(), 2);
        drop(guard);
        // Marked, not yet swept: the sweep is the UI's one-second
        // tick, not the drop itself.
        assert_eq!(registry.entries().len(), 2);
        assert_eq!(registry.sweep_finished_workers(), 1);
        let entries = registry.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, second);
        // Idempotent: nothing left to sweep.
        assert_eq!(registry.sweep_finished_workers(), 0);
    }
    #[test]
    fn panel_finish_then_guard_drop_sweeps_nothing() {
        // The panel finishing first (normal path) makes the
        // worker's later guard drop a no-op.
        let cx = Scope::new();
        let registry = RunRegistry::new(cx);
        let flag = Arc::new(AtomicBool::new(false));
        let id = registry.start("task", "Chat", flag);
        let guard = registry.finish_guard(id);
        registry.finish(id);
        assert!(registry.entries().is_empty());
        drop(guard);
        assert_eq!(registry.sweep_finished_workers(), 0);
        assert!(registry.entries().is_empty());
    }
    #[test]
    fn finish_guard_unknown_id_is_a_noop() {
        let cx = Scope::new();
        let registry = RunRegistry::new(cx);
        let guard = registry.finish_guard(9_999);
        drop(guard);
        assert_eq!(registry.sweep_finished_workers(), 0);
        assert!(registry.entries().is_empty());
    }
    #[test]
    fn finish_guard_marks_done_even_on_panic() {
        let cx = Scope::new();
        let registry = RunRegistry::new(cx);
        let flag = Arc::new(AtomicBool::new(false));
        let id = registry.start("task", "Chat", flag);
        // Only the guard crosses the thread boundary: the registry
        // holds floem signals, which are not Send.
        let guard = registry.finish_guard(id);
        let worker = std::thread::spawn(move || {
            let _guard = guard;
            panic!("simulated worker failure");
        });
        assert!(worker.join().is_err());
        assert_eq!(registry.sweep_finished_workers(), 1);
        assert!(registry.entries().is_empty());
    }
    #[test]
    fn journal_handle_is_shared_with_the_entry() {
        let cx = Scope::new();
        let registry = RunRegistry::new(cx);
        let flag = Arc::new(AtomicBool::new(false));
        let id = registry.start("task", "Act", flag);
        let journal = registry.journal(id).unwrap();
        journal
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push_step(JournalStep {
                title: "run_command".into(),
                detail: "ls -la".into(),
                error: false,
            });
        // The entry's own handle sees the same record.
        let again = registry.journal(id).unwrap();
        let j = again.lock().unwrap_or_else(|p| p.into_inner());
        assert_eq!(j.step_count(), 1);
        assert_eq!(j.steps().next().unwrap().title, "run_command");
        assert!(registry.journal(9_999).is_none());
    }
    #[test]
    fn journal_caps_steps_and_counts_dropped() {
        let mut journal = RunJournal::default();
        for i in 0..205 {
            journal.push_step(JournalStep {
                title: format!("step {i}"),
                detail: String::new(),
                error: false,
            });
        }
        assert_eq!(journal.step_count(), JOURNAL_MAX_STEPS);
        assert_eq!(journal.dropped(), 5);
        // Oldest dropped: the first kept step is step 5.
        assert_eq!(journal.steps().next().unwrap().title, "step 5");
        assert_eq!(
            journal.steps().last().unwrap().title,
            "step 204"
        );
    }
    #[test]
    fn journal_byte_cap_keeps_the_newest_step() {
        let mut journal = RunJournal::default();
        let big = "x".repeat(JOURNAL_MAX_BYTES);
        journal.push_step(JournalStep {
            title: "old".into(),
            detail: big.clone(),
            error: false,
        });
        // One oversized-but-newest step stays.
        journal.push_step(JournalStep {
            title: "new".into(),
            detail: big,
            error: false,
        });
        assert_eq!(journal.step_count(), 1);
        assert_eq!(journal.dropped(), 1);
        assert_eq!(journal.steps().next().unwrap().title, "new");
    }
    #[test]
    fn journal_finish_keeps_the_first_outcome() {
        let mut journal = RunJournal::default();
        journal.finish(JournalOutcome::Done("reply".into()));
        journal.finish(JournalOutcome::Error("late".into()));
        assert_eq!(
            journal.outcome(),
            Some(&JournalOutcome::Done("reply".into()))
        );
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
