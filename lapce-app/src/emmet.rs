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
    /// `lorem` / `lorem12`: placeholder text of that many words.
    lorem: Option<usize>,
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
            if let Some(n) = node.tag.as_deref().and_then(lorem_words) {
                node.lorem = Some(n);
                node.tag = None;
            }
            if node.tag.is_none()
                && node.lorem.is_none()
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

const LOREM: &[&str] = &[
    "lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipisicing", "elit", "sed",
    "do", "eiusmod", "tempor", "incididunt", "ut", "labore", "et", "dolore", "magna",
    "aliqua", "enim", "ad", "minim", "veniam", "quis", "nostrud", "exercitation",
    "ullamco", "laboris", "nisi", "aliquip", "ex", "ea", "commodo", "consequat",
];

/// `lorem` -> 30 words, `lorem5` -> 5 (also `lipsum`). Capped at 500.
fn lorem_words(tag: &str) -> Option<usize> {
    let rest = tag.strip_prefix("lorem").or_else(|| tag.strip_prefix("lipsum"))?;
    if rest.is_empty() {
        return Some(30);
    }
    let n: usize = rest.parse().ok()?;
    (1..=500).contains(&n).then_some(n)
}

/// Placeholder text: the first `n` words of a fixed Latin word list, capitalised, with a full stop.
/// This is an approximation of Emmet's `lorem`, not byte-identical to it.
fn lorem_text(n: usize) -> String {
    let mut words: Vec<String> = (0..n).map(|i| LOREM[i % LOREM.len()].to_string()).collect();
    if let Some(w) = words.first_mut() {
        let mut c = w.chars();
        *w = c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default();
    }
    format!("{}.", words.join(" "))
}

/// Moves a lorem-only child into its parent's text (`p>lorem5`).
fn fold_lorem(nodes: &mut [Node]) {
    for node in nodes {
        for c in &mut node.children {
            fold_lorem(std::slice::from_mut(c));
        }
        if node.text.is_none() {
            if let Some(i) = node.children.iter().position(|c| {
                c.lorem.is_some() && c.id.is_none() && c.classes.is_empty() && c.attrs.is_empty() && c.text.is_none()
            }) {
                let n = node.children[i].lorem.unwrap_or(30);
                node.text = Some(lorem_text(n));
                node.children.remove(i);
            }
        }
    }
}

/// Emmet's colon snippets (`input:email`, `a:mail`, `meta:vp`, `btn:s`, ...) from
/// emmetio/emmet `src/snippets/html.json`: `(tag, [(attr, value)])`. Empty values get a tabstop.
fn alias(name: &str) -> Option<(&'static str, Vec<(&'static str, &'static str)>)> {
    let input = |t: &'static str| ("input", vec![("type", t), ("name", ""), ("id", "")]);
    Some(match name {
        "a:blank" => ("a", vec![("href", "http://"), ("target", "_blank"), ("rel", "noopener noreferrer")]),
        "a:link" => ("a", vec![("href", "http://")]),
        "a:mail" => ("a", vec![("href", "mailto:")]),
        "a:tel" => ("a", vec![("href", "tel:+")]),
        "link:print" => ("link", vec![("href", "print.css"), ("media", "print")]),
        "link:favicon" => ("link", vec![("rel", "icon"), ("type", "image/x-icon"), ("href", "favicon.ico")]),
        "link:mf" | "link:manifest" => ("link", vec![("rel", "manifest"), ("href", "manifest.json")]),
        "link:touch" => ("link", vec![("rel", "apple-touch-icon"), ("href", "favicon.png")]),
        "link:rss" => ("link", vec![("rel", "alternate"), ("type", "application/rss+xml"), ("title", "RSS"), ("href", "rss.xml")]),
        "link:atom" => ("link", vec![("rel", "alternate"), ("type", "application/atom+xml"), ("title", "Atom"), ("href", "atom.xml")]),
        "meta:utf" => ("meta", vec![("http-equiv", "Content-Type"), ("content", "text/html;charset=UTF-8")]),
        "meta:vp" => ("meta", vec![("name", "viewport"), ("content", "width=device-width, initial-scale=1.0")]),
        "meta:compat" => ("meta", vec![("http-equiv", "X-UA-Compatible"), ("content", "IE=7")]),
        "meta:edge" => ("meta", vec![("http-equiv", "X-UA-Compatible"), ("content", "ie=edge")]),
        "meta:redirect" => ("meta", vec![("http-equiv", "refresh"), ("content", "0; url=http://example.com")]),
        "meta:refresh" => ("meta", vec![("http-equiv", "refresh"), ("content", "5")]),
        "meta:kw" => ("meta", vec![("name", "keywords"), ("content", "")]),
        "meta:desc" => ("meta", vec![("name", "description"), ("content", "")]),
        "script:module" => ("script", vec![("type", "module"), ("src", "")]),
        "form:get" => ("form", vec![("action", ""), ("method", "get")]),
        "btn:s" | "button:s" | "button:submit" | "btn:submit" => ("button", vec![("type", "submit")]),
        "btn:r" | "button:r" | "button:reset" | "btn:reset" => ("button", vec![("type", "reset")]),
        "btn:b" | "button:b" | "button:button" | "btn:button" => ("button", vec![("type", "button")]),
        "input:h" | "input:hidden" => ("input", vec![("type", "hidden"), ("name", "")]),
        "input:t" | "input:text" => input("text"),
        "input:search" => input("search"),
        "input:email" => input("email"),
        "input:url" => input("url"),
        "input:p" | "input:password" => input("password"),
        "input:datetime" => input("datetime"),
        "input:date" => input("date"),
        "input:datetime-local" => input("datetime-local"),
        "input:month" => input("month"),
        "input:week" => input("week"),
        "input:time" => input("time"),
        "input:tel" => input("tel"),
        "input:number" => input("number"),
        "input:color" => input("color"),
        "input:c" | "input:checkbox" => input("checkbox"),
        "input:r" | "input:radio" => input("radio"),
        "input:range" => input("range"),
        "input:f" | "input:file" => input("file"),
        "input:s" | "input:submit" => ("input", vec![("type", "submit"), ("value", "")]),
        "input:i" | "input:image" => ("input", vec![("type", "image"), ("src", ""), ("alt", "")]),
        "input:b" | "input:btn" | "input:button" => ("input", vec![("type", "button"), ("value", "")]),
        "input:reset" => ("input", vec![("type", "reset"), ("value", "")]),
        _ => return None,
    })
}

/// A tag name that is a plain element (`div`, `my-element`, `h1`); anything else
/// (`@media`, `input:nope`, `a:`) is not an HTML abbreviation.
fn valid_tag(name: &str) -> bool {
    for p in ["lorem", "lipsum"] {
        if name.strip_prefix(p).is_some_and(|r| !r.is_empty() && r.chars().all(|c| c.is_ascii_digit())) {
            return lorem_words(name).is_some();
        }
    }
    name == "!"
        || name == "html:5"
        || alias(name).is_some()
        || matches!(name, "link:css" | "script:src" | "form:post")
        || (name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '$'))
}

fn all_tags_valid(nodes: &[Node]) -> bool {
    nodes.iter().all(|n| n.tag.as_deref().is_none_or(valid_tag) && all_tags_valid(&n.children))
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
            if matches!(tag.as_deref(), Some("!") | Some("html:5")) {
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
            if let Some((t, a)) = alias(&name) {
                tag = Some(t.into());
                attrs = a.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
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
            let lorem = node.lorem.map(lorem_text);
            let text = node.text.clone().or(lorem).map(|t| esc(&number(&t, n)));
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
    let mut nodes = p.sequence()?;
    if p.i != chars.len() || !all_tags_valid(&nodes) {
        return None;
    }
    fold_lorem(&mut nodes);
    // A lone `lorem5` is just the text.
    if let [n] = nodes.as_slice() {
        if let (Some(w), None, true) = (n.lorem, &n.text, n.id.is_none() && n.classes.is_empty() && n.attrs.is_empty() && n.repeat == 1) {
            return Some(lorem_text(w));
        }
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

const CSS_UNITS: &[&str] = &[
    "px", "em", "rem", "ex", "ch", "vw", "vh", "vmin", "vmax", "pt", "pc", "cm", "mm", "in",
    "fr", "s", "ms", "deg", "rad", "turn", "dvh", "svh", "lvh",
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
                other if CSS_UNITS.contains(&other) => other,
                _ => return None,
            };
            let sign = if neg && i == 0 { "-" } else { "" };
            format!("{sign}{num}{unit}")
        } else if first == '#' {
            piece.to_string()
        } else if piece == "a" {
            "auto".to_string()
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

#[cfg(test)]
mod alias_tests {
    use super::*;

    fn x(s: &str) -> Option<String> {
        expand_html(s, "  ")
    }

    #[test]
    fn colon_snippets_match_emmet_html_json() {
        assert_eq!(x("input:text").unwrap(), "<input type=\"text\" name=\"$1\" id=\"\">");
        assert_eq!(x("input:email").unwrap(), "<input type=\"email\" name=\"$1\" id=\"\">");
        assert_eq!(x("input:submit").unwrap(), "<input type=\"submit\" value=\"$1\">");
        assert_eq!(x("a:mail").unwrap(), "<a href=\"mailto:\">$1</a>");
        assert_eq!(x("a:blank").unwrap(), "<a href=\"http://\" target=\"_blank\" rel=\"noopener noreferrer\">$1</a>");
        assert_eq!(
            x("meta:vp").unwrap(),
            "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">"
        );
        assert_eq!(x("btn:s").unwrap(), "<button type=\"submit\">$1</button>");
        assert_eq!(x("form>input:password").unwrap().contains("type=\"password\""), true);
        assert_eq!(x("html:5"), x("!"));
        // The user's own attributes still win.
        assert_eq!(x("input:text[name=q]").unwrap(), "<input type=\"text\" id=\"$1\" name=\"q\">");
    }

    #[test]
    fn unknown_colon_and_at_names_are_not_tags() {
        for a in ["input:nope", "a:", "@media", "meta:zzz", "1abc", "-x"] {
            assert_eq!(x(a), None, "{a}");
        }
        assert!(x("my-element.x").is_some());
        assert!(x("h1").is_some());
    }

    #[test]
    fn lorem_fills_the_parent_or_stands_alone() {
        assert_eq!(x("lorem3").unwrap(), "Lorem ipsum dolor.");
        assert_eq!(x("p>lorem4").unwrap(), "<p>Lorem ipsum dolor sit.</p>");
        assert!(x("p*2>lorem2").unwrap().matches("<p>Lorem ipsum.</p>").count() == 2);
        assert!(x("lorem").unwrap().split(' ').count() == 30);
        assert_eq!(x("lorem0"), None);
        assert_eq!(x("lorem9999"), None);
        assert!(x("lorem5.x").is_some());
    }
}

#[cfg(test)]
mod css_fix_tests {
    use super::*;

    #[test]
    fn auto_and_unit_validation() {
        assert_eq!(expand_css("m0-a").as_deref(), Some("margin: 0 auto;"));
        assert_eq!(expand_css("ml-a").as_deref(), Some("margin-left: auto;"));
        assert_eq!(expand_css("w50vw").as_deref(), Some("width: 50vw;"));
        assert_eq!(expand_css("h100dvh").as_deref(), Some("height: 100dvh;"));
        assert_eq!(expand_css("m0a"), None);
        assert_eq!(expand_css("m10zz"), None);
        assert_eq!(expand_css("m10-20").as_deref(), Some("margin: 10px 20px;"));
        assert_eq!(expand_css("w100p").as_deref(), Some("width: 100%;"));
    }
}
