//! JSON with comments and trailing commas, as used by VS Code manifests,
//! themes, snippets and language configurations.
use anyhow::{Context, Result};
use serde_json::Value;

/// Remove `//` and `/* */` comments and trailing commas outside of strings.
pub fn strip(input: &str) -> String {
    let input = input.trim_start_matches('\u{feff}');
    let chars: Vec<char> = input.chars().collect();
    let mut no_comments = String::with_capacity(input.len());
    let mut i = 0;
    let mut in_str = false;
    while i < chars.len() {
        let c = chars[i];
        if in_str {
            no_comments.push(c);
            if c == '\\' && i + 1 < chars.len() {
                no_comments.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_str = true;
            no_comments.push(c);
            i += 1;
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            no_comments.push(' ');
        } else {
            no_comments.push(c);
            i += 1;
        }
    }

    // Second pass: drop commas directly before a closing bracket.
    let chars: Vec<char> = no_comments.chars().collect();
    let mut out = String::with_capacity(chars.len());
    let mut in_str = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_str {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_str = false;
            }
        } else if c == '"' {
            in_str = true;
            out.push(c);
        } else if c == ',' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if !matches!(chars.get(j), Some('}') | Some(']')) {
                out.push(c);
            }
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

pub fn parse(input: &str) -> Result<Value> {
    serde_json::from_str(&strip(input)).context("invalid JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_and_trailing_commas() {
        let v = parse(
            "\u{feff}{\n // c\n \"a\": [1, 2, /* x */ ],\n \"u\": \"http://x/*y*/\",\n}",
        )
        .unwrap();
        assert_eq!(v["a"].as_array().unwrap().len(), 2);
        assert_eq!(v["u"], "http://x/*y*/");
    }
}
