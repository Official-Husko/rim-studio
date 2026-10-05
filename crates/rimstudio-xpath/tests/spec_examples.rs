//! Cases derived by hand from the examples of the XPath 1.0 recommendation (sections 2.1 to 2.5
//! and the function library), on a fictional document.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use rimstudio_xpath::XPath;
use rstest::rstest;

/// Selects from the first `chapter` element instead of the document node.
fn from_first_chapter(expr: &str) -> Vec<String> {
    let d = book_doc();
    let ch = XPath::parse("/doc/chapter[1]").unwrap().select(&d).unwrap()[0];
    XPath::parse(expr)
        .unwrap()
        .select_nodes(&d, ch)
        .unwrap()
        .into_iter()
        .map(|n| describe(&d, n))
        .collect()
}

#[rstest]
// section 2.5 abbreviated syntax
#[case("child::para", 7)]
#[case("para", 7)]
#[case("child::*", 8)]
#[case("*", 8)]
#[case("child::text()", 0)]
#[case("child::node()", 8)]
#[case("attribute::*", 0)]
#[case("descendant::para", 7)]
#[case("self::chapter", 1)]
#[case("self::para", 0)]
#[case("descendant-or-self::para", 7)]
#[case("ancestor::doc", 1)]
#[case("ancestor-or-self::chapter", 1)]
#[case("child::*/child::para", 0)]
#[case("para[1]", 1)]
#[case("para[last()]", 1)]
#[case("*/para", 0)]
#[case("para[position() = 2]", 1)]
#[case("para[@type = 'warning']", 6)]
#[case("para[@type = 'warning'][5]", 1)]
#[case("para[5][@type = 'warning']", 1)]
#[case("child::para[attribute::type=\"warning\"][position()=5]", 1)]
#[case("child::para[position()=5][attribute::type=\"warning\"]", 1)]
#[case("title[. = 'Introduction']", 1)]
#[case("title[text() = 'Introduction']", 1)]
#[case("../chapter[title='Body']", 1)]
#[case("../employee[@secretary and @assistant]", 1)]
#[case("../employee[@secretary or @assistant]", 2)]
#[case("../employee[not(@secretary)]", 1)]
fn relative_paths_from_a_chapter(#[case] expr: &str, #[case] want: usize) {
    assert_eq!(from_first_chapter(expr).len(), want, "{expr}");
}

#[test]
fn the_two_orderings_of_predicates_give_different_nodes() {
    assert_eq!(
        from_first_chapter("para[@type = 'warning'][5]"),
        vec!["para[warning]:p6"]
    );
    assert_eq!(
        from_first_chapter("para[5][@type = 'warning']"),
        vec!["para[warning]:p5"]
    );
}

#[rstest]
#[case("/descendant::para[position() = 1]", 1)]
#[case("/descendant::para", 11)]
#[case("//para", 11)]
#[case("//para[1]", 4)]
#[case("/descendant::para[1]", 1)]
#[case("//chapter//para", 11)]
#[case("/doc/chapter/descendant::para", 11)]
#[case("/doc/*/para", 8)]
#[case("//*[self::para or self::item]", 13)]
#[case("//olist/item", 2)]
#[case("/doc/chapter/section/olist/item[2]", 1)]
#[case("/doc/chapter[title = 'Introduction']", 1)]
#[case("/doc/chapter[title]", 3)]
#[case("/doc/chapter[2]//para[@type = 'warning']", 1)]
#[case("//para[@type]", 8)]
#[case("//para[not(@type)]", 3)]
#[case("/doc/employee[@secretary and @assistant]", 1)]
#[case("/doc/employee[@secretary]", 2)]
#[case("/doc/employee[@name = ../employee[1]/@name]", 1)]
#[case("/doc/chapter[last()]/title", 1)]
#[case("/descendant::olist/ancestor::chapter", 1)]
#[case("/descendant-or-self::node()/child::para", 11)]
#[case("//title[. = 'Body']/..", 1)]
#[case("//@type", 8)]
#[case("//@*", 14)]
#[case("//employee/@*", 6)]
#[case("//chapter[2]/section/preceding-sibling::*", 2)]
#[case("//chapter[2]/section/following-sibling::*", 0)]
#[case("/doc/chapter[1]/para[7]/preceding-sibling::para", 6)]
#[case("/doc/chapter[1]/para[position() mod 2 = 0]", 3)]
#[case("/doc/chapter[1]/title/following-sibling::para[2]", 1)]
#[case("//olist/preceding::para", 10)]
#[case("//olist/following::para", 1)]
#[case("//div/ancestor::*", 2)]
#[case("//div/ancestor-or-self::*", 3)]
#[case("/descendant::*[last()]", 1)]
#[case("//*[last()]", 8)]
fn absolute_paths(#[case] expr: &str, #[case] want: usize) {
    assert_eq!(count(&book_doc(), expr), want, "{expr}");
}

#[test]
fn nearest_node_on_reverse_axes_is_position_one() {
    let d = book_doc();
    assert_eq!(
        select(&d, "/doc/chapter[1]/para[7]/preceding-sibling::para[1]"),
        vec!["para[warning]:p6"]
    );
    assert_eq!(
        select(&d, "/doc/chapter[1]/para[7]/preceding-sibling::*[last()]"),
        vec!["title:Introduction"]
    );
    assert_eq!(select(&d, "//item[2]/ancestor::*[1]"), vec!["olist"]);
    assert_eq!(select(&d, "//item[2]/ancestor::*[2]"), vec!["section"]);
    assert_eq!(
        select(&d, "//item[2]/preceding-sibling::item[1]"),
        vec!["item:i1"]
    );
}

#[test]
fn star_is_multiplication_or_wildcard_by_context() {
    let d = book_doc();
    assert_eq!(eval_number(&d, "count(/doc/*) * 2"), 12.0);
    assert_eq!(
        eval_number(&d, "count(/doc/chapter) * count(/doc/employee)"),
        9.0
    );
    assert_eq!(eval_number(&d, "2 * 3"), 6.0);
    assert_eq!(count(&d, "/*"), 1);
    assert_eq!(count(&d, "/doc/*[2]"), 1);
}

#[test]
fn operator_names_can_be_element_names() {
    let b = rimstudio_core::tree::NodeBuilder::new("Defs")
        .elem("and", |b| {
            b.text_elem("or", "1")
                .text_elem("div", "2")
                .text_elem("mod", "3")
        })
        .elem("x", |b| b.text_elem("and", "4"));
    let d = doc_of(b);
    assert_eq!(count(&d, "/Defs/and"), 1);
    assert_eq!(count(&d, "/Defs/and/or"), 1);
    assert_eq!(count(&d, "/Defs/*[or and div and mod]"), 1);
    assert_eq!(count(&d, "/Defs/*[and]"), 1);
    assert_eq!(eval_number(&d, "/Defs/and/div div /Defs/and/or"), 2.0);
    assert_eq!(eval_number(&d, "/Defs/and/mod mod 2"), 1.0);
    assert_eq!(eval_number(&d, "/Defs/x/and * 2"), 8.0);
    assert_eq!(eval_number(&d, "/Defs/and/or + /Defs/and/or"), 2.0);
    assert_eq!(eval_number(&d, "count(//and)"), 2.0);
}

#[test]
fn names_may_contain_hyphens_and_dots_so_minus_needs_spaces() {
    let b = rimstudio_core::tree::NodeBuilder::new("Defs")
        .text_elem("a-b", "10")
        .text_elem("a", "3")
        .text_elem("b", "1")
        .text_elem("v1.5", "7");
    let d = doc_of(b);
    assert_eq!(eval_number(&d, "/Defs/a-b"), 10.0);
    assert_eq!(eval_number(&d, "/Defs/a - /Defs/b"), 2.0);
    assert_eq!(eval_number(&d, "/Defs/a -/Defs/b"), 2.0);
    assert_eq!(eval_number(&d, "/Defs/v1.5"), 7.0);
}

#[test]
fn xpath_1_0_function_library_examples() {
    let d = empty_doc();
    for (e, want) in [
        ("string(substring('12345', 1.5, 2.6))", "234"),
        ("string(substring('12345', 0, 3))", "12"),
        ("string(translate('bar','abc','ABC'))", "BAr"),
        ("string(translate('--aaa--','abc-','ABC'))", "AAA"),
        ("string(round(1.5))", "2"),
        ("string(round(-1.5))", "-1"),
        ("string(floor(-0.5))", "-1"),
        ("string(ceiling(-0.5))", "0"),
        ("string(number(' 12 '))", "12"),
        ("string(boolean('x'))", "true"),
        ("string(1 = 1.0)", "true"),
        ("string(string-length(normalize-space(' a  b ')))", "3"),
    ] {
        assert_eq!(eval_string(&d, e), want, "{e}");
    }
}
