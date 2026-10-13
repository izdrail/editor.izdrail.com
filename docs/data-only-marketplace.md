# Themes & Icons marketplace

Based on editor.izdrail.com main 9f04242. Executable plugins, WASI, LSP and DAP
remain disabled. Marketplace installs only validated data packages.

The panel returns in new and saved layouts. Listing consumes registry pages using
raw server counts, filters WASM entries, and handles query changes without stale
results overwriting new ones. Query strings are URL-encoded. Installation rejects
registry `wasm=true` and any raw manifest `wasm` key, including declarations whose
file is missing. Local discovery also rejects raw WASM declarations before the
old normalizer can erase them. Updates cannot bypass these checks.

Downloads are HTTPS-only, bounded to 32 MiB, with a 30-second request timeout and
no HTTP redirects. Allowed hosts are plugins.lapce.dev and the current exact
registry R2 storage host. If storage moves, the allowlist needs updating rather
than allowing arbitrary hosts. Compressed archive formats are gzip/zstd.

Extraction is staged beside the final package directory. Files must be regular
data files (TOML/images/Markdown/text/license); absolute/traversal paths, links,
special files, duplicate file entries and executable suffixes are rejected.
Limits: 20,000 entries, 128 MiB total declared file size, 144 MiB decompressed
archive input. Raw manifest identity must match selection. At least one color or
icon theme with a name must exist, and referenced resources must remain confined.
Icon theme resource maps are checked relative to their TOML directory.

Only after validation is the new directory renamed into place. Updates rename
old content to a backup first, then rename staged content; failure rolls back.
Each rename is atomic on the same filesystem, but the two-rename update is NOT a
crash-atomic transaction: a process/system crash between renames can leave the
old directory under the hidden staging backup. A per-package lock blocks parallel
installers; a crash may leave a stale .<package>.install-lock requiring removal.
Recovery paths are preserved/reported if rollback fails. No claim of full
crash-recovery journaling or authenticity/signature verification is made.

Install failures reset the spinner and appear in the existing message UI. Success
refreshes config, making themes selectable in the existing Core theme dropdowns.
Global/workspace enable/disable and reload refresh data themes rather than routing
them to the disabled executable backend; workspace disable persistence is fixed.
Uninstall errors are surfaced, refreshes happen on success, and development themes
outside the managed package directory cannot be deleted by this UI path.

## Tests

- `cargo check --workspace -j 2`: passes.
- `cargo test -p lapce-proxy --lib -j 1`: 18 pass, 1 explicit network smoke ignored.
- `cargo test -p lapce-app --lib -j 1`: 23 pass, 1 explicit real-package smoke ignored.
- Installer tests cover raw WASM/missing file, invalid identity, outside resources,
  executable files, links/special files, oversized headers, duplicate files,
  truncated archives, old-package preservation, lock cleanup, safe hosts and sizes.
- Existing app loader test loads color/icon TOML through
  LapceConfig::load_plugin_color_themes/load_plugin_icon_themes and honors disable.
- Explicit smoke: downloaded panekj/vscode-themes 2022.11.0 and
  panekj/lapce-material-icon-theme 0.0.1-beta1 from the live registry, installed
  only into temporary test directories, and successfully loaded both with the app's
  real data-only loaders. No user profile or remote resource changed.

The UI has NOT been visually inspected. Before release verify actual panel
layout, search/list rendering, install/error spinner, selecting a downloaded
color/icon theme, update, disable and uninstall. Network tests are ignored by
normal CI to keep it deterministic. Tests do not prove publisher trust; downloaded
assets remain untrusted data handled by TOML/image parsers.

Sources: exact new repo source https://github.com/izdrail/editor.izdrail.com;
live registry https://plugins.lapce.dev/api/v1/plugins?offset=0;
https://plugins.lapce.dev/api/v1/plugins/panekj/vscode-themes/2022.11.0/download;
https://plugins.lapce.dev/api/v1/plugins/panekj/lapce-material-icon-theme/0.0.1-beta1/download;
https://docs.lapce.dev/development/theme;
https://docs.lapce.dev/get-started/themes.

Apply: `git am 0001-restore-data-only-marketplace.patch` on main 9f04242.
Patch author Stefan Bogdanescu <bogdan.izdrail@gmail.com>. No push.
