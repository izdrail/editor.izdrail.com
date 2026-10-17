//! Hover a Laravel named route: `route('users.index')` shows the verb, URI
//! and action from `routes/*.php`, plus the file and line. Combines
//! `route_names` (name -> definition line) with `route_index` (call ->
//! verb/URI/action), so resolution, callers and limits match the go-to.
//! Static text scan; nothing is executed. Group-assembled names are not
//! resolved (same limit as the go-to).

use std::path::Path;

const MAX_LINE: usize = 4096;
const MAX_FILE: u64 = 1 << 20;

/// The quoted string content covering byte `col`.
fn name_at(line: &str, col: usize) -> Option<String> {
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    for q in ['\'', '"'] {
        for (i, _) in line.match_indices(q) {
            let start = i + 1;
            let Some(len) = line[start..].find(q) else { break };
            let end = start + len;
            if col >= start && col <= end {
                let s = &line[start..end];
                return (s.len() <= 200).then(|| s.to_string());
            }
        }
    }
    None
}

/// Markdown hover for the route name at byte `col` of `line`.
pub fn help(root: Option<&Path>, line: &str, col: usize) -> Option<String> {
    let root = root?;
    if line.len() > MAX_LINE {
        return None;
    }
    let name = name_at(line, col)?;
    let (path, idx) = crate::route_names::resolve(root, line, col)?;
    // Laravel route files only; Symfony targets are left to their own help.
    if !path.extension().is_some_and(|e| e == "php") {
        return None;
    }
    let rel = path.strip_prefix(root).unwrap_or(&path);
    if !rel.starts_with("routes") {
        return None;
    }
    let meta = std::fs::metadata(&path).ok()?;
    if meta.len() > MAX_FILE {
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    let routes = crate::route_index::parse_routes(&text);
    // The enclosing call: the greatest Route:: line at or before the name
    // line. A resource expands to rows that share one line: collapse them.
    let call_line = routes
        .iter()
        .filter(|r| r.line <= idx)
        .map(|r| r.line)
        .max()?;
    let rows: Vec<_> = routes.iter().filter(|r| r.line == call_line).collect();
    let first = rows.first()?;
    // A resource expansion shares one line AND one controller; two separate
    // calls on one physical line do not count as a resource.
    let same_controller = rows.iter().all(|r| {
        r.action.as_ref().map(|(c, _)| c) == first.action.as_ref().map(|(c, _)| c)
    });
    let (what, action) = if rows.len() > 1 && same_controller {
        let class = first
            .action
            .as_ref()
            .map(|(c, _)| c.as_str())
            .unwrap_or("?");
        (
            format!("`{} {}` (resource)", first.verb, first.uri),
            format!("{class} ({} actions)", rows.len()),
        )
    } else {
        let action = match &first.action {
            Some((class, method)) if method.is_empty() => class.clone(),
            Some((class, method)) => format!("{class}@{method}"),
            None => "closure or view".to_string(),
        };
        (format!("`{} {}`", first.verb, first.uri), action)
    };
    Some(format!(
        "**Route** `{name}` - {what}\n\n{action}\n\n_{}:{}\n\nGroup prefixes are not tracked; the URI shows as written._",
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
    fn named_routes_are_explained() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "routes/web.php", "<?php\nuse App\\Http\\Controllers\\UserController;\nRoute::get('/users', [UserController::class, 'index'])->name('users.index');\nRoute::post('/users', [UserController::class, 'store'])\n    ->name('users.store');\nRoute::resource('photos', PhotoController::class);\n");
        let root = Some(r);

        let l = "return to_route('users.index');";
        let md = help(root, l, at(l, "users.index")).unwrap();
        assert!(md.contains("`GET /users`") && md.contains("UserController@index"), "{md}");
        assert!(md.contains("routes/web.php:3"), "{md}");

        // Name on its own line still finds the enclosing call.
        let l = "route('users.store')";
        let md = help(root, l, at(l, "users.store")).unwrap();
        assert!(md.contains("`POST /users`") && md.contains("UserController@store"), "{md}");
        assert!(md.contains("routes/web.php:5"), "{md}");

        // Implicit resource names point at the resource declaration.
        let l = "route('photos.edit', $photo)";
        let md = help(root, l, at(l, "photos.edit")).unwrap();
        assert!(md.contains("resource") && md.contains("PhotoController") && md.contains("7 actions"), "{md}");

        // Unknown name, no call, no workspace: nothing.
        let l = "route('nope.nope')";
        assert_eq!(help(root, l, at(l, "nope")), None);
        let l = "Str::slug('users.index')";
        assert_eq!(help(root, l, at(l, "users.index")), None);
        let l = "route('users.index')";
        assert_eq!(help(None, l, at(l, "users.index")), None);
    }

    #[test]
    fn odd_input_does_not_panic() {
        let d = tempfile::tempdir().unwrap();
        let root = Some(d.path());
        for l in ["route('", "route('é.x')", "route('日本')", "'", "\"", "route()"] {
            for c in 0..=l.len() + 2 {
                let _ = help(root, l, c);
            }
        }
    }
}
