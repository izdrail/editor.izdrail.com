# Debug mode and crash reports

Debug mode helps find out why the editor itself crashed or misbehaved. It is
about the editor's own faults (panics, language server crashes, proxy
problems), not about debugging the programs you edit. It is off by default
and normal behaviour is unchanged.

## Turn it on

Either start the editor with the flag:

```
lapce --debug
```

or set the environment variable (`1`, `true`, `yes` or `on`):

```
LAPCE_DEBUG=1 lapce
```

The flag also reaches the proxy, including a remote `lapce-proxy` started over
SSH or WSL, so remote problems are logged on the remote host.

## What debug mode does

- Trace-level logging for the editor, proxy, core and rpc crates, debug level
  for everything else. Without debug mode the log file is debug level for the
  editor crates and info for the rest, as before.
- Panic hook: every panic appends a report to `crash.log` with a UTC
  timestamp, version, process, thread, message, source location and a full
  backtrace. The backtrace is captured whether or not `RUST_BACKTRACE` is set.
  This also applies to the editor window process, which previously had no
  panic logging when it ran detached.
- Language servers: the command used to start each server, spawn failures,
  and each line the server writes to stderr (capped at 2000 characters per
  line) are logged under the server's name. A server that exits or crashes
  logs `LSP transport closed (server crashed or exited)` with the server name
  and reason. That last line is also logged without debug mode.
- Remote proxy: a dropped or crashed remote proxy connection is logged.

## Where the files are

Everything is in the logs directory, a `logs` folder inside the editor's local
data directory:

| OS | Path |
| --- | --- |
| Linux | `~/.local/share/lapce-<channel>/logs` (or `$XDG_DATA_HOME/...`) |
| macOS | `~/Library/Application Support/dev.lapce.Lapce-<channel>/logs` |
| Windows | `%LOCALAPPDATA%\lapce\Lapce-<channel>\data\logs` |

The command palette entry "Open Logs Directory" opens it. Files:

| File | Content |
| --- | --- |
| `lapce.<date>.log` | editor log, one file per day |
| `lapce-proxy.<date>.log` | proxy log (debug mode only) |
| `crash.log` | crash reports (debug mode only) |
| `stderr.log`, `stdout.log` | output of the detached editor process |

## Size limits

- Daily log files: the 10 newest are kept.
- In debug mode, at startup the oldest `*.log*` files are deleted until the
  logs directory is under 200 MB. The newest file is never deleted.
- `crash.log` rotates to `crash.log.1` at 2 MB, so at most one older copy is
  kept.

## Reporting a bug

Start the editor with `--debug`, reproduce the problem, then attach:

1. `crash.log` (and `crash.log.1` if present)
2. the newest `lapce.<date>.log`, and `lapce-proxy.<date>.log` if it exists
3. the version (`lapce --version`), your OS, and the steps to reproduce

Logs can contain file paths and project names. Read them before posting and
remove anything private.
