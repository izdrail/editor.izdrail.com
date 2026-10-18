# ACP agent picker (item 191)

The composer mode dropdown now lists three first-class agents under
"Agent: ..." rows: opencode (`opencode acp`, native ACP), Claude Code
(`npx @agentclientprotocol/claude-agent-acp`) and Codex
(`npx @agentclientprotocol/codex-acp`) via the official Agent Client
Protocol adapters (agentclientprotocol.com/get-started/agents).
Picking one writes `acp-command` to settings.toml (`ai-chat` section;
the config file is watched) and to the live config, and switches the
composer to ACP mode. The active agent's row is highlighted by
comparing the configured command. The canonical table lives in
`chat::ACP_AGENTS`; a test asserts every entry splits into a valid
program + args through the same `split_command` the ACP transport
uses, so a picker entry can never be a command the transport would
refuse. Missing binaries surface at spawn time as a named error (the
ACP transport's spawn-failure path is already covered).

Note: the settings file previously had no UI path to `acp-command` at
all - it was settings.toml-only. This picker is the first UI writer.
