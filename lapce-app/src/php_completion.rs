//! Native PHP popup adapter. No parsing, filesystem access or UI state here.
use lapce_php::CursorContext;
use lapce_rpc::proxy::PhpIndexSymbol;
use lsp_types::{CompletionItem, CompletionItemKind as K};
#[derive(Clone, Debug, Default)]
pub struct Request {
    pub context: Option<CursorContext>,
    pub owner: Option<String>,
    pub allow_nonpublic: bool,
    pub namespace: bool,
    pub variables: Vec<String>,
    pub unsaved: Vec<PhpIndexSymbol>,
}
/// Only a bounded prefix is passed here. The current AST wins where available.
pub fn request(
    prefix: &str,
    context: Option<CursorContext>,
    class: Option<String>,
) -> Request {
    let trim = prefix.trim_end();
    let token = trim
        .rsplit(|c: char| !c.is_alphanumeric() && !matches!(c, '_' | '$' | '\\'))
        .next()
        .unwrap_or("");
    let before = trim.strip_suffix(token).unwrap_or(trim).trim_end();
    let context = context.or_else(|| {
        Some(if before.ends_with("->") {
            CursorContext::InstanceMember
        } else if before.ends_with("::") {
            CursorContext::StaticMember
        } else if token.starts_with('$') {
            CursorContext::Variable
        } else {
            let word = if prefix.ends_with(char::is_whitespace) {
                token
            } else {
                before.split_whitespace().last().unwrap_or("")
            };
            match word {
                "new" => CursorContext::New,
                "use" => CursorContext::Import,
                "extends" => CursorContext::Extends,
                "implements" => CursorContext::Implements,
                _ => CursorContext::Expression,
            }
        })
    });
    let receiver = before
        .strip_suffix("->")
        .or_else(|| before.strip_suffix("::"))
        .unwrap_or("")
        .trim_end()
        .rsplit(|c: char| !c.is_alphanumeric() && !matches!(c, '_' | '$' | '\\'))
        .next()
        .unwrap_or("");
    let chained = dynamic_receiver(prefix);
    let owner = if chained {
        None
    } else {
        match receiver {
            "$this" | "self" | "static" => class,
            _ if receiver.starts_with('\\') => {
                Some(receiver.trim_start_matches('\\').to_owned())
            }
            _ => None,
        }
    };
    Request {
        context,
        owner,
        allow_nonpublic: matches!(receiver, "$this" | "self" | "static" | "parent"),
        namespace: token.contains('\\'),
        variables: vec![],
        unsaved: vec![],
    }
}
pub fn immediate(prefix: &str) -> bool {
    let t = prefix.trim_end();
    t.ends_with('$')
        || t.ends_with("->")
        || t.ends_with("::")
        || t.ends_with('\\')
        || ((prefix.ends_with(' ') || prefix.ends_with('\t'))
            && matches!(
                t.split_whitespace().last(),
                Some("new" | "use" | "extends" | "implements")
            ))
}
pub fn bucket(context: &Option<CursorContext>, kind: Option<K>) -> u8 {
    use CursorContext::*;
    let Some(context) = context else { return 0 };
    let preferred = match context {
        Variable => matches!(kind, Some(K::VARIABLE | K::FIELD)),
        InstanceMember => matches!(kind, Some(K::METHOD | K::FIELD | K::PROPERTY)),
        StaticMember => matches!(
            kind,
            Some(K::METHOD | K::CONSTANT | K::FIELD | K::ENUM_MEMBER)
        ),
        New | Extends => kind == Some(K::CLASS),
        Implements => kind == Some(K::INTERFACE),
        Import | Type => {
            matches!(kind, Some(K::CLASS | K::INTERFACE | K::ENUM | K::MODULE))
        }
        Expression => matches!(kind, Some(K::FUNCTION | K::CLASS | K::VARIABLE)),
        NonPhp(_) => false,
    };
    if preferred { 0 } else { 1 }
}
fn symbols_alias_qualified(symbols: &[PhpIndexSymbol], qualified: &str) -> bool {
    symbols.iter().any(|s| {
        s.qualified == qualified
            && s.attributes.iter().any(|a| a == "laravel:local-scope")
    })
}
pub fn candidates(
    request: &Request,
    symbols: Vec<PhpIndexSymbol>,
) -> Vec<CompletionItem> {
    if matches!(request.context, Some(CursorContext::NonPhp(_))) {
        return vec![];
    }
    if matches!(request.context, Some(CursorContext::Variable)) {
        return request
            .variables
            .iter()
            .map(|name| CompletionItem {
                label: name.clone(),
                kind: Some(K::VARIABLE),
                insert_text: Some(name.clone()),
                detail: Some("native PHP local declaration".into()),
                ..Default::default()
            })
            .collect();
    }
    let mut symbols = symbols;
    symbols.retain(|s| {
        s.attributes.iter().any(|a| a == "laravel:local-scope")
            || !request
                .unsaved
                .iter()
                .any(|local| local.qualified == s.qualified)
    });
    let locals = request
        .unsaved
        .iter()
        .filter(|local| !symbols_alias_qualified(&symbols, &local.qualified))
        .cloned()
        .collect::<Vec<_>>();
    symbols.extend(locals);
    symbols
        .into_iter()
        .filter_map(|s| {
            let kind = match s.kind.as_str() {
                "class" => K::CLASS,
                "interface" => K::INTERFACE,
                "trait" => K::CLASS,
                "enum" => K::ENUM,
                "function" => K::FUNCTION,
                "method" => K::METHOD,
                "property" => K::FIELD,
                "constant" => K::CONSTANT,
                "namespace" => K::MODULE,
                _ => return None,
            };
            let member = matches!(
                request.context,
                Some(CursorContext::InstanceMember | CursorContext::StaticMember)
            );
            if member {
                if request.owner.is_none() || s.container.is_none() {
                    return None;
                }
                if !matches!(s.kind.as_str(), "method" | "property" | "constant") {
                    return None;
                }
                if matches!(request.context, Some(CursorContext::InstanceMember))
                    && (s.is_static || s.kind == "constant")
                    && !s.attributes.iter().any(|a| a == "laravel:local-scope")
                {
                    return None;
                }
                if matches!(request.context, Some(CursorContext::StaticMember))
                    && !s.is_static
                    && s.kind != "constant"
                {
                    return None;
                }
                // Without a full access checker, only public/implicit-public members.
                if !request.allow_nonpublic
                    && s.visibility.as_deref().is_some_and(|v| v != "public")
                {
                    return None;
                }
            } else if s.container.is_some() {
                return None;
            }
            match request.context {
                Some(CursorContext::New | CursorContext::Extends)
                    if s.kind != "class" =>
                {
                    return None;
                }
                Some(CursorContext::Implements) if s.kind != "interface" => {
                    return None;
                }
                Some(CursorContext::Import | CursorContext::Type)
                    if !matches!(
                        s.kind.as_str(),
                        "class" | "interface" | "trait" | "enum" | "namespace"
                    ) =>
                {
                    return None;
                }
                _ => {}
            }
            let name = if member
                && s.kind == "property"
                && matches!(request.context, Some(CursorContext::InstanceMember))
            {
                s.name.trim_start_matches('$').to_owned()
            } else if request.namespace {
                s.qualified.clone()
            } else {
                s.name.clone()
            };
            // Global declarations insert an absolute name, avoiding an accidental
            // reference to a same-named class in the current namespace.
            let insert = if matches!(request.context, Some(CursorContext::Import)) {
                s.qualified.clone()
            } else if member || s.kind == "namespace" {
                name.clone()
            } else {
                format!("\\{}", s.qualified)
            };
            Some(CompletionItem {
                label: name.clone(),
                kind: Some(kind),
                filter_text: Some(name),
                insert_text: Some(insert),
                detail: Some(format!("native PHP: {}", s.qualified)),
                ..Default::default()
            })
        })
        .collect()
}
/// Bounded line-only lookup. False evidence/context must preserve normal F12.
pub fn definition_member(
    laravel: bool,
    allowed: bool,
    line: &str,
    column: usize,
) -> Option<(String, String)> {
    if !laravel || !allowed {
        return None;
    }
    let end = line
        .get(column..)?
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .map_or(line.len(), |n| column + n);
    let prefix = line.get(..end)?;
    if request(prefix, None, None).context != Some(CursorContext::StaticMember) {
        return None;
    }
    let receiver = receiver(prefix);
    if receiver.is_empty() || receiver.starts_with('$') {
        return None;
    }
    let name = prefix
        .rsplit(|c: char| !(c.is_alphanumeric() || c == '_'))
        .next()?;
    (!name.is_empty()).then(|| (receiver.into(), name.into()))
}
/// Match quality remains Nucleo-owned. These are tie-breakers after its score.
pub fn boundary_bonus(label: &str, input: &str) -> u8 {
    let mut initials = String::new();
    let mut prev = None;
    for c in label.chars() {
        if prev.is_none()
            || prev == Some('_')
            || c.is_uppercase() && prev.is_some_and(|p: char| p.is_lowercase())
        {
            initials.extend(c.to_lowercase());
        }
        prev = Some(c);
    }
    if !input.is_empty() && initials.starts_with(&input.to_lowercase()) {
        1
    } else {
        0
    }
}
pub fn hidden(label: &str, input: &str) -> u8 {
    u8::from(label.starts_with('_') && !input.starts_with('_'))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn symbol(
        name: &str,
        kind: &str,
        owner: Option<&str>,
        is_static: bool,
    ) -> PhpIndexSymbol {
        PhpIndexSymbol {
            path: "A.php".into(),
            name: name.into(),
            qualified: format!("App\\{name}"),
            kind: kind.into(),
            namespace: "App".into(),
            container: owner.map(str::to_owned),
            visibility: Some("public".into()),
            is_static,
            alias: None,
            target: None,
            attributes: vec![],
            byte: 0,
            end: 1,
            line: 0,
            column: 0,
        }
    }
    #[test]
    fn native_definition_skips_cached_comment_and_string_regions() {
        use std::sync::atomic::AtomicUsize;
        for source in ["<?php // Account::settled", "<?php $s='Account::settled';"] {
            let document = lapce_php::Document::new_context(
                lapce_xi_rope::Rope::from(source),
                false,
                &AtomicUsize::new(0),
            )
            .unwrap();
            let offset = source.find("settled").unwrap() + 3;
            assert!(
                definition_member(
                    true,
                    document.completion_allowed(offset),
                    source,
                    offset
                )
                .is_none()
            );
        }
    }
    #[test]
    fn native_definition_gates_and_line_utf8() {
        for (laravel, allowed) in [(false, true), (true, false)] {
            assert!(
                definition_member(laravel, allowed, "Other::save();", 9).is_none()
            );
        }
        assert_eq!(
            definition_member(true, true, "/* λ */ Account::settled();", 21),
            Some(("Account".into(), "settled".into()))
        );
    }
    #[test]
    fn laravel_local_scope_survives_unsaved_original_and_instance_filter() {
        let symbol = PhpIndexSymbol {
            path: "Ledger.php".into(),
            name: "recent".into(),
            qualified: "Domain\\Ledger::recent".into(),
            kind: "method".into(),
            namespace: "Domain".into(),
            container: Some("Domain\\Ledger".into()),
            visibility: Some("public".into()),
            is_static: true,
            alias: None,
            target: Some("Domain\\Ledger::recent".into()),
            attributes: vec!["laravel:local-scope".into()],
            byte: 0,
            end: 0,
            line: 0,
            column: 0,
        };
        for context in [CursorContext::StaticMember, CursorContext::InstanceMember] {
            let mut request = Request {
                context: Some(context),
                owner: Some("Domain\\Ledger".into()),
                ..Default::default()
            };
            let mut original = symbol.clone();
            original.is_static = false;
            original.visibility = Some("protected".into());
            original.attributes.clear();
            request.unsaved.push(original);
            request.allow_nonpublic = true;
            assert_eq!(
                candidates(&request, vec![symbol.clone()])
                    .iter()
                    .filter(|s| s.label == "recent")
                    .count(),
                1
            );
        }
    }
    #[test]
    fn triggers_and_context_buckets() {
        for prefix in ["$", "$x->", "A::", "new ", "use ", "App\\"] {
            assert!(immediate(prefix), "{prefix}");
        }
        assert!(!immediate("hello "));
        assert_eq!(bucket(&Some(CursorContext::New), Some(K::CLASS)), 0);
        assert_eq!(bucket(&Some(CursorContext::New), Some(K::FUNCTION)), 1);
        assert_eq!(hidden("__invoke", "i"), 1);
        assert_eq!(hidden("__invoke", "_"), 0);
        assert_eq!(boundary_bonus("User_Controller", "uc"), 1);
    }
    #[test]
    fn class_member_and_unknown_receiver_filter() {
        let req = request(
            "$this->",
            Some(CursorContext::InstanceMember),
            Some("App\\C".into()),
        );
        let items = candidates(
            &req,
            vec![
                symbol("save", "method", Some("App\\C"), false),
                symbol("global", "function", None, false),
            ],
        );
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "save");
        let unknown = request("$unknown->", None, None);
        assert!(
            candidates(
                &unknown,
                vec![symbol("save", "method", Some("App\\C"), false)]
            )
            .is_empty()
        );
        let req = request("new ", None, None);
        let items = candidates(
            &req,
            vec![
                symbol("C", "class", None, false),
                symbol("f", "function", None, false),
            ],
        );
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].insert_text.as_deref(), Some("\\App\\C"));
    }
}

pub fn receiver(prefix: &str) -> &str {
    if dynamic_receiver(prefix) {
        return "";
    }
    let trim = prefix.trim_end();
    let token = trim
        .rsplit(|c: char| !c.is_alphanumeric() && !matches!(c, '_' | '$' | '\\'))
        .next()
        .unwrap_or("");
    let before = trim.strip_suffix(token).unwrap_or(trim).trim_end();
    before
        .strip_suffix("->")
        .or_else(|| before.strip_suffix("::"))
        .unwrap_or("")
        .trim_end()
        .rsplit(|c: char| !c.is_alphanumeric() && !matches!(c, '_' | '$' | '\\'))
        .next()
        .unwrap_or("")
}
pub fn query_filters(request: &Request) -> (Vec<String>, Option<bool>) {
    use CursorContext::*;
    let kinds: &[&str] = match request.context {
        Some(New | Extends) => &["class"],
        Some(Implements) => &["interface"],
        Some(Import | Type) => &["class", "interface", "trait", "enum", "namespace"],
        Some(InstanceMember) => &["method", "property"],
        Some(StaticMember) => &["method", "property", "constant"],
        _ => &[
            "class",
            "interface",
            "trait",
            "enum",
            "namespace",
            "function",
            "constant",
        ],
    };
    (
        kinds.iter().map(|s| s.to_string()).collect(),
        match request.context {
            Some(InstanceMember) => Some(false),
            Some(StaticMember) => Some(true),
            _ => None,
        },
    )
}
#[cfg(test)]
mod chain_tests {
    use super::*;
    #[test]
    fn chain_is_unknown_not_this() {
        assert_eq!(receiver("$this->repo->"), "");
        assert!(
            request(
                "$this->repo->",
                Some(CursorContext::InstanceMember),
                Some("App\\C".into())
            )
            .owner
            .is_none()
        );
    }
}

#[cfg(test)]
mod qualified_tests {
    use super::*;
    #[test]
    fn leading_backslash_filter_and_insert_are_single_qualified_name() {
        let request = request("new \\App\\Mo", Some(CursorContext::New), None);
        let symbol = PhpIndexSymbol {
            path: "Model.php".into(),
            name: "Model".into(),
            qualified: "App\\Model".into(),
            kind: "class".into(),
            namespace: "App".into(),
            container: None,
            visibility: None,
            is_static: false,
            alias: None,
            target: None,
            attributes: vec![],
            byte: 0,
            end: 0,
            line: 0,
            column: 0,
        };
        let result = candidates(&request, vec![symbol]);
        assert_eq!(result[0].filter_text.as_deref(), Some("App\\Model"));
        assert_eq!(result[0].insert_text.as_deref(), Some("\\App\\Model"));
    }
}

#[cfg(test)]
mod latency_tests {
    use super::*;
    /// Reproduce with `cargo test -p lapce-app native_50k -- --ignored --nocapture`.
    /// Adapter CPU only, excludes indexing, UI and RPC transport.
    #[test]
    #[ignore]
    fn native_50k_adapter_baseline() {
        let symbols = (0..50000)
            .map(|i| PhpIndexSymbol {
                path: format!("vendor/package{i}/Model.php").into(),
                name: format!("Model{i}"),
                qualified: format!("Vendor\\Package{i}\\Model{i}"),
                kind: "class".into(),
                namespace: format!("Vendor\\Package{i}"),
                container: None,
                visibility: None,
                is_static: false,
                alias: None,
                target: None,
                attributes: vec![],
                byte: 0,
                end: 0,
                line: 0,
                column: 0,
            })
            .collect::<Vec<_>>();
        let request = request("new Mo", Some(CursorContext::New), None);
        let mut samples = vec![];
        for _ in 0..5 {
            let start = std::time::Instant::now();
            let items = candidates(&request, symbols.clone());
            assert_eq!(items.len(), 50000);
            samples.push(start.elapsed().as_secs_f64() * 1000.);
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "native50k adapter debug min_ms={:.3} median_ms={:.3} no_lsp=true",
            samples[0], samples[2]
        );
    }
}

fn dynamic_receiver(prefix: &str) -> bool {
    let trim = prefix.trim_end();
    let token = trim
        .rsplit(|c: char| !c.is_alphanumeric() && !matches!(c, '_' | '$' | '\\'))
        .next()
        .unwrap_or("");
    let before = trim.strip_suffix(token).unwrap_or(trim).trim_end();
    let receiver_expression = before
        .strip_suffix("->")
        .or_else(|| before.strip_suffix("::"))
        .unwrap_or("")
        .trim_end();
    if receiver_expression.ends_with(')') || receiver_expression.ends_with(']') {
        return true;
    }
    let receiver = receiver_expression
        .rsplit(|c: char| !c.is_alphanumeric() && !matches!(c, '_' | '$' | '\\'))
        .next()
        .unwrap_or("");
    let preceding = receiver_expression
        .strip_suffix(receiver)
        .unwrap_or("")
        .trim_end();
    preceding.ends_with("->")
        || preceding.ends_with("?->")
        || preceding.ends_with("::")
}
pub fn trigger_session(prefix: &str, current: Option<bool>) -> bool {
    immediate(prefix) && current.unwrap_or(true)
}
#[cfg(test)]
mod session_tests {
    use super::*;
    #[test]
    fn separate_statement_is_not_chain() {
        for prefix in [
            "$this->x = 1; $this->",
            "foo($this->a, $this->",
            "if ($this->x && $this->",
            "[$this->a, $this->",
            "$a . $this->",
        ] {
            assert_eq!(receiver(prefix), "$this", "{prefix}");
        }
        assert_eq!(receiver("$this->repo->"), "");
    }
    #[test]
    fn stale_tree_starts_session_then_gate_suppresses_only_native() {
        assert!(trigger_session("$", None));
        assert!(trigger_session("$this->", None));
        assert!(!trigger_session("new ", Some(false)));
        assert!(trigger_session("new ", Some(true)));
        assert!(!trigger_session("route('na", Some(false)));
    }
}
