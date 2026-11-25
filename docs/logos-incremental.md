# Experimental incremental highlighting (L8)

The Logos path reuses the document's existing background parse worker and revision/cancellation gate. No lexing or native-fold work is added to the UI callback. The UI clones only an Arc to the incremental cache, not document-sized style/checkpoint vectors.

Per-line checkpoints preserve PHP mode and HTML host state. A line inside a multiline token records the token opener as its safe restart. Edits reuse the unchanged prefix and re-lex from that safe point. Incomplete strings/comments/heredocs and Blade islands remain lexical, not parsed or executed.

The fast suffix pass has a 64KiB invalidation allowance. Larger invalidations use a stack-preserving cooperative worker session with 16KiB work permits. The worker suspends its actual scan stack, so quoted/interpolated text, nested braces, heredocs and Blade island depth survive yields without replaying the whole prefix. Cancellation discards unpublished results and releases suspended workers. Scan, paint and checkpoint passes cooperate; framework overlay loops also yield and check cancellation. Diff/copy/allocation setup is background work, not UI work. Work counters bound scanner permits, not operating-system wall-clock latency or allocation time.

Native Lexilla folding cannot be interrupted inside its native call. It is therefore not part of each experimental colour pass. A separate 250ms-debounced background refresh computes native fold ranges, checking cancellation before and after. Existing edit-tracked folds stay in place until a refresh for the same revision is accepted. A stale refresh cannot replace newer colour/cache state. The default Lexilla path is unchanged.

Limits: a multiline token edit may invalidate back to its opener; large edits can require a full cooperative scan. Framework convention overlays still inspect the full token list in cancellable background chunks. This phase does not claim sublinear copying, hard real-time deadlines, or a speed win. Those require the benchmark phase. Both build and runtime opt-in remain required.

Tests include differential full-vs-incremental styles/checkpoints, every insertion position across PHP/Blade multiline fixtures, 2,000 random edit sequences, UTF-8, incomplete input, safe-tail restarts, budget deferral, suspended-worker shutdown, cancellation during long scans, stale fold refresh rejection, default and experimental core tests. Hosted acceptance also compiles/tests the app with Logos support before the ordinary full workspace suite.
