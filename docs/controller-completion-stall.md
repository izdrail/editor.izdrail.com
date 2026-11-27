# Controller completion stall investigation

Status: mitigation, not a reproduced fix for the reported freeze after 2-3 characters in a Laravel controller.

## Narrow change

Completion and framework-symbol hover now borrow the existing syntax-worker project profile. They no longer discover the project by opening sibling PHP headers, walking ancestor directories, and reading Composer files on the UI thread. No new TTL cache is introduced.

`Syntax::init` discovers the profile on open. The syntax worker refreshes it on the next text parse. Changes to composer.json, composer.lock, or artisan do not independently refresh an already-open document until that parse. A default profile means no detected framework.

The completion callback borrows syntax immutably around a separate immutable buffer borrow. The callback only uses the borrowed profile; it does not update syntax. The hosted compiler remains the acceptance gate.

## Evidence and limits

The pre-change c96b1eb Linux release remained responsive in disposable local GUI checks with a Laravel controller, a 222KB lockfile, a 2,000-file workspace (about 20,000 symbols), and controllers up to about 1MB. Tested prefixes included req, request, $request->inp, $this->, response, and Route::ge. These checks do not reproduce the user's setup or establish the root cause.

Hosted GUI acceptance types controller prefixes and requires exact saved text within two seconds per prefix. A failure should be rerun once before deciding whether it is a persistent regression. The latency log is retained with the GUI artifacts.

The ignored hosted benchmark requires the real installed Laravel fixture vendor tree and a lockfile over 100KB. It measures project discovery versus borrowing a profile only. It is not an end-to-end completion speedup measurement.

## Remaining work

Context-gated cold-cache disk scans remain in project completion and class/model/route/template/reference/hover helpers. Move the likely controller paths to background work with revision/session guards in a separate reviewed change. Whole-prefix lexing is also still synchronous. Exact trigger, installed build, and operating system are still needed to reproduce the original report.
