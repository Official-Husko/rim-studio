//! JSON shape, validation and helper behaviour of the owned node tree.

mod tree_support;

use proptest::prelude::*;
use rimstudio_core::tree::{Child, Node, NodeBuilder, TreeError};
use rstest::rstest;
use serde_json::json;
use std::collections::HashSet;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn json_round_trip_is_identity(n in tree_support::node()) {
        prop_assert_eq!(n.validate(), Ok(()));
        let compact = Node::from_json_str(&n.to_json_string());
        prop_assert_eq!(compact.as_ref(), Ok(&n));
        let pretty = Node::from_json_str(&n.to_json_pretty());
        prop_assert_eq!(pretty.as_ref(), Ok(&n));
        let value = Node::from_json_value(n.to_json_value());
        prop_assert_eq!(value.as_ref(), Ok(&n));
    }

    #[test]
    fn serialisation_is_deterministic_and_hash_matches_eq(n in tree_support::node()) {
        let copy = n.clone();
        prop_assert_eq!(n.to_json_string(), copy.to_json_string());
        prop_assert_eq!(n.content_hash(), copy.content_hash());
        let mut set = HashSet::new();
        set.insert(n);
        prop_assert!(set.contains(&copy));
    }

    #[test]
    fn different_trees_have_different_content_hashes(a in tree_support::node(), b in tree_support::node()) {
        prop_assume!(a != b);
        prop_assert_ne!(a.content_hash(), b.content_hash());
    }
}

#[test]
fn written_shape_is_exact() {
    let n = NodeBuilder::new("li")
        .attr("Class", "RS_Comp")
        .text("hi")
        .child(Node::new("e"))
        .build();
    let v = n.to_json_value();
    assert_eq!(
        v,
        json!({"tag": "li", "attrs": [["Class", "RS_Comp"]], "children": ["hi", {"tag": "e", "attrs": [], "children": []}]})
    );
    let keys: Vec<_> = v
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    assert_eq!(keys, vec!["tag", "attrs", "children"]);
}

#[test]
fn attrs_and_children_may_be_absent_on_read() {
    let n = Node::from_json_str(r#"{"tag":"a"}"#).expect("parse");
    assert_eq!(n, Node::new("a"));
    let n = Node::from_json_str(r#"{"tag":"a","children":["x"]}"#).expect("parse");
    assert_eq!(n.leaf_text(), Some("x"));
}

#[rstest]
#[case::unknown_field(r#"{"tag":"a","oops":1}"#, "tree.json")]
#[case::missing_tag(r#"{"attrs":[]}"#, "tree.json")]
#[case::not_an_object(r#"[1]"#, "tree.json")]
#[case::bad_attr_shape(r#"{"tag":"a","attrs":[["k"]]}"#, "tree.json")]
#[case::bad_child(r#"{"tag":"a","children":[1]}"#, "tree.json")]
#[case::duplicate_field(r#"{"tag":"a","tag":"b"}"#, "tree.json")]
#[case::syntax(r#"{"tag":"#, "tree.json")]
#[case::bad_tag(r#"{"tag":"a b"}"#, "tree.invalid_name")]
#[case::empty_tag(r#"{"tag":""}"#, "tree.invalid_name")]
#[case::bad_attr_name(r#"{"tag":"a","attrs":[["1x","v"]]}"#, "tree.invalid_name")]
#[case::duplicate_attr(r#"{"tag":"a","attrs":[["k","1"],["k","2"]]}"#, "tree.duplicate_attr")]
#[case::bad_char_text(r#"{"tag":"a","children":["x\u0001"]}"#, "tree.invalid_char")]
#[case::bad_char_attr(r#"{"tag":"a","attrs":[["k","\u0000"]]}"#, "tree.invalid_char")]
#[case::nested_bad_tag(r#"{"tag":"a","children":[{"tag":"<"}]}"#, "tree.invalid_name")]
fn invalid_json_reports_a_stable_code(#[case] json: &str, #[case] code: &str) {
    let err = Node::from_json_str(json).expect_err("must be rejected");
    assert_eq!(err.code(), code, "{err}");
}

#[test]
fn validate_reports_the_location() {
    let n = NodeBuilder::new("Defs")
        .child(Node::new("ThingDef"))
        .child(NodeBuilder::new("ThingDef").child(Node::new("bad name")))
        .build();
    match n.validate() {
        Err(TreeError::InvalidName { path, name, .. }) => {
            assert_eq!(name, "bad name");
            assert_eq!(path, "Defs/ThingDef[2]/bad name");
        }
        other => panic!("unexpected {other:?}"),
    }
    let dup = Node {
        tag: "a".into(),
        attrs: vec![("k".into(), "1".into()), ("k".into(), "2".into())],
        children: vec![],
    };
    assert_eq!(
        dup.validate().expect_err("dup").code(),
        "tree.duplicate_attr"
    );
    let text = Node {
        tag: "a".into(),
        attrs: vec![],
        children: vec![Child::Text("\u{1}".into())],
    };
    assert_eq!(
        text.validate().expect_err("char").code(),
        "tree.invalid_char"
    );
}

#[test]
fn depth_limit_is_enforced_without_recursion() {
    let mut n = Node::new("leaf");
    for _ in 0..(rimstudio_core::tree::MAX_DEPTH + 10) {
        n = Node {
            tag: "w".into(),
            attrs: vec![],
            children: vec![Child::Element(n)],
        };
    }
    let err = n.validate().expect_err("too deep");
    assert_eq!(err.code(), "tree.too_deep");
    assert_eq!(n.depth(), rimstudio_core::tree::MAX_DEPTH + 11);
}

fn sample() -> Node {
    NodeBuilder::new("ThingDef")
        .attr("ParentName", "RS_Base")
        .text_elem("defName", "RS_TestRifle")
        .elem("comps", |b| {
            b.elem("li", |l| l.attr("Class", "RS_A").text_elem("v", "1"))
                .elem("li", |l| l.attr("Class", "RS_B").text_elem("v", "2"))
        })
        .text_elem("label", "test ")
        .build()
}

#[test]
fn lookup_helpers() {
    let mut n = sample();
    assert_eq!(n.attr("ParentName"), Some("RS_Base"));
    assert!(n.has_attr("ParentName") && !n.has_attr("Nope"));
    assert_eq!(n.child_text("defName"), Some("RS_TestRifle"));
    assert_eq!(n.child("comps").map(|c| c.elements().count()), Some(2));
    assert_eq!(
        n.child("comps").map(|c| c.children_named("li").count()),
        Some(2)
    );
    assert_eq!(n.text_content(), "RS_TestRifle12test ");
    assert_eq!(n.element_count(), 8);
    assert_eq!(n.depth(), 4);

    assert_eq!(
        n.set_attr("ParentName", "RS_Other"),
        Some("RS_Base".to_owned())
    );
    assert_eq!(n.set_attr("Abstract", "True"), None);
    assert_eq!(
        n.attrs,
        vec![
            ("ParentName".into(), "RS_Other".into()),
            ("Abstract".into(), "True".into())
        ]
    );
    assert_eq!(n.remove_attr("ParentName"), Some("RS_Other".to_owned()));
    assert_eq!(n.remove_attr("ParentName"), None);

    n.set_child_text("label", "new");
    n.set_child_text("tags", "t");
    assert_eq!(n.child_text("label"), Some("new"));
    assert_eq!(n.child_text("tags"), Some("t"));
    assert_eq!(
        n.remove_child("tags").map(|c| c.tag),
        Some("tags".to_owned())
    );
    assert!(n.child("tags").is_none());
    if let Some(c) = n.child_mut("defName") {
        c.children.clear();
    }
    assert_eq!(n.child_text("defName"), None);
}

#[rstest]
#[case("comps/li/v", Some("1"))]
#[case("comps/li[2]/v", Some("2"))]
#[case("comps/li[3]/v", None)]
#[case("comps/*/v", Some("1"))]
#[case("/comps/li[2]/v/", Some("2"))]
#[case("comps/li[0]", None)]
#[case("comps/li[x]", None)]
#[case("comps/[1]", None)]
#[case("nothing", None)]
fn find_by_simple_path(#[case] path: &str, #[case] text: Option<&str>) {
    let n = sample();
    assert_eq!(n.find(path).and_then(Node::leaf_text), text, "{path}");
}

#[test]
fn find_all_and_find_mut() {
    let mut n = sample();
    let vs: Vec<_> = n
        .find_all("comps/li/v")
        .iter()
        .filter_map(|v| v.leaf_text())
        .collect();
    assert_eq!(vs, vec!["1", "2"]);
    assert_eq!(n.find("").map(|x| x.tag.as_str()), Some("ThingDef"));
    if let Some(v) = n.find_mut("comps/li[2]/v") {
        v.children = vec![Child::Text("9".into())];
    }
    assert_eq!(n.find("comps/li[2]/v").and_then(Node::leaf_text), Some("9"));
    assert!(n.find_mut("comps/li[7]").is_none());
}

#[test]
fn find_backtracks_across_siblings() {
    let n = NodeBuilder::new("r")
        .child(NodeBuilder::new("a").empty_elem("x"))
        .child(NodeBuilder::new("a").text_elem("y", "found"))
        .build();
    assert_eq!(n.find("a/y").and_then(Node::leaf_text), Some("found"));
}

#[test]
fn builder_conveniences() {
    let n = NodeBuilder::new("t")
        .attr("k", "1")
        .attr("k", "2")
        .attr_opt("o", Some("x"))
        .attr_opt("p", None::<&str>)
        .attrs([("m", "1"), ("n", "2")])
        .li("a")
        .li_each(["b", "c"])
        .text_elem_opt("e", Some("z"))
        .text_elem_opt("f", None::<&str>)
        .empty_elem("g")
        .when(true, |b| b.text("tail"))
        .when(false, |b| b.text("never"))
        .children(vec![Node::new("h")])
        .build();
    assert_eq!(n.attrs.len(), 4);
    assert_eq!(n.attr("k"), Some("2"));
    assert_eq!(n.children_named("li").count(), 3);
    assert!(n.child("f").is_none());
    assert_eq!(
        n.children
            .last()
            .and_then(Child::as_element)
            .map(|e| e.tag.as_str()),
        Some("h")
    );
    assert!(NodeBuilder::new("bad tag").try_build().is_err());
    assert!(NodeBuilder::new("ok").try_build().is_ok());
    let as_node: Node = NodeBuilder::new("n").into();
    assert_eq!(as_node, Node::new("n"));
}

#[test]
fn attribute_order_matters_for_equality() {
    let a = NodeBuilder::new("a").attr("x", "1").attr("y", "2").build();
    let b = NodeBuilder::new("a").attr("y", "2").attr("x", "1").build();
    assert_ne!(a, b);
    assert_ne!(a.content_hash(), b.content_hash());
}

#[test]
fn display_is_compact_json() {
    let n = Node::with_text("a", "b");
    assert_eq!(n.to_string(), r#"{"tag":"a","attrs":[],"children":["b"]}"#);
}

/// Finds every maximal JSON object that looks like a node in a document.
fn collect_nodes<'a>(v: &'a serde_json::Value, out: &mut Vec<&'a serde_json::Value>) {
    match v {
        serde_json::Value::Object(o) if o.contains_key("tag") && o.contains_key("children") => {
            out.push(v)
        }
        serde_json::Value::Object(o) => o.values().for_each(|c| collect_nodes(c, out)),
        serde_json::Value::Array(a) => a.iter().for_each(|c| collect_nodes(c, out)),
        _ => {}
    }
}

#[test]
fn research_vector_node_shape_deserialises() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/research/data/def-engine/golden/vector_outputs.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("research vectors not present, skipping");
        return;
    };
    let doc: serde_json::Value = serde_json::from_str(&text).expect("vectors are JSON");
    let mut nodes = Vec::new();
    collect_nodes(&doc, &mut nodes);
    assert!(!nodes.is_empty(), "the vectors contain node trees");
    for v in nodes {
        let node = Node::from_json_value(v.clone()).expect("vector node decodes");
        assert_eq!(
            &node.to_json_value(),
            v,
            "re-encoding reproduces the vector"
        );
        assert_eq!(node.validate(), Ok(()));
    }
}
