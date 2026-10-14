//! A static index of Laravel and Symfony routes for "Go to Route".
//!
//! Reads `routes/*.php` in the workspace root (bounded) and lists each
//! `Route::get/post/.../resource(...)` call with its URI and controller action,
//! resolving `Controller::class` through the file's `use` lines to
//! `app/...php` (PSR-4 `App\` convention) when that file and method exist.
//! Nothing is executed. Group prefixes, names and middleware are not tracked,
//! so URIs show as written in the call.
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

const MAX_FILE_BYTES: u64 = 1024 * 1024;
const MAX_FILES: usize = 20;
const MAX_ROUTES: usize = 5000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub verb: String,
    pub uri: String,
    /// Controller short or qualified name as written, and the method.
    pub action: Option<(String, String)>,
    /// 0-based line of the `Route::` call.
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteTarget {
    pub label: String,
    pub path: PathBuf,
    /// 0-based line to jump to.
    pub line: usize,
}

const VERBS: &[&str] = &[
    "get", "post", "put", "patch", "delete", "options", "any", "match", "view",
    "resource", "apiResource",
];

struct Scan<'a> {
    s: &'a [u8],
    i: usize,
}

impl Scan<'_> {
    fn skip_ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) -> bool {
        self.skip_ws();
        if self.s.get(self.i) == Some(&c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn string(&mut self) -> Option<String> {
        self.skip_ws();
        let q = *self.s.get(self.i)?;
        if q != b'\'' && q != b'"' {
            return None;
        }
        let start = self.i + 1;
        let mut j = start;
        while j < self.s.len() && self.s[j] != q && self.s[j] != b'\n' {
            j += 1;
        }
        if self.s.get(j) != Some(&q) {
            return None;
        }
        self.i = j + 1;
        Some(String::from_utf8_lossy(&self.s[start..j]).into_owned())
    }
    fn ident(&mut self) -> String {
        self.skip_ws();
        let start = self.i;
        while self.i < self.s.len()
            && (self.s[self.i].is_ascii_alphanumeric()
                || self.s[self.i] == b'_'
                || self.s[self.i] == b'\\')
        {
            self.i += 1;
        }
        String::from_utf8_lossy(&self.s[start..self.i]).into_owned()
    }
}

/// `[Foo::class, 'bar']`, `'Foo@bar'` or `Foo::class` at the scan position.
fn action(sc: &mut Scan) -> Option<(String, String)> {
    sc.skip_ws();
    if let Some(text) = sc.string() {
        let (c, m) = text.split_once('@')?;
        return Some((c.to_string(), m.to_string()));
    }
    if sc.eat(b'[') {
        let class = sc.ident();
        sc.skip_ws();
        if !sc.s[sc.i..].starts_with(b"::class") {
            return None;
        }
        sc.i += "::class".len();
        if !sc.eat(b',') {
            return None;
        }
        let method = sc.string()?;
        return Some((class, method));
    }
    let class = sc.ident();
    if class.is_empty() {
        return None;
    }
    sc.skip_ws();
    if sc.s[sc.i..].starts_with(b"::class") {
        return Some((class, "__invoke".to_string()));
    }
    None
}

/// Routes declared in one PHP file's text.
pub fn parse_routes(text: &str) -> Vec<Route> {
    let bytes = text.as_bytes();
    let mut routes = Vec::new();
    let mut from = 0;
    while let Some(pos) = text[from..].find("Route::") {
        let at = from + pos;
        from = at + "Route::".len();
        let mut sc = Scan { s: bytes, i: from };
        let verb = sc.ident();
        if !VERBS.contains(&verb.as_str()) || !sc.eat(b'(') {
            continue;
        }
        let line = text[..at].bytes().filter(|b| *b == b'\n').count();
        let mut verb_label = verb.to_ascii_uppercase();
        if verb == "match" {
            // First argument: ['get', 'post'].
            if !sc.eat(b'[') {
                continue;
            }
            let mut verbs = Vec::new();
            while let Some(v) = sc.string() {
                verbs.push(v.to_ascii_uppercase());
                if !sc.eat(b',') {
                    break;
                }
            }
            if !sc.eat(b']') || !sc.eat(b',') {
                continue;
            }
            verb_label = verbs.join("|");
        }
        let Some(uri) = sc.string() else { continue };
        let uri = if uri.starts_with('/') { uri } else { format!("/{uri}") };
        let act = if sc.eat(b',') { action(&mut sc) } else { None };
        match verb.as_str() {
            "resource" | "apiResource" => {
                let Some((class, _)) = act else {
                    routes.push(Route { verb: verb_label, uri, action: None, line });
                    continue;
                };
                let base = uri.trim_end_matches('/').to_string();
                let param = format!("{{{}}}", base.rsplit('/').next().unwrap_or("id"));
                let mut rows: Vec<(&str, String, &str)> = vec![
                    ("GET", base.clone(), "index"),
                    ("POST", base.clone(), "store"),
                    ("GET", format!("{base}/{param}"), "show"),
                    ("PUT|PATCH", format!("{base}/{param}"), "update"),
                    ("DELETE", format!("{base}/{param}"), "destroy"),
                ];
                if verb == "resource" {
                    rows.insert(1, ("GET", format!("{base}/create"), "create"));
                    rows.push(("GET", format!("{base}/{param}/edit"), "edit"));
                }
                for (v, u, m) in rows {
                    routes.push(Route {
                        verb: v.to_string(),
                        uri: u,
                        action: Some((class.clone(), m.to_string())),
                        line,
                    });
                }
            }
            _ => routes.push(Route { verb: verb_label, uri, action: act, line }),
        }
        if routes.len() >= MAX_ROUTES {
            break;
        }
    }
    routes
}

/// `use Foo\Bar\Baz;` / `use Foo\Bar\Baz as Q;` lines: short name -> qualified.
pub fn parse_uses(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("use ") else { continue };
        let rest = rest.trim_end_matches(';').trim();
        if rest.contains('{') || rest.starts_with("function ") || rest.starts_with("const ") {
            continue;
        }
        let (full, alias) = match rest.split_once(" as ") {
            Some((f, a)) => (f.trim(), a.trim()),
            None => (rest, rest.rsplit('\\').next().unwrap_or(rest)),
        };
        map.insert(alias.to_string(), full.trim_start_matches('\\').to_string());
    }
    map
}

/// The file and 0-based line of `class::method` under PSR-4 `App\` => `app/`.
pub fn controller_location(
    root: &Path,
    uses: &HashMap<String, String>,
    class: &str,
    method: &str,
) -> Option<(PathBuf, usize)> {
    let class = class.trim_start_matches('\\');
    let full = match uses.get(class) {
        Some(f) => f.clone(),
        None if class.contains('\\') => class.to_string(),
        None => format!("App\\Http\\Controllers\\{class}"),
    };
    let rel = full.strip_prefix("App\\")?;
    let path = root.join("app").join(format!("{}.php", rel.replace('\\', "/")));
    let meta = std::fs::metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    let needle = format!("function {method}");
    let line = text.lines().position(|l| {
        l.find(&needle).is_some_and(|i| {
            !l[i + needle.len()..]
                .starts_with(|c: char| c.is_alphanumeric() || c == '_')
        })
    })?;
    Some((path, line))
}

/// Every route found under `routes/`, each pointing at its controller method
/// when resolvable and at the route definition otherwise.
pub fn discover(root: &Path) -> Vec<RouteTarget> {
    let mut all = discover_laravel(root);
    all.extend(discover_symfony(root));
    all.truncate(MAX_ROUTES);
    all
}

fn discover_laravel(root: &Path) -> Vec<RouteTarget> {
    let Ok(dir) = std::fs::read_dir(root.join("routes")) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = dir
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "php"))
        .collect();
    files.sort();
    files.truncate(MAX_FILES);

    let mut out = Vec::new();
    for file in files {
        let Ok(meta) = std::fs::metadata(&file) else { continue };
        if meta.len() > MAX_FILE_BYTES {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&file) else { continue };
        let uses = parse_uses(&text);
        let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("routes");
        for route in parse_routes(&text) {
            let (label, path, line) = match &route.action {
                Some((class, method)) => {
                    let short = class.rsplit('\\').next().unwrap_or(class);
                    let label = format!("{} {}  {short}@{method}", route.verb, route.uri);
                    match controller_location(root, &uses, class, method) {
                        Some((p, l)) => (label, p, l),
                        None => (label, file.clone(), route.line),
                    }
                }
                None => (
                    format!("{} {}  ({name})", route.verb, route.uri),
                    file.clone(),
                    route.line,
                ),
            };
            out.push(RouteTarget { label, path, line });
            if out.len() >= MAX_ROUTES {
                return out;
            }
        }
    }
    out
}

// ------------------------------------------------------------- Symfony

/// One Symfony route attribute found in a controller file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymfonyRoute {
    pub verb: String,
    pub uri: String,
    /// The controller method (`None` for an invokable class-level route).
    pub method: Option<String>,
    /// 0-based line of the method (or class) the attribute decorates.
    pub line: usize,
}

fn first_path_arg(args: &str) -> Option<String> {
    let mut sc = Scan { s: args.as_bytes(), i: 0 };
    if let Some(s) = sc.string() {
        return Some(s);
    }
    // path: '/x'
    let idx = args.find("path:").or_else(|| args.find("path :"))?;
    let mut sc = Scan { s: args.as_bytes(), i: idx + "path".len() };
    sc.skip_ws();
    sc.i += 1; // ':'
    sc.string()
}

fn methods_arg(args: &str) -> String {
    let Some(idx) = args.find("methods:") else {
        return "ANY".to_string();
    };
    let rest = &args[idx + "methods:".len()..];
    let Some(end) = rest.find(']') else {
        return "ANY".to_string();
    };
    let mut sc = Scan { s: rest[..end].as_bytes(), i: 0 };
    let mut verbs = Vec::new();
    while sc.i < sc.s.len() {
        if let Some(v) = sc.string() {
            verbs.push(v.to_ascii_uppercase());
        } else {
            sc.i += 1;
        }
    }
    if verbs.is_empty() { "ANY".to_string() } else { verbs.join("|") }
}

/// `#[Route(...)]` attributes in one PHP file, with class-level prefixes applied.
pub fn parse_symfony_attributes(text: &str) -> Vec<SymfonyRoute> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut prefix = String::new();
    let mut from = 0;
    while let Some(pos) = text[from..].find("#[Route(") {
        let at = from + pos;
        let args_start = at + "#[Route(".len();
        // Matching ')' (strings may contain parentheses, so skip quoted text).
        let mut depth = 1;
        let mut j = args_start;
        let mut quote = 0u8;
        while j < bytes.len() && depth > 0 {
            let c = bytes[j];
            if quote != 0 {
                if c == quote { quote = 0; }
            } else if c == b'\'' || c == b'"' {
                quote = c;
            } else if c == b'(' {
                depth += 1;
            } else if c == b')' {
                depth -= 1;
            }
            j += 1;
        }
        if depth != 0 {
            break;
        }
        let args = &text[args_start..j - 1];
        from = j;
        // The declaration after this attribute (and any further attributes).
        let mut k = j;
        loop {
            while k < bytes.len() && bytes[k] != b'#' && !bytes[k].is_ascii_alphabetic() {
                k += 1;
            }
            if text[k..].starts_with("#[") {
                match text[k..].find(']') {
                    Some(e) => k += e + 1,
                    None => break,
                }
            } else {
                break;
            }
        }
        let decl_end = text[k..].find(['{', ';']).map(|e| k + e).unwrap_or(text.len());
        let decl = &text[k..decl_end];
        let line = text[..k].bytes().filter(|b| *b == b'\n').count();
        let path = first_path_arg(args).unwrap_or_default();
        let is_class = decl.split_whitespace().any(|w| w == "class")
            && !decl.contains("function");
        if is_class {
            prefix = path;
            continue;
        }
        let Some(fi) = decl.find("function") else { continue };
        let name: String = decl[fi + "function".len()..]
            .trim_start()
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() {
            continue;
        }
        let joined = format!("{}/{}", prefix.trim_end_matches('/'), path.trim_start_matches('/'));
        let uri = if joined == "/" { joined } else { joined.trim_end_matches('/').to_string() };
        out.push(SymfonyRoute {
            verb: methods_arg(args),
            uri: if uri.is_empty() { "/".into() } else { uri },
            method: Some(name),
            line,
        });
    }
    out
}

/// Routes from `config/routes.yaml`: `(path, controller class, method, line)`.
pub fn parse_routes_yaml(text: &str) -> Vec<(String, String, Option<String>, usize)> {
    let mut out = Vec::new();
    let mut path: Option<(String, usize)> = None;
    let mut controller: Option<String> = None;
    let mut flush = |path: &mut Option<(String, usize)>, controller: &mut Option<String>| {
        if let (Some((p, line)), Some(c)) = (path.take(), controller.take()) {
            let (class, method) = match c.split_once("::") {
                Some((a, b)) => (a.to_string(), Some(b.to_string())),
                None => (c, None),
            };
            out.push((p, class, method, line));
        }
        *path = None;
        *controller = None;
    };
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let indented = line.starts_with(' ') || line.starts_with('\t');
        if !indented {
            flush(&mut path, &mut controller);
            continue;
        }
        let l = line.trim();
        if let Some(v) = l.strip_prefix("path:") {
            path = Some((v.trim().trim_matches(['\'', '"']).to_string(), n));
        } else if let Some(v) = l.strip_prefix("controller:") {
            controller = Some(v.trim().trim_matches(['\'', '"']).replace("\\\\", "\\"));
        }
    }
    flush(&mut path, &mut controller);
    out
}

fn collect_php(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 6 || out.len() >= 500 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = rd.filter_map(Result::ok).map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            collect_php(&p, depth + 1, out);
        } else if p.extension().is_some_and(|e| e == "php") {
            out.push(p);
        }
    }
}

/// Symfony routes from `src/Controller` attributes and `config/routes.yaml`.
pub fn discover_symfony(root: &Path) -> Vec<RouteTarget> {
    let mut out = Vec::new();
    let mut files = Vec::new();
    collect_php(&root.join("src").join("Controller"), 0, &mut files);
    for file in files {
        let Ok(meta) = std::fs::metadata(&file) else { continue };
        if meta.len() > MAX_FILE_BYTES {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&file) else { continue };
        let class = file.file_stem().and_then(|s| s.to_str()).unwrap_or("Controller");
        for r in parse_symfony_attributes(&text) {
            let label = match &r.method {
                Some(m) => format!("{} {}  {class}@{m}", r.verb, r.uri),
                None => format!("{} {}  {class}", r.verb, r.uri),
            };
            out.push(RouteTarget { label, path: file.clone(), line: r.line });
            if out.len() >= MAX_ROUTES {
                return out;
            }
        }
    }
    let yaml = root.join("config").join("routes.yaml");
    if let Ok(meta) = std::fs::metadata(&yaml) {
        if meta.len() <= MAX_FILE_BYTES {
            if let Ok(text) = std::fs::read_to_string(&yaml) {
                for (uri, class, method, line) in parse_routes_yaml(&text) {
                    let short = class.rsplit('\\').next().unwrap_or(&class).to_string();
                    let label = match &method {
                        Some(m) => format!("ANY {uri}  {short}@{m}"),
                        None => format!("ANY {uri}  {short}"),
                    };
                    let target = class
                        .strip_prefix("App\\")
                        .map(|rel| root.join("src").join(format!("{}.php", rel.replace('\\', "/"))))
                        .filter(|p| p.is_file());
                    match target {
                        Some(p) => {
                            let l = method
                                .as_deref()
                                .and_then(|m| std::fs::read_to_string(&p).ok().and_then(|t| function_line(&t, m)))
                                .unwrap_or(0);
                            out.push(RouteTarget { label, path: p, line: l });
                        }
                        None => out.push(RouteTarget { label, path: yaml.clone(), line }),
                    }
                }
            }
        }
    }
    out
}

fn function_line(text: &str, method: &str) -> Option<usize> {
    let needle = format!("function {method}");
    text.lines().position(|l| {
        l.find(&needle).is_some_and(|i| {
            !l[i + needle.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_')
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROUTES: &str = r#"<?php
use App\Http\Controllers\UserController;
use App\Http\Controllers\Admin\ReportController as Reports;

Route::get('/users', [UserController::class, 'index'])->name('users');
Route::middleware('auth')->group(function () {
    Route::post("posts", 'PostController@store');
    Route::match(['get', 'post'], '/m', [Reports::class, 'show']);
});
Route::get('/closure', function () { return 1; });
Route::get('/invoke', InvokeController::class);
Route::apiResource('photos', PhotoController::class);
"#;

    #[test]
    fn parses_verbs_uris_actions_and_lines() {
        let r = parse_routes(ROUTES);
        assert_eq!(r[0].verb, "GET");
        assert_eq!(r[0].uri, "/users");
        assert_eq!(r[0].action, Some(("UserController".into(), "index".into())));
        assert_eq!(r[0].line, 4);
        assert_eq!(r[1].verb, "POST");
        assert_eq!(r[1].uri, "/posts");
        assert_eq!(r[1].action, Some(("PostController".into(), "store".into())));
        assert_eq!(r[2].verb, "GET|POST");
        assert_eq!(r[2].action, Some(("Reports".into(), "show".into())));
        assert_eq!(r[3].action, None);
        assert_eq!(r[4].action, Some(("InvokeController".into(), "__invoke".into())));
        // apiResource: index, store, show, update, destroy.
        let photos: Vec<_> = r[5..].iter().map(|x| x.action.clone().unwrap().1).collect();
        assert_eq!(photos, ["index", "store", "show", "update", "destroy"]);
        assert_eq!(r[7].uri, "/photos/{photos}");
    }

    #[test]
    fn resource_includes_create_and_edit() {
        let r = parse_routes("Route::resource('posts', PostController::class);");
        let names: Vec<_> = r.iter().map(|x| x.action.clone().unwrap().1).collect();
        assert_eq!(
            names,
            ["index", "create", "store", "show", "update", "destroy", "edit"]
        );
    }

    #[test]
    fn use_lines_map_short_names_and_aliases() {
        let u = parse_uses(ROUTES);
        assert_eq!(u["UserController"], "App\\Http\\Controllers\\UserController");
        assert_eq!(u["Reports"], "App\\Http\\Controllers\\Admin\\ReportController");
    }

    #[test]
    fn discovers_and_resolves_controller_methods() {
        let dir = tempfile::tempdir().unwrap();
        let routes = dir.path().join("routes");
        let ctl = dir.path().join("app/Http/Controllers");
        std::fs::create_dir_all(&routes).unwrap();
        std::fs::create_dir_all(&ctl).unwrap();
        std::fs::write(routes.join("web.php"), ROUTES).unwrap();
        std::fs::write(
            ctl.join("UserController.php"),
            "<?php\nclass UserController {\n  public function indexed() {}\n  public function index() {}\n}\n",
        )
        .unwrap();
        let found = discover(dir.path());
        let users = found.iter().find(|t| t.label.contains("/users")).unwrap();
        assert_eq!(users.label, "GET /users  UserController@index");
        assert!(users.path.ends_with("app/Http/Controllers/UserController.php"));
        assert_eq!(users.line, 3);
        // Unresolvable controller falls back to the route line.
        let post = found.iter().find(|t| t.label.contains("/posts")).unwrap();
        assert!(post.path.ends_with("routes/web.php"));
        assert_eq!(post.line, 6);
    }

    #[test]
    fn no_routes_dir_means_no_routes() {
        let dir = tempfile::tempdir().unwrap();
        assert!(discover(dir.path()).is_empty());
    }

    const CONTROLLER: &str = r#"<?php
namespace App\Controller;

#[Route('/blog', name: 'blog_')]
class BlogController extends AbstractController
{
    #[Route('/', name: 'index', methods: ['GET'])]
    public function index(): Response {}

    #[Route(path: '/{slug}', methods: ['GET', 'POST'])]
    #[IsGranted('ROLE_USER')]
    public function show(string $slug): Response {}
}
"#;

    #[test]
    fn symfony_attributes_apply_class_prefix_and_methods() {
        let r = parse_symfony_attributes(CONTROLLER);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].uri, "/blog");
        assert_eq!(r[0].verb, "GET");
        assert_eq!(r[0].method.as_deref(), Some("index"));
        assert_eq!(r[0].line, 7);
        assert_eq!(r[1].uri, "/blog/{slug}");
        assert_eq!(r[1].verb, "GET|POST");
        assert_eq!(r[1].method.as_deref(), Some("show"));
    }

    #[test]
    fn routes_yaml_pairs_path_and_controller() {
        let y = "# c\nhome:\n    path: /\n    controller: App\\Controller\\HomeController::index\n\nabout:\n    path: '/about'\n    controller: App\\Controller\\AboutController\n";
        let r = parse_routes_yaml(y);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].0, "/");
        assert_eq!(r[0].1, "App\\Controller\\HomeController");
        assert_eq!(r[0].2.as_deref(), Some("index"));
        assert_eq!(r[1].0, "/about");
        assert_eq!(r[1].2, None);
    }

    #[test]
    fn discovers_symfony_layout() {
        let dir = tempfile::tempdir().unwrap();
        let ctl = dir.path().join("src/Controller");
        std::fs::create_dir_all(&ctl).unwrap();
        std::fs::write(ctl.join("BlogController.php"), CONTROLLER).unwrap();
        let found = discover_symfony(dir.path());
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].label, "GET /blog  BlogController@index");
        assert!(found[0].path.ends_with("src/Controller/BlogController.php"));
        assert_eq!(found[0].line, 7);
    }
}
