//! Experimental PHP/HTML byte styling. No parser, I/O, PHP execution or UI state.
//! Style IDs match LexPHP. Checkpoints currently support inspection, not incremental reuse.
use logos::Logos;
use std::ops::Range;

pub mod style {
    pub const HTML: u8 = 21;
    pub const COMMENT: u8 = 1;
    pub const LINE_COMMENT: u8 = 2;
    pub const DOC: u8 = 3;
    pub const VARIABLE: u8 = 4;
    pub const SINGLE: u8 = 5;
    pub const INTERPOLATED: u8 = 6;
    pub const DOUBLE: u8 = 7;
    pub const HEREDOC: u8 = 8;
    pub const NOWDOC: u8 = 9;
    pub const OPERATOR: u8 = 11;
    pub const NUMBER: u8 = 12;
    pub const KEYWORD: u8 = 13;
    pub const TYPE: u8 = 15;
    pub const CONSTANT: u8 = 16;
    pub const IDENTIFIER: u8 = 17;
    pub const OPEN: u8 = 19;
    pub const CLOSE: u8 = 20;
}

#[derive(Logos, Debug, PartialEq)]
enum Php {
    #[regex(r"[ \t\r\n\x0C]+")]
    Whitespace,
    #[regex(r"\$[\p{L}_\x{80}-\x{10FFFF}][\p{L}\p{N}_\x{80}-\x{10FFFF}]*")]
    Variable,
    #[regex(r"[\p{L}_\x{80}-\x{10FFFF}][\p{L}\p{N}_\x{80}-\x{10FFFF}]*")]
    Identifier,
    #[regex(r"0[xX][0-9a-fA-F]+(_[0-9a-fA-F]+)*|0[bB][01]+(_[01]+)*|0[oO][0-7]+(_[0-7]+)*")]
    #[regex(r"([0-9]+(_[0-9]+)*(\.[0-9]*(_[0-9]+)*)?|\.[0-9]+(_[0-9]+)*)([eE][+-]?[0-9]+(_[0-9]+)*)?")]
    Number,
    #[token("?>")]
    Close,
    #[token("/*")]
    Block,
    #[token("//")]
    #[token("#")]
    Line,
    #[token("#[")]
    Attribute,
    #[token("'")]
    Single,
    #[token("\"")]
    Double,
    #[token("`")]
    Backtick,
    #[token("<<<")]
    Heredoc,
    #[regex(r"[{}()\[\];,:.?+*/%=&|!~^<>@\\$-]")]
    Operator,
}

#[derive(Logos, Debug, PartialEq)]
enum Html {
    #[token("<?=")]
    Echo,
    #[regex(r"<\?[pP][hH][pP][ \t\r\n\x0C]")]
    Open,
    #[regex(r"[^<]+")]
    Text,
    #[token("<")]
    Less,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Html,
    Php,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    pub offset: usize,
    pub mode: Mode,
}
#[derive(Debug, Clone)]
pub struct Output {
    pub styles: Vec<u8>,
    pub checkpoints: Vec<Checkpoint>,
}

pub fn lex(source: &str) -> Output {
    Driver {
        source,
        styles: vec![style::HTML; source.len()],
        checkpoints: vec![],
    }
    .run()
}

struct Driver<'a> {
    source: &'a str,
    styles: Vec<u8>,
    checkpoints: Vec<Checkpoint>,
}
impl Driver<'_> {
    fn paint(&mut self, r: Range<usize>, style: u8) {
        self.styles[r].fill(style);
    }
    fn run(mut self) -> Output {
        let mut offset = 0;
        let mut mode = Mode::Html;
        self.checkpoints.push(Checkpoint { offset, mode });
        while offset < self.source.len() {
            if mode == Mode::Html {
                if self.source[offset..].eq_ignore_ascii_case("<?php") {
                    self.paint(offset..self.source.len(), style::OPEN);
                    self.checkpoints.push(Checkpoint {
                        offset: self.source.len(),
                        mode: Mode::Php,
                    });
                    break;
                }
                let mut lexer = Html::lexer(&self.source[offset..]);
                let token = lexer.next();
                let r = lexer.span();
                let end = offset + r.end;
                if matches!(token, Some(Ok(Html::Open | Html::Echo))) {
                    // Whitespace following <?php belongs to PHP, not the tag.
                    let tag_end = if token == Some(Ok(Html::Open)) {
                        offset + 5
                    } else {
                        end
                    };
                    self.paint(offset..tag_end, style::OPEN);
                    offset = tag_end;
                    mode = Mode::Php;
                    self.checkpoints.push(Checkpoint { offset, mode });
                } else {
                    offset = end;
                }
            } else {
                let end = self.php_token(offset);
                if self.styles[offset] == style::CLOSE {
                    mode = Mode::Html;
                    self.checkpoints.push(Checkpoint { offset: end, mode });
                }
                offset = end;
            }
        }
        Output {
            styles: self.styles,
            checkpoints: self.checkpoints,
        }
    }
    fn php_token(&mut self, start: usize) -> usize {
        let mut lexer = Php::lexer(&self.source[start..]);
        let token = lexer.next();
        let end = start + lexer.span().end;
        let (end, style) = match token {
            Some(Ok(Php::Whitespace)) => (end, 0),
            Some(Ok(Php::Variable)) => (end, style::VARIABLE),
            Some(Ok(Php::Identifier)) => (end, word_style(&self.source[start..end])),
            Some(Ok(Php::Number)) => (end, style::NUMBER),
            Some(Ok(Php::Close)) => (end, style::CLOSE),
            Some(Ok(Php::Single | Php::Double | Php::Backtick)) => {
                let quote = self.source.as_bytes()[start];
                return self.quoted(start, quote);
            }
            Some(Ok(Php::Block)) => {
                let end = self.source[end..]
                    .find("*/")
                    .map_or(self.source.len(), |n| end + n + 2);
                let style = if self.source[start..].starts_with("/**") {
                    style::DOC
                } else {
                    style::COMMENT
                };
                (end, style)
            }
            Some(Ok(Php::Line)) => {
                // PHP ends a line comment on ?>, but never a block comment.
                let rest = &self.source[end..];
                let stop = rest.find(['\r', '\n']).unwrap_or(rest.len());
                let stop = rest[..stop].find("?>").unwrap_or(stop);
                (end + stop, style::LINE_COMMENT)
            }
            Some(Ok(Php::Heredoc)) => {
                if let Some(end) = self.heredoc(start) {
                    return end;
                }
                (end, style::OPERATOR)
            }
            Some(Ok(Php::Attribute | Php::Operator)) => (end, style::OPERATOR),
            _ => (end, 18),
        };
        self.paint(start..end, style);
        end
    }
    fn quoted(&mut self, start: usize, quote: u8) -> usize {
        let bytes = self.source.as_bytes();
        let mut at = start + 1;
        while at < bytes.len() {
            if bytes[at] == b'\\' {
                at = (at + 2).min(bytes.len());
                continue;
            }
            if quote != b'\''
                && ((bytes[at] == b'{' && bytes.get(at + 1) == Some(&b'$'))
                    || (bytes[at] == b'$' && bytes.get(at + 1) == Some(&b'{')))
            {
                at = braced_end(bytes, at + 2, bytes.len());
                continue;
            }
            if bytes[at] == quote {
                at += 1;
                break;
            }
            at += 1;
        }
        // ASCII terminators cannot split UTF-8 characters.
        let style = if quote == b'\'' {
            style::SINGLE
        } else {
            style::DOUBLE
        };
        self.paint(start..at, style);
        if quote != b'\'' {
            self.interpolate(start + 1, at);
        }
        at
    }
    fn interpolate(&mut self, start: usize, end: usize) {
        let bytes = self.source.as_bytes();
        let mut at = start;
        while at < end {
            if bytes[at] == b'\\' {
                at = (at + 2).min(end);
                continue;
            }
            let brace = bytes[at] == b'{' && bytes.get(at + 1) == Some(&b'$');
            let dollar_brace = bytes[at] == b'$' && bytes.get(at + 1) == Some(&b'{');
            if brace || dollar_brace {
                let begin = at;
                at += 2;
                at = braced_end(bytes, at, end);
                self.paint(begin..at, style::INTERPOLATED);
            } else if bytes[at] == b'$' && self.source.is_char_boundary(at) {
                let mut lexer = Php::lexer(&self.source[at..end]);
                if lexer.next() == Some(Ok(Php::Variable)) {
                    let stop = at + lexer.span().end;
                    self.paint(at..stop, style::INTERPOLATED);
                    at = stop;
                } else {
                    at += 1;
                }
            } else {
                at += 1;
            }
        }
    }
    fn heredoc(&mut self, start: usize) -> Option<usize> {
        let line_end = start + self.source[start..].find('\n')? + 1;
        let head = self.source[start + 3..line_end].trim();
        let nowdoc = head.starts_with('\'');
        let label = if head.starts_with(['\'', '"']) {
            if head.len() < 2 || head.as_bytes().last() != head.as_bytes().first() {
                return None;
            }
            &head[1..head.len() - 1]
        } else {
            head
        };
        if label.is_empty()
            || !label.chars().enumerate().all(|(n, c)| {
                c == '_' || c.is_alphabetic() || (n > 0 && c.is_ascii_digit())
            })
        {
            return None;
        }
        let mut end = self.source.len();
        let mut content_end = end;
        let mut at = line_end;
        for line in self.source[line_end..].split_inclusive('\n') {
            let trimmed = line.trim_start_matches([' ', '\t']);
            if let Some(rest) = trimmed.strip_prefix(label) {
                let follows_identifier = rest
                    .chars()
                    .next()
                    .is_some_and(|c| c == '_' || c.is_alphanumeric());
                if !follows_identifier {
                    content_end = at;
                    end = at + line.len() - trimmed.len() + label.len();
                    break;
                }
            }
            at += line.len();
        }
        self.paint(
            start..end,
            if nowdoc {
                style::NOWDOC
            } else {
                style::HEREDOC
            },
        );
        if !nowdoc {
            self.interpolate(line_end, content_end);
        }
        Some(end)
    }
}
// Explicit stack depth avoids recursive parsing on malformed or deeply nested input.
fn braced_end(bytes: &[u8], mut at: usize, end: usize) -> usize {
    let mut depth = 1usize;
    while at < end && depth > 0 {
        match bytes[at] {
            b'\\' => {
                at = (at + 2).min(end);
                continue;
            }
            b'\'' | b'"' => {
                let q = bytes[at];
                at += 1;
                while at < end && bytes[at] != q {
                    if bytes[at] == b'\\' {
                        at += 1;
                    }
                    at = (at + 1).min(end);
                }
            }
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
        at = (at + 1).min(end);
    }
    at
}
fn word_style(word: &str) -> u8 {
    let lower = word.to_ascii_lowercase();
    if "__halt_compiler abstract and array as break callable case catch class clone const continue declare default die do echo else elseif empty enddeclare endfor endforeach endif endswitch endwhile enum eval exit extends final finally fn for foreach from function get global goto if implements include include_once instanceof insteadof interface isset list match namespace new or print private protected public readonly require require_once return set static switch throw trait try unset use var while xor yield".split_whitespace().any(|w| w == lower) { style::KEYWORD }
    else if "bool boolean int integer float double real string object mixed never void iterable resource null false true self parent".split_whitespace().any(|w| w == lower) { style::TYPE }
    else if word.chars().any(|c| c.is_uppercase()) && word.chars().all(|c| !c.is_lowercase()) { style::CONSTANT }
    else { style::IDENTIFIER }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn check(source: &str, needle: &str, expected: u8) {
        let out = lex(source);
        let at = source.find(needle).unwrap();
        assert_eq!(out.styles.len(), source.len());
        assert!(
            out.styles[at..at + needle.len()]
                .iter()
                .all(|s| *s == expected),
            "{needle}: {:?}",
            &out.styles[at..at + needle.len()]
        );
    }
    #[test]
    fn php_basics() {
        check("<?php", "<?php", 19);
        check("<?phpx", "<?phpx", 21);
        let s = "<p>λ</p><?php namespace App; readonly class Café { public private(set) int $âge = 0xFF; } ?>✓";
        for (n, st) in [
            ("<p>λ</p>", 21),
            ("<?php", 19),
            ("namespace", 13),
            ("readonly", 13),
            ("class", 13),
            ("Café", 17),
            ("$âge", 4),
            ("0xFF", 12),
            ("?>", 20),
            ("✓", 21),
        ] {
            check(s, n, st);
        }
    }
    #[test]
    fn strings_and_comments_do_not_switch_modes() {
        for body in ["'?>'", "\"?>\"", "/* ?> */", "/** @var ?> */", "`?>`"] {
            let s = format!("<?php {body}; $after = 1; ?>html");
            check(&s, "$after", 4);
            check(&s, "html", 21);
            assert_eq!(lex(&s).checkpoints.len(), 3);
        }
        check("<?= $x // ends ?>html", "html", 21);
        check("<?php #[Route('/x')]", "#[", 11);
    }
    #[test]
    fn interpolation() {
        for n in [
            "$name",
            "{$user['name']}",
            "{$user[\"name\"]}",
            "${value}",
            "{$x->{foo($y)}}",
        ] {
            check(&format!("<?php \"hello {n}\";"), n, 6);
        }
        check("<?php '\\$literal';", "$literal", 5);
        check("<?php \"\\$literal\";", "$literal", 7);
    }
    #[test]
    fn documents() {
        let s = "<?php $x = <<<TXT\n?> $name\n  TXT;\n$after = 1; ?>html";
        check(s, "?> ", 8); // Separate interpolation token is checked below.
        check(s, "$name", 6);
        check(s, "$after", 4);
        check(s, "html", 21);
        check("<?php <<<'TXT'\n?> $name\n\tTXT;\n$after;", "$name", 9);
        check("<?php <<<TXT\nnot closed λ", "not closed λ", 8);
    }
    #[test]
    fn numeric_forms() {
        for n in [
            "0",
            "12_345",
            "0b10_01",
            "0o7_1",
            "0xF_F",
            ".25",
            "1.",
            "1.2e-3",
            "1_000.5_0E+2",
        ] {
            check(&format!("<?php {n};"), n, 12);
        }
    }
    #[test]
    fn arbitrary_utf8_and_typing_prefixes() {
        let s = "<div>λ<?= \"{$café['✓']}\" ?><?php /* unterminated 世界";
        for (end, _) in s.char_indices().chain(std::iter::once((s.len(), '\0'))) {
            let text = &s[..end];
            let out = lex(text);
            assert_eq!(out.styles.len(), text.len());
            for c in out.checkpoints {
                assert!(text.is_char_boundary(c.offset));
            }
        }
        for ch in [
            '\0',
            'λ',
            'é',
            '😀',
            '\u{10ffff}',
            '\r',
            '\n',
            '<',
            '?',
            '$',
            '\'',
            '"',
            '\\',
        ] {
            for n in 0..64 {
                let s = format!("<?php {}", ch.to_string().repeat(n));
                assert_eq!(lex(&s).styles.len(), s.len());
            }
        }
    }
}
