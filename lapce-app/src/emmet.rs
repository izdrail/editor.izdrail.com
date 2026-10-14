//! Emmet-style abbreviation expansion for HTML.
//!
//! `expand_html` turns `ul>li.item*3` into a snippet string (with `$1`
//! tabstop) that the editor applies through its normal snippet path. The
//! module is pure text in, text out so it can be unit tested without a UI.
//!
//! Supported: `tag`, `#id`, `.class`, `[attr=value attr2="v w"]`, `{text}`,
//! `>` child, `+` sibling, `^` climb, `(group)`, `*n` repeat with `$`
//! numbering, implicit tags (`.a` is a div, `ul>.x` is an li) and a few
//! web-dev shortcuts (`!`, `a`, `img`, `input`, `link:css`, `script:src`).

const VOID: &[&str] = &[
    "br", "hr", "img", "input", "meta", "link", "area", "base", "col", "embed",
    "source", "track", "wbr",
];

#[derive(Debug, Clone, Default)]
struct Node {
    tag: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
    attrs: Vec<(String, String)>,
    text: Option<String>,
    children: Vec<Node>,
    /// A `(group)` has no element of its own; only its children render.
    is_group: bool,
    repeat: usize,
}

struct Parser<'a> {
    s: &'a [char],
    i: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn is_name_char(c: char) -> bool {
        c.is_alphanumeric() || matches!(c, '-' | '_' | ':' | '$' | '@')
    }

    fn name(&mut self) -> String {
        let start = self.i;
        while self.peek().is_some_and(Self::is_name_char) {
            self.i += 1;
        }
        self.s[start..self.i].iter().collect()
    }

    /// Parses a sequence joined by `+`, `>` and `^`. Returns the top-level
    /// siblings. Stops at an unmatched `)`.
    fn sequence(&mut self) -> Option<Vec<Node>> {
        let mut roots: Vec<Node> = Vec::new();
        // Path of child indexes from `roots` to the node that `>` descends into.
        let mut path: Vec<usize> = Vec::new();
        let first = self.item()?;
        roots.push(first);
        path.push(0);
        loop {
            match self.peek() {
                Some('>') => {
                    self.i += 1;
                    let n = self.item()?;
                    let parent = Self::at(&mut roots, &path);
                    parent.children.push(n);
                    let idx = parent.children.len() - 1;
                    path.push(idx);
                }
                Some('+') => {
                    self.i += 1;
                    let n = self.item()?;
                    path.pop();
                    if path.is_empty() {
                        roots.push(n);
                        path.push(roots.len() - 1);
                    } else {
                        let parent = Self::at(&mut roots, &path);
                        parent.children.push(n);
                        let idx = parent.children.len() - 1;
                        path.push(idx);
                    }
                }
                Some('^') => {
                    self.i += 1;
                    // Climb out of the current node, then continue as its
                    // parent's sibling: the following `+`/item attaches there.
                    if path.len() > 1 {
                        path.pop();
                    }
                    // `a>b^c` : c is a sibling of a. Parse the next item here.
                    if self.peek().is_some_and(|c| c != ')' && c != '^') {
                        let n = if self.peek() == Some('+') {
                            self.i += 1;
                            self.item()?
                        } else {
                            self.item()?
                        };
                        path.pop();
                        if path.is_empty() {
                            roots.push(n);
                            path.push(roots.len() - 1);
                        } else {
                            let parent = Self::at(&mut roots, &path);
                            parent.children.push(n);
                            let idx = parent.children.len() - 1;
                            path.push(idx);
                        }
                    }
                }
                _ => break,
            }
        }
        Some(roots)
    }

    fn at<'b>(roots: &'b mut [Node], path: &[usize]) -> &'b mut Node {
        let mut node = &mut roots[path[0]];
        for &i in &path[1..] {
            node = &mut node.children[i];
        }
        node
    }

    fn item(&mut self) -> Option<Node> {
        let mut node = Node {
            repeat: 1,
            ..Default::default()
        };
        if self.peek() == Some('(') {
            self.i += 1;
            let inner = self.sequence()?;
            if self.peek() != Some(')') {
                return None;
            }
            self.i += 1;
            node.is_group = true;
            node.children = inner;
        } else {
            if self.peek() == Some('!') {
                self.i += 1;
                node.tag = Some("!".into());
            } else if self.peek().is_some_and(|c| Self::is_name_char(c)) {
                node.tag = Some(self.name());
            }
            loop {
                match self.peek() {
                    Some('#') => {
                        self.i += 1;
                        node.id = Some(self.name());
                    }
                    Some('.') => {
                        self.i += 1;
                        node.classes.push(self.name());
                    }
                    Some('[') => {
                        self.i += 1;
                        self.attrs(&mut node)?;
                    }
                    Some('{') => {
                        self.i += 1;
                        let mut depth = 1;
                        let start = self.i;
                        while let Some(c) = self.peek() {
                            if c == '{' {
                                depth += 1;
                            } else if c == '}' {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            self.i += 1;
                        }
                        if depth != 0 {
                            return None;
                        }
                        node.text = Some(self.s[start..self.i].iter().collect());
                        self.i += 1;
                    }
                    _ => break,
                }
            }
            if node.tag.is_none()
                && node.id.is_none()
                && node.classes.is_empty()
                && node.attrs.is_empty()
                && node.text.is_none()
            {
                return None;
            }
        }
        if self.peek() == Some('*') {
            self.i += 1;
            let start = self.i;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.i += 1;
            }
            let n: usize = self.s[start..self.i]
                .iter()
                .collect::<String>()
                .parse()
                .ok()?;
            if n == 0 || n > 500 {
                return None;
            }
            node.repeat = n;
        }
        Some(node)
    }

    fn attrs(&mut self, node: &mut Node) -> Option<()> {
        loop {
            while self.peek() == Some(' ') {
                self.i += 1;
            }
            match self.peek()? {
                ']' => {
                    self.i += 1;
                    return Some(());
                }
                _ => {
                    let start = self.i;
                    while self
                        .peek()
                        .is_some_and(|c| c != '=' && c != ' ' && c != ']')
                    {
                        self.i += 1;
                    }
                    let key: String = self.s[start..self.i].iter().collect();
                    if key.is_empty() {
                        return None;
                    }
                    let mut value = String::new();
                    if self.peek() == Some('=') {
                        self.i += 1;
                        if let Some(q @ ('"' | '\'')) = self.peek() {
                            self.i += 1;
                            let vs = self.i;
                            while self.peek()? != q {
                                self.i += 1;
                            }
                            value = self.s[vs..self.i].iter().collect();
                            self.i += 1;
                        } else {
                            let vs = self.i;
                            while self.peek().is_some_and(|c| c != ' ' && c != ']') {
                                self.i += 1;
                            }
                            value = self.s[vs..self.i].iter().collect();
                        }
                    }
                    node.attrs.push((key, value));
                }
            }
        }
    }
}

/// Escapes text for the snippet syntax.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '$' | '\\' | '}') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Replaces `$` runs with the 1-based repeat index, zero padded to the run
/// length (`$$` -> `01`).
fn number(s: &str, index: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '$' {
            let mut n = 0;
            while i < chars.len() && chars[i] == '$' {
                n += 1;
                i += 1;
            }
            out.push_str(&format!("{index:0n$}"));
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

fn implicit_tag(parent: Option<&str>) -> &'static str {
    match parent {
        Some("ul" | "ol") => "li",
        Some("table" | "tbody" | "thead" | "tfoot") => "tr",
        Some("tr") => "td",
        Some("select" | "optgroup") => "option",
        Some("em" | "strong" | "span" | "a" | "p" | "label") => "span",
        _ => "div",
    }
}

struct Ctx {
    indent: String,
    /// Whether the first tabstop was already placed.
    stop: bool,
}

const DOCTYPE: &str = "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n{I}<meta charset=\"UTF-8\">\n{I}<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n{I}<title>${1:Document}</title>\n</head>\n<body>\n{I}$0\n</body>\n</html>";

fn render(
    nodes: &[Node],
    parent: Option<&str>,
    depth: usize,
    ctx: &mut Ctx,
    out: &mut Vec<String>,
) {
    for node in nodes {
        for n in 1..=node.repeat {
            if node.is_group {
                render(&node.children, parent, depth, ctx, out);
                continue;
            }
            let pad = ctx.indent.repeat(depth);
            let mut tag = node.tag.clone();
            if tag.as_deref() == Some("!") {
                let doc = DOCTYPE.replace("{I}", &ctx.indent);
                ctx.stop = true;
                for line in doc.split('\n') {
                    out.push(format!("{pad}{line}"));
                }
                continue;
            }
            let mut attrs: Vec<(String, String)> = Vec::new();
            // Shortcut expansions.
            let name = tag.clone().unwrap_or_default();
            match name.as_str() {
                "a" => attrs.push(("href".into(), String::new())),
                "img" => {
                    attrs.push(("src".into(), String::new()));
                    attrs.push(("alt".into(), String::new()));
                }
                "input" => attrs.push(("type".into(), "text".into())),
                "link" => {
                    attrs.push(("rel".into(), "stylesheet".into()));
                    attrs.push(("href".into(), String::new()));
                }
                "link:css" => {
                    tag = Some("link".into());
                    attrs.push(("rel".into(), "stylesheet".into()));
                    attrs.push(("href".into(), "style.css".into()));
                }
                "script:src" => {
                    tag = Some("script".into());
                    attrs.push(("src".into(), String::new()));
                }
                "btn" | "button" => {
                    tag = Some("button".into());
                }
                "form:post" => {
                    tag = Some("form".into());
                    attrs.push(("action".into(), String::new()));
                    attrs.push(("method".into(), "post".into()));
                }
                _ => {}
            }
            let tag = tag.unwrap_or_else(|| implicit_tag(parent).to_string());
            if let Some(id) = &node.id {
                attrs.push(("id".into(), number(id, n)));
            }
            if !node.classes.is_empty() {
                let c: Vec<String> =
                    node.classes.iter().map(|c| number(c, n)).collect();
                attrs.push(("class".into(), c.join(" ")));
            }
            for (k, v) in &node.attrs {
                // User attributes replace a shortcut default of the same name.
                attrs.retain(|(ek, _)| ek != k);
                attrs.push((k.clone(), number(v, n)));
            }
            let mut open = format!("<{tag}");
            for (k, v) in &attrs {
                if v.is_empty() && !node.attrs.iter().any(|(nk, _)| nk == k) {
                    // Default empty attribute (href, src, alt): tabstop.
                    if !ctx.stop {
                        ctx.stop = true;
                        open.push_str(&format!(" {}=\"$1\"", esc(k)));
                    } else {
                        open.push_str(&format!(" {}=\"\"", esc(k)));
                    }
                } else {
                    open.push_str(&format!(" {}=\"{}\"", esc(k), esc(v)));
                }
            }
            open.push('>');
            if VOID.contains(&tag.as_str()) {
                out.push(format!("{pad}{}", open));
                continue;
            }
            let text = node.text.as_deref().map(|t| esc(&number(t, n)));
            if node.children.is_empty() {
                let inner = match text {
                    Some(t) => t,
                    None if !ctx.stop => {
                        ctx.stop = true;
                        "$1".to_string()
                    }
                    None => String::new(),
                };
                out.push(format!("{pad}{open}{inner}</{tag}>"));
            } else {
                out.push(format!("{pad}{open}"));
                if let Some(t) = text {
                    out.push(format!("{pad}{}{t}", ctx.indent));
                }
                render(&node.children, Some(&tag), depth + 1, ctx, out);
                out.push(format!("{pad}</{tag}>"));
            }
        }
    }
}

/// Expands an HTML abbreviation into snippet text, or `None` when the input
/// is not a valid abbreviation. `indent` is one indent unit (e.g. 4 spaces).
pub fn expand_html(abbr: &str, indent: &str) -> Option<String> {
    let abbr = abbr.trim();
    if abbr.is_empty() {
        return None;
    }
    let chars: Vec<char> = abbr.chars().collect();
    let mut p = Parser { s: &chars, i: 0 };
    let nodes = p.sequence()?;
    if p.i != chars.len() {
        return None;
    }
    let mut ctx = Ctx {
        indent: indent.to_string(),
        stop: false,
    };
    let mut out = Vec::new();
    render(&nodes, None, 0, &mut ctx, &mut out);
    Some(out.join("\n"))
}

/// Finds the abbreviation that ends at the end of `line_before_cursor`: the
/// trailing run with no whitespace (spaces inside `[]` and `{}` allowed).
/// Returns `(char_start_index, abbreviation)`.
pub fn abbreviation_before_cursor(
    line_before_cursor: &str,
) -> Option<(usize, String)> {
    let chars: Vec<char> = line_before_cursor.chars().collect();
    let mut i = chars.len();
    let mut depth = 0i32;
    while i > 0 {
        let c = chars[i - 1];
        match c {
            ']' | '}' => depth += 1,
            '[' | '{' => depth -= 1,
            _ => {}
        }
        if depth < 0 {
            break;
        }
        if depth == 0
            && (c.is_whitespace() || c == '"' || c == '\'' || c == '<' || c == '=')
        {
            break;
        }
        i -= 1;
    }
    if depth != 0 || i == chars.len() {
        return None;
    }
    let abbr: String = chars[i..].iter().collect();
    Some((i, abbr))
}

// ---------------------------------------------------------------- CSS

/// Whole-abbreviation shortcuts that carry their own value.
const CSS_SHORTCUTS: &[(&str, &str, &str)] = &[
    ("df", "display", "flex"),
    ("dg", "display", "grid"),
    ("db", "display", "block"),
    ("dib", "display", "inline-block"),
    ("di", "display", "inline"),
    ("dn", "display", "none"),
    ("pa", "position", "absolute"),
    ("pr", "position", "relative"),
    ("pf", "position", "fixed"),
    ("ps", "position", "sticky"),
    ("fwb", "font-weight", "bold"),
    ("fsi", "font-style", "italic"),
    ("tac", "text-align", "center"),
    ("tal", "text-align", "left"),
    ("tar", "text-align", "right"),
    ("tdn", "text-decoration", "none"),
    ("tdu", "text-decoration", "underline"),
    ("jcc", "justify-content", "center"),
    ("jcsb", "justify-content", "space-between"),
    ("aic", "align-items", "center"),
    ("fxdc", "flex-direction", "column"),
    ("ovh", "overflow", "hidden"),
    ("ova", "overflow", "auto"),
    ("curp", "cursor", "pointer"),
    ("bxzb", "box-sizing", "border-box"),
];

/// Property abbreviations that take a value.
const CSS_PROPS: &[(&str, &str)] = &[
    ("m", "margin"),
    ("mt", "margin-top"),
    ("mr", "margin-right"),
    ("mb", "margin-bottom"),
    ("ml", "margin-left"),
    ("p", "padding"),
    ("pt", "padding-top"),
    ("pr", "padding-right"),
    ("pb", "padding-bottom"),
    ("pl", "padding-left"),
    ("w", "width"),
    ("h", "height"),
    ("maw", "max-width"),
    ("mah", "max-height"),
    ("miw", "min-width"),
    ("mih", "min-height"),
    ("bd", "border"),
    ("bdt", "border-top"),
    ("bdb", "border-bottom"),
    ("bdrs", "border-radius"),
    ("bg", "background"),
    ("bgc", "background-color"),
    ("bgi", "background-image"),
    ("c", "color"),
    ("fz", "font-size"),
    ("fw", "font-weight"),
    ("ff", "font-family"),
    ("lh", "line-height"),
    ("ta", "text-align"),
    ("t", "top"),
    ("r", "right"),
    ("b", "bottom"),
    ("l", "left"),
    ("z", "z-index"),
    ("op", "opacity"),
    ("fx", "flex"),
    ("gap", "gap"),
    ("bxsh", "box-shadow"),
    ("trs", "transition"),
    ("ls", "letter-spacing"),
    ("ord", "order"),
];

/// Properties whose plain numbers are not lengths.
const CSS_UNITLESS: &[&str] = &[
    "font-weight",
    "line-height",
    "z-index",
    "opacity",
    "flex",
    "order",
];

fn css_value(prop: &str, raw: &str) -> Option<String> {
    let (neg, raw) = match raw.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, raw),
    };
    if raw.is_empty() {
        return None;
    }
    // Split on `-` between values (`10-20`, `0-auto`).
    let mut parts: Vec<String> = Vec::new();
    for (i, piece) in raw.split('-').enumerate() {
        if piece.is_empty() {
            return None;
        }
        let first = piece.chars().next().unwrap();
        let v = if first.is_ascii_digit() || first == '.' {
            let split = piece
                .find(|c: char| !(c.is_ascii_digit() || c == '.'))
                .unwrap_or(piece.len());
            let (num, unit) = piece.split_at(split);
            if num.parse::<f64>().is_err() {
                return None;
            }
            let unit = match unit {
                "" if CSS_UNITLESS.contains(&prop)
                    || num.trim_matches('0').trim_matches('.').is_empty() =>
                {
                    ""
                }
                "" => "px",
                "p" => "%",
                "e" => "em",
                "r" => "rem",
                other if other.chars().all(|c| c.is_ascii_alphabetic()) => other,
                _ => return None,
            };
            let sign = if neg && i == 0 { "-" } else { "" };
            format!("{sign}{num}{unit}")
        } else if first == '#' {
            piece.to_string()
        } else if piece.chars().all(|c| c.is_ascii_alphabetic()) {
            piece.to_string()
        } else {
            return None;
        };
        parts.push(v);
    }
    Some(parts.join(" "))
}

/// Expands a CSS abbreviation (`m10-20`, `w100p`, `df`, `bgc#333!`) into a
/// declaration snippet, or `None` when it is not one.
pub fn expand_css(abbr: &str) -> Option<String> {
    let abbr = abbr.trim();
    let (abbr, important) = match abbr.strip_suffix('!') {
        Some(a) => (a, true),
        None => (abbr, false),
    };
    if abbr.is_empty() || !abbr.chars().next()?.is_ascii_alphabetic() {
        return None;
    }
    let tail = if important { " !important" } else { "" };
    if let Some((_, prop, value)) = CSS_SHORTCUTS.iter().find(|(a, _, _)| *a == abbr)
    {
        return Some(format!("{prop}: {value}{tail};"));
    }
    // Longest property abbreviation whose remainder is a value (or nothing).
    let mut best: Option<(&str, &str)> = None;
    for (a, prop) in CSS_PROPS {
        if let Some(rest) = abbr.strip_prefix(a) {
            let ok = rest.is_empty()
                || rest.starts_with(|c: char| {
                    c.is_ascii_digit() || c == '-' || c == '#' || c == '.'
                });
            if ok && best.map_or(true, |(ba, _)| a.len() > ba.len()) {
                best = Some((a, prop));
            }
        }
    }
    let (a, prop) = best?;
    let rest = &abbr[a.len()..];
    if rest.is_empty() {
        return Some(format!("{prop}: $1{tail};"));
    }
    let value = css_value(prop, rest)?;
    Some(format!("{prop}: {}{tail};", esc(&value)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn x(s: &str) -> String {
        expand_html(s, "  ").unwrap()
    }

    #[test]
    fn simple_tag_gets_tabstop() {
        assert_eq!(x("div"), "<div>$1</div>");
        assert_eq!(x(".box"), "<div class=\"box\">$1</div>");
        assert_eq!(x("#app"), "<div id=\"app\">$1</div>");
        assert_eq!(x("p.a.b#c"), "<p id=\"c\" class=\"a b\">$1</p>");
    }

    #[test]
    fn nesting_and_repeat() {
        assert_eq!(
            x("ul>li.item$*2"),
            "<ul>\n  <li class=\"item1\">$1</li>\n  <li class=\"item2\"></li>\n</ul>"
        );
        assert_eq!(x("h1{Hi}+p{x}"), "<h1>Hi</h1>\n<p>x</p>");
    }

    #[test]
    fn climb_and_groups() {
        assert_eq!(
            x("div>p^footer"),
            "<div>\n  <p>$1</p>\n</div>\n<footer></footer>"
        );
        assert_eq!(
            x("(a{x}+b{y})*2"),
            "<a href=\"$1\">x</a>\n<b>y</b>\n<a href=\"\">x</a>\n<b>y</b>"
        );
    }

    #[test]
    fn shortcuts_and_attrs() {
        assert_eq!(x("a"), "<a href=\"$1\"></a>");
        assert_eq!(x("img"), "<img src=\"$1\" alt=\"\">");
        assert_eq!(
            x("a[href=/x title=\"a b\"]{go}"),
            "<a href=\"/x\" title=\"a b\">go</a>"
        );
        assert!(x("!").starts_with("<!DOCTYPE html>"));
        assert_eq!(
            x("table>.r>.c"),
            "<table>\n  <tr class=\"r\">\n    <td class=\"c\">$1</td>\n  </tr>\n</table>"
        );
    }

    #[test]
    fn text_escapes_snippet_chars_and_numbers_dollars() {
        assert_eq!(x("p{n$}*2"), "<p>n1</p>\n<p>n2</p>");
        assert_eq!(x("p{a\\b}"), "<p>a\\\\b</p>");
    }

    #[test]
    fn invalid_returns_none() {
        assert!(expand_html("", "  ").is_none());
        assert!(expand_html("div>", "  ").is_none());
        assert!(expand_html("(div", "  ").is_none());
        assert!(expand_html("ul>li*0", "  ").is_none());
    }

    #[test]
    fn finds_abbreviation_at_cursor() {
        assert_eq!(
            abbreviation_before_cursor("    ul>li*3"),
            Some((4, "ul>li*3".into()))
        );
        assert_eq!(
            abbreviation_before_cursor("x a[href=y z]"),
            Some((2, "a[href=y z]".into()))
        );
        assert_eq!(abbreviation_before_cursor("foo "), None);
    }

    #[test]
    fn css_abbreviations() {
        let c = |s: &str| expand_css(s).unwrap();
        assert_eq!(c("m10"), "margin: 10px;");
        assert_eq!(c("m10-20"), "margin: 10px 20px;");
        assert_eq!(c("m0-auto"), "margin: 0 auto;");
        assert_eq!(c("m-10"), "margin: -10px;");
        assert_eq!(c("w100p"), "width: 100%;");
        assert_eq!(c("fz1.5r"), "font-size: 1.5rem;");
        assert_eq!(c("fw700"), "font-weight: 700;");
        assert_eq!(c("lh1.4"), "line-height: 1.4;");
        assert_eq!(c("mt"), "margin-top: $1;");
        assert_eq!(c("bgc#333!"), "background-color: #333 !important;");
        assert_eq!(c("df"), "display: flex;");
        assert_eq!(c("pa"), "position: absolute;");
        assert_eq!(c("h50vh"), "height: 50vh;");
        assert_eq!(c("z10"), "z-index: 10;");
    }

    #[test]
    fn css_rejects_non_abbreviations() {
        assert!(expand_css("").is_none());
        assert!(expand_css("margin").is_none());
        assert!(expand_css("10px").is_none());
        assert!(expand_css("m10-").is_none());
        assert!(expand_css("hello").is_none());
    }
}
