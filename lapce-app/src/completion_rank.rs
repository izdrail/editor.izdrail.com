//! Ordering and de-duplication rules for the completion popup. Pure
//! functions with no UI types, so they can be unit tested on their own.
use std::{cmp::Ordering, collections::HashMap};

/// What the popup sorts by. A lower `tier` is a higher priority source:
/// LSP, then snippets, then lexer word lists, then buffer words.
#[derive(Clone, Copy, Debug)]
pub struct Rank<'a> {
    pub tier: u8,
    /// Fuzzy match score of the filter text, plus prefix and preselect bonuses.
    pub score: u32,
    /// Fuzzy match score of the label.
    pub label_score: u32,
    pub sort_text: &'a str,
    pub label_len: usize,
}

/// Source first, then match quality inside a source, then the server's
/// `sortText`, then the shorter label.
pub fn compare(a: &Rank, b: &Rank) -> Ordering {
    a.tier
        .cmp(&b.tier)
        .then_with(|| b.score.cmp(&a.score))
        .then_with(|| b.label_score.cmp(&a.label_score))
        .then_with(|| a.sort_text.cmp(b.sort_text))
        .then_with(|| a.label_len.cmp(&b.label_len))
}

/// Drop an item when another source with a higher priority (lower tier)
/// offers the same label. Items of the same source keep each other: two
/// LSP overloads with one label are both real. Order is preserved.
pub fn dedupe_across_sources<T>(
    items: Vec<T>,
    label: impl Fn(&T) -> &str,
    tier: impl Fn(&T) -> u8,
) -> Vec<T> {
    let mut best: HashMap<String, u8> = HashMap::with_capacity(items.len());
    for i in &items {
        let t = tier(i);
        best.entry(label(i).to_string())
            .and_modify(|b| *b = (*b).min(t))
            .or_insert(t);
    }
    items
        .into_iter()
        .filter(|i| best.get(label(i)).is_none_or(|b| tier(i) <= *b))
        .collect()
}

/// Case-insensitive prefix test without allocating (it runs per item on
/// every keystroke).
pub fn has_prefix_ignore_case(text: &str, prefix: &str) -> bool {
    let mut t = text.chars().flat_map(char::to_lowercase);
    prefix
        .chars()
        .flat_map(char::to_lowercase)
        .all(|p| t.next() == Some(p))
}

/// PHP reserved/magic identifiers use exact prefix matching for offline rows.
/// Preserve server-owned candidates and ordinary fuzzy abbreviations.
pub fn php_magic_candidate(input: &str, text: &str, php: bool, server: bool) -> bool {
    !php || server || !input.starts_with("__") || has_prefix_ignore_case(text, input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nucleo::{
        Config, Matcher, Utf32Str,
        pattern::{CaseMatching, Normalization, Pattern},
    };

    fn rank(tier: u8, score: u32) -> Rank<'static> {
        Rank {
            tier,
            score,
            label_score: score,
            sort_text: "",
            label_len: 5,
        }
    }

    fn fuzzy(input: &str, text: &str) -> Option<u32> {
        let mut m = Matcher::new(Config::DEFAULT);
        let p = Pattern::parse(input, CaseMatching::Ignore, Normalization::Smart);
        let mut buf = Vec::new();
        p.score(Utf32Str::new(text, &mut buf), &mut m)
    }

    #[test]
    fn php_magic_offline_rows_reject_scattered_noise_but_keep_servers() {
        assert!(php_magic_candidate("__PROP", "__PROPERTY__", true, false));
        assert!(php_magic_candidate("__prop", "__PROPERTY__", true, false));
        assert!(!php_magic_candidate("__PROP", "sodium_crypto_box_keypair_from_secretkey_and_publickey", true, false));
        assert!(!php_magic_candidate("__PROP", "__OTHER_PROPERTY__", true, false));
        assert!(php_magic_candidate("__PROP", "server_custom", true, true));
        assert!(php_magic_candidate("rdb", "renderBlade", true, false));
        assert!(php_magic_candidate("__PROP", "scattered", false, false));
        assert!(php_magic_candidate("__", "__CLASS__", true, false));
    }

    #[test]
    fn source_order_beats_match_score() {
        let mut v = vec![
            ("word", rank(3, 900)),
            ("lexer", rank(2, 800)),
            ("snippet", rank(1, 700)),
            ("lsp", rank(0, 10)),
        ];
        v.sort_by(|a, b| compare(&a.1, &b.1));
        let names: Vec<_> = v.iter().map(|x| x.0).collect();
        assert_eq!(names, ["lsp", "snippet", "lexer", "word"]);
    }

    #[test]
    fn inside_a_source_better_match_first_then_sort_text_then_length() {
        let a = rank(0, 50);
        let b = rank(0, 80);
        assert_eq!(compare(&b, &a), Ordering::Less);
        let mut c = rank(0, 50);
        c.sort_text = "a";
        let mut d = rank(0, 50);
        d.sort_text = "b";
        assert_eq!(compare(&c, &d), Ordering::Less);
        let mut e = rank(0, 50);
        e.label_len = 3;
        assert_eq!(compare(&e, &a), Ordering::Less);
        assert_eq!(compare(&a, &a), Ordering::Equal);
    }

    #[test]
    fn dedupe_keeps_highest_source_and_order() {
        // (label, tier)
        let items = vec![
            ("count", 3),
            ("where", 0),
            ("count", 2),
            ("where", 2),
            ("where", 3),
            ("count", 0),
            ("solo", 3),
        ];
        let out = dedupe_across_sources(items, |i| i.0, |i| i.1);
        assert_eq!(out, vec![("where", 0), ("count", 0), ("solo", 3)]);
    }

    #[test]
    fn dedupe_keeps_same_source_duplicates() {
        let items = vec![("get", 0), ("get", 0), ("get", 2)];
        let out = dedupe_across_sources(items, |i| i.0, |i| i.1);
        assert_eq!(out, vec![("get", 0), ("get", 0)]);
    }

    #[test]
    fn dedupe_empty() {
        let out = dedupe_across_sources(Vec::<(&str, u8)>::new(), |i| i.0, |i| i.1);
        assert!(out.is_empty());
    }

    #[test]
    fn fuzzy_subsequence_matches_camel_case_initials() {
        assert!(fuzzy("rdb", "renderBlade").is_some());
        assert!(fuzzy("RDB", "renderBlade").is_some());
        assert!(fuzzy("rdb", "readme").is_none());
        assert!(fuzzy("bld", "renderBlade").is_some());
        assert!(fuzzy("zz", "renderBlade").is_none());
    }

    #[test]
    fn fuzzy_prefers_tight_and_boundary_matches() {
        let boundary = fuzzy("rdb", "renderBlade").unwrap();
        let scattered = fuzzy("rdb", "reallyxdoxbxx").unwrap();
        assert!(boundary > scattered, "{boundary} vs {scattered}");
        let exact = fuzzy("route", "route").unwrap();
        let loose = fuzzy("route", "rxoxuxtxe").unwrap();
        assert!(exact > loose);
        let prefix = fuzzy("get", "getUser").unwrap();
        let inner = fuzzy("get", "forgetUser").unwrap();
        assert!(prefix > inner);
    }

    #[test]
    fn prefix_check_is_case_insensitive_and_unicode_safe() {
        assert!(has_prefix_ignore_case("renderBlade", "REN"));
        assert!(has_prefix_ignore_case("Über", "üb"));
        assert!(has_prefix_ignore_case("abc", ""));
        assert!(!has_prefix_ignore_case("ab", "abc"));
        assert!(!has_prefix_ignore_case("renderBlade", "rdb"));
    }
}

/// One physical project scan per window. Invalidating a request never marks a
/// still-running worker free; only its matching completion token can do that.
#[derive(Clone, Debug, Default)]
pub struct ProjectScanState {
    running: Option<u64>,
    next: u64,
}
impl ProjectScanState {
    pub fn busy(&self) -> bool { self.running.is_some() }
    pub fn begin(&mut self) -> Option<u64> {
        if self.busy() { return None; }
        self.next = self.next.wrapping_add(1);
        self.running = Some(self.next);
        self.running
    }
    pub fn finish(&mut self, token: u64) -> bool {
        if self.running != Some(token) { return false; }
        self.running = None;
        true
    }
}
#[cfg(test)]
mod project_scan_state_tests {
    use super::ProjectScanState;
    #[test]
    fn one_worker_and_stale_token_cannot_release_next_worker() {
        let mut state = ProjectScanState::default();
        let a = state.begin().unwrap();
        assert!(state.begin().is_none()); // editor B must wait for A
        assert!(state.finish(a)); // also used after caught worker panic
        let b = state.begin().unwrap();
        assert!(!state.finish(a));
        assert!(state.busy());
        assert!(state.finish(b));
        assert!(!state.busy());
    }
}

pub(crate) fn project_offset(offset: usize, len: usize) -> usize { offset.min(len) }

#[cfg(test)]
mod project_offset_tests {
    #[test]
    fn shortened_buffer_cannot_reuse_out_of_range_offset() {
        assert_eq!(super::project_offset(300, 2), 2);
        assert_eq!(super::project_offset(0, 0), 0);
        assert_eq!(super::project_offset(1, 20), 1);
    }
}
