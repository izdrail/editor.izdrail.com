# Workspace MCP tools in the agent loop (item 194, Antigravity parity: MCP server config)

Act mode runs now connect the workspace's enabled `.lapce/mcp.json`
servers (the same config the MCP status report uses) on the worker
thread at run start, keep the sessions alive for the run, and expose
their tools to the model as `mcp__server__tool` functions with the
server's own input schema. This is the integration the MCP probe was
always pointed at; the probe stays the diagnostic surface.

Trust shape: MCP tools are arbitrary server-side code, so every call
goes through the same approval lane as run_command - the panel shows
`mcp server.tool` plus a 300-byte preview of the arguments and the
owner approves or denies; denial comes back as the tool result. Tool
output is untrusted data (the loop's system contract says so). MCP
tools are Act-only; Ask/Plan runs get no extra tools (the loop rejects
them). Caps: 4 servers, 16 tools total, 64-message handshake and call
guards, 10s connect timeout per server (a hung handshake is killed by
pid on a collector thread), 64 KiB argument cap, the loop's 256 KiB
result cap applies. A server that fails to launch or handshake becomes
a step-row note, never a run failure. Sessions die with the pool
(AcpProcess kills its child on drop). Linux-only like the ACP lane.

Coverage: loop-side tests prove MCP tools execute with object
arguments, non-object arguments are rejected, extra tools are Act-only
and shape-checked; pool tests prove name sanitization/qualification,
bounded summaries, missing-config and dead-binary notes, and a real
end-to-end initialize -> tools/list -> tools/call roundtrip through a
fake MCP server scripted in sh.
