//! A TextMate grammar engine (the grammar format VS Code extensions ship as
//! `.tmLanguage.json`), built on Oniguruma so grammar regexes behave as they
//! do in VS Code. Pure data: grammars are never executed as code.
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, OnceLock},
};

use anyhow::{Result, bail};
use onig::{Regex, Region, SearchOptions};
use serde_json::Value;

/// Position of a rule: (grammar index in the registry, rule index).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RuleRef {
    g: usize,
    r: usize,
}

struct Rx {
    src: String,
    re: OnceLock<Option<Regex>>,
    re_nog: OnceLock<Option<Regex>>,
    has_g: bool,
}

impl Rx {
    fn new(src: &str) -> Self {
        Self {
            src: src.to_owned(),
            re: OnceLock::new(),
            re_nog: OnceLock::new(),
            has_g: src.contains("\\G"),
        }
    }
    /// `\G` only matches where the previous match ended; elsewhere it is
    /// replaced by a character that never occurs, as VS Code does.
    fn at(&self, pos: usize, anchor: usize) -> Option<&Regex> {
        if self.has_g && pos != anchor {
            self.re_nog
                .get_or_init(|| {
                    Regex::new(&self.src.replace("\\G", "\u{FFFF}")).ok()
                })
                .as_ref()
        } else {
            self.get()
        }
    }
    fn get(&self) -> Option<&Regex> {
        self.re
            .get_or_init(|| match Regex::new(&self.src) {
                Ok(r) => Some(r),
                Err(e) => {
                    eprintln!("textmate: bad regex {:?}: {e}", self.src);
                    None
                }
            })
            .as_ref()
    }
}

#[derive(Default)]
struct Capture {
    name: Option<String>,
    patterns: Vec<Pat>,
}

type Caps = Vec<(usize, Capture)>;

enum Pat {
    Rule(usize),
    SelfRef,
    Repo(String),
    Ext(String, Option<String>),
}

enum Kind {
    Group,
    Match {
        re: Rx,
        caps: Caps,
    },
    Span {
        begin: Rx,
        end_src: String,
        end_has_backref: bool,
        end: Rx,
        begin_caps: Caps,
        end_caps: Caps,
        while_: Option<Rx>,
        apply_end_last: bool,
    },
}

struct Rule {
    kind: Kind,
    name: Option<String>,
    content_name: Option<String>,
    patterns: Vec<Pat>,
}

pub struct Grammar {
    pub scope_name: String,
    rules: Vec<Rule>,
    repo: HashMap<String, usize>,
}

fn caps_from(v: Option<&Value>, g: &mut Builder) -> Caps {
    let Some(o) = v.and_then(Value::as_object) else {
        return Vec::new();
    };
    o.iter()
        .filter_map(|(k, c)| {
            let idx: usize = k.parse().ok()?;
            let patterns = c
                .get("patterns")
                .and_then(Value::as_array)
                .map(|a| a.iter().map(|p| g.pat(p)).collect())
                .unwrap_or_default();
            Some((
                idx,
                Capture {
                    name: c.get("name").and_then(Value::as_str).map(str::to_owned),
                    patterns,
                },
            ))
        })
        .collect()
}

struct Builder {
    rules: Vec<Rule>,
}

impl Builder {
    fn pat(&mut self, v: &Value) -> Pat {
        if let Some(inc) = v.get("include").and_then(Value::as_str) {
            let only_include = v.as_object().map(|o| o.len() == 1).unwrap_or(false);
            if only_include || v.get("match").is_none() && v.get("begin").is_none() {
                return if inc == "$self" || inc == "$base" {
                    Pat::SelfRef
                } else if let Some(name) = inc.strip_prefix('#') {
                    Pat::Repo(name.to_owned())
                } else if let Some((scope, name)) = inc.split_once('#') {
                    Pat::Ext(scope.to_owned(), Some(name.to_owned()))
                } else {
                    Pat::Ext(inc.to_owned(), None)
                };
            }
        }
        Pat::Rule(self.rule(v))
    }

    fn rule(&mut self, v: &Value) -> usize {
        let idx = self.rules.len();
        self.rules.push(Rule {
            kind: Kind::Group,
            name: None,
            content_name: None,
            patterns: Vec::new(),
        });
        let text = |k: &str| v.get(k).and_then(Value::as_str);
        let patterns: Vec<Pat> = v
            .get("patterns")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|p| self.pat(p)).collect())
            .unwrap_or_default();
        let both = v.get("captures");
        let kind = if let Some(m) = text("match") {
            Kind::Match {
                re: Rx::new(m),
                caps: caps_from(both, self),
            }
        } else if let Some(b) = text("begin") {
            let end_src = text("end").unwrap_or("$^").to_owned();
            let bc = caps_from(v.get("beginCaptures").or(both), self);
            let ec = caps_from(v.get("endCaptures").or(both), self);
            Kind::Span {
                begin: Rx::new(b),
                end_has_backref: has_backref(&end_src),
                end: Rx::new(&end_src),
                end_src,
                begin_caps: bc,
                end_caps: ec,
                while_: text("while").map(Rx::new),
                apply_end_last: match v.get("applyEndPatternLast") {
                    Some(Value::Bool(b)) => *b,
                    Some(Value::Number(n)) => n.as_i64() == Some(1),
                    _ => false,
                },
            }
        } else if let Some(w) = text("while").filter(|_| text("begin").is_some()) {
            let _ = w;
            Kind::Group
        } else {
            Kind::Group
        };
        let rule = &mut self.rules[idx];
        rule.kind = kind;
        rule.name = text("name").map(str::to_owned);
        rule.content_name = text("contentName").map(str::to_owned);
        rule.patterns = patterns;
        idx
    }
}

fn has_backref(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i + 1 < b.len() {
        if b[i] == b'\\' {
            if b[i + 1].is_ascii_digit() && b[i + 1] != b'0' {
                return true;
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    false
}

impl Grammar {
    pub fn parse(doc: &Value) -> Result<Grammar> {
        let Some(scope) = doc.get("scopeName").and_then(Value::as_str) else {
            bail!("grammar has no scopeName");
        };
        let mut b = Builder { rules: Vec::new() };
        // Rule 0: the grammar's top-level pattern list.
        let root = serde_json::json!({
            "patterns": doc.get("patterns").cloned().unwrap_or(Value::Array(vec![]))
        });
        b.rule(&root);
        let mut repo = HashMap::new();
        if let Some(r) = doc.get("repository").and_then(Value::as_object) {
            for (name, rule) in r {
                let id = b.rule(rule);
                repo.insert(name.clone(), id);
            }
        }
        Ok(Grammar {
            scope_name: scope.to_owned(),
            rules: b.rules,
            repo,
        })
    }
}

/// All loaded grammars, resolvable by scope name.
#[derive(Default)]
pub struct Registry {
    grammars: Vec<Grammar>,
    by_scope: HashMap<String, usize>,
    expanded: Mutex<HashMap<RuleRef, Arc<Vec<RuleRef>>>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, grammar: Grammar) {
        self.by_scope
            .insert(grammar.scope_name.clone(), self.grammars.len());
        self.grammars.push(grammar);
    }

    pub fn add_json(&mut self, doc: &Value) -> Result<String> {
        let g = Grammar::parse(doc)?;
        let scope = g.scope_name.clone();
        self.add(g);
        Ok(scope)
    }

    pub fn has_scope(&self, scope: &str) -> bool {
        self.by_scope.contains_key(scope)
    }

    pub fn scopes(&self) -> impl Iterator<Item = &str> {
        self.by_scope.keys().map(String::as_str)
    }

    fn resolve_pat(&self, g: usize, p: &Pat) -> Option<RuleRef> {
        match p {
            Pat::Rule(r) => Some(RuleRef { g, r: *r }),
            Pat::SelfRef => Some(RuleRef { g, r: 0 }),
            Pat::Repo(name) => self.grammars[g]
                .repo
                .get(name)
                .map(|r| RuleRef { g, r: *r }),
            Pat::Ext(scope, name) => {
                let g2 = *self.by_scope.get(scope)?;
                match name {
                    Some(n) => self.grammars[g2]
                        .repo
                        .get(n)
                        .map(|r| RuleRef { g: g2, r: *r }),
                    None => Some(RuleRef { g: g2, r: 0 }),
                }
            }
        }
    }

    fn rule(&self, rr: RuleRef) -> &Rule {
        &self.grammars[rr.g].rules[rr.r]
    }

    fn collect(
        &self,
        g: usize,
        pats: &[Pat],
        out: &mut Vec<RuleRef>,
        seen: &mut HashSet<RuleRef>,
    ) {
        for p in pats {
            let Some(rr) = self.resolve_pat(g, p) else {
                continue;
            };
            if !seen.insert(rr) {
                continue;
            }
            match self.rule(rr).kind {
                Kind::Group => {
                    let pats = &self.rule(rr).patterns;
                    self.collect(rr.g, pats, out, seen);
                }
                _ => out.push(rr),
            }
        }
    }

    /// Flattened leaf rules reachable from a rule's `patterns`.
    fn patterns_of(&self, rr: RuleRef) -> Arc<Vec<RuleRef>> {
        if let Some(v) = self.expanded.lock().unwrap().get(&rr) {
            return v.clone();
        }
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        self.collect(rr.g, &self.rule(rr).patterns, &mut out, &mut seen);
        let v = Arc::new(out);
        self.expanded.lock().unwrap().insert(rr, v.clone());
        v
    }

    fn patterns_of_list(&self, g: usize, pats: &[Pat]) -> Vec<RuleRef> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        self.collect(g, pats, &mut out, &mut seen);
        out
    }
}

type Scopes = Vec<Arc<str>>;

struct Frame {
    rule: RuleRef,
    name: Option<Arc<str>>,
    content: Option<Arc<str>>,
    end: EndRx,
    while_: Option<Regex>,
}

enum EndRx {
    Shared,
    Owned(Option<Regex>),
}

/// Tokenizer state carried from one line to the next.
pub struct State {
    root: RuleRef,
    root_scope: Arc<str>,
    stack: Vec<Frame>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub start: usize,
    pub end: usize,
    /// Scopes, outermost first.
    pub scopes: Scopes,
}

impl Registry {
    pub fn initial_state(&self, scope: &str) -> Option<State> {
        let g = *self.by_scope.get(scope)?;
        Some(State {
            root: RuleRef { g, r: 0 },
            root_scope: scope.into(),
            stack: Vec::new(),
        })
    }

    fn base_scopes(&self, st: &State) -> Scopes {
        let mut s = vec![st.root_scope.clone()];
        for f in &st.stack {
            if let Some(n) = &f.name {
                s.push(n.clone());
            }
            if let Some(n) = &f.content {
                s.push(n.clone());
            }
        }
        s
    }

    fn parent_scopes_of_top(&self, st: &State) -> Scopes {
        // scopes without the top frame's own name/content
        let mut s = vec![st.root_scope.clone()];
        let n = st.stack.len().saturating_sub(1);
        for f in &st.stack[..n] {
            if let Some(x) = &f.name {
                s.push(x.clone());
            }
            if let Some(x) = &f.content {
                s.push(x.clone());
            }
        }
        s
    }

    /// Tokenise one line. `line` should include its trailing newline.
    pub fn tokenize_line(&self, st: &mut State, line: &str) -> Vec<Token> {
        let mut tokens: Vec<Token> = Vec::new();
        // `while` conditions of open begin/while rules
        let mut keep = st.stack.len();
        for (i, f) in st.stack.iter().enumerate() {
            if let Some(w) = &f.while_ {
                let ok = w
                    .search_with_options(
                        line,
                        0,
                        line.len(),
                        SearchOptions::SEARCH_OPTION_NONE,
                        None,
                    )
                    .map(|p| p == 0)
                    .unwrap_or(false);
                if !ok {
                    keep = i;
                    break;
                }
            }
        }
        st.stack.truncate(keep);

        let len = line.len();
        let mut pos = 0usize;
        let mut anchor = usize::MAX;
        let mut zero_steps = 0usize;
        let mut region = Region::new();
        while pos <= len {
            let top_rule = st.stack.last().map(|f| f.rule).unwrap_or(st.root);
            let (pats, apply_last) = match st.stack.last() {
                Some(f) => match &self.rule(f.rule).kind {
                    Kind::Span { apply_end_last, .. } => {
                        (self.patterns_of(f.rule), *apply_end_last)
                    }
                    _ => (Arc::new(Vec::new()), false),
                },
                None => (self.patterns_of(top_rule), false),
            };

            // candidate: (start, order, end, region, which)
            struct Best {
                start: usize,
                end: usize,
                region: Region,
                which: Option<RuleRef>, // None = end of top frame
            }
            let mut best: Option<Best> = None;

            let mut try_rx = |re: &Regex,
                              _has_g: bool,
                              which: Option<RuleRef>,
                              last: bool| {
                if re
                    .search_with_options(
                        line,
                        pos,
                        len,
                        SearchOptions::SEARCH_OPTION_NONE,
                        Some(&mut region),
                    )
                    .is_some()
                {
                    let (s, e) = region.pos(0).unwrap();
                    let better = match &best {
                        None => true,
                        Some(b) => s < b.start || (s == b.start && last && false),
                    };
                    if better {
                        best = Some(Best {
                            start: s,
                            end: e,
                            region: region.clone(),
                            which,
                        });
                    }
                }
            };

            if !apply_last {
                if let Some(f) = st.stack.last() {
                    match &f.end {
                        EndRx::Owned(Some(re)) => try_rx(re, false, None, false),
                        EndRx::Owned(None) => {}
                        EndRx::Shared => {
                            if let Kind::Span { end, .. } = &self.rule(f.rule).kind {
                                if let Some(re) = end.at(pos, anchor) {
                                    try_rx(re, false, None, false);
                                }
                            }
                        }
                    }
                }
            }
            for rr in pats.iter() {
                let rule = self.rule(*rr);
                let rx = match &rule.kind {
                    Kind::Match { re, .. } => re,
                    Kind::Span { begin, .. } => begin,
                    Kind::Group => continue,
                };
                if let Some(re) = rx.at(pos, anchor) {
                    try_rx(re, false, Some(*rr), false);
                }
            }
            if apply_last {
                if let Some(f) = st.stack.last() {
                    match &f.end {
                        EndRx::Owned(Some(re)) => try_rx(re, false, None, true),
                        EndRx::Owned(None) => {}
                        EndRx::Shared => {
                            if let Kind::Span { end, .. } = &self.rule(f.rule).kind {
                                if let Some(re) = end.at(pos, anchor) {
                                    try_rx(re, false, None, true);
                                }
                            }
                        }
                    }
                }
            }

            let Some(b) = best else {
                if pos < len {
                    tokens.push(Token {
                        start: pos,
                        end: len,
                        scopes: self.base_scopes(st),
                    });
                }
                break;
            };
            if b.start > pos {
                tokens.push(Token {
                    start: pos,
                    end: b.start,
                    scopes: self.base_scopes(st),
                });
            }
            let (ms, me) = (b.start, b.end);
            match b.which {
                None => {
                    // end of the current span
                    let f = st.stack.last().unwrap();
                    let mut scopes = self.parent_scopes_of_top(st);
                    if let Some(n) = &f.name {
                        scopes.push(n.clone());
                    }
                    let caps = match &self.rule(f.rule).kind {
                        Kind::Span { end_caps, .. } => end_caps,
                        _ => unreachable!(),
                    };
                    self.emit(
                        line,
                        ms,
                        me,
                        &scopes,
                        caps,
                        &b.region,
                        f.rule.g,
                        &mut tokens,
                        0,
                    );
                    st.stack.pop();
                }
                Some(rr) => match &self.rule(rr).kind {
                    Kind::Match { caps, .. } => {
                        let mut scopes = self.base_scopes(st);
                        if let Some(n) = &self.rule(rr).name {
                            scopes.push(n.as_str().into());
                        }
                        self.emit(
                            line,
                            ms,
                            me,
                            &scopes,
                            caps,
                            &b.region,
                            rr.g,
                            &mut tokens,
                            0,
                        );
                    }
                    Kind::Span {
                        end_src,
                        end_has_backref,
                        begin_caps,
                        while_,
                        ..
                    } => {
                        let rule = self.rule(rr);
                        let mut scopes = self.base_scopes(st);
                        let name: Option<Arc<str>> =
                            rule.name.as_deref().map(Into::into);
                        if let Some(n) = &name {
                            scopes.push(n.clone());
                        }
                        self.emit(
                            line,
                            ms,
                            me,
                            &scopes,
                            begin_caps,
                            &b.region,
                            rr.g,
                            &mut tokens,
                            0,
                        );
                        let end = if *end_has_backref {
                            EndRx::Owned(
                                Regex::new(&substitute_backrefs(
                                    end_src, line, &b.region,
                                ))
                                .ok(),
                            )
                        } else {
                            EndRx::Shared
                        };
                        let wh = while_.as_ref().and_then(|w| {
                            Regex::new(&substitute_backrefs(&w.src, line, &b.region))
                                .ok()
                        });
                        st.stack.push(Frame {
                            rule: rr,
                            name,
                            content: rule.content_name.as_deref().map(Into::into),
                            end,
                            while_: wh,
                        });
                    }
                    Kind::Group => {}
                },
            }
            if me == pos && ms == pos {
                zero_steps += 1;
                if zero_steps > 20 {
                    // Guard against grammars that match empty text forever.
                    let next =
                        line[pos..].chars().next().map(char::len_utf8).unwrap_or(1);
                    tokens.push(Token {
                        start: pos,
                        end: pos + next,
                        scopes: self.base_scopes(st),
                    });
                    pos += next;
                    zero_steps = 0;
                    anchor = pos;
                    continue;
                }
            } else {
                zero_steps = 0;
            }
            pos = me.max(pos);
            anchor = pos;
        }
        tokens.retain(|t| t.end > t.start);
        tokens
    }

    #[allow(clippy::too_many_arguments)]
    fn emit(
        &self,
        line: &str,
        ms: usize,
        me: usize,
        scopes: &Scopes,
        caps: &Caps,
        region: &Region,
        g: usize,
        tokens: &mut Vec<Token>,
        depth: usize,
    ) {
        if me <= ms {
            return;
        }
        // capture group spans, applied in group order
        let mut spans: Vec<(usize, usize, Option<Arc<str>>, usize)> = Vec::new();
        for (idx, cap) in caps {
            let Some((cs, ce)) = region.pos(*idx) else {
                continue;
            };
            if ce <= cs || cs < ms || ce > me {
                continue;
            }
            spans.push((cs, ce, cap.name.as_deref().map(Into::into), *idx));
        }
        spans.sort_by_key(|s| s.3);
        let mut cuts: Vec<usize> = vec![ms, me];
        for s in &spans {
            cuts.push(s.0);
            cuts.push(s.1);
        }
        cuts.sort_unstable();
        cuts.dedup();
        for w in cuts.windows(2) {
            let (s, e) = (w[0], w[1]);
            let mut sc = scopes.clone();
            for sp in &spans {
                if sp.0 <= s && e <= sp.1 {
                    if let Some(n) = &sp.2 {
                        sc.push(n.clone());
                    }
                }
            }
            tokens.push(Token {
                start: s,
                end: e,
                scopes: sc,
            });
        }
        if depth < 3 {
            for (idx, cap) in caps {
                if cap.patterns.is_empty() {
                    continue;
                }
                let Some((cs, ce)) = region.pos(*idx) else {
                    continue;
                };
                if ce <= cs || cs < ms || ce > me {
                    continue;
                }
                // re-tokenise the captured text with the capture's patterns
                let mut base = scopes.clone();
                if let Some(n) = &cap.name {
                    base.push(n.as_str().into());
                }
                let rules = self.patterns_of_list(g, &cap.patterns);
                let mut sub = Vec::new();
                self.sub_tokenize(line, cs, ce, &base, &rules, &mut sub, depth + 1);
                if !sub.is_empty() {
                    // later tokens override earlier ones on overlap
                    tokens.extend(sub);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn sub_tokenize(
        &self,
        line: &str,
        from: usize,
        to: usize,
        base: &Scopes,
        rules: &[RuleRef],
        out: &mut Vec<Token>,
        depth: usize,
    ) {
        let mut pos = from;
        let mut region = Region::new();
        while pos < to {
            let mut best: Option<(usize, usize, Region, RuleRef)> = None;
            for rr in rules {
                if let Kind::Match { re, .. } = &self.rule(*rr).kind {
                    let Some(re) = re.at(pos, from) else { continue };
                    if re
                        .search_with_options(
                            line,
                            pos,
                            to,
                            SearchOptions::SEARCH_OPTION_NONE,
                            Some(&mut region),
                        )
                        .is_some()
                    {
                        let (s, e) = region.pos(0).unwrap();
                        if best.as_ref().map(|b| s < b.0).unwrap_or(true) {
                            best = Some((s, e, region.clone(), *rr));
                        }
                    }
                }
            }
            let Some((s, e, reg, rr)) = best else { break };
            let rule = self.rule(rr);
            let mut sc = base.clone();
            if let Some(n) = &rule.name {
                sc.push(n.as_str().into());
            }
            if let Kind::Match { caps, .. } = &rule.kind {
                self.emit(line, s, e, &sc, caps, &reg, rr.g, out, depth);
                if caps.is_empty() && e > s {
                    // emit() pushed the whole span already
                }
            }
            pos = if e > pos { e } else { pos + 1 };
        }
    }
}

fn substitute_backrefs(src: &str, line: &str, region: &Region) -> String {
    let b: Vec<char> = src.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == '\\' && i + 1 < b.len() {
            if let Some(d) = b[i + 1].to_digit(10).filter(|d| *d > 0) {
                if let Some((s, e)) = region.pos(d as usize) {
                    for c in line[s..e].chars() {
                        if "\\|()[]{}^$*+?.-".contains(c) {
                            out.push('\\');
                        }
                        out.push(c);
                    }
                }
                i += 2;
                continue;
            }
            out.push(b[i]);
            out.push(b[i + 1]);
            i += 2;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// Tokenised document: one list of (start, end, scope key) per line, byte
/// offsets within the line (without the line terminator).
pub struct Snapshot {
    pub len: usize,
    pub lines: Vec<Vec<(u32, u32, Arc<str>)>>,
}

/// Documents above these limits are left unhighlighted rather than slowing
/// the editor down.
pub const MAX_BYTES: usize = 3 * 1024 * 1024;
pub const MAX_LINE: usize = 20_000;

impl Registry {
    pub fn highlight(&self, scope: &str, text: &str) -> Option<Snapshot> {
        if text.len() > MAX_BYTES {
            return None;
        }
        let mut st = self.initial_state(scope)?;
        let mut lines = Vec::new();
        let mut cache: HashMap<Scopes, Arc<str>> = HashMap::new();
        for raw in text.split_inclusive('\n') {
            let content = raw.trim_end_matches(['\n', '\r']);
            if content.len() > MAX_LINE {
                lines.push(Vec::new());
                continue;
            }
            let mut owned = String::with_capacity(content.len() + 1);
            owned.push_str(content);
            owned.push('\n');
            let toks = self.tokenize_line(&mut st, &owned);
            let mut out: Vec<(u32, u32, Arc<str>)> = Vec::new();
            for t in toks {
                let end = t.end.min(content.len());
                if end <= t.start {
                    continue;
                }
                let key = cache
                    .entry(t.scopes.clone())
                    .or_insert_with(|| scope_key(&t.scopes))
                    .clone();
                if let Some(last) = out.last_mut() {
                    if last.1 as usize == t.start && Arc::ptr_eq(&last.2, &key) {
                        last.1 = end as u32;
                        continue;
                    }
                }
                out.push((t.start as u32, end as u32, key));
            }
            lines.push(out);
        }
        if text.ends_with('\n') || text.is_empty() {
            lines.push(Vec::new());
        }
        Some(Snapshot {
            len: text.len(),
            lines,
        })
    }
}

/// Style key for a scope stack: innermost scope first, space separated, with
/// the root scope dropped. Themes resolve it by trying each scope and trimming
/// dotted suffixes, which mirrors TextMate theme matching.
pub fn scope_key(scopes: &Scopes) -> Arc<str> {
    let mut parts: Vec<&str> = scopes.iter().skip(1).map(|s| &**s).collect();
    parts.reverse();
    if parts.is_empty() {
        return "".into();
    }
    parts.join(" ").into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg(json: &str) -> (Registry, String) {
        let doc: Value = serde_json::from_str(json).unwrap();
        let mut r = Registry::new();
        let s = r.add_json(&doc).unwrap();
        (r, s)
    }

    fn keys(snap: &Snapshot, line: usize, text: &str) -> Vec<(String, String)> {
        let l: Vec<&str> = text.split('\n').collect();
        snap.lines[line]
            .iter()
            .map(|(s, e, k)| {
                (l[line][*s as usize..*e as usize].to_owned(), k.to_string())
            })
            .collect()
    }

    const G: &str = r##"{"scopeName":"source.t","patterns":[
      {"include":"#c"},{"include":"#s"},
      {"match":"\\b(let|fn)\\b","name":"keyword.control.t","captures":{"1":{"name":"storage.t"}}}],
     "repository":{
      "c":{"begin":"/\\*","end":"\\*/","name":"comment.block.t"},
      "s":{"begin":"(\"|')","end":"\\1","name":"string.quoted.t","patterns":[{"match":"\\\\.","name":"constant.character.escape.t"}]}}}"##;

    #[test]
    fn tokenizes_match_begin_end_and_backrefs() {
        let (r, scope) = reg(G);
        let text = "let x = 'a\\n';\n/* multi\nline */ fn";
        let snap = r.highlight(&scope, text).unwrap();
        let l0 = keys(&snap, 0, text);
        assert_eq!(l0[0].0, "let");
        assert!(l0[0].1.contains("keyword.control.t"), "{l0:?}");
        assert!(l0[0].1.starts_with("storage.t"), "{l0:?}");
        let esc = l0.iter().find(|t| t.0 == "\\n").unwrap();
        assert!(
            esc.1
                .starts_with("constant.character.escape.t string.quoted.t"),
            "{esc:?}"
        );
        // multi-line comment keeps state across lines
        let l1 = keys(&snap, 1, text);
        assert!(l1[0].1.contains("comment.block.t"));
        let l2 = keys(&snap, 2, text);
        assert!(l2[0].1.contains("comment.block.t"), "{l2:?}");
        assert!(l2.last().unwrap().1.contains("keyword.control.t"));
        // backreference: a ' string is not closed by "
        let text = "'a\"b'";
        let snap = r.highlight(&scope, text).unwrap();
        let l = keys(&snap, 0, text);
        assert!(l.iter().all(|t| t.1.contains("string.quoted.t")), "{l:?}");
    }

    #[test]
    fn utf8_offsets() {
        let (r, scope) = reg(G);
        let text = "λ let";
        let snap = r.highlight(&scope, text).unwrap();
        let l = keys(&snap, 0, text);
        assert_eq!(l.last().unwrap().0, "let");
    }

    #[test]
    fn empty_match_does_not_hang() {
        let (r, scope) = reg(
            r#"{"scopeName":"source.e","patterns":[{"match":"x*","name":"a.e"}]}"#,
        );
        let snap = r.highlight(&scope, "abc\nxxx\n").unwrap();
        assert_eq!(snap.lines.len(), 3);
    }

    #[test]
    fn anchor_g_and_end_of_line_pops() {
        // The ini grammar pattern: zero-width begin, end (?!\G), inner comment.
        let (r, scope) = reg(r##"{"scopeName":"source.i","patterns":[
            {"begin":"(^[ \\t]+)?(?=;)","end":"(?!\\G)","patterns":[{"begin":";","end":"\\n","name":"comment.i"}]},
            {"match":"^(\\[)(.*?)(\\])","name":"section.i"}]}"##);
        let text = "; c\n[sec]\n";
        let snap = r.highlight(&scope, text).unwrap();
        let l1 = keys(&snap, 1, text);
        assert_eq!(l1[0].1, "section.i", "{l1:?}");
    }
}
