//! Experimental PHP/HTML byte styling. No parser, I/O, PHP execution or UI state.
//! Style IDs match LexPHP. Checkpoints currently support inspection, not incremental reuse.
use logos::Logos;
use std::ops::Range;

pub mod style {
    pub const DEFAULT: u8 = 0;
    pub const CHARACTER: u8 = 10;
    pub const FUNCTION: u8 = 14;
    pub const ERROR: u8 = 18;
    pub const HTML: u8 = 21;
    // Host-only IDs never overlap the native PHP0-21 ABI.
    pub const HTML_TAG: u8 = 32;
    pub const HTML_ATTRIBUTE: u8 = 33;
    pub const HTML_STRING: u8 = 34;
    pub const HTML_COMMENT: u8 = 35;
    pub const HTML_DELIMITER: u8 = 36;
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
    #[regex(r"<\?[pP][hH][pP][ \t\r\n]")]
    Open,
    #[regex(r"[^<]")]
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
    pub host: HtmlContext,
}
#[derive(Debug, Clone)]
pub struct Output {
    pub styles: Vec<u8>,
    pub checkpoints: Vec<Checkpoint>,
}

pub fn lex(source: &str) -> Output { lex_mode(source, false) }

/// Blade is an explicit opt-in host mode. Never executes templates or PHP.
pub fn lex_blade(source: &str) -> Output { lex_mode(source, true) }

fn lex_mode(source: &str, blade: bool) -> Output {
    Driver {
        blade,
        source,
        styles: vec![style::HTML; source.len()],
        checkpoints: vec![],
        html: HtmlContext::Text,
    }
    .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtmlContext {
    Text,
    Tag(bool), // true before the element name
    Quoted(u8),
    Comment,
}
struct Driver<'a> {
    blade: bool,
    source: &'a str,
    styles: Vec<u8>,
    checkpoints: Vec<Checkpoint>,
    html: HtmlContext,
}
impl Driver<'_> {
    fn paint(&mut self, r: Range<usize>, style: u8) {
        self.styles[r].fill(style);
    }
    fn run(mut self) -> Output {
        let mut offset = 0;
        let mut mode = Mode::Html;
        self.checkpoints.push(Checkpoint { offset, mode, host: self.html });
        while offset < self.source.len() {
            if mode == Mode::Html {
                if self.blade {
                    if let Some(end) = self.blade_token(offset) { offset=end; continue; }
                }
                if self.source[offset..].eq_ignore_ascii_case("<?php") {
                    self.paint(offset..self.source.len(), style::OPEN);
                    self.checkpoints.push(Checkpoint {
                        offset: self.source.len(),
                        mode: Mode::Php,
                        host: self.html,
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
                    self.checkpoints.push(Checkpoint { offset, mode, host: self.html });
                } else {
                    offset = self.html_token(offset);
                }
            } else {
                let end = self.php_token(offset);
                if self.styles[offset] == style::CLOSE {
                    mode = Mode::Html;
                    self.checkpoints.push(Checkpoint { offset: end, mode, host: self.html });
                }
                offset = end;
            }
        }
        Output {
            styles: self.styles,
            checkpoints: self.checkpoints,
        }
    }
    // Host syntax survives PHP islands, including islands inside quoted
    // attribute values and HTML comments. PHP tags are recognized before this
    // scanner in every host context, just as PHP's inline-HTML mode requires.
    fn html_token(&mut self, start: usize) -> usize {
        let bytes = self.source.as_bytes();
        let rest = &self.source[start..];
        let (end, scope) = match self.html {
            HtmlContext::Text if rest.starts_with("<!--") => {
                self.html = HtmlContext::Comment;
                (start + 4, style::HTML_COMMENT)
            }
            HtmlContext::Text if bytes[start] == b'<' && bytes.get(start+1)
                .is_some_and(|b| b.is_ascii_alphabetic() || matches!(b,b'/'|b'!'|b'?')) => {
                self.html = HtmlContext::Tag(true);
                (start + 1, style::HTML_DELIMITER)
            }
            HtmlContext::Text => {
                let end = bytes[start+1..].iter().position(|b| *b==b'<' || (self.blade && matches!(*b,b'@'|b'{'))).map_or(bytes.len(), |n| start+1+n);
                // Search byte-wise for ASCII delimiters, never slice UTF-8 at1.
                (end, style::HTML)
            }
            HtmlContext::Comment if rest.starts_with("-->") => {
                self.html = HtmlContext::Text;
                (start+3, style::HTML_COMMENT)
            }
            HtmlContext::Comment => {
                let mut end=start+1;
                while end<bytes.len() && !self.source.is_char_boundary(end) { end+=1; }
                while end<bytes.len() && !self.source[end..].starts_with("-->")
                    && !(self.blade && matches!(bytes[end], b'@'|b'{'))
                    && !self.source[end..].starts_with("<?") {
                    end+=self.source[end..].chars().next().unwrap().len_utf8();
                }
                (end,style::HTML_COMMENT)
            }
            HtmlContext::Quoted(quote) => {
                let mut end=start;
                if bytes[end]==quote { end+=1; self.html=HtmlContext::Tag(false); }
                else {
                    while end<bytes.len() && bytes[end]!=quote
                        && !(self.blade && matches!(bytes[end],b'@'|b'{'))
                        && !(bytes[end]==b'<' && bytes.get(end+1)==Some(&b'?')) { end+=1; }
                }
                if end==start { end+=1; } // Invalid <? prefix, still host text.
                (end,style::HTML_STRING)
            }
            HtmlContext::Tag(name) => {
                if bytes[start]==b'>' {
                    self.html=HtmlContext::Text;
                    (start+1,style::HTML_DELIMITER)
                } else if matches!(bytes[start], b'\''|b'"') {
                    self.html=HtmlContext::Quoted(bytes[start]);
                    (start+1,style::HTML_STRING)
                } else if bytes[start].is_ascii_whitespace() || matches!(bytes[start],b'/'|b'='|b'!'|b'?') {
                    (start+1,style::HTML_DELIMITER)
                } else {
                    let mut end=start+1;
                    while end<bytes.len() && !bytes[end].is_ascii_whitespace()
                        && !(self.blade && matches!(bytes[end],b'@'|b'{'))
                        && !matches!(bytes[end], b'<'|b'>'|b'/'|b'='|b'\''|b'"') { end+=1; }
                    self.html=HtmlContext::Tag(false);
                    (end,if name {style::HTML_TAG} else {style::HTML_ATTRIBUTE})
                }
            }
        };
        self.paint(start..end,scope);
        end
    }
    fn blade_token(&mut self, start: usize) -> Option<usize> {
        let rest=&self.source[start..];
        if rest.starts_with("{{--") {
            let end=rest[4..].find("--}}").map_or(self.source.len(),|n|start+4+n+4);
            self.paint(start..end,style::COMMENT); return Some(end);
        }
        // Escaped Blade constructs are literal output, including escaped echos.
        if rest.starts_with("@@") || rest.starts_with("@{{") || rest.starts_with("@{!!") {
            let mut end=start+2;
            if rest.starts_with("@{{") || rest.starts_with("@{!!") {
                let close=if rest.starts_with("@{{") {"}}"} else {"!!}"};
                end=self.source[start+3..].find(close).map_or(self.source.len(),|n|start+3+n+close.len());
            } else {
                while end<self.source.len() && self.source.as_bytes()[end].is_ascii_alphabetic() {end+=1;}
            }
            let scope=if matches!(self.html,HtmlContext::Quoted(_)) {style::HTML_STRING} else {style::HTML};
            self.paint(start..end,scope); return Some(end);
        }
        let echo=if rest.starts_with("{!!") {Some((3,"!!}"))} else if rest.starts_with("{{") {Some((2,"}}"))} else {None};
        if let Some((len,close))=echo {
            self.paint(start..start+len,style::OPEN);
            let end=self.blade_php(start+len,close,false);
            return Some(end);
        }
        if rest.starts_with('@') && rest.as_bytes().get(1).is_some_and(u8::is_ascii_alphabetic) {
            let mut end=start+2;
            while end<self.source.len() && (self.source.as_bytes()[end].is_ascii_alphanumeric() || self.source.as_bytes()[end]==b'_') {end+=1;}
            let name=&self.source[start+1..end];
            // Avoid treating email addresses as directives.
            if start>0 && self.source.as_bytes()[start-1].is_ascii_alphanumeric() && self.source.as_bytes().get(end)==Some(&b'.') {return None;}
            self.paint(start..end,style::KEYWORD);
            if name=="verbatim" {
                let close="@endverbatim";
                let stop=self.source[end..].find(close).map_or(self.source.len(),|n|end+n);
                self.paint(end..stop,style::HTML);
                let finish=(stop+close.len()).min(self.source.len());
                self.paint(stop..finish,style::KEYWORD);return Some(finish);
            }
            let mut args=end;
            while args<self.source.len() && self.source.as_bytes()[args].is_ascii_whitespace() {args+=1;}
            if self.source.as_bytes().get(args)==Some(&b'(') {
                self.paint(end..args,style::DEFAULT);
                return Some(self.blade_php(args,"",true));
            }
            if name=="php" { return Some(self.blade_php(end,"@endphp",false)); }
            return Some(end);
        }
        None
    }
    // PHP token scanners consume strings/comments/heredocs atomically, so Blade
    // terminators inside those tokens never end an island. Brackets are counted
    // lexically, without attempting to parse expressions or evaluate directives.
    fn blade_php(&mut self, mut at: usize, close: &str, parentheses: bool) -> usize {
        let mut depth=0usize;
        while at<self.source.len() {
            if !parentheses && depth==0 && self.source[at..].starts_with(close) {
                let end=at+close.len();self.paint(at..end,style::CLOSE);return end;
            }
            let byte=self.source.as_bytes()[at];
            let end=self.php_token(at);
            if self.styles[at]==style::OPERATOR {
                if matches!(byte,b'('|b'['|b'{') {depth+=1;}
                if matches!(byte,b')'|b']'|b'}') {
                    depth=depth.saturating_sub(1);
                    if parentheses && depth==0 {return end;}
                }
            }
            at=end;
        }
        at
    }
    fn php_token(&mut self, start: usize) -> usize {
        let mut lexer = Php::lexer(&self.source[start..]);
        let token = lexer.next();
        let end = start + lexer.span().end;
        let (end, style) = match token {
            Some(Ok(Php::Whitespace)) => (end, style::DEFAULT),
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
                let style = if self.source[start..].starts_with("/**")
                    && !self.source[start..].starts_with("/**/") {
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
            _ => (end, style::ERROR),
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
        let style = match quote {
            b'\'' => style::SINGLE,
            b'`' => style::CHARACTER,
            _ => style::DOUBLE,
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
                    let mut stop = at + lexer.span().end;
                    // PHP's simple interpolation can include one array offset
                    // or an object property. Complex expressions need braces.
                    if bytes.get(stop) == Some(&b'[') {
                        let mut cursor = stop + 1;
                        if bytes.get(cursor) == Some(&b'-') { cursor += 1; }
                        while cursor < end && (bytes[cursor].is_ascii_alphanumeric()
                            || bytes[cursor] == b'_' || bytes[cursor] == b'$'
                            || bytes[cursor] >= 0x80) { cursor += 1; }
                        if cursor > stop + 1 && bytes.get(cursor) == Some(&b']') {
                            stop = cursor + 1;
                        }
                    } else if self.source[stop..end].starts_with("->") {
                        let begin = stop + 2;
                        let mut property = Php::lexer(&self.source[begin..end]);
                        if property.next() == Some(Ok(Php::Identifier)) {
                            stop = begin + property.span().end;
                        }
                    }
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
        assert!(!lex("<?phpx").styles.contains(&style::OPEN));
        let s = "<p>λ</p><?php namespace App; readonly class Café { public private(set) int $âge = 0xFF; } ?>✓";
        for (n, st) in [
            ("λ", 21),
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
    fn deterministic_noise_keeps_every_utf8_byte_and_terminates() {
        let alphabet = [
            'λ', '😀', 'a', '0', '\0', '\n', '\r', '\t', '<', '>', '?', '$', '{',
            '}', '\\', '\'', '"', '/', '*', '#', '[', ']',
        ];
        let mut state = 47u64;
        for len in 0..512 {
            let mut source = String::from("<?php ");
            for _ in 0..len {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                source.push(alphabet[(state >> 32) as usize % alphabet.len()]);
            }
            let out = lex(&source);
            assert_eq!(out.styles.len(), source.len());
            for point in out.checkpoints {
                assert!(source.is_char_boundary(point.offset));
            }
            for (i, ch) in source.char_indices() {
                assert!(
                    out.styles[i..i + ch.len_utf8()]
                        .iter()
                        .all(|s| *s == out.styles[i])
                );
            }
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
