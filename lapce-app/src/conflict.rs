//! Merge-conflict hunk surgery on plain text: find the two-way
//! `<<<<<<< / ======= / >>>>>>>` block enclosing a line and produce
//! the replacement for the chosen side. Pure functions; the editor
//! wiring lives in window_tab.rs.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConflictSide {
    /// The lines between `<<<<<<<` and `=======` (HEAD / ours).
    Current,
    /// The lines between `=======` and `>>>>>>>` (theirs).
    Incoming,
    /// Current lines followed by incoming lines.
    Both,
}

/// Resolve the conflict hunk enclosing `line` (0-based). Returns
/// `(start_line, end_line_inclusive, replacement)` where replacement
/// ends with a newline when non-empty, or None when the line is not
/// inside a complete block. diff3-style blocks (with `|||||||`
/// base sections) and unterminated blocks are left alone.
pub fn resolve_conflict_at_line(
    text: &str,
    line: usize,
    side: ConflictSide,
) -> Option<(usize, usize, String)> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut i = 0;
    while i < lines.len() {
        if !lines[i].starts_with("<<<<<<<") {
            i += 1;
            continue;
        }
        let start = i;
        let mut mid = None;
        let mut end = None;
        let mut diff3 = false;
        let mut j = i + 1;
        while j < lines.len() {
            let l = lines[j];
            if l.starts_with("|||||||") {
                diff3 = true;
            } else if l.starts_with("=======") && mid.is_none() {
                mid = Some(j);
            } else if l.starts_with(">>>>>>>") {
                end = Some(j);
                break;
            } else if l.starts_with("<<<<<<<") {
                break; // unterminated outer block; resync at inner
            }
            j += 1;
        }
        match (mid, end) {
            (Some(mid), Some(end)) => {
                if line >= start && line <= end {
                    if diff3 {
                        return None;
                    }
                    let current = &lines[start + 1..mid];
                    let incoming = &lines[mid + 1..end];
                    let mut body: Vec<&str> = Vec::new();
                    match side {
                        ConflictSide::Current => {
                            body.extend_from_slice(current);
                        }
                        ConflictSide::Incoming => {
                            body.extend_from_slice(incoming);
                        }
                        ConflictSide::Both => {
                            body.extend_from_slice(current);
                            body.extend_from_slice(incoming);
                        }
                    }
                    let mut replacement = body.join("\n");
                    if !replacement.is_empty() {
                        replacement.push('\n');
                    }
                    return Some((start, end, replacement));
                }
                i = end + 1;
            }
            _ => i += 1,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "before\n<<<<<<< HEAD\nours one\nours two\n=======\ntheirs\n>>>>>>> feature\nafter\n";

    #[test]
    fn resolves_each_side() {
        // lines: 0 before, 1 <<<, 2-3 ours, 4 ===, 5 theirs, 6 >>>, 7 after
        let (s, e, r) =
            resolve_conflict_at_line(SAMPLE, 3, ConflictSide::Current).unwrap();
        assert_eq!((s, e), (1, 6));
        assert_eq!(r, "ours one\nours two\n");
        let (_, _, r) =
            resolve_conflict_at_line(SAMPLE, 5, ConflictSide::Incoming).unwrap();
        assert_eq!(r, "theirs\n");
        let (_, _, r) =
            resolve_conflict_at_line(SAMPLE, 4, ConflictSide::Both).unwrap();
        assert_eq!(r, "ours one\nours two\ntheirs\n");
        // Marker lines themselves count as inside the block.
        assert!(resolve_conflict_at_line(SAMPLE, 1, ConflictSide::Current)
            .is_some());
        assert!(resolve_conflict_at_line(SAMPLE, 6, ConflictSide::Current)
            .is_some());
    }

    #[test]
    fn ignores_non_conflict_lines_and_picks_enclosing_block() {
        assert_eq!(resolve_conflict_at_line(SAMPLE, 0, ConflictSide::Both), None);
        assert_eq!(resolve_conflict_at_line(SAMPLE, 7, ConflictSide::Both), None);
        let two = format!("{SAMPLE}middle\n<<<<<<< A\nx\n=======\ny\n>>>>>>> B\n");
        // Second block: lines 9 <<<, 10 x, 11 ===, 12 y, 13 >>>
        let (s, e, r) =
            resolve_conflict_at_line(&two, 10, ConflictSide::Incoming).unwrap();
        assert_eq!((s, e), (9, 13));
        assert_eq!(r, "y\n");
    }

    #[test]
    fn leaves_diff3_and_unterminated_blocks_alone() {
        let diff3 = "<<<<<<< HEAD\nours\n||||||| base\nbase\n=======\ntheirs\n>>>>>>> f\n";
        assert_eq!(resolve_conflict_at_line(diff3, 1, ConflictSide::Current), None);
        let unterminated = "<<<<<<< HEAD\nours\n=======\ntheirs\n";
        assert_eq!(
            resolve_conflict_at_line(unterminated, 2, ConflictSide::Current),
            None
        );
        // An empty side resolves to deleting the whole block.
        let empty_side = "<<<<<<< HEAD\n=======\ntheirs\n>>>>>>> f\n";
        let (_, _, r) =
            resolve_conflict_at_line(empty_side, 0, ConflictSide::Current)
                .unwrap();
        assert_eq!(r, "");
    }
}
