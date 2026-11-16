# Antigravity in Gravity Editor

Gravity connects to Antigravity CLI using its supported ACPv1 stdio interface.
The Microsoft Visual Studio VSIX does not run inside this editor. This is an
agent connection, not VSIX compatibility or a replacement Google service.

## Setup

1. Install Antigravity CLI from its official site and confirm `agy --version`
   and `agy acp --help` work in a terminal.
2. Sign in using `agy acp --login` in that terminal. Sign-in stays in the CLI's
   credential store. Do not paste tokens or API keys into the ACP command.
3. Open a local project in Gravity, open AI Chat's mode menu and choose
   Antigravity. The preset launches `agy acp --sandbox --disable-telemetry`
   in the current project. If PATH differs from your shell, set the absolute
   executable path in Settings > AI Chat > ACP command, preserving those args.
4. Send a prompt. Antigravity uses your CLI sign-in, permissions, model settings
   and plan limits. The self-hosted endpoint model selector is unrelated to ACP.

Gravity does not sign you in, buy credits, choose a paid plan or automatically
approve agent operations. Requests to use an agent are subject to that agent's
own permissions and your account's quota or billing rules.

## Supported and limited behavior

- Real ACPv1 handshake, new sessions, cached sign-in, streamed text, plans and
  tool status, cancellation and stored-session loading when advertised.
- File reads are confined to the canonical project root. Client-mediated writes
  are staged for review instead of immediately changing the workspace.
- Authentication methods advertised during initialize are not assumed to mean
  a login is needed. Gravity first attempts a session and reports sign-in only
  when the agent returns Authentication required.
- ACP permission approval is not yet exposed in the native UI. A permission
  request cancels the turn and receives the protocol's cancelled result. It is
  never auto-approved. Terminal capability is not advertised.
- Per-session Antigravity model/thinking-level selection, elicitation forms,
  steering and additional workspace roots are not yet supported. Configure
  supported model defaults in Antigravity CLI. Do not mistake the self-hosted
  API model dropdown for Antigravity model selection.

Compatibility was checked against Antigravity CLI1.5.9 using a clean profile:
actual initialize returned ACPv1 and `Antigravity ACP Server`; unauthenticated
session creation returned Authentication required. A signed-in model turn is
not covered by that check and requires a separate account-backed verification.

Sources checked November16, 2026:
- https://antigravity.google/docs/ide/extensions/visual-studio
- https://antigravity.google/docs/ide/extensions/acp/
- https://antigravity.google/docs/cli/acp/
- https://github.com/google-antigravity/antigravity-cli/releases/tag/1.5.0
