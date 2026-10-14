# Go to Middleware / Job / Event

Command palette: **Go to Middleware / Job / Event** (`palette.app_classes`).

One list of the PHP classes in the standard Laravel folders, tagged by kind, e.g. `[Job] Charge  app/Jobs/Billing/Charge.php`. Type the kind or the name to filter.

| Tag | Folder |
| --- | --- |
| Middleware | `app/Http/Middleware` |
| Job | `app/Jobs` |
| Event | `app/Events` |
| Listener | `app/Listeners` |
| Observer | `app/Observers` |
| Notification | `app/Notifications` |
| Mail | `app/Mail` |
| Seeder | `database/seeders` (and legacy `database/seeds`) |
| Factory | `database/factories` |

Opens at the `class` line. If no declaration is found in the first 64 KB the file opens at line 1 under its file name. Seeders and factories are in the same list, so "seeder" or "factory" in the filter narrows to them. Static text scan only; no PHP is run. Custom folders are not covered. Limits: depth 6, 1500 files in total, 5000 entries, symlinks not followed.

Status: unit tests cover declaration parsing (modifiers, comments, enums, multibyte), per-kind discovery, the file cap, symlinks and random-input fuzz. The palette GUI is not yet verified.
