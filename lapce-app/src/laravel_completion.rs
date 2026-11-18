//! Offline Laravel facade fallback. Never a substitute for a resolved LSP type.
use lsp_types::{CompletionItem, CompletionItemKind, CompletionItemLabelDetails};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
struct Imports {
    namespaced: bool,
    aliases: HashMap<String, Option<String>>,
    shadows: HashSet<String>,
}
impl Imports {
    fn add(&mut self, clause: &[&str], prefix: &str) {
        if clause.first().is_some_and(|s| s.eq_ignore_ascii_case("function") || s.eq_ignore_ascii_case("const")) { return; }
        let (path, alias) = match clause {
            [path] => (*path, path.trim_end_matches('\\').rsplit('\\').next().unwrap_or("")),
            [path, keyword, alias] if keyword.eq_ignore_ascii_case("as") => (*path, *alias),
            _ => return,
        };
        if path.ends_with('\\') || path.is_empty() || alias.is_empty() { return; }
        let alias = alias.to_ascii_lowercase();
        let value = format!("{prefix}{}", path.trim_start_matches('\\'));
        self.aliases.entry(alias).and_modify(|v| *v = None).or_insert(Some(value));
    }
    fn declaration(&mut self, tokens: &[&str]) {
        if tokens.first().is_some_and(|s| s.eq_ignore_ascii_case("function") || s.eq_ignore_ascii_case("const")) { return; }
        if let Some(open) = tokens.iter().position(|t| *t == "{") {
            if open != 1 || tokens.last() != Some(&"}") || !tokens[0].ends_with('\\') { return; }
            for clause in tokens[open + 1..tokens.len() - 1].split(|t| *t == ",") {
                self.add(clause, tokens[0].trim_start_matches('\\'));
            }
        } else {
            for clause in tokens.split(|t| *t == ",") { self.add(clause, ""); }
        }
    }
}

/// Imports live at file/namespace level, never inside traits or closures.
/// Reset on every namespace block, even another block with the same name.
fn scoped_imports(code: &str) -> Imports {
    static TOKENS: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let tokens: Vec<_> = TOKENS.get_or_init(|| regex::Regex::new(r"\\?[A-Za-z_][A-Za-z0-9_\\]*|[^\s]").unwrap()).find_iter(code).map(|m| m.as_str()).collect();
    let mut state = Imports::default();
    let (mut i, mut depth, mut base, mut block) = (0, 0usize, 0usize, false);
    while i < tokens.len() {
        let token = tokens[i];
        if depth == 0 && token.eq_ignore_ascii_case("namespace") {
            let mut end = i + 1;
            while end < tokens.len() && !matches!(tokens[end], ";" | "{") { end += 1; }
            if end == tokens.len() { break; }
            state = Imports::default();
            state.namespaced = end > i + 1;
            block = tokens[end] == "{";
            base = usize::from(block); depth = base; i = end + 1; continue;
        }
        if depth == base && token.eq_ignore_ascii_case("use") && tokens.get(i + 1) != Some(&"(") {
            if let Some(end) = tokens[i + 1..].iter().position(|t| *t == ";") {
                let end = i + 1 + end;
                state.declaration(&tokens[i + 1..end]); i = end + 1; continue;
            }
        }
        if depth == base && ["class", "interface", "trait", "enum"].iter().any(|k| token.eq_ignore_ascii_case(k)) {
            if let Some(name) = tokens.get(i + 1) { state.shadows.insert(name.to_ascii_lowercase()); }
        }
        if token == "{" { depth += 1; }
        if token == "}" {
            depth = depth.saturating_sub(1);
            if block && depth < base { state = Imports::default(); base = 0; block = false; }
        }
        i += 1;
    }
    state
}


pub fn facade_items(before: &str, laravel: bool) -> Option<Vec<CompletionItem>> {
    if !laravel { return None; }
    let code = lapce_core::web_templates::php_completion_code(before)?;
    let before = code.as_str();
    let prefix = before.trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_');
    let receiver_end = prefix.trim_end().strip_suffix("::")?.trim_end();
    let start = receiver_end.rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '\\')).map_or(0, |i| i+1);
    let receiver = &receiver_end[start..];
    if start > 0 && receiver_end[..start].ends_with('$') { return None; }
    let state = scoped_imports(before);
    let receiver_path = receiver.trim_start_matches('\\');
    let resolved = if receiver.starts_with('\\') {
        receiver_path.to_string()
    } else {
        let (root, suffix) = receiver_path.split_once('\\').unwrap_or((receiver_path, ""));
        if state.shadows.contains(&root.to_ascii_lowercase()) { return None; }
        match state.aliases.get(&root.to_ascii_lowercase()) {
            Some(Some(path)) => if suffix.is_empty() { path.clone() } else { format!("{path}\\{suffix}") },
            Some(None) => return None,
            None if state.namespaced => return None,
            None => receiver_path.to_string(),
        }
    };
    let facade = ["Route", "Auth", "Cache", "DB", "Schema", "Storage", "Log", "Request", "Validator", "Response", "App", "Config", "View", "Session", "Redirect", "URL"].into_iter().find(|name| {
        resolved.eq_ignore_ascii_case(name)
            || resolved.eq_ignore_ascii_case(&format!("Illuminate\\Support\\Facades\\{name}"))
    })?;
    let canonical = format!("Illuminate\\Support\\Facades\\{facade}");
    static DATA: std::sync::OnceLock<std::collections::HashMap<&'static str, Vec<serde_json::Value>>> = std::sync::OnceLock::new();
    let packs = DATA.get_or_init(|| [
        ("Route", include_str!("../../metadata/laravel/route-methods.json")),
        ("Auth", include_str!("../../metadata/laravel/auth-methods.json")),
        ("Cache", include_str!("../../metadata/laravel/cache-methods.json")),
        ("DB", include_str!("../../metadata/laravel/db-methods.json")),
        ("Schema", include_str!("../../metadata/laravel/schema-methods.json")),
        ("Storage", include_str!("../../metadata/laravel/storage-methods.json")),
        ("Log", include_str!("../../metadata/laravel/log-methods.json")),
        ("Request", include_str!("../../metadata/laravel/request-methods.json")),
        ("Validator", include_str!("../../metadata/laravel/validator-methods.json")),
        ("Response", include_str!("../../metadata/laravel/response-methods.json")),
        ("App", include_str!("../../metadata/laravel/app-methods.json")),
        ("Config", include_str!("../../metadata/laravel/config-methods.json")),
        ("View", include_str!("../../metadata/laravel/view-methods.json")),
        ("Session", include_str!("../../metadata/laravel/session-methods.json")),
        ("Redirect", include_str!("../../metadata/laravel/redirect-methods.json")),
        ("URL", include_str!("../../metadata/laravel/url-methods.json")),



    ].into_iter().map(|(name, json)| (name, serde_json::from_str(json).expect("checked facade pack"))).collect());
    let data = &packs[facade];
    Some(data.iter().map(|v| {
        let name=v["name"].as_str().unwrap().to_string();
        CompletionItem { label:name.clone(), kind:Some(CompletionItemKind::METHOD),
            label_details:Some(CompletionItemLabelDetails { detail:None, description:Some(format!("{canonical}::{name}")) }),
            detail:Some(format!("Laravel 12 baseline | {}",v["signature"].as_str().unwrap())),
            insert_text:Some(name), ..Default::default() }
    }).collect())
}
#[cfg(test)]
mod tests {
 use super::*;
 #[test] fn route_bare_partial_alias_and_qualified_members() {
  for before in ["<?php Route::", "<?php Route::ge", "<?php \\Illuminate\\Support\\Facades\\Route::", "<?php\nuse Illuminate\\Support\\Facades\\Route as Router;\nRouter::"] {
   let items=facade_items(before,true).unwrap();let get=items.iter().find(|i|i.label=="get").unwrap();
   assert_eq!(get.insert_text.as_deref(),Some("get"));assert!(get.label_details.as_ref().unwrap().description.as_ref().unwrap().ends_with("Facades\\Route::get"));
   assert!(get.detail.as_ref().unwrap().contains("$uri"));
  }
 }
 #[test] fn rejects_strings_comments_nonlaravel_shadowed_and_wrong_receivers() {
  for b in ["<?php 'Route::", "<?php // Route::", "<?php /* Route::", "<div>Route::", "<?php OtherRoute::", "<?php use App\\Route; Route::", "<?php $Route::", "<?php\nuse App\\Route;\nRoute::", "<?php class Route {} Route::"] { assert!(facade_items(b,true).is_none(),"{b}"); }
  assert!(facade_items("<?php Route::",false).is_none());
 }
}

#[cfg(test)]
mod hardening_tests {
 use super::*;
 #[test] fn comments_and_strings_cannot_forge_import_or_shadow_real_alias() {
  assert!(facade_items("<?php\n/*\nuse Illuminate\\Support\\Facades\\Route as Router;\n*/\nRouter::",true).is_none());
  assert!(facade_items("<?php\nuse Illuminate\\Support\\Facades\\Route;\n/* class Route {} */\nRoute::",true).is_some());
  assert!(facade_items("<?php namespace App; Route::",true).is_none());
  assert!(facade_items("<?php namespace\nApp; Route::",true).is_none());
  assert!(facade_items("<?php\nuse App\\{Route};\nRoute::",true).is_none());
  assert!(facade_items("<?php namespace App; Illuminate\\Support\\Facades\\Route::",true).is_none());
  assert!(facade_items("<?php namespace App; \\Illuminate\\Support\\Facades\\Route::",true).is_some());
 }
}

#[cfg(test)]
mod facade_tests {
    use super::*;
    #[test]
    fn each_facade_has_own_signatures_aliases_and_path() {
        for (facade, method) in [("Auth", "user"), ("Cache", "remember"), ("DB", "table"), ("Schema", "create"), ("Storage", "put"), ("Log", "info"), ("Request", "input"), ("Validator", "make"), ("Response", "json"), ("App", "make"), ("Config", "get"), ("View", "make"), ("Session", "get"), ("Redirect", "route"), ("URL", "route")] {
            for code in [format!("<?php {facade}::"), format!("<?php use Illuminate\\Support\\Facades\\{facade} as Service; Service::"), format!("<?php namespace App; \\Illuminate\\Support\\Facades\\{facade}::")] {
                let items = facade_items(&code, true).unwrap();
                let item = items.iter().find(|i| i.label == method).unwrap();
                assert_eq!(item.insert_text.as_deref(), Some(method));
                assert_eq!(item.label_details.as_ref().unwrap().description.as_deref(), Some(format!("Illuminate\\Support\\Facades\\{facade}::{method}").as_str()));
                assert!(item.detail.as_ref().unwrap().contains("Laravel 12 baseline"));
                assert!(items.iter().all(|i| i.label_details.as_ref().unwrap().description.as_ref().unwrap().starts_with(&format!("Illuminate\\Support\\Facades\\{facade}::"))));
            }
            for code in [format!("<?php use App\\{facade}; {facade}::"), format!("<?php class {facade} {{}} {facade}::"), format!("<?php namespace App; {facade}::"), format!("<?php // {facade}::"), format!("<?php '{facade}::")] {
                assert!(facade_items(&code, true).is_none(), "{code}");
            }
        }
    }
}

#[cfg(test)]
mod import_scope_tests {
 use super::*;
 #[test] fn grouped_multiple_namespace_and_typed_imports() {
  for code in [
   "<?php namespace App; use Illuminate\\Support\\Facades\\{Route as Router, Cache}; Router::",
   "<?php use App\\Thing, Illuminate\\Support\\Facades\\Route; Route::",
   "<?php namespace App; use Illuminate\\Support\\Facades as Facades; Facades\\DB::",
   "<?php namespace App { use Illuminate\\Support\\Facades\\{const X, function f, Auth}; Auth::",
   "<?php use function App\\Route; Route::",
   "<?php use const App\\Route; Route::",
  ] { assert!(facade_items(code,true).is_some(),"{code}"); }
 }
 #[test] fn imports_do_not_leak_across_namespaces_or_from_nested_use() {
  for code in [
   "<?php namespace A; use Illuminate\\Support\\Facades\\Route; namespace B; Route::",
   "<?php namespace A { use Illuminate\\Support\\Facades\\Route; } namespace B { Route::",
   "<?php namespace A { use Illuminate\\Support\\Facades\\Route; } namespace A { Route::",
   "<?php namespace A; class Thing { use Illuminate\\Support\\Facades\\Route; } Route::",
   "<?php namespace A; function thing() { use Illuminate\\Support\\Facades\\Route; } Route::",
   "<?php use App\\{Route, Cache}; Route::",
   "<?php use App\\Route, Illuminate\\Support\\Facades\\Route; Route::",
  ] { assert!(facade_items(code,true).is_none(),"{code}"); }
 }
 #[test] fn absolute_names_ignore_imports_and_other_namespaces_shadows() {
  assert!(facade_items("<?php namespace A; class Route {} namespace B; use Illuminate\\Support\\Facades\\Route; Route::",true).is_some());
  assert!(facade_items("<?php use App\\Route; \\Illuminate\\Support\\Facades\\Route::",true).is_some());
 }
}

#[cfg(test)]
mod mixed_markup_tests {
 use super::*;
 #[test] fn html_text_cannot_forge_a_php_import() {
  assert!(facade_items("<p>use Illuminate\\Support\\Facades\\Route as Router;</p><?php Router::",true).is_none());
  assert!(facade_items("<p>namespace App; class Route {}</p><?php Route::",true).is_some());
  assert!(facade_items("<p>anything</p><?php use Illuminate\\Support\\Facades\\{Route as Router}; Router::",true).is_some());
 }
}
