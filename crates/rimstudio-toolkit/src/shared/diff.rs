//! A small unified line diff, written in house so the toolkit needs no diff crate.
//!
//! [`unified_diff`] compares two texts line by line (a longest common subsequence over the lines, with a
//! trimmed common prefix and suffix) and prints the usual unified format with three lines of context. The
//! output is deterministic. Very large inputs fall back to one hunk that replaces everything, so the cost
//! stays bounded.

/// Lines of context around each change.
const CONTEXT: usize = 3;

/// The largest product of line counts the quadratic table is built for.
const MAX_CELLS: usize = 4_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Keep,
    Del,
    Add,
}

fn lines_of(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

fn same_line(a: &str, b: &str) -> bool {
    a.trim_end_matches(['\r', '\n']) == b.trim_end_matches(['\r', '\n'])
}

/// The edit script between two line lists: one entry per output line.
fn script<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<(Op, &'a str)> {
    let mut prefix = 0usize;
    while prefix < old.len()
        && prefix < new.len()
        && old
            .get(prefix)
            .zip(new.get(prefix))
            .is_some_and(|(a, b)| same_line(a, b))
    {
        prefix += 1;
    }
    let mut suffix = 0usize;
    while suffix < old.len().saturating_sub(prefix)
        && suffix < new.len().saturating_sub(prefix)
        && old
            .get(old.len() - 1 - suffix)
            .zip(new.get(new.len() - 1 - suffix))
            .is_some_and(|(a, b)| same_line(a, b))
    {
        suffix += 1;
    }
    let old_mid = old.get(prefix..old.len() - suffix).unwrap_or(&[]);
    let new_mid = new.get(prefix..new.len() - suffix).unwrap_or(&[]);

    let mut out: Vec<(Op, &str)> = old.iter().take(prefix).map(|l| (Op::Keep, *l)).collect();
    out.extend(middle(old_mid, new_mid));
    out.extend(old.iter().skip(old.len() - suffix).map(|l| (Op::Keep, *l)));
    out
}

fn middle<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<(Op, &'a str)> {
    let (n, m) = (old.len(), new.len());
    if n == 0 || m == 0 || n.saturating_mul(m) > MAX_CELLS {
        let mut out: Vec<(Op, &str)> = old.iter().map(|l| (Op::Del, *l)).collect();
        out.extend(new.iter().map(|l| (Op::Add, *l)));
        return out;
    }
    let width = m + 1;
    // lcs[i * width + j] is the length of the common subsequence of old[i..] and new[j..]
    let mut lcs = vec![0u32; (n + 1) * width];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let same = old
                .get(i)
                .zip(new.get(j))
                .is_some_and(|(a, b)| same_line(a, b));
            let value = if same {
                lcs.get((i + 1) * width + j + 1).copied().unwrap_or(0) + 1
            } else {
                let down = lcs.get((i + 1) * width + j).copied().unwrap_or(0);
                let right = lcs.get(i * width + j + 1).copied().unwrap_or(0);
                down.max(right)
            };
            if let Some(cell) = lcs.get_mut(i * width + j) {
                *cell = value;
            }
        }
    }
    let mut out = Vec::with_capacity(n + m);
    let (mut i, mut j) = (0usize, 0usize);
    while i < n || j < m {
        let a = old.get(i);
        let b = new.get(j);
        match (a, b) {
            (Some(x), Some(y)) if same_line(x, y) => {
                out.push((Op::Keep, *x));
                i += 1;
                j += 1;
            }
            (Some(x), Some(_)) => {
                let down = lcs.get((i + 1) * width + j).copied().unwrap_or(0);
                let right = lcs.get(i * width + j + 1).copied().unwrap_or(0);
                if down >= right {
                    out.push((Op::Del, *x));
                    i += 1;
                } else if let Some(y) = b {
                    out.push((Op::Add, *y));
                    j += 1;
                }
            }
            (Some(x), None) => {
                out.push((Op::Del, *x));
                i += 1;
            }
            (None, Some(y)) => {
                out.push((Op::Add, *y));
                j += 1;
            }
            (None, None) => break,
        }
    }
    out
}

fn push_line(out: &mut String, sign: char, line: &str) {
    out.push(sign);
    out.push_str(line.trim_end_matches(['\r', '\n']));
    out.push('\n');
    if !line.ends_with('\n') {
        out.push_str("\\ No newline at end of file\n");
    }
}

/// The unified diff of `old` against `new` for a file called `path`. Empty when the texts hold the same
/// lines. Line endings are compared without their terminator, so a change of line ending alone is not shown.
#[must_use]
pub fn unified_diff(path: &str, old: &str, new: &str) -> String {
    let old_lines = lines_of(old);
    let new_lines = lines_of(new);
    let ops = script(&old_lines, &new_lines);
    if ops.iter().all(|(op, _)| *op == Op::Keep) {
        return String::new();
    }
    // position (1 based) of each script entry in the old and the new text
    let mut positions = Vec::with_capacity(ops.len());
    let (mut o, mut n) = (1usize, 1usize);
    for (op, _) in &ops {
        positions.push((o, n));
        match op {
            Op::Keep => {
                o += 1;
                n += 1;
            }
            Op::Del => o += 1,
            Op::Add => n += 1,
        }
    }
    let mut out = format!("--- a/{path}\n+++ b/{path}\n");
    let mut idx = 0usize;
    while idx < ops.len() {
        // find the next change
        let Some(first) = ops
            .iter()
            .enumerate()
            .skip(idx)
            .find(|(_, (op, _))| *op != Op::Keep)
            .map(|(i, _)| i)
        else {
            break;
        };
        let start = first.saturating_sub(CONTEXT).max(idx);
        // extend the hunk while the next change is within twice the context
        let mut end = first;
        let mut last_change = first;
        let mut k = first;
        while k < ops.len() {
            if ops.get(k).is_some_and(|(op, _)| *op != Op::Keep) {
                last_change = k;
                end = k;
            } else if k - last_change > 2 * CONTEXT {
                break;
            }
            k += 1;
        }
        let stop = (end + 1 + CONTEXT).min(ops.len());
        let (old_start, new_start) = positions.get(start).copied().unwrap_or((1, 1));
        let slice = ops.get(start..stop).unwrap_or(&[]);
        let old_count = slice.iter().filter(|(op, _)| *op != Op::Add).count();
        let new_count = slice.iter().filter(|(op, _)| *op != Op::Del).count();
        let old_start = if old_count == 0 {
            old_start - 1
        } else {
            old_start
        };
        let new_start = if new_count == 0 {
            new_start - 1
        } else {
            new_start
        };
        out.push_str(&format!(
            "@@ -{old_start},{old_count} +{new_start},{new_count} @@\n"
        ));
        for (op, line) in slice {
            let sign = match op {
                Op::Keep => ' ',
                Op::Del => '-',
                Op::Add => '+',
            };
            push_line(&mut out, sign, line);
        }
        idx = stop;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_texts_have_no_diff() {
        assert_eq!(unified_diff("a.txt", "a\nb\n", "a\nb\n"), "");
        assert_eq!(unified_diff("a.txt", "a\r\nb\r\n", "a\nb\n"), "");
    }

    #[test]
    fn a_changed_line_shows_context_and_both_sides() {
        let old = "1\n2\n3\n4\n5\n6\n7\n8\n";
        let new = "1\n2\n3\nfour\n5\n6\n7\n8\n";
        let d = unified_diff("f.xml", old, new);
        assert_eq!(
            d,
            "--- a/f.xml\n+++ b/f.xml\n@@ -1,7 +1,7 @@\n 1\n 2\n 3\n-4\n+four\n 5\n 6\n 7\n"
        );
    }

    #[test]
    fn appended_and_removed_lines_are_counted() {
        let d = unified_diff("f", "a\n", "a\nb\nc\n");
        assert!(d.contains("@@ -1,1 +1,3 @@"), "{d}");
        assert!(d.contains("+b\n+c\n"));
        let d = unified_diff("f", "a\nb\nc\n", "a\n");
        assert!(d.contains("-b\n-c\n"), "{d}");
    }

    #[test]
    fn distant_changes_make_separate_hunks() {
        let old: String = (1..=30).map(|i| format!("{i}\n")).collect();
        let new = old
            .replace("\n2\n", "\ntwo\n")
            .replace("\n29\n", "\ntwenty-nine\n");
        let d = unified_diff("f", &old, &new);
        assert_eq!(d.matches("@@ ").count(), 2, "{d}");
    }

    #[test]
    fn a_missing_final_newline_is_marked() {
        let d = unified_diff("f", "a\nb", "a\nc");
        assert!(d.contains("\\ No newline at end of file"), "{d}");
    }

    #[test]
    fn creating_from_nothing_adds_every_line() {
        let d = unified_diff("f", "", "x\ny\n");
        assert!(d.contains("@@ -0,0 +1,2 @@"), "{d}");
    }

    #[test]
    fn the_output_is_deterministic() {
        let a = "a\nb\nc\nd\n";
        let b = "a\nx\nc\ny\nd\n";
        assert_eq!(unified_diff("p", a, b), unified_diff("p", a, b));
    }
}
