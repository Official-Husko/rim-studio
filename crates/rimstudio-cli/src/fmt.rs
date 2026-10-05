//! Small helpers for reading JSON responses and drawing plain text tables.
//!
//! Responses are handled as `serde_json::Value` so the CLI keeps working when a response gains a
//! field. Every accessor answers an empty value instead of failing.

use std::fmt::Write as _;

use serde_json::Value;

/// The string at a JSON pointer, or the empty string.
#[must_use]
pub(crate) fn str_at<'a>(v: &'a Value, pointer: &str) -> &'a str {
    v.pointer(pointer).and_then(Value::as_str).unwrap_or("")
}

/// The unsigned integer at a JSON pointer, or zero.
#[must_use]
pub(crate) fn u64_at(v: &Value, pointer: &str) -> u64 {
    v.pointer(pointer).and_then(Value::as_u64).unwrap_or(0)
}

/// The number at a JSON pointer.
#[must_use]
pub(crate) fn f64_at(v: &Value, pointer: &str) -> Option<f64> {
    v.pointer(pointer).and_then(Value::as_f64)
}

/// The boolean at a JSON pointer, or false.
#[must_use]
pub(crate) fn bool_at(v: &Value, pointer: &str) -> bool {
    v.pointer(pointer).and_then(Value::as_bool).unwrap_or(false)
}

/// The array at a JSON pointer, or an empty slice.
#[must_use]
pub(crate) fn arr_at<'a>(v: &'a Value, pointer: &str) -> &'a [Value] {
    v.pointer(pointer)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// A number for people: at most four decimals, no trailing zeros.
#[must_use]
pub(crate) fn num(value: f64) -> String {
    if !value.is_finite() {
        return "n/a".to_owned();
    }
    let text = format!("{value:.4}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-" || trimmed == "-0" {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// A number or a dash.
#[must_use]
pub(crate) fn opt_num(value: Option<f64>) -> String {
    value.map_or_else(|| "-".to_owned(), num)
}

/// A plain text table with left aligned columns.
#[derive(Debug, Default)]
pub(crate) struct Table {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    /// A table with the given column names.
    #[must_use]
    pub(crate) fn new(header: &[&str]) -> Self {
        Self {
            header: header.iter().map(|s| (*s).to_owned()).collect(),
            rows: Vec::new(),
        }
    }

    /// Adds a row; missing cells are empty.
    pub(crate) fn row(&mut self, cells: Vec<String>) {
        self.rows.push(cells);
    }

    /// True when no row was added.
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The table as text, one line per row, indented by two spaces.
    #[must_use]
    pub(crate) fn render(&self) -> String {
        let cols = self.header.len();
        let mut widths: Vec<usize> = self.header.iter().map(|h| h.chars().count()).collect();
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate().take(cols) {
                if let Some(w) = widths.get_mut(i) {
                    *w = (*w).max(cell.chars().count());
                }
            }
        }
        let mut out = String::new();
        let mut line = |cells: &[String]| {
            let mut text = String::from("  ");
            for (i, w) in widths.iter().enumerate() {
                let cell = cells.get(i).map_or("", String::as_str);
                let pad = w.saturating_sub(cell.chars().count());
                text.push_str(cell);
                if i + 1 < cols {
                    text.push_str(&" ".repeat(pad + 2));
                }
            }
            let _ = writeln!(out, "{}", text.trim_end());
        };
        line(&self.header);
        for row in &self.rows {
            line(row);
        }
        out
    }
}

/// Counts of the severities of a diagnostic list.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DiagCounts {
    /// Errors.
    pub(crate) errors: usize,
    /// Warnings.
    pub(crate) warnings: usize,
}

/// Counts the errors and warnings of diagnostics in their JSON form.
#[must_use]
pub(crate) fn count_diagnostics(diags: &[Value]) -> DiagCounts {
    let mut c = DiagCounts::default();
    for d in diags {
        match d.get("severity").and_then(Value::as_str) {
            Some("error") => c.errors += 1,
            Some("warning") => c.warnings += 1,
            _ => {}
        }
    }
    c
}

/// Renders diagnostics one per line: `severity code [field]: message`.
#[must_use]
pub(crate) fn render_diagnostics(diags: &[Value]) -> String {
    let mut out = String::new();
    for d in diags {
        let field = d.get("field").and_then(Value::as_str);
        let _ = writeln!(
            out,
            "  {} {}{}: {}",
            str_at(d, "/severity"),
            str_at(d, "/code"),
            field.map_or_else(String::new, |f| format!(" [{f}]")),
            str_at(d, "/message"),
        );
    }
    out
}

/// A byte count for people.
#[must_use]
pub(crate) fn bytes(n: u64) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{} KiB", num(n as f64 / 1024.0))
    } else {
        format!("{} MiB", num(n as f64 / 1024.0 / 1024.0))
    }
}

/// Turns a def name into a label: underscores to spaces, lower case, the fiction prefix kept.
#[must_use]
pub(crate) fn label_of(def_name: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for ch in def_name.chars() {
        if ch == '_' || ch == '-' {
            out.push(' ');
            prev_lower = false;
        } else {
            if ch.is_uppercase() && prev_lower {
                out.push(' ');
            }
            prev_lower = ch.is_lowercase() || ch.is_ascii_digit();
            out.extend(ch.to_lowercase());
        }
    }
    out.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn numbers_are_trimmed() {
        assert_eq!(num(12.0), "12");
        assert_eq!(num(0.12500), "0.125");
        assert_eq!(num(-0.0), "0");
        assert_eq!(num(f64::NAN), "n/a");
        assert_eq!(opt_num(None), "-");
    }

    #[test]
    fn a_table_aligns_columns_and_trims_line_ends() {
        let mut t = Table::new(&["a", "bb"]);
        t.row(vec!["long cell".into(), "x".into()]);
        t.row(vec!["s".into()]);
        let text = t.render();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "  a          bb");
        assert_eq!(lines[1], "  long cell  x");
        assert_eq!(lines[2], "  s");
    }

    #[test]
    fn accessors_never_fail_on_missing_members() {
        let v = json!({"a": {"b": 3}});
        assert_eq!(u64_at(&v, "/a/b"), 3);
        assert_eq!(str_at(&v, "/x"), "");
        assert!(arr_at(&v, "/a").is_empty());
        assert!(!bool_at(&v, "/q"));
    }

    #[test]
    fn diagnostics_are_counted_and_rendered() {
        let d = vec![
            json!({"severity": "error", "code": "a.b", "message": "m", "field": "/x"}),
            json!({"severity": "warning", "code": "c.d", "message": "n"}),
            json!({"severity": "info", "code": "e.f", "message": "o"}),
        ];
        assert_eq!(
            count_diagnostics(&d),
            DiagCounts {
                errors: 1,
                warnings: 1
            }
        );
        let text = render_diagnostics(&d);
        assert!(text.contains("error a.b [/x]: m"));
        assert!(text.contains("warning c.d: n"));
    }

    #[test]
    fn labels_come_from_def_names() {
        assert_eq!(label_of("RS_TestRifle"), "rs test rifle");
        assert_eq!(label_of("rs_new-gun"), "rs new gun");
    }
}
