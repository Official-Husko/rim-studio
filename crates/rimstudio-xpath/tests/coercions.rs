//! The type coercion and comparison rules of XPath 1.0, one table row per rule, plus the
//! truthiness traps that real patches contain (docs/research/xpath-patch-coverage.md, section 5.1).
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use rimstudio_core::tree::NodeBuilder;
use rimstudio_xpath::{Value, XPath, coerce};
use rstest::rstest;

/// `<r><a>1</a><a>2</a><b>2</b><b>3</b><c/></r>`
fn abc_doc() -> rimstudio_core::tree::ArenaDoc {
    doc_of(
        NodeBuilder::new("r")
            .text_elem("a", "1")
            .text_elem("a", "2")
            .text_elem("b", "2")
            .text_elem("b", "3")
            .empty_elem("c"),
    )
}

// ------------------------------------------------------------ truthiness traps (note 5.1)

#[rstest]
// An `or` with a bare string literal is always true: every definition matches.
#[case("Defs/RS_ThingDef[defName = \"RS_Knife\" or \"RS_Nope\"]/label", 6)]
// An `and` with a bare string literal is a no-op: behaves as [defName="RS_Knife"].
#[case("Defs/RS_ThingDef[defName=\"RS_Knife\" and \"Y\"]", 1)]
#[case("Defs/RS_ThingDef[defName=\"RS_Knife\" and \"\"]", 0)]
// A string predicate is truthy (non-empty), so it selects every li.
#[case("Defs/RS_ThingDef/tools/li[\"Steel\"]", 3)]
#[case("Defs/RS_ThingDef/tools/li[\"\"]", 0)]
// Left associative equality chain: (defName = defName) is a boolean, compared to a string
// through boolean conversion. Every def that has a defName matches, the others do not.
#[case("Defs/RS_ThingDef[defName = defName = \"RS_Knife\"]", 5)]
// boolean(true) = boolean("") is false, and for defs without defName false = false is true.
#[case("Defs/RS_ThingDef[defName = defName = \"\"]", 2)]
// A bare name on the right of = is a child element (usually absent): empty set, false.
#[case("Defs/RS_ThingDef/weaponTags/li[text() = Gun]", 0)]
#[case("Defs/RS_ThingDef/weaponTags/li[text() = \"Gun\"]", 1)]
// A number predicate is a position test, a numeric string is not.
#[case("Defs/RS_ThingDef/tools/li[1]", 2)]
#[case("Defs/RS_ThingDef/tools/li[\"1\"]", 3)]
// A node-set predicate is an existence test; a count is a position test.
#[case("Defs/RS_ThingDef[comps]", 2)]
#[case("Defs/RS_ThingDef/comps/li[count(x)]", 2)]
fn truthiness_traps(#[case] expr: &str, #[case] want: usize) {
    assert_eq!(count(&defs_doc(), expr), want, "{expr}");
}

// ------------------------------------------------------------------- number to string

#[rstest]
#[case("string(1 div 0)", "Infinity")]
#[case("string(-1 div 0)", "-Infinity")]
#[case("string(0 div 0)", "NaN")]
#[case("string(0)", "0")]
#[case("string(-0)", "0")]
#[case("string(1)", "1")]
#[case("string(-17)", "-17")]
#[case("string(0.5)", "0.5")]
#[case("string(1.50)", "1.5")]
#[case("string(100000)", "100000")]
#[case("string(2 div 4)", "0.5")]
#[case("string(0.1 + 0.2)", "0.30000000000000004")]
#[case("string(true())", "true")]
#[case("string(false())", "false")]
#[case("string('x')", "x")]
fn string_of_a_value(#[case] expr: &str, #[case] want: &str) {
    assert_eq!(eval_string(&empty_doc(), expr), want, "{expr}");
}

// ------------------------------------------------------------------- string to number

#[rstest]
#[case("number('12')", 12.0)]
#[case("number('  -12.5 ')", -12.5)]
#[case("number('.5')", 0.5)]
#[case("number('7.')", 7.0)]
#[case("number(true())", 1.0)]
#[case("number(false())", 0.0)]
#[case("number(3)", 3.0)]
fn number_of_a_value(#[case] expr: &str, #[case] want: f64) {
    assert_eq!(eval_number(&empty_doc(), expr), want, "{expr}");
}

#[rstest]
#[case("number('')")]
#[case("number('abc')")]
#[case("number('1e3')")]
#[case("number('+1')")]
#[case("number('1 2')")]
#[case("number('Infinity')")]
#[case("number(Defs)")]
#[case("0 div 0")]
#[case("'a' + 1")]
#[case("number(/nothing)")]
fn number_of_a_value_is_nan(#[case] expr: &str) {
    assert!(eval_number(&defs_doc(), expr).is_nan(), "{expr}");
}

// ---------------------------------------------------------------------- to boolean

#[rstest]
#[case("boolean(0)", false)]
#[case("boolean(-0)", false)]
#[case("boolean(0 div 0)", false)]
#[case("boolean(1)", true)]
#[case("boolean(-3.5)", true)]
#[case("boolean(1 div 0)", true)]
#[case("boolean('')", false)]
#[case("boolean('false')", true)]
#[case("boolean(' ')", true)]
#[case("boolean(Defs)", true)]
#[case("boolean(Nope)", false)]
#[case("boolean(Defs/RS_ThingDef/label/text())", true)]
#[case("boolean(Defs/RS_ThingDef[label=''])", true)]
#[case("not(0)", true)]
#[case("not('a')", false)]
#[case("not(Nope)", true)]
#[case("true() and false()", false)]
#[case("true() or false()", true)]
#[case("false() or false()", false)]
fn boolean_of_a_value(#[case] expr: &str, #[case] want: bool) {
    assert_eq!(eval_bool(&defs_doc(), expr), want, "{expr}");
}

// ------------------------------------------------------------------ comparison matrix

#[rstest]
// string and number: compared as numbers
#[case("1 = '1'", true)]
#[case("'1.0' = 1", true)]
#[case("1 = '01'", true)]
#[case("'a' = 1", false)]
#[case("'a' != 1", true)]
#[case("'' = 0", false)]
// boolean with anything: compared as booleans
#[case("true() = 'x'", true)]
#[case("false() = ''", true)]
#[case("true() = 1", true)]
#[case("false() = 0", true)]
#[case("true() = 'false'", true)]
#[case("false() = 'false'", false)]
#[case("true() != 0", true)]
// string with string
#[case("'a' = 'a'", true)]
#[case("'a' = 'A'", false)]
#[case("'a' != 'b'", true)]
// relational operators always compare numbers
#[case("'2' < '10'", true)]
#[case("'a' < 'b'", false)]
#[case("'a' >= 'a'", false)]
#[case("true() > false()", true)]
#[case("'3' > true()", true)]
#[case("2 <= 2", true)]
#[case("3 < 2", false)]
// NaN
#[case("number('x') = number('x')", false)]
#[case("number('x') != number('x')", true)]
#[case("number('x') < 1", false)]
#[case("number('x') >= 1", false)]
// precedence and associativity
#[case("1 < 2 = true()", true)]
#[case("2 > 1 > 0", true)]
#[case("3 > 2 > 1", false)]
#[case("1 = 1 = 1", true)]
#[case("1 + 1 = 2", true)]
fn scalar_comparisons(#[case] expr: &str, #[case] want: bool) {
    assert_eq!(eval_bool(&empty_doc(), expr), want, "{expr}");
}

#[rstest]
// node-set with node-set: existential on string values
#[case("a = b", true)]
#[case("a != b", true)]
#[case("a < b", true)]
#[case("a > b", false)]
#[case("a >= b", true)]
#[case("a <= b", true)]
#[case("c = c", true)]
#[case("d = d", false)]
#[case("d != d", false)]
#[case("a = d", false)]
// node-set with number
#[case("a = 2", true)]
#[case("a = 3", false)]
#[case("a != 2", true)]
#[case("a > 1", true)]
#[case("a > 2", false)]
#[case("0 < a", true)]
#[case("3 < a", false)]
#[case("c = 0", false)]
// node-set with string
#[case("a = '2'", true)]
#[case("a = '2.0'", false)]
#[case("b != '2'", true)]
#[case("a < 'x'", false)]
#[case("'3' = b", true)]
// node-set with boolean: the set is converted with boolean()
#[case("a = true()", true)]
#[case("c = false()", false)]
#[case("c = true()", true)]
#[case("d = false()", true)]
#[case("d != true()", true)]
#[case("true() = a", true)]
#[case("a > false()", true)]
#[case("d < true()", true)]
// the empty set never compares true with =, != on sets or scalars
#[case("d = 'x'", false)]
#[case("d != 'x'", false)]
#[case("d = 0", false)]
// attributes and text
#[case("a/text() = 2", true)]
#[case("string(a) = '1'", true)]
fn node_set_comparisons(#[case] expr: &str, #[case] want: bool) {
    let d = abc_doc();
    let got = XPath::parse(&format!("/r[{expr}]"))
        .unwrap()
        .select(&d)
        .unwrap()
        .len()
        == 1;
    assert_eq!(got, want, "{expr}");
    // and the same expression evaluated directly with the element as context
    let r = d.document_element().unwrap();
    let v = XPath::parse(expr).unwrap().eval(&d, r).unwrap();
    assert_eq!(coerce::to_boolean(&v), want, "{expr}");
}

#[test]
fn node_set_conversions() {
    let d = abc_doc();
    let r = d.document_element().unwrap();
    let ev = |e: &str| XPath::parse(e).unwrap().eval(&d, r).unwrap();
    assert_eq!(coerce::to_string(&d, &ev("a")), "1");
    assert_eq!(coerce::to_number(&d, &ev("a")), 1.0);
    assert_eq!(coerce::to_string(&d, &ev("c")), "");
    assert!(coerce::to_number(&d, &ev("c")).is_nan());
    assert_eq!(coerce::to_string(&d, &ev("d")), "");
    assert!(coerce::to_number(&d, &ev("d")).is_nan());
    assert_eq!(ev("sum(a)"), Value::Number(3.0));
    assert_eq!(ev("sum(d)"), Value::Number(0.0));
    assert_eq!(ev("count(a | b)"), Value::Number(4.0));
    assert_eq!(ev("count(a | a)"), Value::Number(2.0));
    assert_eq!(ev("string(.)"), Value::String("1223".into()));
    assert_eq!(ev("string-length()"), Value::Number(4.0));
    assert_eq!(ev("string(a[2])"), Value::String("2".into()));
    assert_eq!(ev("number(b[1]) + number(b[2])"), Value::Number(5.0));
}

// --------------------------------------------------------------------- arithmetic

#[rstest]
#[case("5 mod 2", 1.0)]
#[case("-5 mod 2", -1.0)]
#[case("5 mod -2", 1.0)]
#[case("-5 mod -2", -1.0)]
#[case("5.5 mod 2", 1.5)]
#[case("2 * 3 + 1", 7.0)]
#[case("1 + 2 * 3", 7.0)]
#[case("(1 + 2) * 3", 9.0)]
#[case("1 - 2 - 3", -4.0)]
#[case("8 div 2 div 2", 2.0)]
#[case("- - 1", 1.0)]
#[case("-(2)", -2.0)]
#[case("1 + '2'", 3.0)]
#[case("true() + 1", 2.0)]
#[case("1 div 4", 0.25)]
#[case("count(Defs/RS_ThingDef) * 2", 14.0)]
#[case("sum(Defs/RS_ThingDef/statBases/Mass)", 8.5)]
#[case("floor(-1.5)", -2.0)]
#[case("floor(1.5)", 1.0)]
#[case("ceiling(-1.5)", -1.0)]
#[case("ceiling(1.2)", 2.0)]
#[case("round(1.5)", 2.0)]
#[case("round(2.5)", 3.0)]
#[case("round(-1.5)", -1.0)]
#[case("round(-2.5)", -2.0)]
#[case("round(1.4)", 1.0)]
#[case("round(-0.6)", -1.0)]
#[case("position()", 1.0)]
#[case("last()", 1.0)]
#[case("count(id('x'))", 0.0)]
#[case("string-length('hello')", 5.0)]
#[case("string-length('h\u{e9}llo')", 5.0)]
#[case("string-length('')", 0.0)]
fn numeric_results(#[case] expr: &str, #[case] want: f64) {
    assert_eq!(eval_number(&defs_doc(), expr), want, "{expr}");
}

#[rstest]
#[case("1 div 0", f64::INFINITY)]
#[case("-1 div 0", f64::NEG_INFINITY)]
#[case("floor(1 div 0)", f64::INFINITY)]
#[case("round(-1 div 0)", f64::NEG_INFINITY)]
fn infinite_results(#[case] expr: &str, #[case] want: f64) {
    assert_eq!(eval_number(&empty_doc(), expr), want, "{expr}");
}

#[test]
fn nan_propagates_through_the_numeric_functions() {
    let d = empty_doc();
    for e in [
        "floor(0 div 0)",
        "ceiling(0 div 0)",
        "round(0 div 0)",
        "5 mod 0",
        "0 div 0 + 1",
        "sum(/nothing) div 0 * 0",
    ] {
        assert!(eval_number(&d, e).is_nan(), "{e}");
    }
    // round of a value between -0.5 and 0 is negative zero, which prints as 0
    assert_eq!(eval_string(&d, "string(round(-0.4))"), "0");
    assert!(eval_number(&d, "round(-0.4)").is_sign_negative());
}

// ------------------------------------------------------------------ string functions

#[rstest]
#[case("concat('a', 'b', 'c')", "abc")]
#[case("concat(1, 2)", "12")]
#[case("concat('x', true(), 0.5)", "xtrue0.5")]
#[case("substring-before('1999/04/01', '/')", "1999")]
#[case("substring-after('1999/04/01', '/')", "04/01")]
#[case("substring-after('1999/04/01', '19')", "99/04/01")]
#[case("substring-before('abc', '')", "")]
#[case("substring-after('abc', '')", "abc")]
#[case("substring-before('abc', 'x')", "")]
#[case("substring-after('abc', 'x')", "")]
#[case("substring('12345', 1.5, 2.6)", "234")]
#[case("substring('12345', 0, 3)", "12")]
#[case("substring('12345', 0 div 0, 3)", "")]
#[case("substring('12345', 1, 0 div 0)", "")]
#[case("substring('12345', -42, 1 div 0)", "12345")]
#[case("substring('12345', -1 div 0, 1 div 0)", "")]
#[case("substring('12345', 2)", "2345")]
#[case("substring('12345', 2, 2)", "23")]
#[case("substring('12345', 5, 10)", "5")]
#[case("substring('12345', 6)", "")]
#[case("substring('12345', -1)", "12345")]
#[case("substring('h\u{e9}llo', 2, 1)", "\u{e9}")]
#[case("normalize-space('  a   b \t\n c ')", "a b c")]
#[case("normalize-space('')", "")]
#[case("normalize-space('   ')", "")]
#[case("translate('bar', 'abc', 'ABC')", "BAr")]
#[case("translate('--aaa--', 'abc-', 'ABC')", "AAA")]
#[case("translate('abc', 'aa', 'xy')", "xbc")]
#[case("translate('abc', '', 'xy')", "abc")]
#[case("name(Defs/*[1])", "RS_ThingDef")]
#[case("name(Defs/RS_ThingDef/@Name)", "Name")]
#[case("name(Defs/RS_ThingDef/label/text())", "")]
#[case("name(Nope)", "")]
#[case("name(/)", "")]
#[case("local-name(Defs)", "Defs")]
#[case("namespace-uri(Defs)", "")]
#[case("name(Defs/RS_ThingDef[2]/label)", "label")]
#[case("string(Defs/RS_ThingDef[2]/defName)", "RS_RifleA")]
#[case("string(Defs/RS_ThingDef[defName = 'RS_Empty']/label)", "")]
#[case("string(Defs/RS_ThingDef/@Abstract)", "True")]
fn string_results(#[case] expr: &str, #[case] want: &str) {
    assert_eq!(eval_string(&defs_doc(), expr), want, "{expr}");
}

#[rstest]
#[case("contains('abc', 'b')", true)]
#[case("contains('abc', '')", true)]
#[case("contains('abc', 'abcd')", false)]
#[case("contains('', '')", true)]
#[case("starts-with('abc', 'ab')", true)]
#[case("starts-with('abc', '')", true)]
#[case("starts-with('abc', 'bc')", false)]
#[case("contains(Defs/RS_ThingDef/label, 'base')", true)]
#[case("contains(Defs/RS_ThingDef/tools/li/label, 'stock')", true)]
fn string_predicates(#[case] expr: &str, #[case] want: bool) {
    assert_eq!(eval_bool(&defs_doc(), expr), want, "{expr}");
}

#[test]
fn functions_default_to_the_context_node() {
    let d = defs_doc();
    let label = XPath::parse("Defs/RS_ThingDef[2]/label")
        .unwrap()
        .select(&d)
        .unwrap()[0];
    let ev = |e: &str| XPath::parse(e).unwrap().eval(&d, label).unwrap();
    assert_eq!(ev("string()"), Value::String("rifle a".into()));
    assert_eq!(ev("string-length()"), Value::Number(7.0));
    assert_eq!(ev("normalize-space()"), Value::String("rifle a".into()));
    assert_eq!(ev("name()"), Value::String("label".into()));
    assert_eq!(ev("local-name()"), Value::String("label".into()));
    assert_eq!(ev("namespace-uri()"), Value::String(String::new()));
    assert!(matches!(ev("number()"), Value::Number(n) if n.is_nan()));
    assert_eq!(ev("count(.)"), Value::Number(1.0));
    assert_eq!(ev("count(../*)"), Value::Number(6.0));
    assert_eq!(ev("boolean(.)"), Value::Boolean(true));
}

#[test]
fn position_and_last_inside_predicates_follow_the_axis_direction() {
    let d = defs_doc();
    assert_eq!(count(&d, "Defs/RS_ThingDef[position() = last()]"), 1);
    assert_eq!(count(&d, "Defs/RS_ThingDef/tools/li[position() = 1]"), 2);
    assert_eq!(count(&d, "Defs/RS_ThingDef/tools/li[last()]"), 2);
    assert_eq!(count(&d, "Defs/RS_ThingDef[position() mod 2 = 1]"), 4);
    assert_eq!(
        count(
            &d,
            "Defs/RS_ThingDef[position() = 1 or position() = last()]"
        ),
        2
    );
    assert_eq!(count(&d, "Defs/RS_KindDef/weights/li[position() > 1]"), 2);
    assert_eq!(count(&d, "Defs/*[position() = 5]"), 1);
}
