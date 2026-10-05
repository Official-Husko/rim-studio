//! The type conversions and comparison rules of XPath 1.0 (sections 3.4, 4.2 to 4.4 of the
//! recommendation).
//!
//! - number to string: `NaN`, `Infinity`, `-Infinity`; zero (of either sign) is `0`; integers
//!   have no decimal point; other numbers use the shortest decimal form that round trips, never
//!   an exponent. This follows the recommendation (as many
//!   digits as needed to tell the value apart from its neighbours); a runtime that prints a fixed
//!   number of significant digits can differ in the last places of fractional results such as
//!   `string(1 div 3)`, which no patch in the corpus depends on.
//! - string to number: optional white space, an optional minus sign, digits with an optional
//!   fraction (or a bare fraction), optional white space; anything else (a plus sign, an
//!   exponent, `Infinity`, hex) is `NaN`.
//! - boolean: a number is true unless zero or NaN, a string unless empty, a node-set unless empty.
//! - comparisons: node-set operands compare existentially; with a boolean on one side the
//!   other side is converted to boolean; relational operators compare numbers; `=` and `!=`
//!   compare numbers when one side is a number and strings otherwise.

use std::borrow::Cow;

use rimstudio_core::tree::ArenaDoc;

use crate::ast::BinOp;
use crate::value::{Value, XNode};

/// XPath white space: space, tab, carriage return, line feed.
#[must_use]
pub fn is_xpath_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n')
}

/// Formats a number the way the `string()` function does.
#[must_use]
pub fn number_to_string(n: f64) -> String {
    if n.is_nan() {
        "NaN".to_owned()
    } else if n.is_infinite() {
        if n > 0.0 { "Infinity" } else { "-Infinity" }.to_owned()
    } else if n == 0.0 {
        "0".to_owned()
    } else {
        // `Display` for f64 prints the shortest round trip digits without an exponent.
        format!("{n}")
    }
}

/// Converts a string to a number the way the `number()` function does.
#[must_use]
pub fn string_to_number(s: &str) -> f64 {
    let t = s.trim_matches(is_xpath_space);
    let body = t.strip_prefix('-').unwrap_or(t);
    let negative = body.len() != t.len();
    let mut seen_digit = false;
    let mut seen_dot = false;
    for c in body.chars() {
        match c {
            '0'..='9' => seen_digit = true,
            '.' if !seen_dot => seen_dot = true,
            _ => return f64::NAN,
        }
    }
    if !seen_digit {
        return f64::NAN;
    }
    let normalized: Cow<'_, str> = if body.starts_with('.') {
        Cow::Owned(format!("0{body}"))
    } else {
        Cow::Borrowed(body)
    };
    match normalized.parse::<f64>() {
        Ok(v) => {
            if negative {
                -v
            } else {
                v
            }
        }
        Err(_) => f64::NAN,
    }
}

/// The string value of a node, empty for a stale handle.
#[must_use]
pub fn node_string<'a>(doc: &'a ArenaDoc, node: XNode) -> Cow<'a, str> {
    match node {
        XNode::Node(id) => doc.string_value(id).unwrap_or(Cow::Borrowed("")),
        XNode::Attr { owner, index } => Cow::Borrowed(
            doc.attr_at(owner, index as usize)
                .map_or("", |(_, value)| value),
        ),
    }
}

/// The `string()` conversion of a value.
#[must_use]
pub fn to_string(doc: &ArenaDoc, v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => number_to_string(*n),
        Value::Boolean(b) => if *b { "true" } else { "false" }.to_owned(),
        Value::NodeSet(nodes) => nodes
            .first()
            .map_or_else(String::new, |n| node_string(doc, *n).into_owned()),
    }
}

/// The `number()` conversion of a value.
#[must_use]
pub fn to_number(doc: &ArenaDoc, v: &Value) -> f64 {
    match v {
        Value::Number(n) => *n,
        Value::Boolean(b) => f64::from(u8::from(*b)),
        Value::String(s) => string_to_number(s),
        Value::NodeSet(nodes) => nodes
            .first()
            .map_or(f64::NAN, |n| string_to_number(&node_string(doc, *n))),
    }
}

/// The `boolean()` conversion of a value.
#[must_use]
pub fn to_boolean(v: &Value) -> bool {
    match v {
        Value::Boolean(b) => *b,
        Value::Number(n) => !(n.is_nan() || *n == 0.0),
        Value::String(s) => !s.is_empty(),
        Value::NodeSet(nodes) => !nodes.is_empty(),
    }
}

fn compare_numbers(op: BinOp, a: f64, b: f64) -> bool {
    match op {
        BinOp::Eq => a == b,
        BinOp::Neq => a != b,
        BinOp::Lt => a < b,
        BinOp::Le => a <= b,
        BinOp::Gt => a > b,
        BinOp::Ge => a >= b,
        _ => false,
    }
}

fn compare_bools(op: BinOp, a: bool, b: bool) -> bool {
    match op {
        BinOp::Eq => a == b,
        BinOp::Neq => a != b,
        _ => compare_numbers(op, f64::from(u8::from(a)), f64::from(u8::from(b))),
    }
}

fn compare_strings(op: BinOp, a: &str, b: &str) -> bool {
    match op {
        BinOp::Eq => a == b,
        BinOp::Neq => a != b,
        _ => compare_numbers(op, string_to_number(a), string_to_number(b)),
    }
}

fn is_equality(op: BinOp) -> bool {
    matches!(op, BinOp::Eq | BinOp::Neq)
}

/// Evaluates a comparison operator (`=`, `!=`, `<`, `<=`, `>`, `>=`) on two values with the
/// full set of XPath 1.0 rules.
#[must_use]
pub fn compare(doc: &ArenaDoc, op: BinOp, left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::NodeSet(a), Value::NodeSet(b)) => {
            if is_equality(op) {
                // Collect the right strings once; the comparison is existential.
                let rs: Vec<Cow<'_, str>> = b.iter().map(|n| node_string(doc, *n)).collect();
                a.iter().any(|x| {
                    let xs = node_string(doc, *x);
                    rs.iter().any(|y| compare_strings(op, &xs, y))
                })
            } else {
                let rn: Vec<f64> = b
                    .iter()
                    .map(|n| string_to_number(&node_string(doc, *n)))
                    .collect();
                a.iter().any(|x| {
                    let xn = string_to_number(&node_string(doc, *x));
                    rn.iter().any(|y| compare_numbers(op, xn, *y))
                })
            }
        }
        (Value::NodeSet(a), other) => compare_set_with(doc, op, a, other, false),
        (other, Value::NodeSet(b)) => compare_set_with(doc, op, b, other, true),
        (a, b) => {
            if is_equality(op) {
                if matches!(a, Value::Boolean(_)) || matches!(b, Value::Boolean(_)) {
                    compare_bools(op, to_boolean(a), to_boolean(b))
                } else if matches!(a, Value::Number(_)) || matches!(b, Value::Number(_)) {
                    compare_numbers(op, to_number(doc, a), to_number(doc, b))
                } else {
                    compare_strings(op, &to_string(doc, a), &to_string(doc, b))
                }
            } else {
                compare_numbers(op, to_number(doc, a), to_number(doc, b))
            }
        }
    }
}

/// A node-set against a non node-set value. `flipped` is true when the node-set is the right
/// operand, so `op` applies as `other op node`.
fn compare_set_with(
    doc: &ArenaDoc,
    op: BinOp,
    set: &[XNode],
    other: &Value,
    flipped: bool,
) -> bool {
    let apply_num = |node_val: f64, o: f64| {
        if flipped {
            compare_numbers(op, o, node_val)
        } else {
            compare_numbers(op, node_val, o)
        }
    };
    match other {
        Value::Boolean(b) => {
            let sb = !set.is_empty();
            if flipped {
                compare_bools(op, *b, sb)
            } else {
                compare_bools(op, sb, *b)
            }
        }
        Value::Number(n) => set
            .iter()
            .any(|x| apply_num(string_to_number(&node_string(doc, *x)), *n)),
        Value::String(s) => {
            if is_equality(op) {
                set.iter()
                    .any(|x| compare_strings(op, &node_string(doc, *x), s))
            } else {
                let o = string_to_number(s);
                set.iter()
                    .any(|x| apply_num(string_to_number(&node_string(doc, *x)), o))
            }
        }
        Value::NodeSet(_) => false,
    }
}

/// Rounds half towards positive infinity, as the `round()` function does. NaN and infinities
/// are returned unchanged, and values between -0.5 and 0 give negative zero.
#[must_use]
pub fn xpath_round(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    if (x - x.trunc()).abs() == 0.5 {
        x.ceil()
    } else {
        x.round()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(f64::NAN, "NaN")]
    #[case(f64::INFINITY, "Infinity")]
    #[case(f64::NEG_INFINITY, "-Infinity")]
    #[case(0.0, "0")]
    #[case(-0.0, "0")]
    #[case(1.0, "1")]
    #[case(-12.0, "-12")]
    #[case(0.5, "0.5")]
    #[case(-0.25, "-0.25")]
    #[case(1e21, "1000000000000000000000")]
    #[case(0.0000001, "0.0000001")]
    #[case(123456789.125, "123456789.125")]
    fn number_to_string_rows(#[case] n: f64, #[case] s: &str) {
        assert_eq!(number_to_string(n), s);
    }

    #[rstest]
    #[case("12", 12.0)]
    #[case("  12.5  ", 12.5)]
    #[case("-3", -3.0)]
    #[case(".5", 0.5)]
    #[case("5.", 5.0)]
    #[case("\n7\t", 7.0)]
    #[case("-.5", -0.5)]
    #[case("007", 7.0)]
    fn string_to_number_valid_rows(#[case] s: &str, #[case] n: f64) {
        assert_eq!(string_to_number(s), n);
    }

    #[rstest]
    #[case("")]
    #[case("   ")]
    #[case("+1")]
    #[case("1e3")]
    #[case("1 2")]
    #[case("0x10")]
    #[case("Infinity")]
    #[case("NaN")]
    #[case("--1")]
    #[case("1.2.3")]
    #[case("-")]
    #[case(".")]
    #[case("a")]
    fn string_to_number_nan_rows(#[case] s: &str) {
        assert!(string_to_number(s).is_nan(), "{s:?}");
    }

    #[test]
    fn negative_zero_from_a_string_keeps_its_sign_but_prints_as_zero() {
        let n = string_to_number("-0");
        assert!(n == 0.0 && n.is_sign_negative());
        assert_eq!(number_to_string(n), "0");
    }

    #[rstest]
    #[case(0.5, 1.0)]
    #[case(1.5, 2.0)]
    #[case(2.5, 3.0)]
    #[case(-0.6, -1.0)]
    #[case(-1.5, -1.0)]
    #[case(-2.5, -2.0)]
    #[case(2.4, 2.0)]
    #[case(1e300, 1e300)]
    fn round_rows(#[case] x: f64, #[case] want: f64) {
        assert_eq!(xpath_round(x), want);
    }

    #[test]
    fn round_small_negative_is_negative_zero() {
        let r = xpath_round(-0.4);
        assert!(r == 0.0 && r.is_sign_negative());
        assert!(xpath_round(f64::NAN).is_nan());
        assert_eq!(xpath_round(f64::INFINITY), f64::INFINITY);
    }
}
