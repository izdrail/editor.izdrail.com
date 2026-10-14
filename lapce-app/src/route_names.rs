//! Go to the definition of a named route from its use: Laravel
//! `route('users.index')`, `to_route(...)`, `->routeIs(...)` jump to the
//! `->name('users.index')` (or `'as' => ...`) line in `routes/*.php`; Symfony
//! `path('blog_index')`, `url(...)`, `generateUrl(...)`, `redirectToRoute(...)`
//! jump to `#[Route(... name: 'blog_index')]` in `src/Controller` or to the
//! route key in `config/routes*.yaml`.
//!
//! Textual: names assembled from a class-level prefix, `Route::resource` /
//! `apiResource` implicit names and group `->name('prefix.')` prefixes are not
//! resolved. Bounded: 1 MB per file, 60 route files, 2000 controller files.
use std::path::{Path, PathBuf};

const MAX_FILE_BYTES: u64 = 1024 * 1024;
const MAX_ROUTE_FILES: usize = 60;
const MAX_CONTROLLERS: usize = 2000;

const CALLERS: &[&str] = &[
    "route", "to_route", "routeIs", "Route::has", "URL::route", "path", "url",
    "generateUrl", "redirectToRoute", "generate",
];

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn string_at(line: &str, col: usize) -> Option<(&str, &str)> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\'' || bytes[i] == b'"' {
            let q = bytes[i] as char;
            let start = i + 1;
            let end = start + line[start..].find(q)?;
            if col >= start && col <= end {
                return Some((&line[start..end], &line[..i]));
            }
            i = end + 1;
        } else {
            i += 1;
        }
    }
    None
}

/// Route name when the string at `col` is the first argument of a route call
/// (plain call, `->call` or `Class::call`; not an unrelated identifier suffix).
fn route_name_at(line: &str, col: usize) -> Option<&str> {
    let (s, before) = string_at(line, col.min(line.len()))?;
    if s.is_empty() || s.len() > 150 || !s.chars().all(|c| is_ident(c) || matches!(c, '.' | '-' | ':')) {
        return None;
    }
    let t = before.trim_end().strip_suffix('(')?.trim_end();
    let hit = CALLERS.iter().any(|c| {
        t.strip_suffix(c).is_some_and(|rest| {
            // `{{ path(` / `$this->generateUrl(` / `Route::has(`: fine; `mypath(` is not.
            rest.chars().next_back().is_none_or(|p| !is_ident(p) && p != '$')
        })
    });
    hit.then_some(s)
}

fn read(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    (meta.is_file() && meta.len() <= MAX_FILE_BYTES)
        .then(|| std::fs::read_to_string(path).ok())
        .flatten()
}

fn has_quoted(l: &str, key: &str, name: &str) -> bool {
    // key('name') / key: 'name' / key="name" / 'key' => 'name'
    l.match_indices(key).any(|(i, _)| {
        let rest = l[i + key.len()..].trim_start_matches(['\'', '"']).trim_start();
        let rest = rest
            .strip_prefix('(')
            .or_else(|| rest.strip_prefix(':'))
            .or_else(|| rest.strip_prefix('='))
            .or_else(|| rest.strip_prefix("=>").map(|r| r))
            .unwrap_or("");
        let rest = rest.trim_start().trim_start_matches('>').trim_start();
        let Some(q) = rest.chars().next().filter(|q| *q == '\'' || *q == '"') else {
            return false;
        };
        rest[1..].strip_prefix(name).is_some_and(|r| r.starts_with(q))
    })
}

fn find_laravel(root: &Path, name: &str) -> Option<(PathBuf, usize)> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(root.join("routes"))
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "php"))
        .collect();
    files.sort();
    for p in files.into_iter().take(MAX_ROUTE_FILES) {
        let Some(text) = read(&p) else { continue };
        if !text.contains(name) {
            continue;
        }
        if let Some(i) = text
            .lines()
            .position(|l| has_quoted(l, "->name", name) || has_quoted(l, "'as'", name))
        {
            return Some((p, i));
        }
    }
    None
}

fn collect_php(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        if out.len() >= MAX_CONTROLLERS {
            return;
        }
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        let mut es: Vec<_> = rd.filter_map(Result::ok).collect();
        es.sort_by_key(|e| e.file_name());
        for e in es {
            let p = e.path();
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(p),
                Ok(t) if t.is_file() && p.extension().is_some_and(|x| x == "php") => out.push(p),
                _ => {}
            }
        }
    }
}

fn find_symfony(root: &Path, name: &str) -> Option<(PathBuf, usize)> {
    // YAML: a top-level key `name:` in config/routes.yaml or config/routes/*.yaml.
    let mut yamls = vec![root.join("config").join("routes.yaml")];
    if let Ok(rd) = std::fs::read_dir(root.join("config").join("routes")) {
        let mut v: Vec<_> = rd
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "yaml" || e == "yml"))
            .collect();
        v.sort();
        yamls.extend(v.into_iter().take(MAX_ROUTE_FILES));
    }
    for p in yamls {
        let Some(text) = read(&p) else { continue };
        let key = format!("{name}:");
        if let Some(i) = text.lines().position(|l| l.starts_with(&key)) {
            return Some((p, i));
        }
    }
    // Attributes / annotations in src/.
    let mut files = Vec::new();
    collect_php(&root.join("src"), &mut files);
    for p in files {
        let Some(text) = read(&p) else { continue };
        if !text.contains(name) || !text.contains("Route") {
            continue;
        }
        if let Some(i) = text.lines().position(|l| {
            l.contains("Route")
                && (has_quoted(l, "name", name) || has_quoted(l, "'name'", name))
        }) {
            return Some((p, i));
        }
    }
    None
}

/// Where the route named at byte `col` of `line` is defined.
pub fn resolve(root: &Path, line: &str, col: usize) -> Option<(PathBuf, usize)> {
    let name = route_name_at(line, col)?;
    find_laravel(root, name).or_else(|| find_symfony(root, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(line: &str, needle: &str) -> usize {
        line.find(needle).unwrap() + 1
    }

    fn put(root: &Path, rel: &str, text: &str) -> PathBuf {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
        p
    }

    #[test]
    fn classifies_route_calls() {
        fn c<'a>(l: &'a str, n: &str) -> Option<&'a str> {
            route_name_at(l, at(l, n))
        }
        assert_eq!(c("return redirect()->route('users.index');", "users"), Some("users.index"));
        assert_eq!(c("route(\"users.show\", $u)", "users"), Some("users.show"));
        assert_eq!(c("{{ path('blog_index') }}", "blog"), Some("blog_index"));
        assert_eq!(c("$this->generateUrl('blog_show', ['id' => 1])", "blog"), Some("blog_show"));
        assert_eq!(c("if (request()->routeIs('admin.*'))", "admin"), None);
        assert_eq!(c("route('users.show', 'x')", "'x'"), None);
        assert_eq!(c("myroute('users.index')", "users"), None);
        assert_eq!(c("$x = something('users.index');", "users"), None);
    }

    #[test]
    fn finds_laravel_and_symfony_definitions() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let web = put(r, "routes/web.php", "<?php\nRoute::get('/users', [UserController::class, 'index'])->name('users.index');\nRoute::get('/a', fn() => 1)->name( \"a.b\" );\nRoute::get('/c', fn() => 1)->name('users.index.other');\n");
        let y = put(r, "config/routes.yaml", "# c\nhome:\n    path: /\n");
        let c = put(r, "src/Controller/BlogController.php", "<?php\nclass BlogController {\n    #[Route('/blog', name: 'blog_index', methods: ['GET'])]\n    public function index() {}\n    /** @Route(\"/s\", name=\"blog_show\") */\n    public function show() {}\n}\n");
        let l = "route('users.index')";
        assert_eq!(resolve(r, l, at(l, "users")), Some((web.clone(), 1)));
        let l = "route('a.b')";
        assert_eq!(resolve(r, l, at(l, "a.b")), Some((web, 2)));
        let l = "path('home')";
        assert_eq!(resolve(r, l, at(l, "home")), Some((y, 1)));
        let l = "path('blog_index')";
        assert_eq!(resolve(r, l, at(l, "blog_")), Some((c.clone(), 2)));
        let l = "path('blog_show')";
        assert_eq!(resolve(r, l, at(l, "blog_")), Some((c, 4)));
        let l = "route('nope')";
        assert_eq!(resolve(r, l, at(l, "nope")), None);
        // A prefix of a longer name is not a match.
        let l = "route('users')";
        assert_eq!(resolve(r, l, at(l, "users")), None);
    }
}
