# Agent deadline excludes owner approval wait (item 209, host QA follow-up)

An Act run that stopped for a run_command approval counted the owner's
think time against the 180-second agent deadline: a careful owner
reading the card came back to "Agent time limit reached" with no
command output and no final answer. Human wait is not agent runaway,
so the deadline no longer counts it.

agent_loop::run takes an approval_wait accumulator (Arc<AtomicU64>,
milliseconds) alongside the cancellation flag; the three deadline
checks subtract the recorded wait from the wall clock
(saturating_sub, so a clock hiccup can never go negative). Both
approval arms in the chat send path (run_command and MCP) measure
their await_approval span and add it on BOTH the answer and timeout
paths. Intended approval timeout: 30 minutes per card (generous
walk-away room; a forgotten card still ends the wait on its own
terms), and a card that does time out surfaces as "Command approval
timed out" on the progress row and to the model - never as the
misleading "Agent time limit reached" (the gate 489 bug: the wait
accumulation sat behind `?`, so a timed-out card recorded no wait and
the real deadline killed the run minutes later). Verified in review:
the approved command still executes after the wait (run_shell_command
result becomes the tool result fed back to the model) and the
deadline is only evaluated between steps.

Coverage: deadline_excludes_owner_approval_wait runs a 1-second-limit
loop with 5 seconds of recorded wait (passes) and the same shape with
zero limit and no wait (still fails).
