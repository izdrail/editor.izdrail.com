//! Go to the declaration of a Laravel middleware alias used in a route file or
//! controller: `->middleware('auth')`, `->middleware(['auth', 'verified'])`,
//! `Route::middleware('throttle:60,1')`, `->withoutMiddleware('guest')`.
//! The cursor must be on the alias; a `:parameters` suffix is ignored. It opens
//! the line `'auth' => ...` in `bootstrap/app.php` (Laravel 11+ `alias([...])`) or
//! `app/Http/Kernel.php` (`$middlewareAliases` / `$routeMiddleware`).
//!
//! Textual only; nothing is executed. Class names (`Auth::class`, strings with
//! a backslash) are left to normal Go to Definition, and so are aliases that the
//! framework or a package registers itself. Bounded: 1 MB per file.
use std::path::{Path, PathBuf};

const FILES: &[&str] = &["bootstrap/app.php", "app/Http/Kernel.php"];
const MAX_FILE_BYTES: u64 = 1_000_000;

/// Only a direct first string argument or a string in the first literal
/// array is an alias. Nested calls and later arguments belong elsewhere.
fn argument_context(before: &str) -> bool {
    for method in ["->middleware", "::middleware", "->withoutMiddleware", "::withoutMiddleware"] {
        for (pos, _) in before.match_indices(method) {
            let Some(rest) = before[pos + method.len()..].trim_start().strip_prefix('(') else { continue };
            let mut rest = rest.trim_start();
            if rest.is_empty() { return true; }
            let Some(array) = rest.strip_prefix('[') else { continue };
            rest = array.trim_start();
            loop {
                if rest.is_empty() { return true; }
                let Some(q) = rest.chars().next().filter(|c| matches!(c, '\'' | '"')) else { break };
                let bytes = rest.as_bytes();
                let mut end = 1;
                while end < bytes.len() && bytes[end] != q as u8 {
                    if bytes[end] == b'\\' { end += 1; }
                    end += 1;
                }
                if end >= bytes.len() { break; }
                let Some(next) = rest[end + 1..].trim_start().strip_prefix(',') else { break };
                rest = next.trim_start();
            }
        }
    }
    false
}

pub fn alias_at(line: &str, col: usize) -> Option<&str> {
    if line.len() > 4096 { return None; }
    let col = col.min(line.len());
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !matches!(bytes[i], b'\'' | b'"') { i += 1; continue; }
        let quote = bytes[i];
        let mut end = i + 1;
        while end < bytes.len() && bytes[end] != quote {
            if bytes[end] == b'\\' { end += 1; }
            end += 1;
        }
        if end >= bytes.len() { return None; }
        if col >= i && col <= end {
            if !argument_context(&line[..i]) { return None; }
            let name = line[i + 1..end].split(':').next()?;
            return (!name.is_empty() && name.len() <= 60
                && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')))
                .then_some(name);
        }
        i = end + 1;
    }
    None
}

fn declares(l: &str, name: &str) -> bool {
    ['\'', '"'].iter().any(|q| {
        let key = format!("{q}{name}{q}");
        l.match_indices(&key).any(|(i, _)| l[i + key.len()..].trim_start().starts_with("=>"))
    })
}

/// File and 0-based line of the alias declaration for the name under `col`.
pub fn resolve(root: &Path, line: &str, col: usize) -> Option<(PathBuf, usize)> {
    let name = alias_at(line, col)?;
    for f in FILES {
        let p = root.join(f);
        let Ok(meta) = std::fs::metadata(&p) else { continue };
        if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        let Ok(text) = lapce_index::symbols::php_without_comments(&text) else { continue };
        if let Some(i) = text.lines().position(|l| declares(l, name)) {
            return Some((p, i));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(root: &Path, rel: &str, text: &str) -> PathBuf {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
        p
    }

    fn at(line: &str, needle: &str) -> usize {
        line.find(needle).unwrap() + 1
    }

    #[test]
    fn alias_quotes_and_exact_middleware_argument_context() {
        for line in ["->middleware('auth')", "Route::middleware(['auth', 'verified'])",
            "->withoutMiddleware('auth:sanctum')"] {
            let open = line.find("'auth").unwrap();
            for col in [open, open + 1, open + 4] {
                assert_eq!(alias_at(line, col), Some("auth"), "{line} at {col}");
            }
        }
        for line in ["customMiddleware('auth')", "->notMiddleware('auth')",
            "->middleware(foo('auth'))", "->middleware($name, 'auth')",
            "->middleware('guest')->name('auth')", "->middleware(['guest'], 'auth')"] {
            assert_eq!(alias_at(line, line.find("auth").unwrap()), None, "{line}");
        }
    }

    #[test]
    fn aliases_open_their_declaration() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let app = put(r, "bootstrap/app.php", "<?php\nreturn Application::configure()\n    ->withMiddleware(function ($m) {\n        $m->alias([\n            'admin' => AdminOnly::class,\n        ]);\n    })");
        let kernel = put(r, "app/Http/Kernel.php", "<?php\nclass Kernel {\n    protected $middlewareAliases = [\n        'auth' => \\App\\Http\\Middleware\\Authenticate::class,\n        \"verified\"=>V::class,\n    ];\n}");
        let go = |l: &str, n: &str| resolve(r, l, at(l, n));
        let l = "Route::get('/a', fn() => 1)->middleware('admin');";
        assert_eq!(go(l, "admin"), Some((app, 4)));
        let l = "Route::middleware(['auth', 'verified'])->group(function () {";
        assert_eq!(go(l, "auth"), Some((kernel.clone(), 3)));
        assert_eq!(go(l, "verified"), Some((kernel.clone(), 4)));
        let l = "->middleware('auth:sanctum')";
        assert_eq!(go(l, "auth"), Some((kernel, 3)));
    }

    #[test]
    fn other_strings_are_ignored() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "app/Http/Kernel.php", "<?php\n'auth' => A::class,\n'x' => B::class,\n");
        for (l, n) in [
            ("Route::get('auth', fn() => 1);", "auth"),
            ("->middleware('auth')->name('x');", "x"),
            ("->middleware('guest')", "guest"),
            ("->middleware(Auth::class)", "Auth"),
            ("->middleware('App\\\\Http\\\\Mw')", "Http"),
        ] {
            assert_eq!(resolve(r, l, at(l, n)), None, "{l}");
        }
    }

    #[test]
    fn commented_alias_mappings_do_not_hide_the_live_declaration() {
        let d = tempfile::tempdir().unwrap();
        let path = put(d.path(), "app/Http/Kernel.php", "<?php\nclass Kernel {\n protected $middlewareAliases = [\n // 'auth' => Old::class,\n /*\n 'auth' => BlockOld::class,\n */\n # 'auth' => HashOld::class,\n 'auth' => LiveAuth::class,\n ];\n}\n");
        let line = "->middleware('auth')";
        assert_eq!(resolve(d.path(), line, at(line, "auth")), Some((path, 8)));
    }

    #[test]
    fn odd_input_does_not_panic() {
        let d = tempfile::tempdir().unwrap();
        for l in ["'", "middleware('", "middleware('é", "", "->middleware('日本')"] {
            for c in 0..=l.len() + 2 {
                let _ = resolve(d.path(), l, c);
            }
        }
    }
}
