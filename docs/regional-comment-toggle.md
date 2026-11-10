# Mixed-language comment toggles

HTML, Vue and mixed PHP files choose comment delimiters from the current lexer region instead of the file extension alone. JavaScript bodies use `//`, CSS bodies use `/* ... */`, and HTML tags use `<!-- ... -->`.

Selections must resolve to one delimiter pair. A selection spanning both tags and a script/style body is left unchanged, as is ambiguous plain text. Select the body alone when commenting embedded code. Blade and Twig retain their existing file-level comment behavior; this change does not claim region support for them.

Ordinary edits do not run a new lexer pass. A comment toggle reuses a current syntax snapshot; a stale snapshot gets a bounded fresh pass for files up to 1 MiB. Larger stale files are left unchanged until highlighting catches up.

The preserved lexer23 v8 patch is the starting point. Current gates include real edit-consumer round trips, stale snapshot and language switch controls, mixed-region no-ops and actual GUI toggles in HTML script/style bodies.
