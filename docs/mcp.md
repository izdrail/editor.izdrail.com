# MCP client (item 141, protocol contract)

`mcp.rs` is the pure Model Context Protocol client contract for talking
to stdio MCP servers, verified against
modelcontextprotocol.io/specification/2025-06-18 (basic/lifecycle +
server/tools) on 2026-10-17.

## What exists now

- Message builders: `initialize` (protocol `2025-06-18`, no client
  capabilities advertised), `notifications/initialized`, `tools/list`
  (cursor pagination), `tools/call`.
- Parsers: server hello (capabilities, instructions), tool specs
  (name/title/description/inputSchema, nameless tools skipped),
  tool results (text blocks concatenated, non-text blocks counted,
  `isError` surfaced).
- Workspace config: `.lapce/mcp.json` (`{ "servers": [{ name, command,
  args, enabled }] }`), validated on parse.

## Trust model (deliberate)

- `.lapce/mcp.json` can arrive via `git clone`, so discovery NEVER
  auto-spawns a server. Starting a configured server is an explicit
  user action (session layer + palette command land next).
- MCP tools are arbitrary server-side code: `tools/call` is gated at
  the same tier as Act mode (the spec itself asks for a human in the
  loop). Ask/Plan never invoke MCP tools.
- Wire framing is NDJSON, shared with the ACP transport
  (`agent_acp_process`); a single message is capped at 1 MiB, so a
  server returning a larger single result gets an error, not a hang.

## Panel surfacing (item 144)

The command palette action "MCP Servers: Connect and Show Tools"
(workbench command `start_mcp_servers`) reads `.lapce/mcp.json`, spawns
each enabled server on a worker thread (10s timeout per server, kill by
pid on timeout, always reaped), handshakes and lists tools, then opens
`.lapce/mcp/status.md` as an editor buffer: per-server status, tool
names and descriptions, or the missing-config example. Connections are
probe-and-disconnect; tool calls stay Act-gated. With no local
workspace the report opens from the temp directory instead.

## Next milestones

Session/worker (spawn + handshake + cached tool list surfaced in the
panel), then agent-loop catalog integration behind the Act review UI.
