//! Hover a Laravel middleware alias - `->middleware('auth')`,
//! `Route::middleware(['auth', 'verified'])` - and see the class the alias
//! maps to, from `bootstrap/app.php` (Laravel 11+) or `app/Http/Kernel.php`.
//! Uses the exact extraction of the go-to (`middleware_alias`), so the two
//! always agree. Nothing is executed.

use std::path::Path;

const MAX_LINE: usize = 4096;
const MAX_FILE: u64 = 1 << 20;

/// The class on the alias's declaration line: `'auth' => Foo::class,` gives
/// `Foo` (leading backslash and `::class` trimmed).
fn mapped_class(l: &str, name: &str) -> Option<String> {
    for q in ['\'', '"'] {
        let key = format!("{q}{name}{q}");
        for (i, _) in l.match_indices(&key) {
            let Some(rest) = l[i + key.len()..].trim_start().strip_prefix("=>")
            else {
                continue;
            };
            let v = rest.trim();
            let v = v.trim_end_matches([',', ']', ')']).trim();
            if !v.is_empty() && v.len() <= 80 {
                return Some(
                    v.trim_start_matches('\\')
                        .trim_end_matches("::class")
                        .to_string(),
                );
            }
        }
    }
    None
}

/// Markdown hover for the alias at byte `col` of `line`.
pub fn help(root: Option<&Path>, line: &str, col: usize) -> Option<String> {
    let root = root?;
    if line.len() > MAX_LINE {
        return None;
    }
    let name = crate::middleware_alias::alias_at(line, col)?.to_string();
    let (path, idx) = crate::middleware_alias::resolve(root, line, col)?;
    let meta = std::fs::metadata(&path).ok()?;
    if meta.len() > MAX_FILE {
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    let decl = text.lines().nth(idx)?;
    let rel = path.strip_prefix(root).unwrap_or(&path);
    let what = mapped_class(decl, &name)
        .map(|c| format!(" -> `{c}`"))
        .unwrap_or_default();
    Some(format!(
        "**`{name}`** - middleware alias{what}\n\n_{}:{}_",
        rel.display(),
        idx + 1,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
    }

    fn at(line: &str, needle: &str) -> usize {
        line.rfind(needle).unwrap() + 1
    }

    #[test]
    fn aliases_are_explained() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "bootstrap/app.php", "<?php\nreturn Application::configure()\n    ->withMiddleware(function ($m) {\n        $m->alias([\n            'admin' => AdminOnly::class,\n        ]);\n    })");
        put(r, "app/Http/Kernel.php", "<?php\nclass Kernel {\n    protected $middlewareAliases = [\n        'auth' => \\App\\Http\\Middleware\\Authenticate::class,\n    ];\n}");
        let root = Some(r);

        let l = "Route::get('/a', fn() => 1)->middleware('admin');";
        let md = help(root, l, at(l, "admin")).unwrap();
        assert!(md.contains("**`admin`** - middleware alias -> `AdminOnly`"), "{md}");
        assert!(md.contains("bootstrap/app.php:5"), "{md}");

        let l = "->middleware('auth:sanctum')";
        let md = help(root, l, at(l, "auth")).unwrap();
        assert!(
            md.contains("**`auth`** - middleware alias -> `App\\Http\\Middleware\\Authenticate`"),
            "{md}"
        );
        assert!(md.contains("app/Http/Kernel.php:4"), "{md}");

        // Unknown alias and no middleware context: nothing.
        let l = "->middleware('nope')";
        assert_eq!(help(root, l, at(l, "nope")), None);
        let l = "$x = 'auth';";
        assert_eq!(help(root, l, at(l, "auth")), None);
        assert_eq!(help(None, "->middleware('auth')", 16), None);
    }

    #[test]
    fn odd_input_does_not_panic() {
        let d = tempfile::tempdir().unwrap();
        let root = Some(d.path());
        for l in ["->middleware('", "'", "middleware('é')", "middleware('日本')", "()"] {
            for c in 0..=l.len() + 2 {
                let _ = help(root, l, c);
            }
        }
    }
}
