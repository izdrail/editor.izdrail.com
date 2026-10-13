//! Local, line-based folding. No language server or executable plugin is used.
//! The Lexilla highlighting path does not build tree-sitter trees, so indentation
//! provides a language-independent fallback. Rendering must use the same mapping
//! for paint, hit testing, movement and scrolling before exposing folding UI.

/// The header and an exclusive end boundary. Lines strictly between are hidden.
/// `end` may equal the number of lines when a block reaches EOF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FoldRange {
    pub start: usize,
    pub end: usize,
}

impl FoldRange {
    pub fn contains(self, line: usize) -> bool {
        self.start < line && line < self.end
    }
}

/// Find indentation blocks in one pass, including nested blocks. Tabs advance
/// to tab stops. Blank lines do not open blocks or cause dedents; trailing blank
/// lines are excluded. The closing/dedented line remains visible.
pub fn indentation_ranges(text: &str, tab_width: usize) -> Vec<FoldRange> {
    let tab_width = tab_width.max(1);
    let mut ranges = Vec::new();
    let mut stack: Vec<(usize, usize)> = Vec::new();
    let mut previous: Option<(usize, usize)> = None;
    let mut last_nonblank = None;
    for (line, content) in text.lines().enumerate() {
        if content.trim().is_empty() {
            continue;
        }
        let indent = content
            .chars()
            .take_while(|ch| matches!(ch, ' ' | '\t'))
            .fold(0, |column, ch| {
                if ch == '\t' {
                    column + tab_width - column % tab_width
                } else {
                    column + 1
                }
            });
        while stack.last().is_some_and(|&(_, level)| indent <= level) {
            let (start, _) = stack.pop().unwrap();
            ranges.push(FoldRange {
                start,
                end: last_nonblank.unwrap() + 1,
            });
        }
        if let Some((start, level)) = previous {
            if indent > level {
                stack.push((start, level));
            }
        }
        previous = Some((line, indent));
        last_nonblank = Some(line);
    }
    if let Some(last) = last_nonblank {
        ranges.extend(stack.into_iter().map(|(start, _)| FoldRange {
            start,
            end: last + 1,
        }));
    }
    // Outer ranges precede nested ones, with at most one range per header.
    ranges.sort_unstable_by_key(|range| (range.start, std::cmp::Reverse(range.end)));
    ranges
}

/// A projection shared by rendering, pointer and keyboard navigation. This
/// model is deliberately separate from UI so mapping cannot become paint-only.
#[derive(Debug, Clone, Default)]
pub struct FoldProjection {
    folded: Vec<FoldRange>,
}

impl FoldProjection {
    pub fn new(mut folded: Vec<FoldRange>) -> Self {
        folded.retain(|range| range.end > range.start.saturating_add(1));
        folded.sort_unstable_by_key(|range| {
            (range.start, std::cmp::Reverse(range.end))
        });
        // Keep only visible outer ranges. Nested ranges are held by the caller's
        // fold state so unfolding an outer block can restore its inner folds.
        let mut visible: Vec<FoldRange> = Vec::new();
        for range in folded {
            if visible.last().is_some_and(|outer| range.start < outer.end) {
                continue;
            }
            visible.push(range);
        }
        Self { folded: visible }
    }

    pub fn is_hidden(&self, line: usize) -> bool {
        self.folded.iter().any(|range| range.contains(line))
    }

    /// Hidden buffer lines map to their header. No buffer offsets are changed.
    pub fn visible_line(&self, line: usize) -> usize {
        let mut hidden = 0;
        for range in &self.folded {
            if line <= range.start {
                break;
            }
            if line < range.end {
                return range.start - hidden;
            }
            hidden += range.end - range.start - 1;
        }
        line - hidden
    }

    pub fn buffer_line(&self, visible: usize) -> usize {
        let mut line = visible;
        for range in &self.folded {
            if line <= range.start {
                break;
            }
            line += range.end - range.start - 1;
        }
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_php_and_dedented_closing_braces() {
        let ranges = indentation_ranges(
            "class A {\n    function f() {\n        run();\n    }\n}\n",
            4,
        );
        assert_eq!(
            ranges,
            vec![
                FoldRange { start: 0, end: 4 },
                FoldRange { start: 1, end: 3 }
            ]
        );
    }
    #[test]
    fn blank_lines_and_eof() {
        assert_eq!(
            indentation_ranges("def f():\n\n    x\n    y\n\n", 4),
            vec![FoldRange { start: 0, end: 4 }]
        );
        assert!(indentation_ranges("\n \t\r\n", 4).is_empty());
        assert!(indentation_ranges("no indent\nnext\n", 4).is_empty());
    }
    #[test]
    fn tabs_use_stops_crlf_and_unicode_are_safe() {
        assert_eq!(
            indentation_ranges("header\r\n \tx = 'λ🙂'\r\n    y\r\nnext\r\n", 4),
            vec![FoldRange { start: 0, end: 3 }]
        );
        assert_eq!(
            indentation_ranges("x\n\ty\n", 0),
            vec![FoldRange { start: 0, end: 2 }]
        );
    }
    #[test]
    fn sibling_blocks_do_not_hide_each_other() {
        assert_eq!(
            indentation_ranges("a\n  b\nc\n  d", 4),
            vec![
                FoldRange { start: 0, end: 2 },
                FoldRange { start: 2, end: 4 }
            ]
        );
    }
    #[test]
    fn nested_and_adjacent_projection_round_trips() {
        let projection = FoldProjection::new(vec![
            FoldRange { start: 2, end: 4 },
            FoldRange { start: 0, end: 5 },
            FoldRange { start: 5, end: 8 },
        ]);
        let visible = [0, 5, 8, 9, 10];
        for (index, line) in visible.into_iter().enumerate() {
            assert_eq!(projection.buffer_line(index), line);
            assert_eq!(projection.visible_line(line), index);
            assert!(!projection.is_hidden(line));
        }
        assert_eq!(projection.visible_line(3), 0);
        assert_eq!(projection.visible_line(7), 1);
        assert!(projection.is_hidden(4));
        assert!(!projection.is_hidden(5));
    }
    #[test]
    fn invalid_empty_and_duplicate_ranges_are_ignored() {
        let projection = FoldProjection::new(vec![
            FoldRange { start: 0, end: 0 },
            FoldRange { start: 2, end: 3 },
            FoldRange { start: 4, end: 7 },
            FoldRange { start: 4, end: 7 },
        ]);
        assert_eq!(projection.buffer_line(5), 7);
        assert_eq!(projection.visible_line(7), 5);
    }
}
