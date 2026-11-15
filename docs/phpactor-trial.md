# Phpactor language-server trial (rank 12)

Rank 12 of the specialization plan is an opt-in phpactor language server for
PHP workspaces. The plumbing landed earlier (managed download of a pinned
phpactor phar, registry seeding, per-workspace opt-in via
`language-servers.toml`, Laravel-aware gating); this document records a live
trial of that plumbing against the rank-12 gates and the gaps it exposed.

## Trial setup

- Fixture workspace: `index.php` + `src/Greeter.php`, a `Trial\Greeter` class
  with a docblock and one `greet(string $name): string` method, opened with a
  fresh `$HOME` so only the seeded `language-servers.toml` opted the workspace
  in.
- The editor downloaded the managed phar itself:
  `~/.local/share/lapce-nightly/language-servers/phpactor/phpactor-2026.10.07.0.phar`,
  SHA-256 verified against the pin in `lapce-proxy/src/lsp/registry.rs`
  (`b28bf616...db16b`).
- Host note: this machine's `php` on `PATH` is a Docker wrapper without
  stdio forwarding, so the first spawn died immediately. The editor surfaced
  that correctly as a "php: Language server exited" toast and kept working
  with lexical highlighting - the offline fallback is intact when a server
  fails to start. The trial continued with a real static PHP 8.4 CLI first on
  `PATH`.

## What worked

- Managed download and checksum verification of the pinned phar.
- Server spawn and initialize once a real `php` is on `PATH`.
- Hover: `$greeter` shows `variable greeter: Greeter` (type inferred through
  the `new` expression); `Trial\Greeter` shows the class signature with its
  docblock, resolved across files. None of this exists in the offline packs.
- Completion: `$greeter->` + Ctrl+Space offers the `greet` method.
- Diagnostics: an incomplete `$greeter->` statement produced a red squiggle
  and a status-bar error count via `publishDiagnostics`.
- Coexistence: the offline local hover providers (Blade, Tailwind, config
  values, routes, schema, symbols, ...) still answer first for what they
  cover; the server answers the rest. Lexilla highlighting is unchanged.

## Gaps found and fixed in this item

- Stale hover results: the app sent every hover request with id `0` and
  applied whatever response arrived last, so a slow response could overwrite
  a newer hover. Requests now carry a real id (`HoverData::request_id`),
  responses to superseded requests are dropped (`response_is_current`), and
  the response's echoed id must match.
- Hover never dismissed on pointer leave: `pointer_leave` only cancelled the
  hover timer, so the tooltip stayed up over other panels and swallowed the
  next click. It now also deactivates the hover and invalidates in-flight
  requests when the hover belongs to that editor.
- Tests cover id supersession, invalidation, and the current/stale decision.

## Server version reporting (closed in item 450)

- The initialize response's `serverInfo` is now kept on the proxy client and
  reported in a "php: phpactor 2026.10.07.0 ready" toast when the server
  starts, completing the Preparing/ready/error lifecycle. Servers without
  `serverInfo` fall back to the registry id (`ready_message` in
  `lapce-proxy/src/lsp.rs`, covered by tests).

## Gate checklist (rank 12)

- Offline fallback unchanged: yes - all local providers and Lexilla are
  untouched in behaviour; a dead server degrades to exactly the pre-trial
  experience.
- Opt-in processes: yes - the server only starts for workspaces that opt in;
  the phar download is on demand, pinned, and checksum-verified.
- Source/version reporting: yes - download provenance is pinned and
  logged, and the running server's name/version is reported on start
  (item 450).
- Stable lexical fallback while semantic results arrive: yes - highlighting
  is immediate and independent of server readiness.
- Stale-result rejection: diagnostics already rejected stale versions
  (`window_tab.rs` rev check); hover now rejects superseded responses (this
  item). Completion already rejected stale request ids.
