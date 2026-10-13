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
