//! Standalone JavaScript definition extraction. Painting still uses Lexilla;
//! this parser is separate and ships its own pinned grammar.
use crate::{EntryKind, IndexEntry, IndexError, Result};

/// Bridge the modern LanguageFn wrapper to the workspace's single native
/// tree-sitter 0.22 runtime without adding a second `links = tree-sitter`.
fn javascript_language() -> tree_sitter::Language {
    // SAFETY: LANGUAGE is the generated, statically linked JavaScript grammar
    // function. It returns a non-null pointer to immutable static TSLanguage
    // data, not allocated memory. Grammar 0.23.1 emits ABI 14, accepted by
    // runtime 0.22.6 (13..=14). set_language checks this range again.
    unsafe {
        let pointer = (tree_sitter_javascript::LANGUAGE.into_raw())();
        assert!(!pointer.is_null(), "JavaScript grammar returned null");
        tree_sitter::Language::from_raw(pointer.cast())
    }
}

/// Extract named function/class declarations and identifier-named methods.
/// No reference/type resolution, local variables, imports or computed names.
pub fn javascript_definitions(path: &str, source: &str) -> Result<Vec<IndexEntry>> {
    extract_definitions(path, source, "javascript", javascript_language())
}

fn typescript_language(tsx: bool) -> tree_sitter::Language {
    let grammar = if tsx { tree_sitter_typescript::LANGUAGE_TSX }
        else { tree_sitter_typescript::LANGUAGE_TYPESCRIPT };
    // SAFETY: both pinned generated grammars return immutable non-null
    // static language data, ABI 14, accepted by runtime 0.22.6.
    unsafe {
        let pointer = (grammar.into_raw())();
        assert!(!pointer.is_null(), "TypeScript grammar returned null");
        tree_sitter::Language::from_raw(pointer.cast())
    }
}

/// TypeScript declarations, using the TSX grammar for .tsx sources.
pub fn typescript_definitions(path: &str, source: &str, tsx: bool) -> Result<Vec<IndexEntry>> {
    extract_definitions(path, source, "typescript", typescript_language(tsx))
}

/// PHP and mixed PHP/HTML declarations. Blade directives are not PHP syntax.
pub fn php_definitions(path: &str, source: &str) -> Result<Vec<IndexEntry>> {
    // SAFETY: pinned generated static PHP grammar is non-null ABI 14,
    // accepted by the single workspace runtime (13..=14).
    let language = unsafe {
        let pointer = (tree_sitter_php::LANGUAGE_PHP.into_raw())();
        assert!(!pointer.is_null(), "PHP grammar returned null");
        tree_sitter::Language::from_raw(pointer.cast())
    };
    extract_definitions(path, source, "php", language)
}

fn extract_definitions(path: &str, source: &str, language_id: &str, language: tree_sitter::Language) -> Result<Vec<IndexEntry>> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language)
        .map_err(|err| IndexError::SymbolParse(err.to_string()))?;
    let tree = parser.parse(source, None)
        .ok_or_else(|| IndexError::SymbolParse("Definition parse returned no tree".into()))?;
    let mut cursor = tree.walk();
    let mut entries = Vec::new();
    loop {
        let node = cursor.node();
        if matches!(node.kind(), "function_declaration" | "generator_function_declaration" | "class_declaration" | "method_definition" | "interface_declaration" | "type_alias_declaration" | "enum_declaration" | "function_definition" | "method_declaration" | "trait_declaration") {
            if let Some(name) = node.child_by_field_name("name") {
                if matches!(name.kind(), "identifier" | "property_identifier" | "type_identifier" | "name") && !name.is_missing() {
                    if let Ok(text) = name.utf8_text(source.as_bytes()) {
                        if !text.is_empty() {
                            let point = name.start_position();
                            entries.push(IndexEntry {
                                path: path.to_owned(), kind: EntryKind::SymbolDef,
                                language: language_id.into(), symbol: Some(text.into()),
                                line: point.row as u64 + 1, col: point.column as u64,
                                // Avoid repeating a potentially large declaration body.
                                body: text.into(),
                            });
                        }
                    }
                }
            }
        }
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return Ok(entries);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn php_declarations_and_embedded_html_have_source_positions() {
        let source = "<h1>Example</h1>\n<?php\ninterface Contract {}\ntrait Shared {}\nclass Cart { public function total() {} }\nfunction checkout() {}\nenum State { case Open; }";
        let entries = php_definitions("cart.php", source).unwrap();
        let names: Vec<_> = entries.iter().map(|e| e.symbol.as_deref().unwrap()).collect();
        assert_eq!(names, ["Contract", "Shared", "Cart", "total", "checkout", "State"]);
        assert_eq!((entries[0].line, entries[0].col), (3, 10));
        assert!(entries.iter().all(|e| e.language == "php"));
        assert!(php_definitions("a.php", "<?php /* function fake() {} */ checkout(); $x = 'class Fake {}';").unwrap().is_empty());
    }

    #[test]
    fn typescript_and_tsx_grammars_accept_runtime_and_extract_types() {
        for tsx in [false, true] {
            let language = typescript_language(tsx);
            assert_eq!(language.version(), 14);
            tree_sitter::Parser::new().set_language(&language).unwrap();
            let source = "interface Cart {}\ntype Total = number;\nenum State { Open }\nexport function Checkout(): void {}";
            let entries = typescript_definitions("cart.ts", source, tsx).unwrap();
            let names: Vec<_> = entries.iter().map(|e| e.symbol.as_deref().unwrap()).collect();
            assert_eq!(names, ["Cart", "Total", "State", "Checkout"]);
            assert!(entries.iter().all(|e| e.language == "typescript"));
        }
        let entries = typescript_definitions("page.tsx", "export function Page() { return <div>Hello</div>; }", true).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].symbol.as_deref(), Some("Page"));
    }

    #[test]
    fn generated_grammar_abi_is_accepted_by_workspace_runtime() {
        let language = javascript_language();
        assert_eq!(language.version(), 14);
        assert!((tree_sitter::MIN_COMPATIBLE_LANGUAGE_VERSION..=tree_sitter::LANGUAGE_VERSION)
            .contains(&language.version()));
        tree_sitter::Parser::new().set_language(&language).unwrap();
    }

    #[test]
    fn extracts_declarations_and_methods_with_byte_positions() {
        let source = "// function fake() {}\nexport function Checkout() {}\nclass Cart {\n  total() {}\n  ['computed']() {}\n}\nfunction* stream() {}";
        let entries = javascript_definitions("src/cart.js", source).unwrap();
        let names: Vec<_> = entries.iter().map(|entry| entry.symbol.as_deref().unwrap()).collect();
        assert_eq!(names, ["Checkout", "Cart", "total", "stream"]);
        assert_eq!((entries[0].line, entries[0].col), (2, 16));
        assert_eq!((entries[2].line, entries[2].col), (4, 2));
        assert!(entries.iter().all(|entry| entry.language == "javascript" && entry.path == "src/cart.js" && entry.kind == EntryKind::SymbolDef));
    }

    #[test]
    fn does_not_treat_calls_strings_or_anonymous_expressions_as_definitions() {
        let entries = javascript_definitions("a.js", "Checkout(); const text = 'function fake() {}'; const handler = function() {}; const arrow = () => 1;").unwrap();
        assert!(entries.is_empty());
        assert!(javascript_definitions("empty.js", "").unwrap().is_empty());
    }
}
