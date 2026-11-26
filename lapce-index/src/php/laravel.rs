//! Laravel metadata on the existing PHP symbol graph. No source execution or
//! separate framework index. Called only with Composer evidence of Laravel.
use super::scanner::{FileSymbols, Kind, Symbol};
use std::sync::atomic::{AtomicBool, Ordering};
use tree_sitter::Node;
fn source<'a>(n: Node, s: &'a str) -> &'a str {
    s.get(n.byte_range()).unwrap_or("")
}
fn class_name<'a>(n: Node, s: &'a str) -> Option<&'a str> {
    n.child_by_field_name("name").map(|n| source(n, s))
}
fn doc<'a>(n: Node<'a>, s: &'a str) -> Option<(Node<'a>, &'a str)> {
    let p = n.prev_named_sibling()?;
    (p.kind() == "comment" && source(p, s).starts_with("/**"))
        .then(|| (p, source(p, s)))
}
fn resolve(name: &str, ns: &str, symbols: &[Symbol]) -> String {
    if name.starts_with('\\') {
        return name.trim_start_matches('\\').into();
    }
    let (head, tail) = name.split_once('\\').unwrap_or((name, ""));
    if let Some(import) = symbols.iter().find(|s| {
        s.kind == Kind::Import
            && s.namespace == ns
            && s.alias
                .as_deref()
                .is_some_and(|a| a.eq_ignore_ascii_case(head))
    }) {
        if let Some(target) = &import.target {
            return if tail.is_empty() {
                target.clone()
            } else {
                format!("{target}\\{tail}")
            };
        }
    }
    if ns.is_empty() {
        name.into()
    } else {
        format!("{ns}\\{name}")
    }
}
fn role(class: &Symbol) -> Option<&'static str> {
    class.related.iter().find_map(|p| match p.as_str() {
        "Illuminate\\Database\\Eloquent\\Model"
        | "Illuminate\\Foundation\\Auth\\User"
        | "Illuminate\\Database\\Eloquent\\Relations\\Pivot" => {
            Some("eloquent-model")
        }
        "Illuminate\\Support\\Facades\\Facade" => Some("facade"),
        "Illuminate\\Support\\ServiceProvider" => Some("service-provider"),
        "Illuminate\\Foundation\\Http\\FormRequest" => Some("form-request"),
        "Illuminate\\Console\\Command" => Some("artisan-command"),
        "Illuminate\\Contracts\\Queue\\ShouldQueue" => Some("queued-job"),
        "Illuminate\\Support\\Collection"
        | "Illuminate\\Database\\Eloquent\\Collection" => Some("collection"),
        _ => None,
    })
}
fn scope_attribute(
    method: Node,
    s: &str,
    namespace: &str,
    symbols: &[Symbol],
) -> bool {
    let Some(attrs) = method.child_by_field_name("attributes") else {
        return false;
    };
    let mut nodes = vec![attrs];
    while let Some(node) = nodes.pop() {
        if node.kind() == "attribute" {
            if let Some(name) = node.named_child(0) {
                if resolve(source(name, s), namespace, symbols)
                    == "Illuminate\\Database\\Eloquent\\Attributes\\Scope"
                {
                    return true;
                }
            }
        }
        let mut c = node.walk();
        nodes.extend(node.named_children(&mut c));
    }
    false
}
/// Enrich only statically evidenced declarations. Alias locations point to real
/// source methods/comments, preserving exact byte ranges for navigation.
pub fn enrich(root: Node, s: &str, out: &mut FileSymbols, cancel: &AtomicBool) {
    let mut pending = vec![root];
    let mut visited = 0;
    let mut aliases = Vec::new();
    while let Some(n) = pending.pop() {
        visited += 1;
        if visited > 50_000 || cancel.load(Ordering::Relaxed) {
            break;
        }
        if n.kind() == "class_declaration" {
            let Some(name) = class_name(n, s) else {
                continue;
            };
            let Some(index) = out.symbols.iter().position(|sym| {
                sym.kind == Kind::Class
                    && sym.byte
                        == n.child_by_field_name("name").unwrap().start_byte()
                    && sym.name == name
            }) else {
                continue;
            };
            let class = out.symbols[index].clone();
            let Some(kind) = role(&class) else {
                continue;
            };
            out.symbols[index]
                .attributes
                .push(format!("laravel:{kind}"));
            if let Some(body) = n.child_by_field_name("body") {
                let mut c = body.walk();
                for method in body
                    .named_children(&mut c)
                    .filter(|m| m.kind() == "method_declaration")
                {
                    let Some(method_name) = class_name(method, s) else {
                        continue;
                    };
                    let Some(original) = out
                        .symbols
                        .iter()
                        .find(|sym| {
                            sym.kind == Kind::Method
                                && sym.container.as_deref() == Some(&class.qualified)
                                && sym.name == method_name
                        })
                        .cloned()
                    else {
                        continue;
                    };
                    if kind == "eloquent-model" {
                        let legacy = method_name
                            .strip_prefix("scope")
                            .filter(|n| n.starts_with(char::is_uppercase));
                        let attribute_scope = scope_attribute(
                            method,
                            s,
                            &class.namespace,
                            &out.symbols,
                        );
                        let callable = legacy
                            .map(|n| {
                                let mut chars = n.chars();
                                chars
                                    .next()
                                    .map(|first| {
                                        first.to_lowercase().collect::<String>()
                                            + chars.as_str()
                                    })
                                    .unwrap_or_default()
                            })
                            .or_else(|| {
                                attribute_scope.then(|| method_name.to_owned())
                            });
                        if let Some(callable) = callable {
                            if original.visibility.as_deref() != Some("private")
                                && aliases.len() < 4096
                                && !out.symbols.iter().any(|sym| {
                                    sym.container == original.container
                                        && sym.name == callable
                                        && sym.name != method_name
                                })
                            {
                                let mut alias = original.clone();
                                alias.name = callable.clone();
                                alias.qualified =
                                    format!("{}::{callable}", class.qualified);
                                alias.target = Some(original.qualified.clone());
                                alias.visibility = Some("public".into());
                                alias.is_static = true;
                                alias.attributes.push("laravel:local-scope".into());
                                aliases.push(alias);
                            }
                        }
                        // Actual relationship methods already exist as native members.
                        // Add a role only when a return directly calls a known relation.
                        if let Some(body) = method.child_by_field_name("body") {
                            let mut c = body.walk();
                            if let Some(ret) = body
                                .named_children(&mut c)
                                .find(|n| n.kind() == "return_statement")
                            {
                                let mut c = ret.walk();
                                if let Some(call) = ret
                                    .named_children(&mut c)
                                    .find(|n| n.kind() == "member_call_expression")
                                {
                                    if call
                                        .child_by_field_name("object")
                                        .is_some_and(|o| source(o, s) == "$this")
                                    {
                                        if let Some(relation) = call
                                            .child_by_field_name("name")
                                            .map(|n| source(n, s))
                                            .filter(|name| {
                                                matches!(
                                                    *name,
                                                    "hasOne"
                                                        | "hasMany"
                                                        | "belongsTo"
                                                        | "belongsToMany"
                                                        | "hasOneThrough"
                                                        | "hasManyThrough"
                                                        | "morphOne"
                                                        | "morphMany"
                                                        | "morphTo"
                                                        | "morphToMany"
                                                        | "morphedByMany"
                                                )
                                            })
                                        {
                                            if let Some(symbol) =
                                                out.symbols.iter_mut().find(|sym| {
                                                    sym.qualified
                                                        == original.qualified
                                                })
                                            {
                                                symbol.attributes.push(format!(
                                                    "laravel:relation:{relation}"
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if kind == "facade" {
                if let Some((comment, body)) = doc(n, s) {
                    for line in body.lines() {
                        let line = line
                            .trim()
                            .trim_start_matches('/')
                            .trim_start_matches('*')
                            .trim();
                        let Some(rest) = line.strip_prefix("@method ") else {
                            continue;
                        };
                        let Some(rest) = rest.strip_prefix("static ") else {
                            continue;
                        };
                        let Some((head, _)) = rest.split_once('(') else {
                            continue;
                        };
                        let Some(name) =
                            head.split_whitespace().last().filter(|name| {
                                !name.is_empty()
                                    && name
                                        .chars()
                                        .all(|c| c.is_alphanumeric() || c == '_')
                            })
                        else {
                            continue;
                        };
                        if aliases.len() >= 4096
                            || aliases.iter().any(|sym: &Symbol| {
                                sym.container.as_deref() == Some(&class.qualified)
                                    && sym.name == name
                            })
                            || out.symbols.iter().any(|sym| {
                                sym.container.as_deref() == Some(&class.qualified)
                                    && sym.name == name
                            })
                        {
                            continue;
                        }
                        let mut symbol = class.clone();
                        symbol.kind = Kind::Method;
                        symbol.name = name.into();
                        symbol.qualified = format!("{}::{name}", class.qualified);
                        symbol.container = Some(class.qualified.clone());
                        symbol.is_static = true;
                        symbol.visibility = Some("public".into());
                        symbol.attributes = vec!["laravel:facade-phpdoc".into()];
                        symbol.byte = comment.start_byte();
                        symbol.end = comment.end_byte();
                        symbol.line = comment.start_position().row;
                        symbol.column = comment.start_position().column;
                        symbol.target = Some(class.qualified.clone());
                        aliases.push(symbol);
                    }
                }
            }
        }
        let mut c = n.walk();
        pending.extend(n.named_children(&mut c));
    }
    out.symbols.extend(aliases);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roles_and_false_scope_attributes_are_conservative() {
        let text = r#"<?php namespace App;
        use Illuminate\Support\ServiceProvider;
        use Illuminate\Foundation\Http\FormRequest;
        use Illuminate\Console\Command;
        use Illuminate\Contracts\Queue\ShouldQueue;
        use Illuminate\Support\Collection;
        class Provider extends ServiceProvider {}
        class Request extends FormRequest {}
        class Task extends Command {}
        class Job implements ShouldQueue {}
        class Items extends Collection {}
        class Invoice extends \Illuminate\Database\Eloquent\Model {
            #[NotScope] protected function fake($q) {}
            public function scopeActive($q) {}
            public function active($q) {}
        }
        "#;
        let result =
            super::super::scanner::scan_laravel(text, &AtomicBool::new(false), true)
                .unwrap();
        for role in [
            "service-provider",
            "form-request",
            "artisan-command",
            "queued-job",
            "collection",
        ] {
            assert!(
                result
                    .symbols
                    .iter()
                    .any(|s| s.attributes.contains(&format!("laravel:{role}"))),
                "{role}"
            );
        }
        assert!(
            !result
                .symbols
                .iter()
                .any(|s| s.name == "fake" && s.is_static)
        );
        assert!(
            !result
                .symbols
                .iter()
                .any(|s| s.name == "active" && s.is_static)
        );
    }
    #[test]
    fn model_scopes_aliases_and_role() {
        let text = "<?php namespace Domain; use Illuminate\\Database\\Eloquent\\Model as Base; class Invoice extends Base { public function scopePaid($q){} #[\\Illuminate\\Database\\Eloquent\\Attributes\\Scope] protected function recent($q){} public function user(){return $this->belongsTo(User::class);} private function scopeHidden($q){} }";
        let result =
            super::super::scanner::scan_laravel(text, &AtomicBool::new(false), true)
                .unwrap();
        assert!(result.symbols.iter().any(|s| s.name == "Invoice"
            && s.attributes.contains(&"laravel:eloquent-model".into())));
        for name in ["paid", "recent"] {
            let s = result
                .symbols
                .iter()
                .find(|s| s.name == name && s.is_static)
                .unwrap();
            assert!(s.target.is_some());
            assert_eq!(s.visibility.as_deref(), Some("public"));
        }
        assert!(!result.symbols.iter().any(|s| s.name == "hidden"));
        assert!(result.symbols.iter().any(|s| {
            s.name == "user"
                && s.attributes
                    .iter()
                    .any(|a| a == "laravel:relation:belongsTo")
        }));
    }
    #[test]
    fn composer_gate_and_exact_model_identity() {
        let text = "<?php class Fake extends Model {function scopePaid($q){}}";
        let symbols =
            super::super::scanner::scan_laravel(text, &AtomicBool::new(false), true)
                .unwrap();
        assert!(!symbols.symbols.iter().any(|s| s.name == "paid"));
        let text = "<?php class Real extends \\Illuminate\\Database\\Eloquent\\Model {function scopePaid($q){}}";
        let symbols = super::super::scanner::scan_laravel(
            text,
            &AtomicBool::new(false),
            false,
        )
        .unwrap();
        assert!(!symbols.symbols.iter().any(|s| s.name == "paid"));
    }
    #[test]
    fn facade_doc_and_alias_definition() {
        let text =
            "<?php namespace App; /** @method static string send(string $text)
 * @method static string send(string $text)
 */ class Mail extends \\Illuminate\\Support\\Facades\\Facade {}";
        let result =
            super::super::scanner::scan_laravel(text, &AtomicBool::new(false), true)
                .unwrap();
        let method = result.symbols.iter().find(|s| s.name == "send").unwrap();
        assert_eq!(
            result.symbols.iter().filter(|s| s.name == "send").count(),
            1
        );
        assert!(method.is_static);
        assert_eq!(method.target.as_deref(), Some("App\\Mail"));
        assert!(text[method.byte..method.end].contains("@method"));
    }
}
