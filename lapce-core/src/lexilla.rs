//! Lexilla configuration and viewport theme conversion for the editor.
use crate::language::LapceLanguage;
use lapce_rpc::style::{LineStyle, Style};
use lapce_xi_rope::Rope;
use lexilla_rs::Snapshot;
use std::sync::Arc;

/// PHP files starting with `<?php` (after optional BOM, whitespace and
/// shebang) use the standalone lexer unless they emit non-whitespace inline
/// HTML. Mixed templates use hypertext so HTML/script/style keep their scopes.
pub fn is_pure_php(source: &str) -> bool {
    let s = source.trim_start_matches('\u{feff}').trim_start();
    // PHP CLI entry scripts (Laravel's artisan, Symfony's bin/console) start
    // with a `#!` shebang line before the `<?php` open tag.
    let s = s
        .strip_prefix("#!")
        .and_then(|rest| rest.split_once('\n').map(|(_, after)| after))
        .map(str::trim_start)
        .unwrap_or(s);
    if !s.get(..5).is_some_and(|p| p.eq_ignore_ascii_case("<?php")) {
        return false;
    }
    // Most source files never close PHP, so avoid a second lex there. A
    // literal substring is not enough: strings, block comments and heredocs
    // can contain ?> without leaving PHP. Reuse the standalone lexer's
    // close-tag and inline-HTML states rather than another PHP scanner.
    if !s.contains("?>") {
        return true;
    }
    let Ok(snapshot) = Snapshot::lex("php", s, &[]) else {
        return false;
    };
    let mut closed = false;
    for (byte, style) in s.bytes().zip(snapshot.styles().iter().copied()) {
        if style == 20 { // SCE_PHP_CLOSE_TAG
            closed = true;
        }
        if closed && style == 21 && !byte.is_ascii_whitespace() {
            return false;
        }
    }
    true
}

/// Byte offsets just after real PHP close tags in a source suffix beginning
/// with `<?`. Strings, comments, heredocs and nowdocs use the same states as
/// highlighting. Call once per bounded document, not once per PHP block.
pub fn php_close_tag_offsets(source: &str) -> Option<Vec<usize>> {
    if !source.starts_with("<?") {
        return None;
    }
    let snapshot = Snapshot::lex("php", source, &[]).ok()?;
    let styles = snapshot.styles();
    Some(styles.iter().enumerate().filter_map(|(i, &style)| {
        (style == 20 && styles.get(i + 1) != Some(&20)).then_some(i + 1)
    }).collect())
}

/// Word lists for the `laravel` lexer: the four PHP lists, then facades, route
/// methods, Artisan commands, helpers and Eloquent methods.
pub const LARAVEL_WORD_LISTS: &[&str] = &[
    PHP_WORD_LISTS[0],
    PHP_WORD_LISTS[1],
    PHP_WORD_LISTS[2],
    PHP_WORD_LISTS[3],
    "Artisan Auth Blade Broadcast Bus Cache Config Cookie Crypt Date DB Event Exceptions File Gate Hash Http Lang Log Mail Notification Password Process Queue RateLimiter Redirect Redis Request Response Route Schedule Schema Session Storage URL Validator View Vite",
    "get post put patch delete options any match resource apiResource resources apiResources middleware prefix name domain group where whereNumber whereAlpha whereUuid controller view redirect permanentRedirect fallback singleton namespace scopeBindings withoutMiddleware",
    "about cache:clear cache:forget cache:table config:cache config:clear db:seed db:wipe down env event:cache event:clear key:generate make:cast make:channel make:command make:component make:controller make:event make:exception make:factory make:job make:listener make:mail make:middleware make:migration make:model make:notification make:observer make:policy make:provider make:request make:resource make:rule make:scope make:seeder make:test make:view migrate migrate:fresh migrate:install migrate:refresh migrate:reset migrate:rollback migrate:status model:prune optimize optimize:clear queue:failed queue:flush queue:listen queue:restart queue:retry queue:work route:cache route:clear route:list schedule:list schedule:run schedule:work serve storage:link test tinker up vendor:publish view:cache view:clear",
    "abort abort_if abort_unless action app app_path auth back base_path bcrypt broadcast cache class_basename collect config config_path cookie csrf_field csrf_token database_path dd decrypt dispatch dispatch_sync dump e encrypt env event fake info lang_path logger method_field now old policy public_path redirect report request rescue resolve resource_path response retry route session storage_path tap throw_if throw_unless today trans trans_choice url validator value view with",
    "all avg chunk count create delete distinct doesntExist doesntHave exists find findOrFail findMany first firstOrCreate firstOrFail firstOrNew firstWhere forceDelete from get groupBy has having inRandomOrder insert join latest leftJoin limit lockForUpdate max min offset oldest orderBy orderByDesc orWhere paginate pluck query rightJoin select selectRaw simplePaginate skip sum take toSql update updateOrCreate upsert value when whereBetween whereDate whereHas whereIn whereNotIn whereNotNull whereNull whereRaw with withCount withTrashed without onlyTrashed restore hasMany hasOne belongsTo belongsToMany morphTo morphMany morphOne hasManyThrough hasOneThrough",
];

/// Compatibility entry point. Syntax and completion use the full profile.
pub fn is_laravel_project(file: &std::path::Path) -> bool {
    crate::project_profile::detect(file).laravel
}

/// Word lists for the standalone `php` lexer: keywords, builtin functions,
/// builtin types, other keywords.
pub const PHP_WORD_LISTS: &[&str] = &[
    "__halt_compiler abstract and array as break callable case catch class clone const continue declare default die do echo else elseif empty enddeclare endfor endforeach endif endswitch endwhile enum eval exit extends final finally fn for foreach function global goto if implements include include_once instanceof insteadof interface isset list match namespace new or print private protected public readonly require require_once return static switch throw trait try unset use var while xor yield",
    "array_key_exists array_keys array_map array_merge array_filter array_pop array_push array_reduce array_shift array_slice array_unique array_values count explode implode in_array is_array is_callable is_int is_null is_numeric is_object is_string json_decode json_encode preg_match preg_match_all preg_replace preg_split sprintf str_contains str_replace str_starts_with strlen strpos strtolower strtoupper substr trim ucfirst var_dump printf file_get_contents file_put_contents sort usort ksort intval floatval strval date time microtime sleep define defined",
    "bool boolean int integer float double string array object callable iterable mixed void null never self parent static true false resource",
    "__CLASS__ __DIR__ __FILE__ __FUNCTION__ __LINE__ __METHOD__ __NAMESPACE__ __TRAIT__ PHP_EOL PHP_INT_MAX PHP_VERSION E_ALL",
];

/// Shared by plain TypeScript and the TSX host-code pass.
pub const TYPESCRIPT_KEYWORDS: &str = "abstract accessor any as asserts async await bigint boolean break case catch class const constructor continue debugger declare default delete do else enum export extends false finally for from function get if implements import in infer instanceof interface intrinsic is keyof let module namespace never new null number object of override package private protected public readonly require return satisfies set static string super switch symbol this throw true try type typeof undefined unique unknown var void while with yield";

pub fn highlight(language: LapceLanguage, text: &Rope) -> Option<Arc<Snapshot>> {
    highlight_in(language, text, false)
}

/// Like [`highlight`]. `laravel` says the file belongs to a Laravel project,
/// which selects the `laravel` lexer for pure-PHP files instead of `php`.
pub fn highlight_in(
    language: LapceLanguage,
    text: &Rope,
    laravel: bool,
) -> Option<Arc<Snapshot>> {
    let project = crate::project_profile::ProjectProfile {
        laravel,
        ..Default::default()
    };
    highlight_profile(language, text, &project)
}

pub fn highlight_profile(
    language: LapceLanguage,
    text: &Rope,
    project: &crate::project_profile::ProjectProfile,
) -> Option<Arc<Snapshot>> {
    use LapceLanguage::*;
    let laravel = project.laravel;
    let source = text.to_string();
    if language == Twig {
        return crate::web_templates::twig(&source).map(Arc::new);
    }
    if language == Jsx || language == Tsx {
        return crate::web_templates::jsx(&source, language == Tsx).map(Arc::new);
    }
    let php_lists = crate::web_metadata::php_lists(project);
    let php_refs: Vec<&str> = php_lists.iter().map(String::as_str).collect();
    let (lexer, keywords): (&str, &[&str]) = match language {
        // C and C++ share the lexer but not the language: .c files must not
        // paint C++-only keywords (real C code uses them as identifiers - redis
        // src/sds.c 7.4.0 declares `sdstemplate(const char *template, ...)`), and
        // modern C keywords (restrict, _Atomic, static_assert, typeof, ...)
        // were missing entirely (CPython 3.13 and redis src/atomicvar.h).
        C => (
            "cpp",
            &[
                "_Alignas _Alignof _Atomic _BitInt _Bool _Complex _Decimal128 _Decimal32 _Decimal64 _Generic _Imaginary _Noreturn _Static_assert _Thread_local alignas alignof asm auto bool break case char const constexpr continue default do double else enum extern false float for goto if inline int long nullptr register restrict return short signed sizeof static static_assert struct switch thread_local true typedef typeof typeof_unqual union unsigned void volatile while",
            ],
        ),
        Cpp => (
            "cpp",
            &[
                "alignas alignof asm auto bool break case catch char class const constexpr continue default delete do double else enum explicit export extern false float for friend if inline int long namespace new noexcept nullptr operator private protected public register return short signed sizeof static struct switch template this throw true try typedef typename union unsigned using virtual void volatile wchar_t while",
            ],
        ),
        Typescript => ("cpp", &[TYPESCRIPT_KEYWORDS]),
        Javascript => (
            "cpp",
            &[
                "async await break case catch class const continue debugger default delete do else enum export extends false finally for from function if implements import in instanceof interface let new null of package private protected public return static super switch this throw true try typeof var void while with yield",
            ],
        ),
        Csharp | Java => (
            "cpp",
            &[
                "abstract as base bool boolean break byte case catch char class const continue decimal default delegate do double else enum event extends false final finally float for foreach if implements import in int interface internal is long namespace native new null object operator out override package private protected public readonly ref return sealed short sizeof static string struct super switch synchronized this throw throws true try typeof uint ulong unsafe using var virtual void volatile while",
            ],
        ),
        Python => (
            "python",
            &[
                "False None True and as assert async await break class continue def del elif else except finally for from global if import in is lambda nonlocal not or pass raise return try while with yield",
                "abs all any bool bytes dict enumerate float int len list map max min object open print range repr set str sum super tuple type zip",
            ],
        ),
        // Blade templates: HTML, Blade directives/echo blocks and PHP in one lexer.
        // Outside a Laravel project the framework word lists stay empty.
        Blade if laravel => ("blade", LARAVEL_WORD_LISTS),
        Blade => ("blade", PHP_WORD_LISTS),
        Php if laravel && is_pure_php(&source) => ("laravel", &php_refs),
        Php if is_pure_php(&source) => ("php", &php_refs),
        // Vue single-file components are HTML templates with <script>/<style>
        // blocks, which the hypertext lexer already paints; there is no PHP.
        Html | Php | Vue => (
            "hypertext",
            &[
                "a body div head html input link meta p script span style title",
                "async await const function let new return var",
                "",
                "",
                "__halt_compiler abstract and array as break callable case catch class clone const continue declare default die do echo else elseif empty enddeclare endfor endforeach endif endswitch endwhile enum eval exit extends false final finally fn for foreach function global goto if implements include include_once instanceof insteadof interface isset list match namespace new null or print private protected public readonly require require_once return static switch throw trait true try unset use var while xor yield",
            ],
        ),
        Css | Scss => ("css", &[]),
        Json | Json5 | Jsonc => ("json", &["false null true"]),
        Bash => (
            "bash",
            &[
                "case do done elif else esac fi for function if in select then time until while",
            ],
        ),
        Rust => (
            "rust",
            &[
                "as async await break const continue crate dyn else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while",
            ],
        ),
        Sql => (
            "sql",
            &[
                "alter and as asc begin between by case create delete desc distinct drop else end exists from group having in index inner insert into is join left like limit not null on or order outer primary references right select set table then union unique update values when where",
            ],
        ),
        Toml => ("toml", &[]),
        // Booleans/null get the keyword scope; the lexer only styles values
        // against this list.
        Yaml => ("yaml", &["true false yes no on off null True False TRUE FALSE Null NULL"]),
        Xml => ("xml", &[]),
        // Paint whole ATX heading lines, not just the # marker (upstream
        // default styles the marker only).
        Markdown => ("markdown", &[]),
        Diff => ("diff", &[]),
        Make => ("makefile", &[]),
        Ini => ("props", &[]),
        _ => return None,
    };
    let properties = match language {
        Scss => vec![lexilla_rs::Property::Scss(true)],
        Json5 => vec![lexilla_rs::Property::Json5(true)],
        // Comments without JSON5's unquoted keys / single quotes.
        Jsonc => vec![lexilla_rs::Property::JsonComments(true)],
        Javascript | Typescript => vec![lexilla_rs::Property::Javascript(true)],
        Markdown => vec![lexilla_rs::Property::MarkdownHeaderFill(true)],
        _ => Vec::new(),
    };
    match Snapshot::lex_with_properties(lexer, &source, keywords, &properties) {
        Ok(mut snapshot) => {
            if matches!(language, Blade | Html | Php) {
                crate::web_templates::embed_web(&source, &mut snapshot);
            } else if language == Vue {
                crate::web_templates::embed_vue(&source, &mut snapshot);
            }
            Some(Arc::new(snapshot))
        }
        Err(error) => {
            tracing::warn!(?error, lexer, "Lexilla highlighting failed");
            None
        }
    }
}

pub fn line_styles(snapshot: &Snapshot, text: &Rope, line: usize) -> Vec<LineStyle> {
    let last = text.line_of_offset(text.len());
    if line > last || snapshot.len() != text.len() {
        return Vec::new();
    }
    let start = text.offset_of_line(line);
    let end = if line == last {
        text.len()
    } else {
        text.offset_of_line(line + 1)
    };
    snapshot
        .runs(start..end)
        .filter_map(|(range, style)| {
            let scope = snapshot.scope(style)?;
            Some(LineStyle {
                start: range.start - start,
                end: range.end - start,
                style: Style {
                    fg_color: Some(scope.to_owned()),
                },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn line_conversion_uses_theme_scopes_and_utf8_byte_offsets() {
        let text = Rope::from("/* λ\n🙂 */\r\nint value = 42;\n");
        let snapshot = highlight(LapceLanguage::Cpp, &text).unwrap();
        let first = line_styles(&snapshot, &text, 0);
        assert!(
            first
                .iter()
                .any(|s| s.style.fg_color.as_deref() == Some("comment"))
        );
        let code = line_styles(&snapshot, &text, 2);
        assert!(code.iter().any(|s| s.start == 0
            && s.end == 3
            && s.style.fg_color.as_deref() == Some("keyword")));
        assert!(
            code.iter()
                .any(|s| s.style.fg_color.as_deref() == Some("constant"))
        );
        assert!(line_styles(&snapshot, &text, 999).is_empty());
    }
    #[test]
    fn configured_catalogue_modes_are_available() {
        use LapceLanguage::*;
        for language in [
            C, Cpp, Javascript, Typescript, Csharp, Java, Python, Html, Php, Css,
            Scss, Json, Json5, Jsonc, Bash, Rust, Sql, Toml, Yaml, Xml, Blade, Jsx,
            Tsx, Vue, Markdown, Diff, Make, Ini,
        ] {
            assert!(
                highlight(language, &Rope::from("text\n")).is_some(),
                "{language:?}"
            );
        }
    }
    #[test]
    fn data_and_sheet_languages_have_theme_scopes() {
        // css/json/yaml/toml/sql once registered without lexical classes, so
        // every style mapped to no theme scope and the files rendered
        // uncoloured despite being routed.
        use LapceLanguage::*;
        for (language, text, needle, scope) in [
            (Css, "a { color: red }", "color", "property"),
            (Css, "a { color: red }", "red", "constant"),
            (Css, ".b:hover { margin: 0 }", "hover", "attribute"),
            (Css, ".b:hover { margin: 0 }", "b", "type"),
            (Json, "{\"ok\": true}", "\"ok\"", "property"),
            (Json, "{\"ok\": true}", "true", "keyword"),
            (Yaml, "name: lapce\n", "name", "property"),
            (Toml, "[package]\nname = \"lapce\"\n", "[package]", "keyword"),
            (Toml, "[package]\nname = \"lapce\"\n", "name", "property"),
            (Toml, "[package]\nname = \"lapce\"\n", "\"lapce\"", "string"),
            (Sql, "SELECT id FROM users", "SELECT", "keyword"),
            (Sql, "SELECT id FROM users", "id", ""),
            // rust tags lifetimes as "annotation"; the theme's nearest key is attribute.
            (Rust, "fn f<'a>(x: &'a str)", "'a", "attribute"),
            // bash scalars/params were tagged "identifier" upstream and rendered
            // uncoloured; they map to the variable scope. Command names
            // (SCE_SH_IDENTIFIER) intentionally stay uncoloured.
            (Bash, "echo $HOME\n", "$HOME", "variable"),
            (Bash, "y=$1\n", "$1", "variable"),
            (Bash, "echo done\n", "echo", ""),
            // enum (PHP 8.1) in both the pure-php lexer and the templated
            // hypertext routing; the two keyword lists must agree.
            (Php, "<?php enum Suit {}", "enum", "keyword"),
            (Php, "<div><?php enum Suit {} ?></div>", "enum", "keyword"),
            // yaml scalar values colour as text; upstream left them default.
            (Yaml, "name: tests\n", "tests", "string"),
            (Yaml, "cron: '0 0 * * *'\n", "'0 0 * * *'", "string"),
            (Yaml, "port: 3306\n", "3306", "constant"),
            (Yaml, "flag: true\n", "true", "keyword"),
            (Yaml, "- one\n", "one", "string"),
            (Yaml, "- name: Run\n", "name", "property"),
            // JSON5 mode (routed for .json5): comments, unquoted keys,
            // single-quoted strings, trailing commas and extended numbers.
            (Json5, "// c\n{a: 'x', b: 0xA, c: Infinity,}\n", "// c", "comment"),
            (Json5, "// c\n{a: 'x', b: 0xA, c: Infinity,}\n", "a", "property"),
            (Json5, "// c\n{a: 'x', b: 0xA, c: Infinity,}\n", "'x'", "string"),
            (Json5, "// c\n{a: 'x', b: 0xA, c: Infinity,}\n", "0xA", "constant"),
            (Json5, "// c\n{a: 'x', b: 0xA, c: Infinity,}\n", "Infinity", "constant"),
            // JSONC mode: comments colour, but unquoted keys and
            // single-quoted strings stay errors - it is not JSON5.
            (Jsonc, "{// c\n\"a\": 1}\n", "// c", "comment"),
            (Jsonc, "{// c\n\"a\": 1}\n", "\"a\"", "property"),
        ] {
            let snapshot = highlight(language, &Rope::from(text)).unwrap();
            let at = text.find(needle).unwrap();
            let got = snapshot.scope(snapshot.styles()[at]);
            if scope.is_empty() {
                assert_ne!(got, Some("keyword"), "{language:?} {needle}");
            } else {
                assert_eq!(got, Some(scope), "{language:?} {needle}");
            }
        }
    }
    #[test]
    fn json_strict_mode_rejects_json5_constructs() {
        // The strict JSON routing must not adopt JSON5 semantics: comments,
        // unquoted keys and single-quoted strings stay unscoped (the lexer's
        // error style maps to no theme scope).
        use LapceLanguage::*;
        for (text, needle) in [
            ("// c\n{\"a\": 1}\n", "// c"),
            ("{a: 1}\n", "a"),
            ("{a: 'x'}\n", "'x'"),
        ] {
            let snapshot = highlight(Json, &Rope::from(text)).unwrap();
            let at = text.find(needle).unwrap();
            assert_eq!(snapshot.scope(snapshot.styles()[at]), None, "{text} {needle}");
        }
    }
    #[test]
    fn jsonc_accepts_comments_without_json5_strings_or_keys() {
        for text in ["// line\n{\"key\": 1}\n", "/* block */\n{\"key\": 1}\n"] {
            let snapshot = highlight(LapceLanguage::Jsonc, &Rope::from(text)).unwrap();
            assert_eq!(snapshot.scope(snapshot.styles()[0]), Some("comment"));
        }
        for (text, needle) in [("{bare: 1}", "bare"), ("{\"key\": 'value'}", "'value'")] {
            let snapshot = highlight(LapceLanguage::Jsonc, &Rope::from(text)).unwrap();
            let at = text.find(needle).unwrap();
            assert_eq!(snapshot.scope(snapshot.styles()[at]), None);
        }
        assert_eq!(LapceLanguage::Jsonc.comment_tokens(), ("//", ""));
        assert_eq!(LapceLanguage::Json.comment_tokens(), ("", ""));
    }

    #[test]
    fn typescript_keywords_match_tsx_without_leaking_to_javascript() {
        let text = "type Row = { readonly id: number }; declare const key: keyof Row;";
        for language in [LapceLanguage::Typescript, LapceLanguage::Tsx] {
            let snapshot = highlight(language, &Rope::from(text)).unwrap();
            for needle in ["type", "readonly", "number", "declare", "keyof"] {
                let at = text.find(needle).unwrap();
                assert_eq!(snapshot.scope(snapshot.styles()[at]), Some("keyword"),
                    "{language:?} {needle}");
            }
        }
        for language in [LapceLanguage::Javascript, LapceLanguage::Jsx] {
            let snapshot = highlight(language, &Rope::from(text)).unwrap();
            for needle in ["type", "readonly", "number", "declare", "keyof"] {
                let at = text.find(needle).unwrap();
                assert_ne!(snapshot.scope(snapshot.styles()[at]), Some("keyword"),
                    "{language:?} {needle}");
            }
        }
    }

    #[test]
    fn c_keywords_cover_modern_c_without_leaking_cpp_keywords() {
        // redis src/sds.c (7.4.0) declares `sdstemplate(const char *template, ...)`;
        // CPython 3.13 Objects/longobject.c uses `static_assert` and redis
        // src/atomicvar.h uses `_Atomic`. .c files paint the C keyword set;
        // C++-only words stay keyword-scoped only in .cpp files.
        let code = "sds sdstemplate(const char *template, void *cb) {\n    const char *p = template;\n    static_assert(sizeof(long) >= 4, \"long\");\n    _Atomic int n = 0;\n    int *restrict r = &n;\n    return;\n}\n";
        let c = highlight(LapceLanguage::C, &Rope::from(code)).unwrap();
        for keyword in ["static_assert", "_Atomic", "restrict"] {
            let at = code.find(keyword).unwrap();
            assert_eq!(c.scope(c.styles()[at]), Some("keyword"), "C {keyword}");
        }
        let mut start = 0;
        while let Some(at) = code[start..].find("template").map(|i| start + i) {
            assert_ne!(
                c.scope(c.styles()[at]),
                Some("keyword"),
                "C template at {at}"
            );
            start = at + 1;
        }
        // The standalone `template` parameter stays a keyword in C++; the
        // substring inside the `sdstemplate` identifier never is.
        let cpp = highlight(LapceLanguage::Cpp, &Rope::from(code)).unwrap();
        let at = code.find("p = template").unwrap() + 4;
        assert_eq!(cpp.scope(cpp.styles()[at]), Some("keyword"), "Cpp template");
        let at = code.find("sdstemplate").unwrap() + 3;
        assert_ne!(cpp.scope(cpp.styles()[at]), Some("keyword"), "Cpp sdstemplate");
    }

    #[test]
    fn prose_and_patch_languages_have_theme_scopes() {
        // markdown/diff/props shipped with no usable lexical classes; makefile
        // was already fine. These routings also cover .env and Makefile, the
        // everyday files of a PHP/Laravel project.
        use LapceLanguage::*;
        for (language, text, needle, scope) in [
            (Markdown, "# T\n", "T", "markup.heading"),
            (Markdown, "**b**\n", "**b**", "markup.bold"),
            (Markdown, "*i*\n", "*i*", "markup.italic"),
            (Markdown, "`c`\n", "`c`", "string"),
            (Diff, "--- a/f\n", "--- a/f", "keyword"),
            (Diff, "+added\n", "+added", "source_control.added"),
            (Diff, "-removed\n", "-removed", "source_control.removed"),
            (Diff, "! changed\n", "! changed", "source_control.modified"),
            (Ini, "[s]\nKEY=value\n", "[s]", "keyword"),
            (Ini, "[s]\nKEY=value\n", "KEY", "property"),
            (Make, "# c\n", "# c", "comment"),
        ] {
            let snapshot = highlight(language, &Rope::from(text)).unwrap();
            let at = text.find(needle).unwrap();
            let got = snapshot.scope(snapshot.styles()[at]);
            assert_eq!(got, Some(scope), "{language:?} {needle}");
        }
    }
    #[test]
    fn syntax_worker_publishes_only_lexilla_and_rejects_cancelled_result() {
        let mut syntax = crate::syntax::Syntax::from_language(LapceLanguage::Cpp);
        syntax.parse(7, Rope::from("int value = 42;"), None);
        assert_eq!(syntax.rev, 7);
        assert!(syntax.layers.is_none() && syntax.styles.is_none());
        assert!(syntax.lexilla.is_some());
        syntax
            .cancel_flag
            .store(1, std::sync::atomic::Ordering::Release);
        syntax.parse(8, Rope::from("changed"), None);
        assert_eq!(syntax.rev, 7);
    }
    #[test]
    fn syntax_worker_publishes_local_folds_and_rejects_cancelled_folds() {
        let mut syntax = crate::syntax::Syntax::from_language(LapceLanguage::Php);
        syntax.parse(
            9,
            Rope::from("<?php\nfunction f() {\n    run();\n}\n"),
            None,
        );
        assert_eq!(
            syntax.folding_ranges,
            vec![crate::folding::FoldRange { start: 1, end: 3 }]
        );
        assert!(syntax.layers.is_none());
        syntax
            .cancel_flag
            .store(1, std::sync::atomic::Ordering::Release);
        syntax.parse(10, Rope::from("flat"), None);
        assert_eq!(syntax.rev, 9);
        assert_eq!(syntax.folding_ranges.len(), 1);
    }
    #[test]
    fn plaintext_also_gets_local_folding() {
        let mut syntax = crate::syntax::Syntax::plaintext();
        syntax.parse(1, Rope::from("heading\n    body\nnext"), None);
        assert!(syntax.lexilla.is_none());
        assert_eq!(
            syntax.folding_ranges,
            vec![crate::folding::FoldRange { start: 0, end: 2 }]
        );
        syntax.parse(2, Rope::from("flat"), None);
        assert!(syntax.folding_ranges.is_empty());
    }
    #[test]
    fn unknown_language_is_plain_text() {
        assert!(highlight(LapceLanguage::PlainText, &Rope::from("hello")).is_none());
    }

    const PHP_FIXTURE: &str = "<?php\n/* outer /* nested */ still */\n/** doc */\n#[Attribute]\nnamespace App;\nfinal class User {\n    public function name(string $first): string {\n        $n = 0x1F + 1_000;\n        $s = \"hi $first {$n}\";\n        $t = 'it\\'s';\n        // line\n        return <<<EOT\n  Heredoc $first\n  EOT;\n    }\n}\n$raw = <<<'RAW'\nnow $doc\nRAW;\n?>\n<p><?= $x ?></p>\n";

    fn php_scope_at(
        snapshot: &Snapshot,
        text: &str,
        needle: &str,
        nth: usize,
    ) -> Option<&'static str> {
        let pos = text.match_indices(needle).nth(nth).expect("needle").0;
        snapshot.scope(snapshot.styles()[pos])
    }

    #[test]
    fn php_processing_instructions_advance_and_real_close_tags_are_indexed() {
        let text = "<?php $x = '?><div>fake</div>'; ?><?xml version='1.0' ?><p>real</p><?php echo 1; ?>";
        for lexer in ["php", "laravel"] {
            let snap = Snapshot::lex(lexer, text, &[]).unwrap();
            assert_eq!(snap.len(), text.len());
            assert_eq!(snap.styles()[text.find("<p>").unwrap()], 21);
        }
        let ends = php_close_tag_offsets(text).unwrap();
        assert_eq!(ends, vec![text.find("; ?>").unwrap() + 4, text.len()]);
        assert!(php_close_tag_offsets("not PHP").is_none());
    }

    #[test]
    fn laravel_attribute_arguments_do_not_count_quoted_brackets() {
        for text in [
            "<?php\n#[Example(\"] ?> not HTML\", [\"[\", \"]\"])]\nfunction example(): string { return \"valid\"; }\necho strlen(example());",
            "<?php\n#[Example('] ?> not HTML')]\nfunction example() {}\necho strlen('valid');",
            "<?php\n#[Example(/* ] ?> */ ['text'])]\nfunction example() {}\necho strlen('valid');",
            "<?php\r\n#[Example(\"λ🙂 \\\" ] ?>\", [\"[\", \"]\"])]\r\nfunction example() {}\r\necho strlen('valid');",
            "<?php\n#[Example(// ] ignored\n ['text'])]\nfunction example() {}\necho strlen('valid');",
            "<?php\n#[Example(# ] ignored\n ['text'])]\nfunction example() {}\necho strlen('valid');",
        ] {
            assert!(is_pure_php(text));
            let snap = Snapshot::lex("laravel", text, LARAVEL_WORD_LISTS).unwrap();
            assert_eq!(php_scope_at(&snap, text, "function example", 0), Some("keyword"));
            assert_eq!(php_scope_at(&snap, text, "echo strlen", 0), Some("keyword"));
            assert_eq!(php_scope_at(&snap, text, "strlen", 0), Some("function"));
        }
    }

    #[test]
    fn laravel_unfinished_attributes_are_bounded_and_line_comment_closers_still_work() {
        for text in ["<?php #[Example(\"λ🙂 ] ?>", "<?php #[Example(/* ] ?>", "<?php #[Example(['x'"] {
            let snap = Snapshot::lex("laravel", text, LARAVEL_WORD_LISTS).unwrap();
            assert_eq!(snap.len(), text.len());
        }
        let text = "<?php #[Example(// ?> <p>HTML</p>";
        let snap = Snapshot::lex("laravel", text, LARAVEL_WORD_LISTS).unwrap();
        assert_eq!(snap.styles()[text.find("?>").unwrap()], 20);
        assert_eq!(snap.styles()[text.find("<p>").unwrap()], 21);
    }

    #[test]
    fn heredoc_variables_interpolate_but_escaped_dollars_and_nowdoc_do_not() {
        let text = "<?php\r\n$text = <<< TXT\r\nλ🙂 $name \\$escaped \\\\$active {$braced} $2 literal\r\n  TXT;\r\n$raw = <<< 'RAW'\r\n$literal {$also_literal}\r\nRAW;\r\necho strlen($text);";
        for lexer in ["php", "laravel"] {
            let lists = if lexer == "php" { PHP_WORD_LISTS } else { LARAVEL_WORD_LISTS };
            let snap = Snapshot::lex(lexer, text, lists).unwrap();
            assert_eq!(snap.len(), text.len());
            for needle in ["$name", "$active", "$braced"] {
                assert_ne!(php_scope_at(&snap, text, needle, 0), Some("string"), "{lexer}: {needle}");
            }
            for needle in ["$escaped", "$2 literal", "$literal", "$also_literal"] {
                assert_eq!(php_scope_at(&snap, text, needle, 0), Some("string"), "{lexer}: {needle}");
            }
            assert_eq!(php_scope_at(&snap, text, "echo strlen", 0), Some("keyword"));
            let unfinished = "<?php $text = <<<TXT\r\n$name λ🙂\\";
            let edited = Snapshot::lex(lexer, unfinished, lists).unwrap();
            assert_eq!(edited.len(), unfinished.len());
            assert_ne!(php_scope_at(&edited, unfinished, "$name", 0), Some("string"));
            assert_eq!(Snapshot::lex(lexer, text, lists).unwrap().styles(), snap.styles());
        }
    }

    #[test]
    fn php_heredoc_and_nowdoc_accept_cr_only_line_endings() {
        for ending in ["\r", "\n", "\r\n"] {
            for label in ["TXT", "\"TXT\"", "'TXT'"] {
                let text = format!("<?php{ending}$text = <<<{label}{ending}λ🙂 ?> body $name{ending}TXT;{ending}echo strlen($text);");
                assert!(is_pure_php(&text));
                for lexer in ["php", "laravel"] {
                    let lists = if lexer == "php" { PHP_WORD_LISTS } else { LARAVEL_WORD_LISTS };
                    let snap = Snapshot::lex(lexer, &text, lists).unwrap();
                    assert_eq!(snap.len(), text.len());
                    assert_eq!(php_scope_at(&snap, &text, "?> body", 0), Some("string"));
                    assert_eq!(php_scope_at(&snap, &text, "echo strlen", 0), Some("keyword"), "{lexer}: {ending:?} {label}");
                    assert_eq!(php_scope_at(&snap, &text, "strlen", 0), Some("function"));
                }
            }
        }
    }

    #[test]
    fn heredoc_line_boundaries_survive_trailing_backslashes_and_unfinished_edits() {
        for ending in ["\r", "\n", "\r\n"] {
            let text = format!("<?php{ending}$text = <<<TXT{ending}λ🙂 \\{ending}  TXT;{ending}echo strlen($text);");
            for lexer in ["php", "laravel"] {
                let lists = if lexer == "php" { PHP_WORD_LISTS } else { LARAVEL_WORD_LISTS };
                let snap = Snapshot::lex(lexer, &text, lists).unwrap();
                assert_eq!(php_scope_at(&snap, &text, "echo strlen", 0), Some("keyword"));
                let unfinished = text.split("  TXT;").next().unwrap();
                let edited = Snapshot::lex(lexer, unfinished, lists).unwrap();
                assert_eq!(edited.len(), unfinished.len());
                assert_eq!(Snapshot::lex(lexer, &text, lists).unwrap().styles(), snap.styles());
            }
        }
    }

    #[test]
    fn php_heredoc_openers_accept_horizontal_whitespace_before_labels() {
        for gap in [" ", "\t", " \t "] {
            for label in ["TXT", "\"TXT\"", "'TXT'"] {
                let text = format!("<?php\r\n$text = <<<{gap}{label}\r\nλ🙂 ?> body $name\r\nTXT;\r\necho strlen($text);");
                assert!(is_pure_php(&text), "{gap:?} {label}");
                for lexer in ["php", "laravel"] {
                    let lists = if lexer == "php" { PHP_WORD_LISTS } else { LARAVEL_WORD_LISTS };
                    let snap = Snapshot::lex(lexer, &text, lists).unwrap();
                    assert_eq!(snap.len(), text.len());
                    assert_eq!(php_scope_at(&snap, &text, "?> body", 0), Some("string"));
                    assert_eq!(php_scope_at(&snap, &text, "echo strlen", 0), Some("keyword"));
                    assert_eq!(php_scope_at(&snap, &text, "strlen", 0), Some("function"));
                    let unfinished = text.split("TXT;\r\n").next().unwrap();
                    let edited = Snapshot::lex(lexer, unfinished, lists).unwrap();
                    assert_eq!(edited.len(), unfinished.len());
                    assert_eq!(php_scope_at(&edited, unfinished, "?> body", 0), Some("string"));
                    assert_eq!(Snapshot::lex(lexer, &text, lists).unwrap().styles(), snap.styles());
                }
            }
        }
    }

    #[test]
    fn php_complex_interpolation_inner_quotes_do_not_close_the_outer_string() {
        let text = r#"<?php $text = "Hello {$user["?>"]} ?> still string"; echo strlen($text);"#;
        assert!(is_pure_php(text));
        for lexer in ["php", "laravel"] {
            let lists = if lexer == "php" { PHP_WORD_LISTS } else { LARAVEL_WORD_LISTS };
            let snap = Snapshot::lex(lexer, text, lists).unwrap();
            assert_eq!(php_scope_at(&snap, text, "?> still string", 0), Some("string"));
            assert_eq!(php_scope_at(&snap, text, "echo strlen", 0), Some("keyword"));
            assert_eq!(php_scope_at(&snap, text, "strlen", 0), Some("function"));
        }
    }

    #[test]
    fn php_curly_interpolation_quotes_comments_and_unfinished_edits_are_bounded() {
        for text in [
            r#"<?php $x = "λ🙂 {$user["?>}"]} tail"; echo strlen($x);"#,
            r#"<?php $x = "{$user['?>}']} tail"; echo strlen($x);"#,
            r#"<?php $x = "{$user["a\"?>}"]} tail"; echo strlen($x);"#,
            r#"<?php $x = "{$user[/* ?>} */ 'key']} tail"; echo strlen($x);"#,
            r#"<?php $x = "{$obj->{$names["?>}"]}} tail"; echo strlen($x);"#,
            r#"<?php $x = `{$user["?>}"]} tail`; echo strlen($x);"#,
        ] {
            assert!(is_pure_php(text));
            for lexer in ["php", "laravel"] {
                let lists = if lexer == "php" { PHP_WORD_LISTS } else { LARAVEL_WORD_LISTS };
                let snap = Snapshot::lex(lexer, text, lists).unwrap();
                assert_eq!(snap.len(), text.len());
                assert_eq!(php_scope_at(&snap, text, "tail", 0), Some("string"));
                assert_eq!(php_scope_at(&snap, text, "echo strlen", 0), Some("keyword"));
                assert_eq!(php_scope_at(&snap, text, "strlen", 0), Some("function"));
                let unfinished = text.split(" tail").next().unwrap();
                assert_eq!(Snapshot::lex(lexer, unfinished, lists).unwrap().len(), unfinished.len());
                assert_eq!(Snapshot::lex(lexer, text, lists).unwrap().styles(), snap.styles());
            }
        }
        let escaped = r#"<?php $x = "\{$user} tail"; echo 1;"#;
        for lexer in ["php", "laravel"] {
            let snap = Snapshot::lex(lexer, escaped, &[]).unwrap();
            assert_eq!(php_scope_at(&snap, escaped, "tail", 0), Some("string"));
        }
    }

    #[test]
    fn php_and_laravel_string_scanner_styles_stay_identical() {
        let strings = [
            "''", "\"\"", "``", "'λ🙂 \\\' ?>'", "\"λ🙂 $name ?>\"",
            "`printf $name ?>`", "\"{$user[\"?>}\"]} tail\"",
            "\"{$user['key']} {$obj->{$names[\"}\"]}}\"",
            "\"{$user[/* } ?> */ 'key']}\"",
            "\"{$user[// }\n'key']} tail\"",
            "\"{$user[# }\r\n'key']} tail\"",
            "\"\\$literal \\\" $name\"", "'unfinished\\", "\"unfinished\\",
            "\"{$user[\"unfinished", "\"{$user[/* unfinished",
        ];
        for string in strings {
            let text = format!("<?php $value = {string}; $after = 1; ?>");
            let php = Snapshot::lex("php", &text, &[]).unwrap();
            let laravel = Snapshot::lex("laravel", &text, &[]).unwrap();
            assert_eq!(php.len(), text.len());
            assert_eq!(php.styles(), laravel.styles(), "{text}");
        }
    }

    #[test]
    fn php_backticks_keep_close_tags_in_strings_and_resume_code() {
        let text = "<?php\r\n$name = 'λ🙂'; $out = `printf \"hello\" $name ?> still_command \\` escaped`; echo strlen($out);";
        assert!(is_pure_php(text));
        for lexer in ["php", "laravel"] {
            let lists = if lexer == "php" { PHP_WORD_LISTS } else { LARAVEL_WORD_LISTS };
            let snap = Snapshot::lex(lexer, text, lists).unwrap();
            assert_eq!(snap.len(), text.len());
            for needle in ["printf", "?>", "still_command", "escaped"] {
                assert_eq!(php_scope_at(&snap, text, needle, 0), Some("string"), "{lexer}: {needle}");
            }
            assert_ne!(php_scope_at(&snap, text, "$name ?>", 0), Some("string"));
            assert_eq!(php_scope_at(&snap, text, "echo strlen", 0), Some("keyword"));
            assert_eq!(php_scope_at(&snap, text, "strlen", 0), Some("function"));
            let unfinished = "<?php $out = `printf ?> λ🙂\r\ncontinued\\";
            let snap = Snapshot::lex(lexer, unfinished, lists).unwrap();
            assert_eq!(snap.len(), unfinished.len());
            assert_eq!(php_scope_at(&snap, unfinished, "continued", 0), Some("string"));
            let edited = format!("{unfinished}x`; echo 1;");
            let fixed = Snapshot::lex(lexer, &edited, lists).unwrap();
            assert_eq!(php_scope_at(&fixed, &edited, "echo", 0), Some("keyword"));
            assert_eq!(Snapshot::lex(lexer, unfinished, lists).unwrap().styles(), snap.styles());
        }
    }

    #[test]
    fn php_float_literals_include_leading_and_trailing_decimal_points() {
        let text = "<?php\r\n$values = [.5, 1., 2.e3, .5e-2, 1_000.5, 0.5];\r\necho strlen('λ🙂');";
        for lexer in ["php", "laravel"] {
            let lists = if lexer == "php" { PHP_WORD_LISTS } else { LARAVEL_WORD_LISTS };
            let snap = Snapshot::lex(lexer, text, lists).unwrap();
            for value in [".5,", "1.", "2.e3", ".5e-2", "1_000.5", "0.5"] {
                let start = text.find(value).unwrap();
                let len = value.trim_end_matches(',').len();
                for pos in start..start+len {
                    assert_eq!(snap.styles()[pos], 12, "{lexer} {value} at {pos}");
                }
            }
            assert_eq!(php_scope_at(&snap, text, "echo strlen", 0), Some("keyword"));
            let controls = "<?php $x = 'left' . 'right'; $obj->value; ...$args; .5; 1.";
            let snap = Snapshot::lex(lexer, controls, lists).unwrap();
            let bare = controls.find(". 'right'").unwrap();
            assert_ne!(snap.styles()[bare], 12);
            let spread = controls.find("...$args").unwrap();
            for pos in spread..spread+3 {
                assert_ne!(snap.styles()[pos], 12);
            }
            for value in [".5", "1."] {
                let start = controls.rfind(value).unwrap();
                for pos in start..start+value.len() {
                    assert_eq!(snap.styles()[pos], 12);
                }
            }
            assert_eq!(Snapshot::lex(lexer, text, lists).unwrap().styles().len(), text.len());
        }
    }

    #[test]
    fn php_magic_constants_are_case_insensitive_but_regular_constants_are_not() {
        let text = "<?php __CLASS__; __class__; __DiR__; __file__; __function__; __line__; __method__; __namespace__; __trait__; PHP_VERSION; php_version; $x = '__class__'; // __class__\r\n";
        for lexer in ["php", "laravel"] {
            let lists = if lexer == "php" { PHP_WORD_LISTS } else { LARAVEL_WORD_LISTS };
            let snap = Snapshot::lex(lexer, text, lists).unwrap();
            let expected = snap.styles()[text.find("__CLASS__").unwrap()];
            for name in ["__class__", "__DiR__", "__file__", "__function__", "__line__", "__method__", "__namespace__", "__trait__"] {
                assert_eq!(snap.styles()[text.find(name).unwrap()], expected, "{lexer}: {name}");
            }
            assert_ne!(snap.styles()[text.find("PHP_VERSION").unwrap()], snap.styles()[text.find("php_version").unwrap()]);
            assert_eq!(php_scope_at(&snap, text, "__class__'", 0), Some("string"));
            assert_eq!(php_scope_at(&snap, text, "__class__\r", 0), Some("comment"));
        }
    }

    #[test]
    fn magic_constant_matching_respects_active_and_custom_constant_lists() {
        for lexer in ["php", "laravel"] {
            let mut lists = if lexer == "php" { PHP_WORD_LISTS } else { LARAVEL_WORD_LISTS }.to_vec();
            lists[3] = "CustomConstant";
            let text = "<?php __CLASS__; __class__; CustomConstant; customconstant; __CLASSICAL__;";
            let snap = Snapshot::lex(lexer, text, &lists).unwrap();
            assert_eq!(snap.styles()[text.find("__CLASS__").unwrap()], 17);
            assert_eq!(snap.styles()[text.find("__class__").unwrap()], 17);
            assert_eq!(snap.styles()[text.find("CustomConstant").unwrap()], 16);
            assert_eq!(snap.styles()[text.find("customconstant").unwrap()], 17);
            assert_eq!(snap.styles()[text.find("__CLASSICAL__").unwrap()], 17);
        }
    }

    #[test]
    fn php_base_words_are_ascii_case_insensitive_in_both_scanners() {
        let text = "<?php IF (TRUE) { ECHO STRLEN('λ🙂'); ReTuRn NuLl; } FuNcTiOn test(InT $Value): VoId {} $IF = 'RETURN'; // ECHO\r\nPHP_VERSION; php_version;";
        for lexer in ["php", "laravel"] {
            let lists = if lexer == "php" { PHP_WORD_LISTS } else { LARAVEL_WORD_LISTS };
            let snap = Snapshot::lex(lexer, text, lists).unwrap();
            assert_eq!(snap.len(), text.len());
            for needle in ["IF (", "ECHO STRLEN", "ReTuRn", "FuNcTiOn"] {
                assert_eq!(php_scope_at(&snap, text, needle, 0), Some("keyword"), "{lexer}: {needle}");
            }
            assert_eq!(php_scope_at(&snap, text, "STRLEN", 0), Some("function"));
            for needle in ["TRUE", "NuLl", "InT", "VoId"] {
                assert_eq!(php_scope_at(&snap, text, needle, 0), Some("type"), "{lexer}: {needle}");
            }
            assert_ne!(php_scope_at(&snap, text, "$IF", 0), Some("keyword"));
            assert_eq!(php_scope_at(&snap, text, "RETURN'", 0), Some("string"));
            assert_eq!(php_scope_at(&snap, text, "ECHO\r", 0), Some("comment"));
            let upper = text.find("PHP_VERSION").unwrap();
            let lower = text.find("php_version").unwrap();
            assert_ne!(snap.styles()[upper], snap.styles()[lower], "constants remain case-sensitive");
            let mut custom = lists.to_vec();
            custom[1] = "MixedCaseFunction";
            custom[2] = "MixedCaseType";
            let text = "<?php MIXEDCASEFUNCTION(); mixedcasetype $value;";
            let snap = Snapshot::lex(lexer, text, &custom).unwrap();
            assert_eq!(php_scope_at(&snap, text, "MIXEDCASEFUNCTION", 0), Some("function"));
            assert_eq!(php_scope_at(&snap, text, "mixedcasetype", 0), Some("type"));
        }
    }

    #[test]
    fn standalone_php_lexer_covers_modern_php() {
        let t = PHP_FIXTURE;
        let snap =
            Snapshot::lex("php", t, PHP_WORD_LISTS).expect("php lexer registered");
        assert_eq!(snap.len(), t.len());
        // keywords, types
        assert_eq!(php_scope_at(&snap, t, "namespace", 0), Some("keyword"));
        assert_eq!(php_scope_at(&snap, t, "final", 0), Some("keyword"));
        assert_eq!(php_scope_at(&snap, t, "function", 0), Some("keyword"));
        assert_eq!(php_scope_at(&snap, t, "string $first", 0), Some("type"));
        // PHP comments end at the first closer, not at a nesting level.
        assert_eq!(php_scope_at(&snap, t, "/* outer", 0), Some("comment"));
        assert_ne!(php_scope_at(&snap, t, "still", 0), Some("comment"));
        assert_eq!(php_scope_at(&snap, t, "/** doc", 0), Some("comment"));
        assert_eq!(php_scope_at(&snap, t, "// line", 0), Some("comment"));
        // #[Attribute] is not a comment
        assert_ne!(php_scope_at(&snap, t, "#[Attribute]", 0), Some("comment"));
        // numbers with hex and separators
        assert_eq!(php_scope_at(&snap, t, "0x1F", 0), Some("constant"));
        assert_eq!(php_scope_at(&snap, t, "1_000", 0), Some("constant"));
        // strings
        assert_eq!(php_scope_at(&snap, t, "\"hi", 0), Some("string"));
        assert_eq!(php_scope_at(&snap, t, "'it", 0), Some("string"));
        // heredoc body (indented closer) and nowdoc body
        assert_eq!(php_scope_at(&snap, t, "Heredoc", 0), Some("string"));
        assert_eq!(php_scope_at(&snap, t, "now ", 0), Some("string"));
        // the code after the heredoc and nowdoc is lexed as code again
        assert_eq!(php_scope_at(&snap, t, "return", 0), Some("keyword"));
        // tags: <?php and <?= are open tags, inline HTML stays default
        assert_eq!(php_scope_at(&snap, t, "<?php", 0), Some("operator"));
        assert_eq!(php_scope_at(&snap, t, "<?=", 0), Some("operator"));
        assert_eq!(php_scope_at(&snap, t, "<p>", 0), None);
    }

    #[test]
    fn standalone_php_lexer_handles_unicode_and_empty_input() {
        let t = "<?php echo 'λ🙂'; // ünï\n$café = 1;\n";
        let snap = Snapshot::lex("php", t, PHP_WORD_LISTS).unwrap();
        assert_eq!(snap.len(), t.len());
        assert_eq!(php_scope_at(&snap, t, "echo", 0), Some("keyword"));
        assert_eq!(php_scope_at(&snap, t, "'λ", 0), Some("string"));
        assert!(Snapshot::lex("php", "", PHP_WORD_LISTS).unwrap().is_empty());
        // unterminated constructs must not panic or hang
        for bad in [
            "<?php /* open",
            "<?php <<<EOT\nx",
            "<?php 'abc",
            "<?php \"a $",
        ] {
            let s = Snapshot::lex("php", bad, PHP_WORD_LISTS).unwrap();
            assert_eq!(s.len(), bad.len());
        }
    }

    #[test]
    fn php_routing_is_content_based() {
        assert!(is_pure_php("<?php\necho 1;"));
        assert!(is_pure_php("\u{feff}  \n<?PHP echo 1;"));
        assert!(!is_pure_php("<html>\n<?php echo 1; ?>"));
        assert!(!is_pure_php("<?= $x ?>"));
        assert!(!is_pure_php(""));
        // PHP CLI entry scripts carry a shebang line before the open tag.
        assert!(is_pure_php("#!/usr/bin/env php\n<?php\necho 1;"));
        assert!(is_pure_php("#!/usr/bin/env php\r\n<?php\necho 1;"));
        // A shebang does not make non-PHP pure, and PHP templates stay templated.
        assert!(!is_pure_php("#!/bin/sh\necho hi"));
        assert!(!is_pure_php("#!/usr/bin/env php"));
        assert!(!is_pure_php("#!/usr/bin/env php\n<html>"));
        // pure PHP: standalone lexer (a nowdoc body is a string there)
        let pure = "<?php\n$a = <<<'X'\nbody\nX;\n";
        let snap = highlight(LapceLanguage::Php, &Rope::from(pure)).unwrap();
        assert_eq!(php_scope_at(&snap, pure, "body", 0), Some("string"));
        // template: still highlighted through hypertext
        let tpl = "<p>x</p>\n<?php echo 1; ?>";
        assert!(highlight(LapceLanguage::Php, &Rope::from(tpl)).is_some());
    }

    #[test]
    fn php_first_templates_route_by_real_inline_html_not_tag_substrings() {
        for source in [
            "<?php $x = 1; ?>\n<section class=\"content\">Hello</section>",
            "#!/usr/bin/env php\n<?php echo 1; ?>\n<div>CLI output</div>",
            "<?php // closes the PHP region ?>\n<div>Output</div>",
            "<?php echo 1; ?>plain text",
        ] {
            assert!(!is_pure_php(source), "{source}");
        }
        for source in [
            "<?php echo '?>';",
            "<?php echo \"?>\";",
            "<?php /* ?> */ echo 1;",
            "<?php $x = <<<'END'\n?>\nEND;\n",
            "<?php $x = <<<END\n?>\nEND;\n",
            "<?php echo 1; ?>\r\n  ",
            "<?php echo 1; ?>\n<?php echo 2;",
        ] {
            assert!(is_pure_php(source), "{source}");
        }
        let source = "<?php $title = 'Hello'; ?>\n<section class=\"content\">Hello</section>";
        for laravel in [false, true] {
            let snapshot = highlight_in(LapceLanguage::Php, &Rope::from(source), laravel).unwrap();
            let tag = source.find("section").unwrap();
            assert_eq!(snapshot.scope(snapshot.styles()[tag]), Some("tag"));
        }
    }

    const LARAVEL_FIXTURE: &str = "<?php\nnamespace App\\Http;\n#[Middleware('auth')]\nRoute::middleware('auth')->get('/home', fn () => view('home'));\nRoute::get('/x', [UserController::class, 'index'])->middleware(['verified', 'throttle:60,1']);\n$users = User::where('active', 1)->get();\n$c = config('auth');\n$s = $this->guard('web');\n$h = <<<EOT\n  Hello {$name}\n  EOT;\nArtisan::call('migrate:fresh');\n// php artisan make:model Post\nmake:model;\n$a = $x ? a:b : 1;\n";

    fn lv(
        snap: &Snapshot,
        t: &str,
        needle: &str,
        nth: usize,
    ) -> Option<&'static str> {
        php_scope_at(snap, t, needle, nth)
    }

    #[test]
    fn laravel_lexer_colours_framework_constructs() {
        let t = LARAVEL_FIXTURE;
        let snap =
            Snapshot::lex("laravel", t, LARAVEL_WORD_LISTS).expect("laravel lexer");
        assert_eq!(snap.len(), t.len());
        // Only explicitly listed facades, never every PascalCase static call.
        assert_eq!(lv(&snap, t, "Route", 0), Some("type"));
        assert_ne!(lv(&snap, t, "User", 0), Some("type"));
        assert_eq!(lv(&snap, t, "Artisan", 0), Some("type"));
        // route methods and Eloquent methods after ::/->  (only after ::)
        assert_eq!(lv(&snap, t, "middleware('auth')->get", 0), Some("function"));
        assert_eq!(lv(&snap, t, "where", 0), Some("function"));
        // helpers only when called
        assert_eq!(lv(&snap, t, "view('home')", 0), Some("function"));
        assert_eq!(lv(&snap, t, "config('auth')", 0), Some("function"));
        // PHP attribute span
        assert_eq!(lv(&snap, t, "#[Middleware", 0), Some("type"));
        assert_eq!(lv(&snap, t, "Middleware('auth')]", 0), Some("type"));
        // middleware aliases only in a middleware() context
        assert_eq!(lv(&snap, t, "'verified'", 0), Some("keyword"));
        assert_eq!(lv(&snap, t, "'throttle:60,1'", 0), Some("keyword"));
        assert_eq!(lv(&snap, t, "'auth')->get", 0), Some("keyword"));
        assert_eq!(lv(&snap, t, "'auth');\n$s", 0), Some("string"));
        assert_eq!(lv(&snap, t, "'web'", 0), Some("string"));
        // heredoc with interpolation
        assert_eq!(lv(&snap, t, "Hello", 0), Some("string"));
        // artisan command in code, and plain PHP still works
        assert_eq!(lv(&snap, t, "make:model;", 0), Some("function"));
        assert_eq!(lv(&snap, t, "namespace", 0), Some("keyword"));
        assert_eq!(lv(&snap, t, "// php", 0), Some("comment"));
        // a ternary "a:b" must not be mistaken for an artisan command
        assert_ne!(lv(&snap, t, "a:b", 0), Some("function"));
    }

    #[test]
    fn laravel_lexer_blade_directives_in_inline_html() {
        let t = "<?php ?>\n<div>@if($a) x @endif @foreach($l as $i) mail@me.com @endforeach</div>\n";
        let snap = Snapshot::lex("laravel", t, LARAVEL_WORD_LISTS).unwrap();
        assert_eq!(lv(&snap, t, "@if", 0), Some("keyword"));
        assert_eq!(lv(&snap, t, "@endif", 0), Some("keyword"));
        assert_eq!(lv(&snap, t, "@foreach", 0), Some("keyword"));
        assert_eq!(lv(&snap, t, "@endforeach", 0), Some("keyword"));
        assert_eq!(lv(&snap, t, "@me", 0), None, "email is not a directive");
        assert_eq!(lv(&snap, t, "<div>", 0), None);
    }

    const BLADE_FIXTURE: &str = "@extends('layouts.app')\n{{-- note @if --}}\n@section('content')\n<div class=\"card {{ $cls }}\" id='a' @disabled($off) data-x=raw>\n  @if ($user->isAdmin() && count($items) > 0)\n    <p>Hello, {{ $user->name ?? 'guest' }} &amp; {!! $html !!}</p>\n  @elseif(true) @else @endif\n  @foreach ($items as $item)\n    <li>{{ route('items.show', $item) }}</li>\n  @endforeach\n</div>\n<!-- c {{ $x }} -->\n@php\n  $total = 1 + 2; // sum\n  $s = \"a\";\n@endphp\n@php($y = 3)\n@verbatim {{ raw }} @if @endverbatim\n@{{ vue }} mail@me.com @@if\n<script>if (a < b) { x = \"{{ $js }}\"; }</script>\n<?php echo $legacy; ?>\n@endsection\n";

    fn bl(snap: &Snapshot, needle: &str, nth: usize) -> Option<&'static str> {
        php_scope_at(snap, BLADE_FIXTURE, needle, nth)
    }

    #[test]
    fn blade_lexer_covers_directives_echo_php_and_html() {
        let t = BLADE_FIXTURE;
        let snap =
            Snapshot::lex("blade", t, LARAVEL_WORD_LISTS).expect("blade lexer");
        // directives and their PHP arguments
        assert_eq!(bl(&snap, "@extends", 0), Some("keyword"));
        assert_eq!(bl(&snap, "'layouts.app'", 0), Some("string"));
        assert_eq!(bl(&snap, "@section", 0), Some("keyword"));
        assert_eq!(bl(&snap, "@if (", 0), Some("keyword"));
        assert_eq!(bl(&snap, "&&", 0), Some("operator"));
        assert_eq!(bl(&snap, "@elseif", 0), Some("keyword"));
        assert_eq!(bl(&snap, "@else ", 0), Some("keyword"));
        assert_eq!(bl(&snap, "@endif", 0), Some("keyword"));
        assert_eq!(bl(&snap, "@foreach", 0), Some("keyword"));
        assert_eq!(bl(&snap, "@endsection", 0), Some("keyword"));
        // blade comment hides directives
        assert_eq!(bl(&snap, "{{-- note", 0), Some("comment"));
        assert_eq!(bl(&snap, "@if --}}", 0), Some("comment"));
        // echo blocks: delimiters keyword, PHP inside
        assert_eq!(bl(&snap, "{{ $cls", 0), Some("keyword"));
        assert_eq!(bl(&snap, "'guest'", 0), Some("string"));
        assert_eq!(bl(&snap, "{!!", 0), Some("keyword"));
        assert_eq!(bl(&snap, "!!}", 0), Some("keyword"));
        assert_eq!(bl(&snap, "route(", 0), Some("function"));
        assert_eq!(bl(&snap, "'items.show'", 0), Some("string"));
        // HTML around everything
        assert_eq!(bl(&snap, "<div", 0), Some("tag"));
        assert_eq!(bl(&snap, "class", 0), Some("attribute"));
        assert_eq!(bl(&snap, "\"card ", 0), Some("string"));
        assert_eq!(bl(&snap, "id=", 0), Some("attribute"));
        assert_eq!(bl(&snap, "raw>", 0), Some("string"));
        assert_eq!(bl(&snap, "</p>", 0), Some("tag"));
        assert_eq!(bl(&snap, "&amp;", 0), Some("constant"));
        assert_eq!(bl(&snap, "<!-- c", 0), Some("comment"));
        assert_eq!(bl(&snap, "{{ $x }} -->", 0), Some("keyword"));
        assert_eq!(bl(&snap, "Hello", 0), None);
        // @php ... @endphp with full PHP
        assert_eq!(bl(&snap, "@php\n", 0), Some("keyword"));
        assert_eq!(bl(&snap, "// sum", 0), Some("comment"));
        assert_eq!(bl(&snap, "\"a\"", 0), Some("string"));
        assert_eq!(bl(&snap, "@endphp", 0), Some("keyword"));
        assert_eq!(bl(&snap, "@php($y", 0), Some("keyword"));
        // verbatim, escapes, email, script, raw php tags
        assert_eq!(bl(&snap, "@if @endverbatim", 0), None);
        assert_eq!(bl(&snap, "@endverbatim", 0), Some("keyword"));
        assert_eq!(bl(&snap, "{{ raw }}", 0), None);
        assert_eq!(bl(&snap, "@{{ vue }}", 0), None);
        assert_eq!(bl(&snap, "@me", 0), None);
        assert_eq!(bl(&snap, "@@if", 0), None);
        assert_eq!(bl(&snap, "<script>", 0), Some("tag"));
        assert_eq!(bl(&snap, "if (a < b)", 0), None);
        assert_eq!(bl(&snap, "{{ $js }}", 0), Some("keyword"));
        assert_eq!(bl(&snap, "</script>", 0), Some("tag"));
        assert_eq!(bl(&snap, "<?php", 0), Some("operator"));
        assert_eq!(bl(&snap, "echo", 0), Some("keyword"));
    }

    #[test]
    fn blade_lexer_survives_unterminated_input() {
        for bad in [
            "{{",
            "{{ $a",
            "{!! $a",
            "{{--",
            "@if(",
            "@if($a",
            "@php",
            "@php\n$a = '",
            "@verbatim x",
            "<div class=\"",
            "<div {{",
            "<!--",
            "<?php",
            "@{{",
            "<",
            "@section('a', ['x' => \"y",
            "<script>{{ ",
            "\u{feff}@if(λ)🙂",
        ] {
            let s = Snapshot::lex("blade", bad, LARAVEL_WORD_LISTS).unwrap();
            assert_eq!(s.len(), bad.len(), "{bad:?}");
        }
        assert!(
            Snapshot::lex("blade", "", LARAVEL_WORD_LISTS)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn blade_routing_by_file_name() {
        use std::path::Path;
        assert_eq!(
            LapceLanguage::from_path(Path::new("resources/views/home.blade.php")),
            LapceLanguage::Blade
        );
        assert_eq!(
            LapceLanguage::from_path(Path::new("Home.BLADE.PHP")),
            LapceLanguage::Blade
        );
        assert_eq!(
            LapceLanguage::from_path(Path::new("app/Http/Home.php")),
            LapceLanguage::Php
        );
        let text = Rope::from("@if($a) <b>{{ $a }}</b> @endif\n");
        for laravel in [true, false] {
            let snap =
                highlight_in(LapceLanguage::Blade, &text, laravel).expect("blade");
            let s = text.to_string();
            let pos = s.find("@if").unwrap();
            assert_eq!(snap.scope(snap.styles()[pos]), Some("keyword"));
        }
        // non-Laravel Blade has no framework helper colouring
        let t = Rope::from("{{ route('a') }}\n");
        let l = highlight_in(LapceLanguage::Blade, &t, true).unwrap();
        let p = highlight_in(LapceLanguage::Blade, &t, false).unwrap();
        assert_eq!(l.scope(l.styles()[3]), Some("function"));
        assert_ne!(p.scope(p.styles()[3]), Some("function"));
    }

    #[test]
    fn laravel_lexer_survives_unterminated_input() {
        for bad in [
            "<?php Route::",
            "<?php #[",
            "<?php ->middleware('au",
            "<?php <<<E\n",
            "@if",
        ] {
            let s = Snapshot::lex("laravel", bad, LARAVEL_WORD_LISTS).unwrap();
            assert_eq!(s.len(), bad.len());
        }
    }

    #[test]
    fn laravel_project_detection_and_routing() {
        let dir =
            std::env::temp_dir().join(format!("lvtest-{}", std::process::id()));
        let app = dir.join("app/Http");
        std::fs::create_dir_all(&app).unwrap();
        let file = app.join("C.php");
        assert!(!is_laravel_project(&file));
        let dir2 =
            std::env::temp_dir().join(format!("lvtest2-{}", std::process::id()));
        std::fs::create_dir_all(dir2.join("src")).unwrap();
        std::fs::write(
            dir2.join("composer.json"),
            r#"{"require":{"laravel/framework":"^11"}}"#,
        )
        .unwrap();
        assert!(is_laravel_project(&dir2.join("src/X.php")));
        let dir3 =
            std::env::temp_dir().join(format!("lvtest3-{}", std::process::id()));
        std::fs::create_dir_all(dir3.join("src")).unwrap();
        std::fs::write(
            dir3.join("composer.json"),
            r#"{"require":{"symfony/console":"^7"}}"#,
        )
        .unwrap();
        assert!(!is_laravel_project(&dir3.join("src/X.php")));
        std::fs::write(dir.join("artisan"), "#!/usr/bin/env php").unwrap();
        assert!(is_laravel_project(&dir.join("app/Other.php")));
        let code = "<?php\nRoute::get('/');\n";
        let rope = Rope::from(code);
        let lv_snap = highlight_in(LapceLanguage::Php, &rope, true).unwrap();
        assert_eq!(php_scope_at(&lv_snap, code, "Route", 0), Some("type"));
        let plain = highlight_in(LapceLanguage::Php, &rope, false).unwrap();
        assert_eq!(php_scope_at(&plain, code, "Route", 0), None);
        for d in [dir, dir2, dir3] {
            let _ = std::fs::remove_dir_all(d);
        }
    }
}
