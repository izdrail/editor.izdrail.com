//! Logos byte classification plus structural declaration validation.
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Kind {
    Namespace,
    Class,
    Interface,
    Trait,
    Enum,
    Function,
    Method,
    Constant,
    Property,
    Attribute,
    Import,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub qualified: String,
    pub kind: Kind,
    pub namespace: String,
    pub container: Option<String>,
    pub visibility: Option<String>,
    pub is_static: bool,
    pub alias: Option<String>,
    pub target: Option<String>,
    pub attributes: Vec<String>,
    pub byte: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FileSymbols {
    pub symbols: Vec<Symbol>,
    pub malformed: bool,
}
fn text<'a>(node: tree_sitter::Node, source: &'a str) -> &'a str {
    source.get(node.byte_range()).unwrap_or("")
}
fn qualified(ns: &str, name: &str, container: Option<&str>, kind: &Kind) -> String {
    if let Some(container) = container {
        if matches!(kind, Kind::Method | Kind::Property | Kind::Constant) {
            return format!("{container}::{name}");
        }
    }
    if ns.is_empty() {
        name.to_owned()
    } else {
        format!("{ns}\\{name}")
    }
}
fn named(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    node.child_by_field_name("name").or_else(|| {
        let mut c = node.walk();
        node.named_children(&mut c).find(|n| {
            matches!(n.kind(), "name" | "qualified_name" | "variable_name")
        })
    })
}
struct Scan<'a> {
    source: &'a str,
    styles: Vec<u8>,
    out: FileSymbols,
    cancel: &'a AtomicBool,
}
impl Scan<'_> {
    fn add(
        &mut self,
        node: tree_sitter::Node,
        name_node: tree_sitter::Node,
        kind: Kind,
        ns: &str,
        container: Option<&str>,
    ) {
        let name = text(name_node, self.source);
        if name.is_empty() || name_node.is_missing() {
            return;
        }
        // Logos rejects declarations in comments/strings even on recovering ASTs.
        if self.styles.get(name_node.start_byte()).is_some_and(|s| {
            matches!(
                *s,
                gravity_lex::style::COMMENT
                    | gravity_lex::style::LINE_COMMENT
                    | gravity_lex::style::DOC
                    | gravity_lex::style::SINGLE
                    | gravity_lex::style::INTERPOLATED
                    | gravity_lex::style::DOUBLE
                    | gravity_lex::style::HEREDOC
                    | gravity_lex::style::NOWDOC
                    | gravity_lex::style::CHARACTER
                    | gravity_lex::style::HTML
            )
        }) {
            return;
        }
        let parent = if matches!(kind, Kind::Property | Kind::Constant) {
            node.parent().unwrap_or(node)
        } else {
            node
        };
        let mut cur = parent.walk();
        let children = parent.named_children(&mut cur).collect::<Vec<_>>();
        let visibility = children
            .iter()
            .find(|n| n.kind() == "visibility_modifier")
            .map(|n| text(*n, self.source).to_owned());
        let is_static = children.iter().any(|n| n.kind() == "static_modifier");
        let attrs = parent
            .child_by_field_name("attributes")
            .map(|n| text(n, self.source).to_owned())
            .into_iter()
            .collect();
        let point = name_node.start_position();
        self.out.symbols.push(Symbol {
            name: name.to_owned(),
            qualified: qualified(ns, name, container, &kind),
            kind,
            namespace: ns.to_owned(),
            container: container.map(str::to_owned),
            visibility,
            is_static,
            alias: None,
            target: None,
            attributes: attrs,
            byte: name_node.start_byte(),
            end: name_node.end_byte(),
            line: point.row,
            column: point.column,
        });
    }
    fn walk(
        &mut self,
        node: tree_sitter::Node,
        ns: &str,
        container: Option<&str>,
        depth: usize,
    ) {
        if depth > 256 {
            self.out.malformed = true;
            return;
        }
        if self.cancel.load(Ordering::Relaxed) {
            return;
        }
        let mut namespace = ns.to_owned();
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if self.cancel.load(Ordering::Relaxed) {
                break;
            }
            if child.kind() == "namespace_definition" {
                let name = child.child_by_field_name("name");
                let newns = name.map(|n| text(n, self.source)).unwrap_or("");
                if let Some(name) = name {
                    self.add(child, name, Kind::Namespace, "", None);
                }
                if let Some(body) = child.child_by_field_name("body") {
                    self.walk(body, newns, None, depth + 1);
                } else {
                    namespace = newns.to_owned();
                }
                continue;
            }
            if matches!(child.kind(), "namespace_use_declaration") {
                self.imports(child, &namespace);
                continue;
            }
            let kind = match child.kind() {
                "class_declaration" => Some(Kind::Class),
                "interface_declaration" => Some(Kind::Interface),
                "trait_declaration" => Some(Kind::Trait),
                "enum_declaration" => Some(Kind::Enum),
                "function_definition" => Some(Kind::Function),
                "method_declaration" => Some(Kind::Method),
                "const_element" | "enum_case" => Some(Kind::Constant),
                "property_element" => Some(Kind::Property),
                "attribute" => Some(Kind::Attribute),
                _ => None,
            };
            let mut class = None;
            if let Some(kind) = kind {
                if let Some(name) = named(child) {
                    if matches!(
                        kind,
                        Kind::Class | Kind::Interface | Kind::Trait | Kind::Enum
                    ) {
                        class = Some(qualified(
                            &namespace,
                            text(name, self.source),
                            None,
                            &kind,
                        ));
                    }
                    self.add(child, name, kind, &namespace, container);
                }
            }
            // Enter nested declarations, but anonymous classes must not inherit
            // the enclosing named class as their member owner.
            if child.kind() == "anonymous_class" {
                continue;
            }
            let owner = class.as_deref().or(container);
            self.walk(child, &namespace, owner, depth + 1);
        }
    }
    fn imports(&mut self, node: tree_sitter::Node, ns: &str) {
        let prefix = if node.child_by_field_name("body").is_some() {
            let mut c = node.walk();
            node.named_children(&mut c)
                .find(|n| n.kind() == "namespace_name")
                .map(|n| text(n, self.source))
                .unwrap_or("")
                .to_owned()
        } else {
            String::new()
        };
        fn clauses<'a>(
            n: tree_sitter::Node<'a>,
            out: &mut Vec<tree_sitter::Node<'a>>,
        ) {
            if n.kind() == "namespace_use_clause" {
                out.push(n);
                return;
            }
            let mut c = n.walk();
            for ch in n.named_children(&mut c) {
                clauses(ch, out);
            }
        }
        let mut nodes = vec![];
        clauses(node, &mut nodes);
        for clause in nodes {
            let mut c = clause.walk();
            let Some(name) = clause
                .named_children(&mut c)
                .find(|n| matches!(n.kind(), "name" | "qualified_name"))
            else {
                continue;
            };
            let target = if prefix.is_empty() {
                text(name, self.source).trim_start_matches('\\').to_owned()
            } else {
                format!(
                    "{}\\{}",
                    prefix.trim_end_matches('\\'),
                    text(name, self.source)
                )
            };
            let alias = clause
                .child_by_field_name("alias")
                .map(|n| text(n, self.source).to_owned())
                .unwrap_or_else(|| {
                    target.rsplit('\\').next().unwrap_or("").to_owned()
                });
            let before = self.out.symbols.len();
            self.add(clause, name, Kind::Import, ns, None);
            if self.out.symbols.len() > before {
                let s = self.out.symbols.last_mut().unwrap();
                s.name = alias.clone();
                s.qualified = qualified(ns, &alias, None, &Kind::Import);
                s.alias = Some(alias);
                s.target = Some(target);
            }
        }
    }
}
pub fn scan(source: &str, cancel: &AtomicBool) -> Result<FileSymbols, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("cancelled".into());
    }
    let lexed = gravity_lex::lex(source);
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&lapce_php::language(false))
        .map_err(|e| e.to_string())?;
    let tree = parser
        .parse(source, None)
        .ok_or("PHP parser returned no tree")?;
    let mut scanner = Scan {
        source,
        styles: lexed.styles,
        out: FileSymbols {
            malformed: tree.root_node().has_error(),
            ..Default::default()
        },
        cancel,
    };
    scanner.walk(tree.root_node(), "", None, 0);
    if cancel.load(Ordering::Relaxed) {
        return Err("cancelled".into());
    }
    Ok(scanner.out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn namespace_members_attributes_aliases_and_fake_text() {
        let source = r#"<?php namespace App; use Vendor\Thing as Alias; use Vendor\{Foo, Bar as Baz}; #[Entity] class User { private static $name; public function save() {} const ID=1; } interface I {} trait T {} enum E {case A;} function helper(){} // class Fake {}
        $s='class Nope {}'; namespace Other { class C {} }"#;
        let out = scan(source, &AtomicBool::new(false)).unwrap();
        let names = out
            .symbols
            .iter()
            .map(|s| s.qualified.as_str())
            .collect::<Vec<_>>();
        for want in [
            "App\\User",
            "App\\User::$name",
            "App\\User::save",
            "App\\User::ID",
            "App\\I",
            "App\\T",
            "App\\E",
            "App\\E::A",
            "App\\helper",
            "Other\\C",
        ] {
            assert!(names.contains(&want), "{want}: {names:?}");
        }
        assert!(
            !names
                .iter()
                .any(|n| n.contains("Fake") || n.contains("Nope"))
        );
        let property = out
            .symbols
            .iter()
            .find(|s| s.kind == Kind::Property)
            .unwrap();
        assert_eq!(property.visibility.as_deref(), Some("private"));
        assert!(property.is_static);
        assert!(out.symbols.iter().any(|s| s.alias.as_deref() == Some("Baz")
            && s.target.as_deref() == Some("Vendor\\Bar")));
        assert!(out.symbols.iter().any(|s| s.kind == Kind::Attribute));
    }
    #[test]
    fn anonymous_members_are_not_namespace_symbols() {
        let out=scan("<?php namespace App; $x=new class {public function hidden(){} private $value;}; class Named {function shown(){}}",&AtomicBool::new(false)).unwrap();
        assert!(
            !out.symbols
                .iter()
                .any(|s| s.name == "hidden" || s.name == "$value")
        );
        assert!(
            out.symbols
                .iter()
                .any(|s| s.qualified == "App\\Named::shown")
        );
    }
    #[test]
    fn malformed_unicode_and_cancellation() {
        let s = scan(
            "<?php namespace Ω; class Δ {} function broken(",
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(s.malformed);
        assert!(s.symbols.iter().any(|s| s.qualified == "Ω\\Δ"));
        assert!(scan("<?php class C{}", &AtomicBool::new(true)).is_err());
    }
}
