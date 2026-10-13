//! VS Code `language-configuration.json`.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LanguageConfig {
    pub line_comment: Option<String>,
    pub block_comment: Option<(String, String)>,
    pub brackets: Vec<(String, String)>,
    pub auto_closing_pairs: Vec<(String, String)>,
    pub surrounding_pairs: Vec<(String, String)>,
    pub word_pattern: Option<String>,
    pub increase_indent_pattern: Option<String>,
    pub decrease_indent_pattern: Option<String>,
    pub folding_start: Option<String>,
    pub folding_end: Option<String>,
}

fn pairs(v: Option<&Value>) -> Vec<(String, String)> {
    let Some(a) = v.and_then(Value::as_array) else {
        return Vec::new();
    };
    a.iter()
        .filter_map(|p| match p {
            Value::Array(x) if x.len() == 2 => {
                Some((x[0].as_str()?.to_owned(), x[1].as_str()?.to_owned()))
            }
            Value::Object(o) => Some((
                o.get("open")?.as_str()?.to_owned(),
                o.get("close")?.as_str()?.to_owned(),
            )),
            _ => None,
        })
        .collect()
}

fn pattern(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => Some(o.get("pattern")?.as_str()?.to_owned()),
        _ => None,
    }
}

pub fn parse(doc: &Value) -> LanguageConfig {
    let comments = doc.get("comments");
    let block = comments
        .and_then(|c| c.get("blockComment"))
        .and_then(Value::as_array)
        .and_then(|a| {
            Some((
                a.first()?.as_str()?.to_owned(),
                a.get(1)?.as_str()?.to_owned(),
            ))
        });
    let indent = doc.get("indentationRules");
    let markers = doc.get("folding").and_then(|f| f.get("markers"));
    LanguageConfig {
        line_comment: comments.and_then(|c| c.get("lineComment")).and_then(|l| {
            match l {
                Value::String(s) => Some(s.clone()),
                Value::Object(o) => o.get("comment")?.as_str().map(str::to_owned),
                _ => None,
            }
        }),
        block_comment: block,
        brackets: pairs(doc.get("brackets")),
        auto_closing_pairs: pairs(doc.get("autoClosingPairs")),
        surrounding_pairs: pairs(doc.get("surroundingPairs")),
        word_pattern: pattern(doc.get("wordPattern")),
        increase_indent_pattern: pattern(
            indent.and_then(|i| i.get("increaseIndentPattern")),
        ),
        decrease_indent_pattern: pattern(
            indent.and_then(|i| i.get("decreaseIndentPattern")),
        ),
        folding_start: pattern(markers.and_then(|m| m.get("start"))),
        folding_end: pattern(markers.and_then(|m| m.get("end"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_comments_and_pairs() {
        let doc = crate::jsonc::parse(
            r#"{"comments":{"lineComment":"//","blockComment":["/*","*/"]},
            "brackets":[["{","}"],["[","]"]],"autoClosingPairs":[{"open":"\"","close":"\"","notIn":["string"]},["(",")"]]}"#,
        )
        .unwrap();
        let c = parse(&doc);
        assert_eq!(c.line_comment.as_deref(), Some("//"));
        assert_eq!(c.block_comment, Some(("/*".into(), "*/".into())));
        assert_eq!(c.brackets.len(), 2);
        assert_eq!(c.auto_closing_pairs.len(), 2);
    }
}
