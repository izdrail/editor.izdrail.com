# PHP/web specialization: first static milestone

Base: main `a499bd6489ccdb656fe538311bbd4b1c593dee27`.

## Delivered

- Typed snapshot properties, applied before lexing, with language validation and
  session-local state. SCSS uses the existing LexCSS property. JS/TS use LexCPP
  with dollar identifiers, template strings and no C preprocessor.
- PHP and Laravel scanners stop line comments at `?>`, use non-nesting PHP block
  comments, bound trailing escapes and expose builtin function scopes. Laravel
  no longer guesses that every PascalCase static call is a facade.
- A shared static project profile reaches syntax and completion. Nearest Composer
  roots are boundaries; lockfile versions replace constraints; Laravel/Symfony/WP
  evidence can coexist. Standalone WP plugin/theme headers and core versions are
  recognised. Reads are bounded; no PHP, Composer or console command is executed.
  Project metadata readers ignore special files (including named pipes), retain
  regular-file symlinks and cap reads on the opened handle. The opened PHP file
  is checked for a plugin header before scanning up to 128 neighboring entries,
  so large directories do not hide that direct evidence. JSON over 1 MiB and
  WordPress versions over 8 KiB are ignored; plugin/theme headers use the first
  8 KiB. The regular-file precheck is not an adversarial filesystem-race sandbox.
  Evidence is reread, so config changes/removal apply on the next request. There
  is no TTL cache, override UI or filesystem watcher in this milestone.
- A Twig lexer (id 146) and `.twig` mode. Region delimiters, comments, quotes,
  balanced expression punctuation, whitespace controls and verbatim are covered.
  HTML plus recognised inline JS/CSS is composed at identical UTF-8 byte offsets.
- Conservative script/style delegation for HTML, Blade and mixed PHP. Script MIME
  modes other than JS/module and unknown modes remain with the host. Blade bodies
  containing template syntax remain with the host rather than destroying template
  styles. This is not full embedded-language parsing.
- Deterministically generated WordPress global-function, Symfony component class,
  and Twig tag/filter/function/test packs. Full source provenance and input hashes
  are in `metadata/web/manifest.json`; source notices are retained in
  `metadata/web/licenses/`. No KDE, old WP TextMate, GPL hook or proprietary server
  material was imported.
- Profile-aware popup entries show pack version, WP signatures and reference-index
  links. PHP string/comment/member contexts suppress global API completion; Twig
  filter/test/tag/function contexts select distinct packs. Lexer lists are a lossy
  projection, not the symbol store. This is not receiver/type resolution.

## Compatibility policy

Pinned fallbacks: WP stubs **6.8.3**, Symfony components **7.3.0**, Twig **3.21.1**.
These are explicit baseline targets, not claims about latest versions.
Symfony classes are offered only for exact installed package versions. WP known
core versions must match the pack; standalone plugins with unknown core version
get a labelled baseline. Twig syntax/builtins target the shipped 3.21.1 pack;
project extension merging and Twig installed-version selection are not implemented.
PHP scanner syntax targets the existing PHP 8 lexical subset, not full PHP 8.x
compiler parity. CSS/SCSS preserve the vendored lexer's documented limitations.
JS/TS remain a C-family lexical baseline. JSX/TSX deliberately keep the existing
TextMate fallback instead of calling C++ colouring JSX support.

## Regeneration

Check out these exact sources without running their scripts:

- https://github.com/php-stubs/wordpress-stubs at
  `abeb5a8b58fda7ac21f15ee596f302f2959a7114` (v6.8.3, MIT)
- https://github.com/twigphp/Twig at
  `285123877d4dd97dd7c11842ac5fb7e86e60d81d` (v3.21.1, BSD-3-Clause)
- https://github.com/symfony/symfony at
  `5babe8da13d379288ec141f9b5e325a1a6c39611` (v7.3.0, MIT)

Run `python3 scripts/generate-web-metadata.py WP_DIR TWIG_DIR SYMFONY_DIR`.
It checks source commit IDs and extracts audited declaration forms as text, not
by loading PHP. The generator is deliberately not an AST parser. WP methods and
non-global namespaces are excluded; Symfony source fixtures/bridges/bundles are
excluded. Exact source commits are required; changed declaration syntax must be
reviewed before updating. Run twice and compare generated hashes.

## Verification and remaining work

Headless tests cover typed-property isolation, PHP comment/function regressions,
nearest-root/version/removal behavior, standalone WP headers/version changes,
metadata separation, Twig fixtures, incomplete edits/UTF-8/CRLF, routing and popup
contexts. Existing first-catalogue concurrency and snapshot tests remain enabled.

Not complete: shared PHP scanner consolidation/complex interpolation parity,
AST metadata extraction, generated PHP standard-library pack, arbitrary package
versions, Doctrine and third-party bundles, Twig extensions/interpolated string
subexpressions, contextual WP hooks/callback signatures, schemas/service/template
navigation, local symbol shadowing/import-aware insertion, richer snippets,
JSX/TSX grammar integration, incremental performance, overrides/watcher cache,
LSP or AI features. Performance and real UI/theme/viewport verification are still
required. No patent clearance or exhaustive framework coverage is claimed.

## WordPress header detection

Plugin/theme labels follow WordPress's line-prefix, case-insensitive matching
and CR-only line handling. Empty labels and label mentions in ordinary PHP
strings do not establish a WordPress project. Closing comment/PHP tags are
removed before checking whether the value is empty. This remains a bounded
static detector, not PHP parsing.

Sources: https://developer.wordpress.org/reference/functions/get_file_data/
and https://developer.wordpress.org/reference/functions/_cleanup_header_comment/ .

## Twig negated test completion

Both `is` and `is not` select the test catalogue in unfinished Twig expressions
and statement tags. Whitespace/newlines between the operator words are accepted;
strings and comments still suppress completion. This changes the category of
static completion only, not expression parsing or type inference.
Reference: https://twig.symfony.com/doc/3.x/templates.html .

## Twig inline comments

Inside Twig expressions and statement tags, `#` comments run to the next newline.
Closing delimiters inside the comment are ignored; scanning resumes on the next
line. Hash characters in quoted strings remain string text, and comment text
suppresses the static completion popup. This follows Twig 3.15+ behavior within
the shipped Twig 3.21.1 target.
Reference: https://twig.symfony.com/doc/3.x/templates.html .

Twig completion also uses lexer styles when selecting its open region and
operator context. Comment bytes act as whitespace, and quoted/commented fake
openers cannot choose the region. Test/filter completion therefore survives an
inline comment between the operator and the name being typed.

PHP global fallback completion treats block comments as whitespace before its
member-access check. A comment between `->`, `?->` or `::` and the typed member
name does not enable the global API catalogue. Ordinary global contexts remain
available, including after a completed comment. This is context suppression,
not receiver-type completion.

PHP/framework highlighting word lists are built only for pure PHP lexer routes.
Other languages and mixed HTML/PHP do not assemble that unrelated catalogue;
their existing lexer word lists and routing remain unchanged.

Syntax jobs already marked cancelled return before project metadata scanning or
full-document highlighting. Checks after highlighting and folding remain, so
cancellation that arrives during either step still prevents snapshot publication.

PHP global API fallback is also suppressed while typing a dollar-prefixed
variable name. The independent buffer-variable source remains available; normal
function contexts after an assignment or completed expression are unchanged.

WordPress plugin/theme header detection tolerates an incomplete UTF-8 character
at the 8 KiB scan boundary. The read limit and ASCII header matching stay the
same; unrelated invalid bytes no longer hide a valid header.

Regular plugin files larger than 1 MiB still supply header evidence for themselves
and neighboring PHP files. Only the first 8 KiB is read; JSON/version size limits,
regular-file checks and the 128-neighbor bound are unchanged.

PHP completion context follows PHP's byte-based identifier rule for non-ASCII
text. Combining marks and other UTF-8 characters no longer bypass variable or
member guards; Twig's identifier handling is unchanged.

The PHP buffer-variable source also keeps full non-ASCII UTF-8 names, including
combining marks. Labels, filtering and insertion preserve the same bytes; ASCII
digit starts and the special `$this` variable remain excluded.

The pure-PHP completion guard treats a cursor directly after a closing `?>` tag
as outside PHP. Reopened PHP blocks resume the global catalogue normally.

Twig dot-attribute positions no longer offer the global function catalogue.
Filters/tests after an attribute, range operands and dynamic attribute
expressions keep their existing completion categories.

Twig `apply` tags offer filters for their initial name, including whitespace
controls and inline comments. Function calls inside filter arguments still use
the function category.

Immutable Twig lexer word lists are initialized once and shared by highlighting
and completion across threads. Project evidence remains reread on each request;
this does not introduce a project-metadata cache.

Rejected PHP completion contexts return before fallback catalogue allocation.
Accepted contexts keep PHP, Laravel and WordPress entries unchanged.

Twig numeric highlighting keeps the two-dot range operator separate from its
numeric operands. Decimal fractions and digit separators retain number styling.

Twig numbers include complete e/E exponents with optional signs and digit
separators. Incomplete exponent suffixes stay outside the numeric token, and
range operators remain separate.

Twig digit separators stay numeric only between digits. Trailing or repeated
underscores are left outside integer, fraction and exponent tokens.

Twig verbatim regions require a complete `endverbatim` closing tag, including
the closing delimiter. Tag-like raw text with extra tokens or an incomplete
closing tag no longer ends the region early; whitespace controls still work.
