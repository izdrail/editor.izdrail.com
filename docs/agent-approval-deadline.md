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
their await_approval span and add it. await_approval keeps its own
300-second cap, so a forgotten card still fails the run on its own
terms. Verified in review: the approved command still executes after
the wait (run_shell_command result becomes the tool result fed back
to the model) and the deadline is only evaluated between steps.

Coverage: deadline_excludes_owner_approval_wait runs a 1-second-limit
loop with 5 seconds of recorded wait (passes) and the same shape with
zero limit and no wait (still fails).
