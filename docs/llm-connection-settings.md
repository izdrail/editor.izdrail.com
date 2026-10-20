# Set up the LLM connection (item 253)

Open Settings, select AI Chat, and use **LLM connection** above the general
settings. Endpoint defaults to https://llm.izdrail.com and model to `auto`.
Enter the provider API key in the masked field and choose Save connection.
The key is never displayed, copied by the field or put in a general editor
buffer. Entry supports typing, paste, Backspace and Escape (clear draft).
Empty entry preserves the saved key; Remove saved key explicitly deletes it.
Settings file changes are picked up by the existing config watcher.

Keys are stored by normalized completion URL in `llm-credentials.json` inside
the app config directory, separate from settings.toml and Debug serialization.
This is local plaintext, NOT an encrypted OS keychain. Unix files are created
0600 and replaced through a temporary file; other systems use their inherited
filesystem ACLs. Do not share/commit the file or a config-directory archive.
An invalid file is not silently overwritten. No real key belongs in tests.

LAPCE_LLM_API_KEY remains an environment override and takes precedence over a
saved key. Keys saved for one endpoint are not used for another endpoint.
Authenticated requests do not follow redirects. All chat, model-list,
capability-probe and tool-agent requests use the same credential selection.
401/403 guidance points here rather than asking users to set an env variable.

Test connection sends an explicit one-token generation request using the
entered endpoint/model and saved key (or env override). Save a draft key
first. It sends no project code but can use provider quota. Status reports OK,
401 Unauthorized, 403 Forbidden, another HTTP code, or network failure without
showing request details or provider response bodies. It verifies acceptance,
not model quality or later streaming/tool capabilities.

Verification: fixture-only credential persistence/isolation/permissions tests,
input bounds, override precedence and status messages. Real settings layout,
focus/paste workflow, live gateway and config reload need host-run checks.
