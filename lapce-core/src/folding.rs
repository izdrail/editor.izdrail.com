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

    /// The buffer line ranges with no visible line, in order: each folded header's
    /// following lines up to its exclusive end.
    pub fn hidden_ranges(&self) -> Vec<std::ops::Range<usize>> {
        self.folded
            .iter()
            .map(|range| range.start + 1..range.end)
            .collect()
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


fn range_order(range: &FoldRange) -> (usize, std::cmp::Reverse<usize>) {
    (range.start, std::cmp::Reverse(range.end))
}

/// Which blocks of a document are folded. Pure data: no UI and no buffer access.
///
/// `ranges` are the blocks that can be folded, `folded` the blocks that currently are.
/// Folded blocks nested in another folded block stay in `folded` so unfolding the outer
/// block brings the inner fold back; [`FoldProjection`] only exposes the outermost ones.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FoldState {
    ranges: Vec<FoldRange>,
    folded: Vec<FoldRange>,
}

impl FoldState {
    pub fn ranges(&self) -> &[FoldRange] {
        &self.ranges
    }

    pub fn folded(&self) -> &[FoldRange] {
        &self.folded
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty() && self.folded.is_empty()
    }

    /// Replace the foldable blocks with freshly computed ones. A fold stays if a block still
    /// starts on the same line (its end is updated), otherwise it is dropped.
    pub fn set_ranges(&mut self, mut ranges: Vec<FoldRange>) {
        ranges.retain(|range| range.end > range.start + 1);
        ranges.sort_unstable_by_key(range_order);
        ranges.dedup_by_key(|range| range.start);
        let folded = self
            .folded
            .iter()
            .filter_map(|old| ranges.iter().find(|new| new.start == old.start).copied())
            .collect();
        self.ranges = ranges;
        self.folded = folded;
    }

    pub fn is_folded(&self, header: usize) -> bool {
        self.folded.iter().any(|range| range.start == header)
    }

    /// Whether a block can be folded from this header line.
    pub fn is_foldable(&self, header: usize) -> bool {
        self.ranges.iter().any(|range| range.start == header)
    }

    /// Blocks around `line` (header included), innermost first.
    fn around(&self, line: usize) -> Vec<FoldRange> {
        let mut around: Vec<FoldRange> = self
            .ranges
            .iter()
            .filter(|range| range.start <= line && line < range.end)
            .copied()
            .collect();
        around.sort_unstable_by_key(|range| std::cmp::Reverse(range.start));
        around
    }

    /// Fold the innermost block around `line` that is not folded yet, so repeating the
    /// command folds outwards. Returns the folded block.
    pub fn fold(&mut self, line: usize) -> Option<FoldRange> {
        let range = self
            .around(line)
            .into_iter()
            .find(|range| !self.is_folded(range.start))?;
        self.folded.push(range);
        self.folded.sort_unstable_by_key(range_order);
        Some(range)
    }

    /// Unfold the innermost folded block around `line` (a folded header counts).
    pub fn unfold(&mut self, line: usize) -> bool {
        let Some(range) = self
            .folded
            .iter()
            .filter(|range| range.start <= line && line < range.end)
            .max_by_key(|range| range.start)
            .copied()
        else {
            return false;
        };
        self.folded.retain(|folded| *folded != range);
        true
    }

    /// Unfold if a block folded at `line` exists, otherwise fold the block around it.
    pub fn toggle(&mut self, line: usize) -> bool {
        if self.is_folded(line) {
            self.unfold(line)
        } else {
            self.fold(line).is_some()
        }
    }

    pub fn fold_all(&mut self) -> bool {
        let changed = self.folded != self.ranges;
        self.folded = self.ranges.clone();
        changed
    }

    pub fn unfold_all(&mut self) -> bool {
        let changed = !self.folded.is_empty();
        self.folded.clear();
        changed
    }

    /// Make `line` visible by unfolding every folded block that hides it. Search and
    /// go to definition use this when they land in folded text.
    pub fn reveal(&mut self, line: usize) -> bool {
        let before = self.folded.len();
        self.folded.retain(|range| !range.contains(line));
        self.folded.len() != before
    }

    pub fn is_hidden(&self, line: usize) -> bool {
        self.folded.iter().any(|range| range.contains(line))
    }

    /// An edit replaced `old_lines` lines starting at `start_line` with `new_lines` lines.
    /// Blocks before it stay, blocks after it move by the change in line count and blocks
    /// touching it (header, body or the replaced lines) are dropped, which unfolds them.
    /// The next [`Self::set_ranges`] brings the foldable blocks up to date.
    pub fn apply_edit(&mut self, start_line: usize, old_lines: usize, new_lines: usize) {
        let old_end = start_line + old_lines;
        let delta = new_lines as isize - old_lines as isize;
        let adjust = |range: &FoldRange| {
            if range.end <= start_line {
                Some(*range)
            } else if range.start >= old_end {
                Some(FoldRange {
                    start: range.start.checked_add_signed(delta)?,
                    end: range.end.checked_add_signed(delta)?,
                })
            } else {
                None
            }
        };
        self.ranges = self.ranges.iter().filter_map(adjust).collect();
        self.folded = self.folded.iter().filter_map(adjust).collect();
    }

    pub fn projection(&self) -> FoldProjection {
        FoldProjection::new(self.folded.clone())
    }

    /// The buffer line ranges to hide.
    pub fn hidden_ranges(&self) -> Vec<std::ops::Range<usize>> {
        self.projection().hidden_ranges()
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

    fn r(start: usize, end: usize) -> FoldRange {
        FoldRange { start, end }
    }

    fn state() -> FoldState {
        // 0: outer { 1: a { 2,3 } 4: b { 5,6 } } 8: c { 9 }
        let mut state = FoldState::default();
        state.set_ranges(vec![r(8, 10), r(4, 7), r(1, 4), r(0, 8)]);
        state
    }

    #[test]
    fn fold_innermost_first_then_outwards() {
        let mut s = state();
        assert_eq!(s.fold(2), Some(r(1, 4)));
        assert_eq!(s.fold(2), Some(r(0, 8)));
        assert_eq!(s.fold(2), None);
        assert_eq!(s.hidden_ranges(), vec![1..8]);
        // Header line of a block folds that block.
        let mut s = state();
        assert_eq!(s.fold(4), Some(r(4, 7)));
        assert_eq!(s.fold(11), None);
    }

    #[test]
    fn nested_fold_state_survives_outer_toggle() {
        let mut s = state();
        s.fold(2);
        s.fold(5);
        assert_eq!(s.hidden_ranges(), vec![2..4, 5..7]);
        s.toggle(0);
        assert_eq!(s.hidden_ranges(), vec![1..8]);
        assert!(s.is_folded(1) && s.is_folded(4));
        // Unfolding the outer block restores the inner folds.
        assert!(s.toggle(0));
        assert_eq!(s.hidden_ranges(), vec![2..4, 5..7]);
    }

    #[test]
    fn unfold_picks_innermost_folded_block() {
        let mut s = state();
        s.fold_all();
        assert_eq!(s.hidden_ranges(), vec![1..8, 9..10]);
        assert!(s.unfold(3));
        assert!(!s.is_folded(1) && s.is_folded(0));
        assert!(s.unfold(0));
        assert!(!s.unfold(0));
        assert!(s.unfold_all());
        assert!(s.hidden_ranges().is_empty());
        assert!(!s.unfold_all());
    }

    #[test]
    fn reveal_unfolds_every_block_hiding_the_line() {
        let mut s = state();
        s.fold_all();
        assert!(s.is_hidden(5));
        assert!(s.reveal(5));
        // Outer and the block containing line 5 are open, siblings stay folded.
        assert!(!s.is_hidden(5));
        assert!(s.is_folded(1) && s.is_folded(8) && !s.is_folded(0) && !s.is_folded(4));
        // Revealing a header or visible line changes nothing.
        assert!(!s.reveal(1));
        assert!(!s.reveal(0));
    }

    #[test]
    fn edits_shift_drop_and_keep_folds() {
        let mut s = state();
        s.fold(2);
        s.fold(9);
        // Insert two lines inside the unfolded block `4..7`: later folds move, earlier stay.
        s.apply_edit(5, 1, 3);
        assert!(s.is_folded(1) && s.is_folded(10) && !s.is_folded(9));
        assert_eq!(s.hidden_ranges(), vec![2..4, 11..12]);
        // Typing on the header of a fold unfolds it.
        s.apply_edit(1, 1, 1);
        assert!(!s.is_folded(1) && s.is_folded(10));
        // Deleting lines before moves it up.
        s.apply_edit(0, 1, 0);
        assert!(s.is_folded(9));
        // Edits ending exactly where a block starts do not touch it.
        let mut s = state();
        s.fold(8);
        s.apply_edit(7, 1, 1);
        assert!(s.is_folded(8));
        // Edit before a block that underflows is dropped, not wrapped.
        s.apply_edit(0, 20, 0);
        assert!(s.folded().is_empty() && s.ranges().is_empty());
    }

    #[test]
    fn set_ranges_keeps_matching_folds_only() {
        let mut s = state();
        s.fold(2);
        s.fold(9);
        // Block 1 grew, block 8 vanished.
        s.set_ranges(vec![r(0, 9), r(1, 6), r(4, 7)]);
        assert_eq!(s.folded(), &[r(1, 6)]);
        // Ranges too small to hide anything and duplicate headers are ignored.
        s.set_ranges(vec![r(3, 4), r(3, 3), r(5, 9), r(5, 7), r(1, 6)]);
        assert_eq!(s.ranges(), &[r(1, 6), r(5, 9)]);
        assert_eq!(s.folded(), &[r(1, 6)]);
    }
}
