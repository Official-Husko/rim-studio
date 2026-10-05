//! Path, axis, predicate and node-set behaviour on fictional documents.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use rimstudio_core::tree::{NodeBuilder, NodeKind};
use rimstudio_xpath::{XPath, XPathError};
use rstest::rstest;

#[test]
fn relative_and_absolute_paths_start_at_the_document() {
    let d = defs_doc();
    assert_eq!(count(&d, "Defs/RS_ThingDef"), 7);
    assert_eq!(count(&d, "/Defs/RS_ThingDef"), 7);
    assert_eq!(count(&d, "RS_ThingDef"), 0);
    assert_eq!(count(&d, "*/RS_ThingDef"), 7);
    assert_eq!(count(&d, "/"), 1);
    assert_eq!(select(&d, "/"), vec!["#doc"]);
}

#[test]
fn whitespace_and_newlines_are_trimmed_and_tolerated() {
    let d = defs_doc();
    assert_eq!(
        count(&d, "\n  Defs/RS_ThingDef[ defName =\n 'RS_Knife' ]\t\n"),
        1
    );
    let xp = XPath::parse("  Defs \n").unwrap();
    assert_eq!(xp.as_str(), "Defs");
}

#[rstest]
#[case("Defs/RS_ThingDef[defName=\"RS_RifleA\"]", 2)]
#[case("Defs/RS_ThingDef[defName='RS_Knife']", 1)]
#[case("Defs/RS_ThingDef[defName=\"RS_Nope\"]", 0)]
#[case("Defs/RS_ThingDef[defName=\"RS_RifleA\" or defName=\"RS_Knife\"]", 3)]
#[case("Defs/RS_ThingDef[defName=\"RS_RifleA\" and label=\"duplicate\"]", 1)]
#[case("Defs/RS_ThingDef[@Abstract=\"True\"]", 1)]
#[case("Defs/RS_ThingDef[@Name]", 1)]
#[case("Defs/RS_ThingDef[not(defName)]", 2)]
#[case("Defs/RS_ThingDef[defName]", 5)]
#[case("Defs/RS_ThingDef[label=\"\"]", 1)]
#[case("Defs/RS_ThingDef[comps/li/@Class=\"RS_CompB\"]", 1)]
#[case("Defs/RS_ThingDef[comps/li[@Class=\"RS_CompA\"]]", 2)]
#[case("Defs/RS_ThingDef[statBases/Mass > 3]", 2)]
#[case("Defs/RS_ThingDef[statBases/Mass >= 4]", 1)]
#[case("Defs/RS_ThingDef[statBases/Mass < 2]", 1)]
#[case("Defs/RS_ThingDef[defName != \"RS_RifleA\"]", 3)]
#[case("Defs/RS_ThingDef[contains(defName, \"Rifle\")]", 3)]
#[case("Defs/RS_ThingDef[starts-with(defName, \"RS_R\")]", 3)]
#[case("Defs/RS_ThingDef[string-length(defName) = 8]", 2)]
#[case("Defs/*[defName = \"RS_Make\"]", 1)]
#[case("Defs/RS_ThingDef[defName = \"RS_RifleA\"]/statBases/Mass", 1)]
#[case("Defs/RS_ThingDef/statBases/*", 4)]
#[case("Defs/RS_ThingDef/tools/li[power > 8]", 2)]
#[case("Defs/RS_ThingDef/tools/li[power > 8]/label/text()", 2)]
fn predicate_shapes(#[case] expr: &str, #[case] want: usize) {
    assert_eq!(count(&defs_doc(), expr), want, "{expr}");
}

#[test]
fn positional_predicates_use_proximity_position() {
    let d = defs_doc();
    assert_eq!(
        select(&d, "Defs/RS_ThingDef[2]"),
        vec!["RS_ThingDef#RS_RifleA"]
    );
    assert_eq!(
        select(&d, "Defs/RS_ThingDef[last()]"),
        vec!["RS_ThingDef#RS_Empty"]
    );
    assert_eq!(count(&d, "Defs/RS_ThingDef[8]"), 0);
    assert_eq!(count(&d, "Defs/RS_ThingDef[0]"), 0);
    assert_eq!(count(&d, "Defs/RS_ThingDef[1.5]"), 0);
    assert_eq!(count(&d, "Defs/RS_ThingDef[position() > 6]"), 1);
    assert_eq!(count(&d, "Defs/RS_ThingDef[position() = last() - 1]"), 1);
    // The second predicate counts again among the survivors of the first.
    assert_eq!(
        select(&d, "Defs/RS_ThingDef[defName][2]"),
        vec!["RS_ThingDef#RS_RifleB"]
    );
    assert_eq!(
        select(&d, "Defs/RS_ThingDef[2][defName]"),
        vec!["RS_ThingDef#RS_RifleA"]
    );
}

#[test]
fn double_slash_positions_apply_per_parent() {
    let d = defs_doc();
    // every li that is the first li of its parent
    assert_eq!(count(&d, "//li[1]"), 8);
    // the first li of the whole document
    assert_eq!(select(&d, "(//li)[1]").len(), 1);
    assert_eq!(count(&d, "//li"), 13);
    assert_eq!(count(&d, "Defs//li"), count(&d, "//li"));
    assert_eq!(count(&d, "//weaponTags/li[last()]"), 2);
}

#[test]
fn text_and_attribute_nodes() {
    let d = defs_doc();
    assert_eq!(
        select(&d, "Defs/RS_ThingDef[defName=\"RS_Knife\"]/label/text()"),
        vec!["text:knife"]
    );
    assert_eq!(count(&d, "Defs/RS_ThingDef/label/text()"), 5);
    assert_eq!(count(&d, "Defs/RS_ThingDef/@*"), 3);
    assert_eq!(
        select(&d, "Defs/RS_ThingDef/@ParentName"),
        vec!["@ParentName=RS_BaseGun"]
    );
    assert_eq!(count(&d, "Defs/RS_ThingDef/node()"), 20);
    assert_eq!(count(&d, "Defs/comment()"), 0);
    assert_eq!(count(&d, "Defs/processing-instruction()"), 0);
}

#[test]
fn select_rejects_attribute_results_but_select_nodes_allows_them() {
    let d = defs_doc();
    let xp = XPath::parse("Defs/RS_ThingDef/@Name").unwrap();
    assert_eq!(xp.select(&d).err(), Some(XPathError::AttributeResult));
    assert_eq!(xp.select_nodes(&d, d.root()).unwrap().len(), 1);
}

#[test]
fn non_node_set_results_are_errors_for_select() {
    let d = defs_doc();
    for (expr, found) in [
        ("count(Defs)", "number"),
        ("string(Defs)", "string"),
        ("true()", "boolean"),
        ("1 = 1", "boolean"),
    ] {
        let e = XPath::parse(expr).unwrap().select(&d).err();
        assert_eq!(e, Some(XPathError::NotNodeSet { found }), "{expr}");
    }
}

#[test]
fn axes_from_a_context_node() {
    let d = book_doc();
    let ch = XPath::parse("/doc/chapter[2]").unwrap().select(&d).unwrap()[0];
    let rel = |expr: &str| -> Vec<String> {
        let xp = XPath::parse(expr).unwrap();
        xp.select_nodes(&d, ch)
            .unwrap()
            .into_iter()
            .map(|n| describe(&d, n))
            .collect()
    };
    assert_eq!(rel("self::chapter").len(), 1);
    assert_eq!(rel("parent::*"), vec!["doc"]);
    assert_eq!(rel("ancestor::*"), vec!["doc",]);
    assert_eq!(rel("ancestor-or-self::*"), vec!["doc", "chapter"]);
    assert_eq!(
        rel("ancestor-or-self::node()"),
        vec!["#doc", "doc", "chapter"]
    );
    assert_eq!(rel("following-sibling::chapter").len(), 1);
    assert_eq!(rel("preceding-sibling::chapter").len(), 1);
    assert_eq!(rel("following-sibling::*").len(), 1 + 3);
    assert_eq!(rel("preceding-sibling::*").len(), 1);
    assert_eq!(rel("child::title"), vec!["title:Body"]);
    assert_eq!(rel("descendant::para").len(), 3);
    assert_eq!(rel("descendant-or-self::*").len(), 9);
    // preceding excludes ancestors, following excludes descendants
    assert_eq!(rel("preceding::para").len(), 7);
    assert_eq!(rel("following::para").len(), 1);
    assert_eq!(rel("following::employee").len(), 3);
    assert_eq!(rel(".."), vec!["doc"]);
    assert_eq!(rel("../chapter[1]/title"), vec!["title:Introduction"]);
    assert_eq!(rel("."), vec!["chapter"]);
}

#[test]
fn reverse_axes_count_positions_nearest_first() {
    let d = book_doc();
    // the nearest ancestor div of d1 is ancestor::*[1]
    let p = XPath::parse("//div/para").unwrap().select(&d).unwrap()[0];
    let r = |expr: &str| -> Vec<String> {
        XPath::parse(expr)
            .unwrap()
            .select_nodes(&d, p)
            .unwrap()
            .into_iter()
            .map(|n| describe(&d, n))
            .collect()
    };
    assert_eq!(r("ancestor::*[1]"), vec!["div"]);
    assert_eq!(r("ancestor::*[2]"), vec!["chapter"]);
    assert_eq!(r("ancestor::*[last()]"), vec!["doc"]);
    assert_eq!(r("preceding::chapter[1]/title"), vec!["title:Body"]);
    assert_eq!(r("preceding::title[1]"), vec!["title:Conclusion"]);
    assert_eq!(r("preceding::title[last()]"), vec!["title:Introduction"]);
    // results are always handed back in document order
    assert_eq!(r("ancestor::*"), vec!["doc", "chapter", "div"]);
}

#[test]
fn attribute_context_axes() {
    let d = book_doc();
    let attr = XPath::parse("/doc/employee[1]/@secretary")
        .unwrap()
        .select_nodes(&d, d.root())
        .unwrap()[0];
    let owner = attr.attr_owner().unwrap();
    // the attribute node is not a handle, so evaluate through the owner and compare
    let from_owner = XPath::parse("@secretary/..")
        .unwrap()
        .select_from(&d, owner)
        .unwrap();
    assert_eq!(from_owner, vec![owner]);
    assert_eq!(count(&d, "/doc/employee/@secretary/parent::employee"), 2);
    assert_eq!(count(&d, "/doc/employee/@*/ancestor::doc"), 1);
    assert_eq!(count(&d, "/doc/employee[1]/@name/following::employee"), 2);
    assert_eq!(count(&d, "/doc/employee[1]/@name/preceding::title"), 3);
    assert_eq!(count(&d, "/doc/employee/@*/following-sibling::*"), 0);
    assert_eq!(count(&d, "/doc/employee/@*/child::node()"), 0);
}

#[test]
fn attribute_nodes_follow_their_element_in_document_order() {
    let d = book_doc();
    let got = select(
        &d,
        "/doc/employee[1] | /doc/employee[1]/@secretary | /doc/employee[1]/@name | /doc/chapter[3]",
    );
    assert_eq!(
        got,
        vec!["chapter", "employee", "@name=RS_Ann", "@secretary=RS_Bo"]
    );
}

#[test]
fn unions_are_in_document_order_without_duplicates() {
    let d = defs_doc();
    let got = select(
        &d,
        "Defs/RS_RecipeDef | Defs/RS_ThingDef[defName='RS_Knife'] | Defs/RS_ThingDef[defName='RS_Knife']",
    );
    assert_eq!(got, vec!["RS_ThingDef#RS_Knife", "RS_RecipeDef#RS_Make"]);
    assert_eq!(count(&d, "(Defs/RS_ThingDef | Defs/RS_ThingDef)/label"), 6);
    assert_eq!(count(&d, "Defs/RS_ThingDef/.. | Defs/RS_RecipeDef/.."), 1);
    assert_eq!(count(&d, "Defs/RS_ThingDef/../RS_RecipeDef"), 1);
}

#[test]
fn parent_steps_deduplicate() {
    let d = defs_doc();
    assert_eq!(count(&d, "Defs/RS_ThingDef/statBases/Mass/../.."), 3);
    assert_eq!(count(&d, "//li/.."), 8);
    assert_eq!(
        count(&d, "//*/.."),
        count(&d, "/descendant-or-self::node()[child::*]")
    );
}

#[test]
fn filter_expressions_and_function_bases() {
    let d = defs_doc();
    assert_eq!(
        select(&d, "(Defs/RS_ThingDef)[2]"),
        vec!["RS_ThingDef#RS_RifleA"]
    );
    assert_eq!(select(&d, "(Defs/RS_ThingDef)[last()]/defName").len(), 1);
    assert_eq!(count(&d, "(Defs/RS_ThingDef[defName])[position() <= 2]"), 2);
    assert_eq!(
        count(&d, "(/Defs/RS_ThingDef | /Defs/RS_RecipeDef)/defName"),
        6
    );
    assert_eq!(count(&d, "id('RS_Knife')/label"), 0);
}

#[test]
fn string_literal_as_node_set_is_a_type_error() {
    let d = defs_doc();
    let xp = XPath::parse("'abc'/x").unwrap();
    assert_eq!(xp.select(&d).err().map(|e| e.code()), Some("xpath.type"));
    let xp = XPath::parse("count('abc')").unwrap();
    assert_eq!(
        xp.eval_document(&d).err().map(|e| e.code()),
        Some("xpath.type")
    );
    let xp = XPath::parse("Defs | 1").unwrap();
    assert_eq!(xp.select(&d).err().map(|e| e.code()), Some("xpath.type"));
    let xp = XPath::parse("'x'[1]").unwrap();
    assert_eq!(xp.select(&d).err().map(|e| e.code()), Some("xpath.type"));
}

#[test]
fn unsupported_and_rejected_constructs_have_stable_codes() {
    let d = defs_doc();
    let xp = XPath::parse("Defs/namespace::*").unwrap();
    assert_eq!(
        xp.select(&d).err().map(|e| e.code()),
        Some("xpath.unsupported-axis")
    );
    let xp = XPath::parse("lang('en')").unwrap();
    assert_eq!(
        xp.eval_document(&d).err().map(|e| e.code()),
        Some("xpath.unsupported")
    );
    assert_eq!(
        XPath::parse("$x").err().map(|e| e.code()),
        Some("xpath.unbound-variable")
    );
    assert_eq!(
        XPath::parse("a:b").err().map(|e| e.code()),
        Some("xpath.unbound-prefix")
    );
    assert_eq!(
        XPath::parse("foo()").err().map(|e| e.code()),
        Some("xpath.unknown-function")
    );
    assert_eq!(
        XPath::parse("not()").err().map(|e| e.code()),
        Some("xpath.arity")
    );
    assert_eq!(
        XPath::parse("{EggsToPatch}").err().map(|e| e.code()),
        Some("xpath.parse")
    );
    assert_eq!(
        XPath::parse("").err().map(|e| e.code()),
        Some("xpath.parse")
    );
}

#[test]
fn stale_context_is_an_error() {
    let mut d = defs_doc();
    let id = d.document_element().unwrap();
    let child = d.first_child(id).unwrap();
    d.remove(child).unwrap();
    let xp = XPath::parse("a").unwrap();
    assert_eq!(
        xp.select_from(&d, child).err(),
        Some(XPathError::StaleContext)
    );
}

#[test]
fn evaluation_sees_mutations() {
    let mut d = defs_doc();
    let xp = XPath::parse("Defs/RS_ThingDef[defName='RS_New']").unwrap();
    assert_eq!(xp.select(&d).unwrap().len(), 0);
    let defs = d.document_element().unwrap();
    let el = d
        .append_element(defs, "RS_ThingDef", rimstudio_core::tree::OriginId::NONE)
        .unwrap();
    let dn = d
        .append_element(el, "defName", rimstudio_core::tree::OriginId::NONE)
        .unwrap();
    d.append_text(dn, "RS_New", rimstudio_core::tree::OriginId::NONE)
        .unwrap();
    assert_eq!(xp.select(&d).unwrap(), vec![el]);
    // a node moved to the front sorts first
    let first = d.first_child(defs).unwrap();
    d.detach(el).unwrap();
    d.insert_before(first, el).unwrap();
    let all = XPath::parse("Defs/RS_ThingDef")
        .unwrap()
        .select(&d)
        .unwrap();
    assert_eq!(all.first().copied(), Some(el));
    assert_eq!(all.len(), 8);
}

#[test]
fn detached_trees_evaluate_relative_and_absolute_paths_inside_themselves() {
    let mut d = defs_doc();
    let frag = d
        .import(
            &NodeBuilder::new("li")
                .text_elem("a", "1")
                .text_elem("a", "2")
                .build(),
            rimstudio_core::tree::OriginId::NONE,
        )
        .unwrap();
    let xp = XPath::parse("a").unwrap();
    assert_eq!(xp.select_from(&d, frag).unwrap().len(), 2);
    let abs = XPath::parse("/li/a").unwrap();
    assert_eq!(abs.select_from(&d, frag).unwrap().len(), 0);
    let up = XPath::parse("a/..").unwrap();
    assert_eq!(up.select_from(&d, frag).unwrap(), vec![frag]);
    let d_up = XPath::parse("a[2]/preceding-sibling::a | a[1] | a[2]").unwrap();
    assert_eq!(d_up.select_from(&d, frag).unwrap().len(), 2);
    let root_kind = d.kind(frag);
    assert_eq!(root_kind, Some(NodeKind::Element));
}

#[test]
fn xmlns_declarations_are_not_attribute_nodes() {
    let b = NodeBuilder::new("Defs").elem("A", |b| {
        b.attr("xmlns", "urn:x")
            .attr("xmlns:p", "urn:p")
            .attr("k", "v")
    });
    let d = doc_of(b);
    assert_eq!(select(&d, "Defs/A/@*"), vec!["@k=v"]);
    assert_eq!(select(&d, "Defs/A[@xmlns]").len(), 0);
}

#[test]
fn select_from_matches_select_for_the_document_node() {
    let d = defs_doc();
    let xp = XPath::parse("Defs/RS_ThingDef[defName]").unwrap();
    assert_eq!(
        xp.select(&d).unwrap(),
        xp.select_from(&d, d.root()).unwrap()
    );
    let doc_el = d.document_element().unwrap();
    assert_eq!(
        XPath::parse("RS_ThingDef[defName]")
            .unwrap()
            .select_from(&d, doc_el)
            .unwrap(),
        xp.select(&d).unwrap()
    );
}

#[test]
fn display_of_a_parsed_expression_parses_to_the_same_expression() {
    for src in [
        "Defs/RS_ThingDef[defName=\"x\"]/statBases",
        "//li[@Class='A'][not(x)]",
        "(a | b)[1]/c//d",
        "a[b = c or d != 'e'][position() mod 2 = 0]",
        "-(1 + 2) * 3 div 4",
        "ancestor-or-self::node()/following::text()",
        "a/.//b",
        "..//a/../@b",
    ] {
        let one = XPath::parse(src).unwrap();
        let two = XPath::parse(&one.to_string()).unwrap();
        assert_eq!(one.expr(), two.expr(), "{src}");
    }
}
