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
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&javascript_language())
        .map_err(|err| IndexError::SymbolParse(err.to_string()))?;
    let tree = parser.parse(source, None)
        .ok_or_else(|| IndexError::SymbolParse("JavaScript parse returned no tree".into()))?;
    let mut cursor = tree.walk();
    let mut entries = Vec::new();
    loop {
        let node = cursor.node();
        if matches!(node.kind(), "function_declaration" | "generator_function_declaration" | "class_declaration" | "method_definition") {
            if let Some(name) = node.child_by_field_name("name") {
                if matches!(name.kind(), "identifier" | "property_identifier") && !name.is_missing() {
                    if let Ok(text) = name.utf8_text(source.as_bytes()) {
                        if !text.is_empty() {
                            let point = name.start_position();
                            entries.push(IndexEntry {
                                path: path.to_owned(), kind: EntryKind::SymbolDef,
                                language: "javascript".into(), symbol: Some(text.into()),
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
