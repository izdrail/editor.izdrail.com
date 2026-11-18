//! Offline Laravel Route facade fallback. Never a substitute for a resolved LSP type.
use lsp_types::{CompletionItem, CompletionItemKind, CompletionItemLabelDetails};

pub fn route_items(before: &str, laravel: bool) -> Option<Vec<CompletionItem>> {
    if !laravel { return None; }
    let code = lapce_core::web_templates::php_completion_code(before)?;
    let before = code.as_str();
    let prefix = before.trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_');
    let receiver_end = prefix.trim_end().strip_suffix("::")?.trim_end();
    let start = receiver_end.rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '\\')).map_or(0, |i| i+1);
    let receiver = &receiver_end[start..];
    if start > 0 && receiver_end[..start].ends_with('$') { return None; }
    static IMPORTS: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let imports = IMPORTS.get_or_init(||regex::Regex::new(r"(?i)\buse\s+([^;{}]+)\s*;").unwrap());
    let mut imported = None;
    let mut unsupported_import = false;
    for cap in imports.captures_iter(before) {
        let parts: Vec<_> = cap[1].split_whitespace().collect();
        let Some(path) = parts.first() else { continue; };
        if cap[1].contains(',') || !(parts.len() == 1 || (parts.len() == 3 && parts[1].eq_ignore_ascii_case("as"))) { unsupported_import = true; continue; }
        let alias = if parts.len()==3 && parts[1].eq_ignore_ascii_case("as") { parts[2] } else { path.rsplit('\\').next().unwrap_or("") };
        if alias.eq_ignore_ascii_case(receiver) { imported=Some(path.trim_start_matches('\\').to_string()); }
    }
    let canonical = "Illuminate\\Support\\Facades\\Route";
    static NAMESPACE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let namespaced = NAMESPACE.get_or_init(|| regex::Regex::new(r"(?i)\bnamespace\s").unwrap()).is_match(before);
    // Grouped imports are deliberately not resolved. Do not assume a global alias.
    static GROUPED: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    unsupported_import |= GROUPED.get_or_init(|| regex::Regex::new(r"(?i)\buse\s+[^;]*\{").unwrap()).is_match(before);
    static SHADOW: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let valid = (receiver.trim_start_matches('\\').eq_ignore_ascii_case(canonical) && (receiver.starts_with('\\') || !namespaced))
        || imported.as_deref().is_some_and(|p|p.eq_ignore_ascii_case(canonical))
        || (receiver.eq_ignore_ascii_case("Route") && imported.is_none()
            && !namespaced && !unsupported_import
            && !SHADOW.get_or_init(|| regex::Regex::new(r"(?i)\b(?:class|interface|trait|enum)\s+Route\b").unwrap()).is_match(before));
    if !valid { return None; }
    static DATA: std::sync::OnceLock<Vec<serde_json::Value>> = std::sync::OnceLock::new();
    let data = DATA.get_or_init(||serde_json::from_str(include_str!("../../metadata/laravel/route-methods.json")).expect("checked Route facade pack"));
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
   let items=route_items(before,true).unwrap();let get=items.iter().find(|i|i.label=="get").unwrap();
   assert_eq!(get.insert_text.as_deref(),Some("get"));assert!(get.label_details.as_ref().unwrap().description.as_ref().unwrap().ends_with("Facades\\Route::get"));
   assert!(get.detail.as_ref().unwrap().contains("$uri"));
  }
 }
 #[test] fn rejects_strings_comments_nonlaravel_shadowed_and_wrong_receivers() {
  for b in ["<?php 'Route::", "<?php // Route::", "<?php /* Route::", "<div>Route::", "<?php OtherRoute::", "<?php use App\\Route; Route::", "<?php $Route::", "<?php\nuse App\\Route;\nRoute::", "<?php class Route {} Route::"] { assert!(route_items(b,true).is_none(),"{b}"); }
  assert!(route_items("<?php Route::",false).is_none());
 }
}

#[cfg(test)]
mod hardening_tests {
 use super::*;
 #[test] fn comments_and_strings_cannot_forge_import_or_shadow_real_alias() {
  assert!(route_items("<?php\n/*\nuse Illuminate\\Support\\Facades\\Route as Router;\n*/\nRouter::",true).is_none());
  assert!(route_items("<?php\nuse Illuminate\\Support\\Facades\\Route;\n/* class Route {} */\nRoute::",true).is_some());
  assert!(route_items("<?php namespace App; Route::",true).is_none());
  assert!(route_items("<?php namespace\nApp; Route::",true).is_none());
  assert!(route_items("<?php\nuse App\\{Route};\nRoute::",true).is_none());
  assert!(route_items("<?php namespace App; Illuminate\\Support\\Facades\\Route::",true).is_none());
  assert!(route_items("<?php namespace App; \\Illuminate\\Support\\Facades\\Route::",true).is_some());
 }
}
