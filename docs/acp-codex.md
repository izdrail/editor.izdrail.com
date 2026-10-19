# Codex over ACP

[Upstream setup and authentication](https://github.com/agentclientprotocol/codex-acp/).

Node/npm is required. The published adapter package includes a compatible
Codex dependency. Upstream documents ChatGPT and API-key authentication;
a separate global Codex install is not required by that package.

## Editor setup

Open a local project and use the composer mode dropdown. Choose
"Agent: Codex". The picker writes `acp-command` in the `[ai-chat]`
section of settings.toml, updates the live configuration, and selects
ACP mode. It launches:

```text
npx @agentclientprotocol/codex-acp
```

The canonical commands are in `chat::ACP_AGENTS`; see
[ACP agent picker](acp-agent-picker.md). Missing programs are reported
when the subprocess is spawned. Confirm your CLI authentication using
the upstream instructions before starting a prompt.

## Access and review

The current session advertises workspace-confined file reads and staged
file writes. Proposed writes require review before application; terminal
client capability is disabled. This is the behavior in
`agent_acp_session::AcpSession::start_resumable`, not a claim that an
external agent subprocess is sandboxed. Run only agents you trust.

## Verification limits

Sources and launch configuration checked on 19 October 2026. These are
setup instructions, not a claim of a successful authenticated end-to-end
run with this agent. Credentials belong in the agent's supported local
authentication or environment configuration, never in project source.
