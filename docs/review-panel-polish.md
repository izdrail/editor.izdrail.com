# Review panel layout and walkthrough run line (item 214, host QA follow-up)

Host render of new-file creation in Act (qwen3:4b) verified the flow
end to end and surfaced three presentation bugs:

- The proposal card's Apply and Discard labels overlapped and
  clipped: plain unstyled labels in a spacing-less v_stack. The card
  now stacks with width_full + gap(6), and the buttons are short
  ("Apply accepted hunks", "Discard proposal") with constrained
  width, padding and a pointer cursor. The explanatory sentence the
  long label carried is already covered by the notice line above the
  card ("Review every before/after file below. Nothing changed
  yet.").
- The checkpoint card's Restore label had the same disease and gets
  the same treatment ("Restore checkpoint (all owned buffers)").
- The walkthrough artifact's Run section printed the raw session id
  (a 48-character hex blob). walkthrough_artifact now takes a
  run_label: the Act auto-prepare path records "Act: <request title>"
  (the run_title helper, first line capped at 40 chars) on the review
  data when a proposal is prepared, and the artifact leads with it,
  keeping the raw id on a second line for tracing. Pasted-JSON
  reviews carry no run context, so their artifacts keep the id-only
  Run line.

Coverage: the walkthrough_artifact test now pins the labelled Run
section and the id-only fallback.
