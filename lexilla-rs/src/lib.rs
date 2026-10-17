//! Safe UTF-8 snapshot interface to the statically linked Lexilla catalogue.
//! The native session is thread-local to each call and never escapes into Rust.
use std::{
    ffi::{CStr, CString, c_char, c_void},
    ops::Range,
};

unsafe extern "C" {
    fn lx_new(name: *const c_char, text: *const c_char, n: usize) -> *mut c_void;
    fn lx_free(session: *mut c_void);
    fn lx_property(
        session: *mut c_void,
        key: *const c_char,
        value: *const c_char,
    ) -> i32;
    fn lx_words(session: *mut c_void, index: i32, words: *const c_char) -> i32;
    fn lx_lex(
        session: *mut c_void,
        start: usize,
        n: usize,
        init: i32,
        fold: i32,
    ) -> i32;
    fn lx_styles(session: *mut c_void) -> *const u8;
    fn lx_named(session: *mut c_void) -> i32;
    fn lx_tags(session: *mut c_void, index: i32) -> *const c_char;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    InvalidName,
    UnavailableLexer,
    Keywords,
    Property,
    Lexing(i32),
}

/// Supported, typed lexical settings. Unknown property names cannot cross the ABI.
#[derive(Debug, Clone, Copy)]
pub enum Property {
    Scss(bool),
    Javascript(bool),
}
impl Property {
    fn pairs(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Scss(true) => &[("lexer.css.scss.language", "1")],
            Self::Scss(false) => &[("lexer.css.scss.language", "0")],
            Self::Javascript(true) => &[
                ("lexer.cpp.allow.dollars", "1"),
                ("lexer.cpp.enable.preprocessor", "0"),
                ("lexer.cpp.backquoted.strings", "1"),
            ],
            Self::Javascript(false) => &[
                ("lexer.cpp.allow.dollars", "0"),
                ("lexer.cpp.enable.preprocessor", "1"),
                ("lexer.cpp.backquoted.strings", "0"),
            ],
        }
    }
}

struct Session(*mut c_void);
impl Drop for Session {
    fn drop(&mut self) {
        unsafe { lx_free(self.0) };
    }
}

/// Immutable byte styles for one complete document revision.
#[derive(Debug, Clone)]
pub struct Snapshot {
    styles: Vec<u8>,
    scopes: Vec<Option<&'static str>>,
}
impl Snapshot {
    pub fn lex(name: &str, text: &str, keywords: &[&str]) -> Result<Self, Error> {
        Self::lex_with_properties(name, text, keywords, &[])
    }

    pub fn lex_with_properties(
        name: &str,
        text: &str,
        keywords: &[&str],
        properties: &[Property],
    ) -> Result<Self, Error> {
        // Reject settings for the wrong language, even if a legacy lexer accepts
        // arbitrary property names without reporting an error.
        if properties.iter().any(|p| {
            !matches!(
                (name, p),
                ("css", Property::Scss(_)) | ("cpp", Property::Javascript(_))
            )
        }) {
            return Err(Error::Property);
        }
        let name = CString::new(name).map_err(|_| Error::InvalidName)?;
        // SAFETY: both buffers remain valid through the call. C++ copies text.
        let raw = unsafe { lx_new(name.as_ptr(), text.as_ptr().cast(), text.len()) };
        if raw.is_null() {
            return Err(Error::UnavailableLexer);
        }
        let session = Session(raw);
        for property in properties {
            for (key, value) in property.pairs() {
                let key = CString::new(*key).map_err(|_| Error::Property)?;
                let value = CString::new(*value).map_err(|_| Error::Property)?;
                // -1 also means unchanged; validated keys above are authoritative.
                if unsafe { lx_property(session.0, key.as_ptr(), value.as_ptr()) }
                    < -1
                {
                    return Err(Error::Property);
                }
            }
        }
        for (index, words) in keywords.iter().enumerate() {
            let words = CString::new(*words).map_err(|_| Error::Keywords)?;
            if unsafe { lx_words(session.0, index as i32, words.as_ptr()) } < -1 {
                return Err(Error::Keywords);
            }
        }
        let status = unsafe { lx_lex(session.0, 0, text.len(), 0, 0) };
        if status != 0 {
            return Err(Error::Lexing(status));
        }
        let styles = if text.is_empty() {
            Vec::new()
        } else {
            // SAFETY: the session owns exactly text.len() initialized style bytes;
            // copy them before RAII releases it. No pointer crosses threads.
            unsafe {
                std::slice::from_raw_parts(lx_styles(session.0), text.len()).to_vec()
            }
        };
        let count = unsafe { lx_named(session.0) }.clamp(0, 256) as usize;
        let mut scopes = vec![None; 256];
        for (index, scope) in scopes.iter_mut().enumerate().take(count) {
            let tags = unsafe { lx_tags(session.0, index as i32) };
            if !tags.is_null() {
                *scope = scope_from_tags(
                    &unsafe { CStr::from_ptr(tags) }.to_string_lossy(),
                );
            }
        }
        Ok(Self { styles, scopes })
    }

    /// Overlay a byte-aligned region, translating style IDs by theme scope.
    /// The caller chooses the region; lexer-local style IDs never leak across
    /// language boundaries. Unmapped identifiers remain uncoloured.
    pub fn overlay(&mut self, offset: usize, other: &Self, skip_default: bool) {
        for (i, &style) in other.styles.iter().enumerate() {
            if offset + i >= self.len() {
                break;
            }
            if skip_default && style == 0 {
                continue;
            }
            let scope = other.scope(style);
            let target = self
                .scopes
                .iter()
                .position(|s| *s == scope)
                .unwrap_or_else(|| {
                    // At most the small fixed theme-scope set is used by the adapter.
                    let index =
                        (240..256).find(|&i| self.scopes[i].is_none()).unwrap_or(0);
                    self.scopes[index] = scope;
                    index
                });
            self.styles[offset + i] = target as u8;
        }
    }

    pub fn scope(&self, style: u8) -> Option<&'static str> {
        self.scopes[style as usize]
    }
    pub fn len(&self) -> usize {
        self.styles.len()
    }
    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
    }
    pub fn styles(&self) -> &[u8] {
        &self.styles
    }

    /// Extract only the requested viewport/line range, never millions of full-file runs.
    /// Input must be the text range of this snapshot, at UTF-8 boundaries.
    pub fn runs(
        &self,
        range: Range<usize>,
    ) -> impl Iterator<Item = (Range<usize>, u8)> + '_ {
        let mut start = range.start.min(self.len());
        let end = range.end.min(self.len());
        std::iter::from_fn(move || {
            if start >= end {
                return None;
            }
            let style = self.styles[start];
            let mut next = start + 1;
            while next < end && self.styles[next] == style {
                next += 1;
            }
            let run = start..next;
            start = next;
            Some((run, style))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_and_unknown_are_safe() {
        assert!(Snapshot::lex("cpp", "", &[]).unwrap().is_empty());
        assert!(matches!(
            Snapshot::lex("not-a-lexer", "x", &[]),
            Err(Error::UnavailableLexer)
        ));
        assert!(matches!(
            Snapshot::lex("bad\0name", "x", &[]),
            Err(Error::InvalidName)
        ));
    }
    #[test]
    fn viewport_styles_equal_full_snapshot() {
        for (lexer, text, keywords) in [
            (
                "cpp",
                "/* across\nlines */\r\nint café = 42; // 🙂\n",
                &["int return"][..],
            ),
            (
                "python",
                "def greet():\n    s = \"λ🙂\"\n    return s\n",
                &["def return"][..],
            ),
            (
                "hypertext",
                "<script>const a = 1;</script>\n<?php echo 'λ'; ?>",
                &["script", "const", "", "", "echo"][..],
            ),
        ] {
            let snapshot = Snapshot::lex(lexer, text, keywords).unwrap();
            assert_eq!(snapshot.len(), text.len());
            let mut start = 0;
            for line in text.split_inclusive('\n') {
                let end = start + line.len();
                let mut reconstructed = Vec::new();
                for (range, style) in snapshot.runs(start..end) {
                    assert!(
                        text.is_char_boundary(range.start)
                            && text.is_char_boundary(range.end)
                    );
                    reconstructed.extend(std::iter::repeat_n(style, range.len()));
                }
                assert_eq!(reconstructed, snapshot.styles()[start..end]);
                start = end;
            }
            assert_eq!(
                Snapshot::lex(lexer, text, keywords).unwrap().styles(),
                snapshot.styles()
            );
        }
    }
    #[test]
    fn edit_and_undo_use_fresh_line_states() {
        let before = "/* open\nint hidden;\n";
        let after = "/* closed */\nint visible;\n";
        let first = Snapshot::lex("cpp", before, &["int"]).unwrap();
        let edited = Snapshot::lex("cpp", after, &["int"]).unwrap();
        assert_ne!(
            first.styles()[before.find("int").unwrap()],
            edited.styles()[after.find("int").unwrap()]
        );
        assert_eq!(
            Snapshot::lex("cpp", before, &["int"]).unwrap().styles(),
            first.styles()
        );
    }
}

// Lexilla metadata is per-lexer, including embedded language styles. Unknown
// and default/identifier styles intentionally keep the editor foreground.
fn scope_from_tags(tags: &str) -> Option<&'static str> {
    let tags: Vec<_> = tags.split_whitespace().collect();
    if tags.contains(&"comment") {
        Some("comment")
    } else if tags.contains(&"keyword") || tags.contains(&"preprocessor") {
        Some("keyword")
    } else if tags.contains(&"string") || tags.contains(&"regex") {
        Some("string")
    } else if tags.contains(&"numeric")
        || tags.contains(&"number")
        || tags.contains(&"literal")
    {
        Some("constant")
    } else if tags.contains(&"operator") {
        Some("operator")
    } else if tags.contains(&"function") {
        Some("function")
    } else if tags.contains(&"property") {
        Some("property")
    } else if tags.contains(&"variable") {
        Some("variable")
    } else if tags.contains(&"tag") {
        Some("tag")
    } else if tags.contains(&"type") {
        Some("type")
    } else {
        None
    }
}

#[cfg(test)]
mod concurrency_tests {
    #[test]
    fn concurrent_first_catalogue_access_is_safe() {
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(16));
        let workers: Vec<_> = (0..16)
            .map(|_| {
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    for _ in 0..20 {
                        assert!(
                            super::Snapshot::lex("cpp", "int x = 1;", &["int"])
                                .is_ok()
                        );
                    }
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
    }
}

#[cfg(test)]
mod web_regressions {
    use super::*;
    #[test]
    fn properties_are_typed_language_checked_and_session_local() {
        let t = "$accent: red; // sass\nbody { color: $accent; }";
        let css = Snapshot::lex("css", t, &[]).unwrap();
        let sass =
            Snapshot::lex_with_properties("css", t, &[], &[Property::Scss(true)])
                .unwrap();
        assert_ne!(css.styles(), sass.styles());
        assert_eq!(css.styles(), Snapshot::lex("css", t, &[]).unwrap().styles());
        assert_eq!(
            Snapshot::lex_with_properties("php", "", &[], &[Property::Scss(true)])
                .unwrap_err(),
            Error::Property
        );
        let t = "const $name = `hello ${world}`;";
        let js = Snapshot::lex_with_properties(
            "cpp",
            t,
            &["const"],
            &[Property::Javascript(true)],
        )
        .unwrap();
        assert_eq!(
            js.scope(js.styles()[t.find("hello").unwrap()]),
            Some("string")
        );
    }
    #[test]
    fn php_scanners_share_comment_and_function_fixes() {
        for lexer in ["php", "laravel"] {
            let t = "<?php // hi ?> <p>λ</p>\n<?php /* a /* b */ strlen('x');";
            let s = Snapshot::lex(lexer, t, &["", "strlen"]).unwrap();
            let scope = |word: &str| s.scope(s.styles()[t.find(word).unwrap()]);
            assert_eq!(scope("// hi"), Some("comment"));
            assert_eq!(scope("?>"), Some("operator"));
            assert_ne!(scope("<p>"), Some("comment"));
            assert_eq!(scope("strlen"), Some("function"));
            for text in [
                "<?php 'x\\",
                "<?php \"x\\",
                "<?php /*",
                "<?php // ?>",
                "<?php <<<'X'\nλ",
            ] {
                assert_eq!(
                    Snapshot::lex(lexer, text, &[]).unwrap().len(),
                    text.len()
                );
            }
        }
    }
}
