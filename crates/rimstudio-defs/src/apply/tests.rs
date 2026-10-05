//! Unit tests of patch application, over fictional defs (`RS_` names).

use std::collections::BTreeMap;

use proptest::prelude::*;
use rimstudio_core::diag::Severity;
use rimstudio_core::ids::{FileId, ModIdx};
use rimstudio_core::mods::ActiveSet;
use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_xpath::XPath;

use super::*;
use crate::custom_ops::{CustomPatchOp, SettingsConditional};
use crate::merge::merge;
use crate::patch_ops::parse_patches;
use crate::provenance::ModEntry;
use crate::types::{DefFile, FileContent};

fn order() -> ModOrder {
    ModOrder::new(vec![
        ModEntry::new(ModIdx(0), "rs.core", "RS Core"),
        ModEntry::new(ModIdx(1), "rs.other", "RS Other"),
    ])
}

fn def(name: &str) -> NodeBuilder {
    NodeBuilder::new("RS_T").text_elem("defName", name)
}

fn doc_of(defs: Vec<NodeBuilder>) -> UnifiedDoc {
    let file = DefFile {
        mod_idx: ModIdx(0),
        file: FileId(0),
        rel_path: "Defs/a.xml".into(),
        content: FileContent::parsed(NodeBuilder::new("Defs").children(defs).build()),
    };
    merge(&order(), &[file])
}

fn op(class: &str) -> NodeBuilder {
    NodeBuilder::new("Operation").attr("Class", class)
}

fn value(children: Vec<NodeBuilder>) -> NodeBuilder {
    NodeBuilder::new("value").children(children)
}

fn run_with(doc: &mut UnifiedDoc, ops: Vec<NodeBuilder>, ctx: &PatchContext) -> PatchReport {
    let nodes: Vec<Node> = ops.into_iter().map(NodeBuilder::build).collect();
    let parsed = parse_patches(&nodes);
    apply_patches(doc, &[(ModIdx(1), parsed)], ctx)
}

fn ctx() -> PatchContext {
    PatchContext::new(
        ActiveSet::from_ids(["rs.core", "rs.other"]),
        ["RS Core".to_owned(), "RS Other".to_owned()],
    )
}

fn run(doc: &mut UnifiedDoc, ops: Vec<NodeBuilder>) -> PatchReport {
    run_with(doc, ops, &ctx())
}

fn top(doc: &UnifiedDoc, i: usize) -> Node {
    let id = doc.top_level()[i];
    doc.arena().to_node(id).unwrap()
}

fn kids(n: &Node, tag: &str) -> Vec<String> {
    n.child(tag)
        .map(|c| c.elements().map(Node::text_content).collect())
        .unwrap_or_default()
}

const TARGET: &str = "/Defs/RS_T[defName=\"A\"]";

fn path(sub: &str) -> String {
    format!("{TARGET}{sub}")
}

#[test]
fn add_appends_and_prepend_keeps_the_order_of_the_value_children() {
    let mut doc = doc_of(vec![
        def("A").child(NodeBuilder::new("l").text_elem("li", "a")),
    ]);
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationAdd")
                .text_elem("xpath", path("/l"))
                .child(value(vec![NodeBuilder::new("li").text("b")])),
            op("PatchOperationAdd")
                .text_elem("xpath", path("/l"))
                .text_elem("order", "Prepend")
                .child(value(vec![
                    NodeBuilder::new("li").text("p1"),
                    NodeBuilder::new("li").text("p2"),
                ])),
        ],
    );
    assert_eq!(r.results(), vec![true, true]);
    assert_eq!(kids(&top(&doc, 0), "l"), vec!["p1", "p2", "a", "b"]);
}

#[test]
fn add_to_an_attribute_appends_or_prepends_text() {
    let mut doc = doc_of(vec![def("A").attr("Tag", "mid")]);
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationAdd")
                .text_elem("xpath", path("/@Tag"))
                .child(NodeBuilder::new("value").text("-end")),
            op("PatchOperationAdd")
                .text_elem("xpath", path("/@Tag"))
                .text_elem("order", "Prepend")
                .child(NodeBuilder::new("value").text("start-")),
        ],
    );
    assert_eq!(r.results(), vec![true, true]);
    assert_eq!(top(&doc, 0).attr("Tag"), Some("start-mid-end"));
    // an element value cannot go into an attribute
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationAdd")
                .text_elem("xpath", path("/@Tag"))
                .child(value(vec![NodeBuilder::new("x")])),
        ],
    );
    assert_eq!(r.results(), vec![false]);
    assert_eq!(r.exceptions(), 1);
}

#[test]
fn add_to_a_text_node_is_an_exception() {
    let mut doc = doc_of(vec![def("A")]);
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationAdd")
                .text_elem("xpath", path("/defName/text()"))
                .child(value(vec![NodeBuilder::new("x")])),
        ],
    );
    assert_eq!(r.results(), vec![false]);
    assert!(
        r.events[0]
            .error
            .as_deref()
            .unwrap_or("")
            .contains("text node")
    );
    assert_eq!(r.diagnostics.count(&diag_codes::PATCH_EXCEPTION), 1);
    assert_eq!(r.diagnostics.count(&diag_codes::PATCH_FAILED), 0);
}

#[test]
fn insert_reverses_several_value_children_in_both_orders() {
    let mut doc = doc_of(vec![
        def("A").child(
            NodeBuilder::new("l")
                .text_elem("li", "a")
                .text_elem("li", "b"),
        ),
    ]);
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationInsert")
                .text_elem("xpath", path("/l/li[1]"))
                .child(value(vec![
                    NodeBuilder::new("li").text("x"),
                    NodeBuilder::new("li").text("y"),
                ])),
            op("PatchOperationInsert")
                .text_elem("xpath", path("/l/li[last()]"))
                .text_elem("order", "Append")
                .child(value(vec![
                    NodeBuilder::new("li").text("m"),
                    NodeBuilder::new("li").text("n"),
                ])),
        ],
    );
    assert_eq!(r.results(), vec![true, true]);
    assert_eq!(kids(&top(&doc, 0), "l"), vec!["y", "x", "a", "b", "n", "m"]);
}

#[test]
fn insert_on_an_attribute_is_an_exception() {
    let mut doc = doc_of(vec![def("A").attr("Tag", "t")]);
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationInsert")
                .text_elem("xpath", path("/@Tag"))
                .child(value(vec![NodeBuilder::new("x")])),
        ],
    );
    assert_eq!(r.results(), vec![false]);
    assert_eq!(r.exceptions(), 1);
}

#[test]
fn replace_swaps_nodes_text_nodes_and_whole_defs() {
    let mut doc = doc_of(vec![
        def("A").text_elem("label", "old").text_elem("size", "1"),
        def("B"),
    ]);
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationReplace")
                .text_elem("xpath", path("/label"))
                .child(value(vec![NodeBuilder::new("label").text("new")])),
            op("PatchOperationReplace")
                .text_elem("xpath", path("/size/text()"))
                .child(NodeBuilder::new("value").text("2")),
            op("PatchOperationReplace")
                .text_elem("xpath", "/Defs/RS_T[defName=\"B\"]")
                .child(value(vec![def("B2"), def("B3")])),
        ],
    );
    assert_eq!(r.results(), vec![true, true, true]);
    let a = top(&doc, 0);
    assert_eq!(a.child_text("label"), Some("new"));
    assert_eq!(a.child_text("size"), Some("2"));
    let tops: Vec<String> = (0..doc.top_level().len())
        .map(|i| top(&doc, i).child_text("defName").unwrap_or("").to_owned())
        .collect();
    assert_eq!(tops, vec!["A", "B2", "B3"]);
    // the replaced defs have no mod: a patch created them
    assert_eq!(doc.origin_of(doc.top_level()[1]), None);
    assert!(doc.origin_of(doc.top_level()[0]).is_some());
}

#[test]
fn replace_with_an_empty_value_removes_the_target() {
    let mut doc = doc_of(vec![def("A").text_elem("label", "x")]);
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationReplace")
                .text_elem("xpath", path("/label"))
                .child(NodeBuilder::new("value")),
        ],
    );
    assert_eq!(r.results(), vec![true]);
    assert!(top(&doc, 0).child("label").is_none());
}

#[test]
fn remove_and_replace_survive_nested_targets_in_one_snapshot() {
    let mut doc = doc_of(vec![def("A").child(
        NodeBuilder::new("outer").child(NodeBuilder::new("outer").text_elem("leaf", "1")),
    )]);
    // selects the outer element and the inner one: the inner is detached with its parent
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationRemove").text_elem("xpath", "//outer"),
            op("PatchOperationReplace")
                .text_elem("xpath", "//RS_T")
                .child(value(vec![def("Z")])),
        ],
    );
    assert_eq!(r.results(), vec![true, true]);
    assert_eq!(r.exceptions(), 0);
    assert_eq!(top(&doc, 0).child_text("defName"), Some("Z"));
    doc.arena().check_integrity().unwrap();
}

#[test]
fn remove_of_an_attribute_node_is_an_exception_and_of_nothing_fails() {
    let mut doc = doc_of(vec![def("A").attr("Tag", "t")]);
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationRemove").text_elem("xpath", path("/@Tag")),
            op("PatchOperationRemove").text_elem("xpath", path("/none")),
        ],
    );
    assert_eq!(r.results(), vec![false, false]);
    assert_eq!(r.diagnostics.count(&diag_codes::PATCH_EXCEPTION), 1);
    assert_eq!(r.diagnostics.count(&diag_codes::PATCH_FAILED), 1);
    assert_eq!(top(&doc, 0).attr("Tag"), Some("t"));
}

#[test]
fn set_name_keeps_the_content_and_drops_the_attributes() {
    let mut doc = doc_of(vec![
        def("A").child(
            NodeBuilder::new("old")
                .attr("Class", "X")
                .child(NodeBuilder::new("b"))
                .text("text"),
        ),
    ]);
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationSetName")
                .text_elem("xpath", path("/old"))
                .text_elem("name", "fresh"),
        ],
    );
    assert_eq!(r.results(), vec![true]);
    let a = top(&doc, 0);
    let fresh = a.child("fresh").unwrap();
    assert!(fresh.attrs.is_empty());
    assert_eq!(fresh.children.len(), 2);
    assert!(a.child("old").is_none());
}

#[test]
fn a_renamed_top_level_node_has_no_mod_like_in_the_game() {
    let mut doc = doc_of(vec![def("A")]);
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationSetName")
                .text_elem("xpath", TARGET)
                .text_elem("name", "RS_U"),
        ],
    );
    assert_eq!(r.results(), vec![true]);
    let tops = doc.top_level();
    assert_eq!(tops.len(), 1);
    assert_eq!(doc.arena().name(tops[0]), Some("RS_U"));
    assert_eq!(doc.origin_of(tops[0]), None);
    let name = doc.arena().child_named(tops[0], "defName").unwrap();
    assert!(doc.origin_of(name).is_some(), "children keep their origin");
}

#[test]
fn set_name_with_an_invalid_name_is_an_exception_and_changes_nothing() {
    let mut doc = doc_of(vec![def("A").text_elem("old", "1")]);
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationSetName")
                .text_elem("xpath", path("/old"))
                .text_elem("name", "1 bad name"),
        ],
    );
    assert_eq!(r.results(), vec![false]);
    assert_eq!(r.exceptions(), 1);
    assert_eq!(top(&doc, 0).child_text("old"), Some("1"));
}

#[test]
fn attribute_operations_follow_their_truth_table() {
    let mut doc = doc_of(vec![def("A").attr("Name", "N").attr("Abstract", "False")]);
    let attr_op = |class: &str, name: &str, val: Option<&str>| {
        let mut b = op(class)
            .text_elem("xpath", TARGET)
            .text_elem("attribute", name);
        if let Some(v) = val {
            b = b.text_elem("value", v);
        }
        b
    };
    let r = run(
        &mut doc,
        vec![
            attr_op("PatchOperationAttributeAdd", "Tag", Some("1")),
            attr_op("PatchOperationAttributeAdd", "Tag", Some("2")),
            attr_op("PatchOperationAttributeSet", "Name", Some("N2")),
            attr_op("PatchOperationAttributeSet", "Extra", Some("e")),
            attr_op("PatchOperationAttributeRemove", "Abstract", None),
            attr_op("PatchOperationAttributeRemove", "Nope", None),
        ],
    );
    assert_eq!(r.results(), vec![true, false, true, true, true, false]);
    let a = top(&doc, 0);
    assert_eq!(
        a.attrs,
        vec![
            ("Name".to_owned(), "N2".to_owned()),
            ("Tag".to_owned(), "1".to_owned()),
            ("Extra".to_owned(), "e".to_owned()),
        ]
    );
}

#[test]
fn attribute_operations_on_text_nodes_and_without_a_name_are_exceptions() {
    let mut doc = doc_of(vec![def("A")]);
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationAttributeSet")
                .text_elem("xpath", path("/defName/text()"))
                .text_elem("attribute", "x")
                .text_elem("value", "1"),
            op("PatchOperationAttributeSet").text_elem("xpath", TARGET),
        ],
    );
    assert_eq!(r.results(), vec![false, false]);
    assert_eq!(r.exceptions(), 2);
}

#[test]
fn add_mod_extension_creates_the_container_at_the_end() {
    let mut doc = doc_of(vec![
        def("A"),
        def("B").child(NodeBuilder::new("modExtensions").child(NodeBuilder::new("li"))),
    ]);
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationAddModExtension")
                .text_elem("xpath", "/Defs/RS_T")
                .child(value(vec![NodeBuilder::new("li").attr("Class", "RS_Ext")])),
        ],
    );
    assert_eq!(r.results(), vec![true]);
    let a = top(&doc, 0);
    assert_eq!(
        a.elements().last().map(|e| e.tag.as_str()),
        Some("modExtensions")
    );
    assert_eq!(a.child("modExtensions").unwrap().elements().count(), 1);
    assert_eq!(
        top(&doc, 1)
            .child("modExtensions")
            .unwrap()
            .elements()
            .count(),
        2
    );
}

#[test]
fn conditional_follows_the_truth_table() {
    let mut doc = doc_of(vec![def("A")]);
    let test = |xpath: &str| {
        NodeBuilder::new("x")
            .attr("Class", "PatchOperationTest")
            .text_elem("xpath", xpath)
    };
    let branch = |tag: &str, child: &str| {
        NodeBuilder::new(tag)
            .attr("Class", "PatchOperationAdd")
            .text_elem("xpath", TARGET)
            .child(value(vec![NodeBuilder::new(child)]))
    };
    let hit = TARGET;
    let miss = "/Defs/RS_T[defName=\"Q\"]";
    let rename = |b: NodeBuilder, tag: &str| {
        let n = b.build();
        NodeBuilder::new(tag)
            .attrs(n.attrs.iter().cloned())
            .children(n.children)
    };
    let cond = |xpath: &str, children: Vec<NodeBuilder>| {
        op("PatchOperationConditional")
            .text_elem("xpath", xpath)
            .children(children)
    };
    let r = run(
        &mut doc,
        vec![
            cond(hit, vec![]),
            cond(miss, vec![rename(test(hit), "match")]),
            cond(hit, vec![rename(test(hit), "nomatch")]),
            cond(hit, vec![branch("match", "m1"), branch("nomatch", "n1")]),
            cond(miss, vec![branch("match", "m2"), branch("nomatch", "n2")]),
            cond(miss, vec![rename(test(miss), "nomatch")]),
        ],
    );
    // no branches: false; only match and no hit: true; only nomatch and a hit: true;
    // matching branches run; a failing nomatch branch is the result
    assert_eq!(r.results(), vec![false, true, true, true, true, false]);
    let a = top(&doc, 0);
    assert!(a.child("m1").is_some() && a.child("n2").is_some());
    assert!(a.child("n1").is_none() && a.child("m2").is_none());
}

#[test]
fn find_mod_matches_names_exactly_and_defaults_to_true() {
    let mut doc = doc_of(vec![def("A")]);
    let add = |tag: &str, child: &str| {
        NodeBuilder::new(tag)
            .attr("Class", "PatchOperationAdd")
            .text_elem("xpath", TARGET)
            .child(value(vec![NodeBuilder::new(child)]))
    };
    let mods = |names: &[&str]| {
        NodeBuilder::new("mods").children(
            names
                .iter()
                .map(|n| NodeBuilder::new("li").text(*n))
                .collect::<Vec<_>>(),
        )
    };
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationFindMod")
                .child(mods(&["RS Other"]))
                .child(add("match", "a")),
            op("PatchOperationFindMod")
                .child(mods(&["rs other"]))
                .child(add("match", "b"))
                .child(add("nomatch", "c")),
            op("PatchOperationFindMod")
                .child(mods(&["Nope"]))
                .child(add("match", "z")),
            op("PatchOperationFindMod").child(mods(&["RS Other"])),
            op("PatchOperationFindMod")
                .child(mods(&["Nope", "RS Core"]))
                .child(add("match", "e")),
        ],
    );
    assert_eq!(r.results(), vec![true; 5]);
    let a = top(&doc, 0);
    let added: Vec<&str> = a.elements().skip(1).map(|e| e.tag.as_str()).collect();
    assert_eq!(added, vec!["a", "c", "e"]);
}

#[test]
fn sequence_stops_at_the_first_false_without_rolling_back() {
    let mut doc = doc_of(vec![def("A")]);
    let add = |child: &str| {
        NodeBuilder::new("li")
            .attr("Class", "PatchOperationAdd")
            .text_elem("xpath", TARGET)
            .child(value(vec![NodeBuilder::new(child)]))
    };
    let test_miss = NodeBuilder::new("li")
        .attr("Class", "PatchOperationTest")
        .text_elem("xpath", "/Defs/RS_T[defName=\"Q\"]");
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationSequence").child(
                NodeBuilder::new("operations")
                    .child(add("s1"))
                    .child(test_miss)
                    .child(add("s2")),
            ),
        ],
    );
    assert_eq!(r.results(), vec![false]);
    assert_eq!(r.diagnostics.count(&diag_codes::PATCH_FAILED), 1);
    let a = top(&doc, 0);
    assert!(a.child("s1").is_some());
    assert!(a.child("s2").is_none());
}

#[test]
fn an_exception_in_a_nested_operation_aborts_the_sequence() {
    let mut doc = doc_of(vec![def("A")]);
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationSequence").child(
                NodeBuilder::new("operations")
                    .child(
                        NodeBuilder::new("li")
                            .attr("Class", "PatchOperationAdd")
                            .text_elem("xpath", TARGET),
                    )
                    .child(
                        NodeBuilder::new("li")
                            .attr("Class", "PatchOperationAdd")
                            .text_elem("xpath", TARGET)
                            .child(value(vec![NodeBuilder::new("never")])),
                    ),
            ),
            op("PatchOperationSequence"),
        ],
    );
    assert_eq!(r.results(), vec![false, false]);
    assert_eq!(r.exceptions(), 2);
    assert!(top(&doc, 0).child("never").is_none());
}

#[test]
fn success_modes_rewrite_the_result_after_the_work() {
    let mut doc = doc_of(vec![def("A")]);
    let add = |child: &str, success: &str| {
        op("PatchOperationAdd")
            .text_elem("xpath", TARGET)
            .child(value(vec![NodeBuilder::new(child)]))
            .text_elem("success", success)
    };
    let r = run(
        &mut doc,
        vec![
            add("a", "Invert"),
            add("b", "Never"),
            op("PatchOperationRemove")
                .text_elem("xpath", path("/zzz"))
                .text_elem("success", "Invert"),
            op("PatchOperationRemove")
                .text_elem("xpath", path("/zzz"))
                .text_elem("success", "Always"),
        ],
    );
    assert_eq!(r.results(), vec![false, false, true, true]);
    let a = top(&doc, 0);
    assert!(a.child("a").is_some() && a.child("b").is_some());
    assert_eq!(r.diagnostics.count(&diag_codes::PATCH_FAILED), 2);
}

#[test]
fn unknown_classes_fail_keep_their_raw_element_and_honour_success() {
    let mut doc = doc_of(vec![def("A")]);
    let r = run(
        &mut doc,
        vec![
            op("RS_Mod.RS_Op").text_elem("param", "7"),
            op("RS_Mod.RS_Op").text_elem("success", "Always"),
        ],
    );
    assert_eq!(r.results(), vec![false, true]);
    assert_eq!(r.diagnostics.count(&diag_codes::PATCH_BASE_CLASS), 2);
    let raw = r.events[0].raw.as_ref().unwrap();
    assert_eq!(raw.child_text("param"), Some("7"));
    assert_eq!(r.events[0].class, "RS_Mod.RS_Op");
}

struct Panics;
impl CustomPatchOp for Panics {
    fn class(&self) -> &str {
        "RS_Mod.Panics"
    }
    fn apply(&self, doc: &mut UnifiedDoc, _: &mut PatchCtx<'_>) -> Result<bool, PatchError> {
        // leave the document in a changed state before failing
        let root = doc.defs_root();
        let _ =
            doc.arena_mut()
                .append_element(root, "RS_Half", rimstudio_core::tree::OriginId::NONE);
        let empty: Vec<u8> = Vec::new();
        let first = empty.first().copied().unwrap();
        Ok(first > 0)
    }
}

struct Touches;
impl CustomPatchOp for Touches {
    fn class(&self) -> &str {
        "RS_Mod.Touches"
    }
    fn apply(&self, doc: &mut UnifiedDoc, ctx: &mut PatchCtx<'_>) -> Result<bool, PatchError> {
        let found = doc.find_defs("RS_T", "A");
        for id in found {
            let origin = ctx.origin(doc);
            let el = doc.edit().append_element(id, "marker", origin)?;
            ctx.touch(el);
            ctx.changed(id);
        }
        Ok(true)
    }
}

#[test]
fn a_panicking_operation_becomes_a_diagnostic_and_the_next_operation_runs() {
    let mut doc = doc_of(vec![def("A")]);
    let mut reg = CustomRegistry::new();
    reg.register(Panics);
    let ctx = ctx().with_custom(reg);
    let nodes: Vec<Node> = vec![
        op("RS_Mod.Panics").build(),
        op("PatchOperationAdd")
            .text_elem("xpath", TARGET)
            .child(value(vec![NodeBuilder::new("after")]))
            .build(),
    ];
    // parse with the registry so the class becomes Custom
    let env = ParseEnv {
        registry: &ctx.custom,
        active: &ctx.active,
    };
    let mut sink = rimstudio_core::diag::DiagSink::new();
    let ops: Vec<PatchOp> = nodes
        .iter()
        .map(|n| parse_operation(n, &env, Site::default(), None, &mut sink))
        .collect();
    assert!(matches!(ops[0].kind, PatchKind::Custom { .. }));
    assert!(sink.is_empty());
    let r = apply_patches(&mut doc, &[(ModIdx(1), ops)], &ctx);
    assert_eq!(r.results(), vec![false, true]);
    assert_eq!(r.diagnostics.count(&diag_codes::PATCH_EXCEPTION), 1);
    assert!(
        r.events[0]
            .error
            .as_deref()
            .unwrap_or("")
            .contains("panicked")
    );
    assert!(top(&doc, 0).child("after").is_some());
    doc.arena().check_integrity().unwrap();
}

#[test]
fn a_registered_handler_also_runs_for_operations_parsed_without_the_registry() {
    let mut doc = doc_of(vec![def("A")]);
    let mut reg = CustomRegistry::new();
    reg.register(Touches);
    let ctx = ctx().with_custom(reg);
    let r = run_with(&mut doc, vec![op("RS_Mod.Touches")], &ctx);
    assert_eq!(r.results(), vec![true]);
    assert!(top(&doc, 0).child("marker").is_some());
    assert_eq!(doc.patched_by(doc.top_level()[0]).len(), 1);
    assert_eq!(r.events[0].touched_defs, 1);
}

#[test]
fn settings_conditional_reads_the_supplied_setting() {
    let mut reg = CustomRegistry::new();
    reg.register(SettingsConditional::new(
        "RS_Mod.RS_SettingsConditional",
        BTreeMap::from([("rsOn".to_owned(), true), ("rsOff".to_owned(), false)]),
    ));
    let ctx = ctx().with_custom(reg);
    let branch = |tag: &str, child: &str| {
        NodeBuilder::new(tag)
            .attr("Class", "PatchOperationAdd")
            .text_elem("xpath", TARGET)
            .child(value(vec![NodeBuilder::new(child)]))
    };
    let cond = |setting: &str, with: Vec<NodeBuilder>| {
        op("RS_Mod.RS_SettingsConditional")
            .text_elem("settingName", setting)
            .children(with)
    };
    let mut doc = doc_of(vec![def("A")]);
    let nodes = vec![
        cond("rsOn", vec![branch("match", "t1"), branch("nomatch", "f1")]),
        cond(
            "rsOff",
            vec![branch("match", "t2"), branch("nomatch", "f2")],
        ),
        cond("rsOn", vec![]),
        cond("rsOff", vec![branch("match", "t4")]),
        cond("rsOn", vec![branch("nomatch", "f5")]),
        cond("rsMissing", vec![branch("match", "t6")]),
    ];
    let r = run_with(&mut doc, nodes, &ctx);
    assert_eq!(r.results(), vec![true, true, false, true, true, false]);
    assert_eq!(r.diagnostics.count(&diag_codes::PATCH_SETTING_MISSING), 1);
    let a = top(&doc, 0);
    let tags: Vec<&str> = a.elements().skip(1).map(|e| e.tag.as_str()).collect();
    assert_eq!(tags, vec!["t1", "f2"]);
}

#[test]
fn the_index_follows_edits_made_by_earlier_operations() {
    let mut doc = doc_of(vec![def("A"), def("B")]);
    let find = |name: &str| format!("/Defs/RS_T[defName=\"{name}\"]");
    let marker = |xp: String, tag: &str| {
        op("PatchOperationAdd")
            .text_elem("xpath", xp)
            .child(value(vec![NodeBuilder::new(tag)]))
    };
    let r = run(
        &mut doc,
        vec![
            // rename A through its defName text, then address it by the new name
            op("PatchOperationReplace")
                .text_elem("xpath", format!("{}/defName/text()", find("A")))
                .child(NodeBuilder::new("value").text("A2")),
            marker(find("A2"), "viaNew"),
            marker(find("A"), "viaOld"),
            // change the element type of B, then address it by the new type
            op("PatchOperationSetName")
                .text_elem("xpath", find("B"))
                .text_elem("name", "RS_U"),
            marker("/Defs/RS_U[defName=\"B\"]".to_owned(), "viaType"),
            marker(find("B"), "viaOldType"),
            // remove A2, then it cannot be found
            op("PatchOperationRemove").text_elem("xpath", find("A2")),
            marker(find("A2"), "gone"),
            // add a def by patch and address it
            op("PatchOperationAdd")
                .text_elem("xpath", "/Defs")
                .child(value(vec![def("C")])),
            marker(find("C"), "viaAdded"),
        ],
    );
    assert_eq!(
        r.results(),
        vec![
            true, true, false, true, true, false, true, false, true, true
        ]
    );
    assert_eq!(top(&doc, 0).tag, "RS_U");
    assert!(top(&doc, 0).child("viaType").is_some());
    assert!(top(&doc, 1).child("viaAdded").is_some());
    assert_eq!(doc.top_level().len(), 2);
}

#[test]
fn indexed_expressions_with_trailing_steps_and_unions_work() {
    let mut doc = doc_of(vec![
        def("A").text_elem("label", "a"),
        NodeBuilder::new("RS_U")
            .text_elem("defName", "A")
            .text_elem("label", "u"),
        def("B").text_elem("label", "b"),
    ]);
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationAttributeSet")
                .text_elem("xpath", "/Defs/RS_T[defName=\"A\" or defName=\"B\"]/label")
                .text_elem("attribute", "seen")
                .text_elem("value", "1"),
            op("PatchOperationAttributeSet")
                .text_elem(
                    "xpath",
                    "/Defs/RS_U[defName=\"A\"] | /Defs/RS_T[defName=\"B\"]",
                )
                .text_elem("attribute", "u")
                .text_elem("value", "1"),
            op("PatchOperationTest").text_elem("xpath", "/Defs/RS_T[defName=\"A\"]/label/@seen"),
        ],
    );
    assert_eq!(r.results(), vec![true, true, true]);
    assert_eq!(top(&doc, 0).child("label").unwrap().attr("seen"), Some("1"));
    assert_eq!(top(&doc, 1).attr("u"), Some("1"));
    assert_eq!(top(&doc, 1).child("label").unwrap().attr("seen"), None);
    assert_eq!(top(&doc, 2).attr("u"), Some("1"));
}

#[test]
fn patched_by_records_the_operation_for_the_touched_def_only() {
    let mut doc = doc_of(vec![def("A"), def("B")]);
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationAdd")
                .text_elem("xpath", TARGET)
                .child(value(vec![NodeBuilder::new("x")])),
            op("PatchOperationRemove").text_elem("xpath", path("/none")),
        ],
    );
    assert_eq!(r.results(), vec![true, false]);
    let tops = doc.top_level();
    let a = doc.patched_by(tops[0]);
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].op_index, 0);
    assert_eq!(a[0].class, "PatchOperationAdd");
    assert_eq!(a[0].mod_idx, ModIdx(1));
    assert_eq!(a[0].description, format!("PatchOperationAdd({TARGET})"));
    assert!(doc.patched_by(tops[1]).is_empty());
}

#[test]
fn nodes_a_patch_created_carry_the_patch_as_origin() {
    let mut doc = doc_of(vec![def("A")]);
    run(
        &mut doc,
        vec![
            op("PatchOperationAdd")
                .text_elem("xpath", TARGET)
                .child(value(vec![NodeBuilder::new("x").text_elem("y", "1")])),
        ],
    );
    let a = doc.top_level()[0];
    let x = doc.arena().child_named(a, "x").unwrap();
    let y = doc.arena().child_named(x, "y").unwrap();
    for id in [x, y] {
        assert_eq!(
            doc.origin_kind(id),
            Some(OriginKind::Patch {
                mod_idx: ModIdx(1),
                file: None,
                op_index: 0
            })
        );
    }
    assert!(
        doc.origin_kind(a)
            .is_some_and(|k| matches!(k, OriginKind::File { .. }))
    );
}

#[test]
fn xpath_problems_are_exceptions_with_the_expression_in_the_message() {
    let mut doc = doc_of(vec![def("A")]);
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationTest").text_elem("xpath", "///"),
            op("PatchOperationTest").text_elem("xpath", "count(//RS_T)"),
            op("PatchOperationTest"),
            op("PatchOperationTest").text_elem("xpath", "unknownfn()"),
        ],
    );
    assert_eq!(r.results(), vec![false; 4]);
    assert_eq!(r.exceptions(), 4);
    assert!(r.events[0].error.as_deref().unwrap_or("").contains("///"));
    assert!(
        r.events[1]
            .error
            .as_deref()
            .unwrap_or("")
            .contains("number")
    );
}

#[test]
fn emitted_diagnostics_point_at_the_mod_of_the_operation() {
    let mut doc = doc_of(vec![def("A")]);
    let r = run(
        &mut doc,
        vec![op("PatchOperationRemove").text_elem("xpath", path("/none"))],
    );
    let samples = r.diagnostics.samples(&diag_codes::PATCH_FAILED);
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].mod_idx, Some(ModIdx(1)));
    assert_eq!(samples[0].severity, Severity::Error);
}

// ---------------------------------------------------------------- the index against the evaluator

fn arb_defs() -> impl Strategy<Value = Vec<(u8, Vec<u8>)>> {
    // (type number, defName numbers): a def may carry several defName children
    prop::collection::vec((0u8..3, prop::collection::vec(0u8..6, 0..3)), 0..14)
}

fn build_defs(spec: &[(u8, Vec<u8>)]) -> Vec<NodeBuilder> {
    spec.iter()
        .map(|(t, names)| {
            let mut b = NodeBuilder::new(format!("RS_T{t}"));
            for n in names {
                b = b.text_elem("defName", format!("n{n}"));
            }
            b.text_elem("label", "x")
        })
        .collect()
}

proptest! {
    #[test]
    fn the_index_gives_the_same_nodes_as_the_xpath_evaluator(
        spec in arb_defs(),
        t1 in 0u8..3, n1 in 0u8..6, t2 in 0u8..3, n2 in 0u8..6,
        shape in 0usize..4,
    ) {
        let mut doc = doc_of(build_defs(&spec));
        let a = format!("RS_T{t1}[defName=\"n{n1}\"]");
        let b = format!("RS_T{t2}[defName=\"n{n2}\"]");
        let expr = match shape {
            0 => format!("Defs/{a}"),
            1 => format!("/Defs/{a}/label"),
            2 => format!("Defs/RS_T{t1}[defName=\"n{n1}\" or defName=\"n{n2}\"]"),
            _ => format!("Defs/{a} | /Defs/{b}"),
        };
        let xp = XPath::parse(&expr).unwrap();
        let root = doc.arena().root();
        let full: Vec<_> = xp.select_nodes(doc.arena(), root).unwrap();
        let indexed = select(&mut doc, Some(&expr)).unwrap();
        prop_assert_eq!(indexed, full);
    }

    #[test]
    fn the_index_stays_exact_after_random_edits(
        spec in arb_defs(),
        edits in prop::collection::vec((0u8..4, 0u8..6, 0u8..6, 0u8..3), 0..8),
    ) {
        let mut doc = doc_of(build_defs(&spec));
        for (kind, from, to, t) in edits {
            let find = format!("/Defs/RS_T{t}[defName=\"n{from}\"]");
            let node = match kind {
                0 => op("PatchOperationReplace")
                    .text_elem("xpath", format!("{find}/defName/text()"))
                    .child(NodeBuilder::new("value").text(format!("n{to}"))),
                1 => op("PatchOperationRemove").text_elem("xpath", find),
                2 => op("PatchOperationAdd")
                    .text_elem("xpath", "/Defs")
                    .child(value(vec![NodeBuilder::new(format!("RS_T{t}")).text_elem("defName", format!("n{to}"))])),
                _ => op("PatchOperationSetName")
                    .text_elem("xpath", find)
                    .text_elem("name", format!("RS_T{to}")),
            };
            run(&mut doc, vec![node]);
        }
        // compare every possible lookup with the evaluator
        for t in 0u8..3 {
            for n in 0u8..6 {
                let expr = format!("Defs/RS_T{t}[defName=\"n{n}\"]");
                let xp = XPath::parse(&expr).unwrap();
                let root = doc.arena().root();
                let full = xp.select_nodes(doc.arena(), root).unwrap();
                let indexed = select(&mut doc, Some(&expr)).unwrap();
                prop_assert_eq!(indexed, full, "{}", expr);
            }
        }
        doc.arena().check_integrity().unwrap();
    }
}

#[test]
fn a_def_replaced_inside_a_sequence_can_be_found_again_by_the_next_step_of_the_same_operation() {
    let mut doc = doc_of(vec![def("A").text_elem("label", "old"), def("B")]);
    let replace = |label: &str| {
        NodeBuilder::new("li")
            .attr("Class", "PatchOperationReplace")
            .text_elem("xpath", TARGET)
            .child(value(vec![def("A").text_elem("label", label)]))
    };
    let r = run(
        &mut doc,
        vec![
            op("PatchOperationSequence").child(
                NodeBuilder::new("operations")
                    .child(replace("first"))
                    .child(replace("second"))
                    .child(
                        NodeBuilder::new("li")
                            .attr("Class", "PatchOperationAdd")
                            .text_elem("xpath", TARGET)
                            .child(value(vec![NodeBuilder::new("extra")])),
                    ),
            ),
        ],
    );
    assert_eq!(r.results(), vec![true], "{:?}", r.events);
    assert_eq!(top(&doc, 0).child_text("label"), Some("second"));
    assert!(top(&doc, 0).child("extra").is_some());
}
