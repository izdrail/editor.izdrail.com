# VS Code command shim experiment

This is a proof of concept, not VS Code extension compatibility. It deliberately
leaves the existing Themes & Icons / VSIX importer data-only. No marketplace JS
is discovered, installed as executable code, auto-activated or executed. No WASM,
WASI, Node, npm, language server, debugger or browser host is reintroduced.

## Try it

In the editor's command palette run `Experimental VS Code: Run sandbox hello`.
It loads the bundled CommonJS example with a real VS Code-shaped `package.json`,
registers `sandbox.hello`, reads the declared configuration default, executes the
command and sends `Hello from the sandbox` to the editor's existing message UI.
This is the only GUI execution entry. It is an explicit opt-in on every invocation.

For a local developer package, using the same runtime:

```sh
cargo run -p lapce-vscode-experiment -- /path/to/package sandbox.hello
```

The package must contain package.json and a confined .js `main` entry. Only one
CommonJS JS file is supported. Nothing is downloaded. `examples/hello` is a usable
sample. The CLI emits JSON `{value, messages}`. Run with no arguments for the
bundled demo. The worker mode is an internal subprocess entry, not an installer.

## Exact API subset

- `require('vscode')` only. Every other require rejects, including relative files.
- `commands.registerCommand(id, callback)` only for unique IDs declared in
  `contributes.commands`; returns a disposable.
- `commands.executeCommand(id, ...args)` only for commands registered by this
  extension in this invocation. It cannot invoke editor commands.
- `commands.getCommands()` returns only those extension commands.
- `window.showInformationMessage`, `showWarningMessage`, `showErrorMessage` take
  a string; return a Promise of undefined. No action buttons, modal or options.
- `workspace.getConfiguration(section).get(key, fallback)` and `.has(key)` expose
  ONLY package-declared defaults, never user settings. `workspace.isTrusted=false`.
- `activate(context)` gets `subscriptions` and `extensionMode=2` (development).
  There is no path, storage, secrets, workspace, document or environment access.
- JSON return values and Promise-based commands work. No timers or ongoing jobs.

Each execution activates anew, executes exactly one command and discards the
runtime. `activationEvents` is not a general dispatcher. `deactivate` is not run;
there is no persistent extension lifecycle or saved state. Declarative command
labels/defaults are interpreted; themes, snippets, languages and other
contributions belong to the separate declarative importer and are not duplicated.

Unsupported APIs fail rather than falling through to editor functionality:
filesystem, network, processes, terminals, built-in commands, settings updates,
text edits, documents, language providers, LSP, debugger, webviews, trees,
status bar, extension storage, native npm modules, ESM, multiple JS files,
import loaders and WebAssembly. This does not run typical complex VS Code plugins.

## Isolation and limits

The JS engine is pinned rquickjs/QuickJS 0.11.0, with default features disabled:
no module loader, dynamic native library loading or host bindings. JS gets no
process, std, os, fetch, WebAssembly or Node globals. This is native embedded JS,
not JS compiled to WASM. The editor uses its own executable as a short-lived
worker; the CLI uses its own executable. Packages cannot select the worker.

Workers clear their environment and close all inherited descriptors above stderr
before loading JS (requires Linux close_range support). The GUI permits only
one experiment worker at a time. Before reading input or creating the JS runtime,
Linux x86_64 workers set no-new-privileges and a seccomp syscall allowlist. File
opens, sockets, exec, clone, ptrace and filesystem mutation are denied. Allowed
syscalls cover pipe I/O, memory management, signals, futex, clock, randomness,
process IDs and exit. The worker has no new file/network/process capability.
Other operating systems/architectures fail closed; there is no unsandboxed fallback.
A denied/unsupported seccomp setup fails the command.

- 16 MiB JS heap and 512 KiB JS stack.
- Linux address-space limit 256 MiB, CPU limit 2 seconds, core dumps disabled.
- Parent kills and reaps workers after 2 seconds wall time. No interrupt-handler
  reliance: adversarial interruption triggered a QuickJS cleanup abort in early
  testing, so execution is never in the editor/UI process.
- Each manifest/JS input at most 1 MiB; at most 64 declared commands.
- Worker request at most 3 MiB; arguments/results at most 64 KiB.
- Messages at most 64, each at most 2048 JS code units, validated again by parent.
- Parent pipe output reader is bounded; output is parsed as data, never commands.

## Risk model and remaining work

Untrusted JS is expected to loop, allocate, throw, queue jobs and call missing
APIs. It can abort its worker or waste its time budget, but not crash the editor
through a normal JS exception. seccomp adds a real OS boundary against file,
network and process syscalls even if a native engine defect is exploited. This
is defense in depth, not a claim of audited containment: kernel vulnerabilities,
side channels, native engine bugs, inherited descriptors, concurrency/resource
exhaustion and loader/runtime behavior remain review concerns. Currently the GUI
runs only the bundled fixture, not arbitrary downloaded code. Do not expand that
entry to marketplace packages without further sandbox and publisher review.

No compatibility, publisher authenticity or complete sandbox audit is claimed.
The extension-directory loader resolves and confines paths but is not an atomic
filesystem snapshot against concurrent malicious file replacement. Use local
controlled packages for the CLI experiment.

Tests exercise the actual worker executable: CommonJS load, declarative command,
config/message output, async results, fresh-state isolation, blocked Node/host
capabilities, blocked editor commands, input/output/heap/message limits and
infinite loops in top-level, activation, command and Promise jobs. A native
policy test confirms denied open/socket/exec/clone syscalls in a disposable child.
GUI pixels and interaction require a separate visual gate before calling the
editor demo visually verified. No container definition exists in this repo.

Research baseline:
https://github.com/izdrail/research.izdrail.com/tree/main/research/vscode-extensions-lapce
