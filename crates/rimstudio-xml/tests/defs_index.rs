//! The streaming def indexer against the tree reader.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use proptest::prelude::*;
use rimstudio_core::tree::Node;
use rimstudio_xml::defs_scan::{DefIndexRecord, index_file, index_file_detailed};
use rimstudio_xml::render::{RenderOpts, render_defs_file};
use rimstudio_xml::{ParseMode, parse_document};

#[test]
fn indexes_the_attributes_and_the_first_def_name_and_label() {
    let xml = "\u{FEFF}<?xml version=\"1.0\"?>\n<Defs>\n  <!-- c -->\n  <ThingDef Name=\"RS_Base\" Abstract=\"True\">\n    <label>base</label>\n  </ThingDef>\n  <ThingDef ParentName=\"RS_Base\" Class=\"RS.Thing\" MayRequire=\"rs.mod\">\n    <defName> RS_Gun </defName>\n    <label>R&amp;D <![CDATA[gun]]></label>\n    <defName>RS_Second</defName>\n    <comps><li><defName>nested</defName></li></comps>\n  </ThingDef>\n  <RecipeDef/>\n</Defs>";
    let recs = index_file(xml.as_bytes());
    assert_eq!(recs.len(), 3);
    assert_eq!(recs[0].def_type, "ThingDef");
    assert_eq!(recs[0].name.as_deref(), Some("RS_Base"));
    assert!(recs[0].is_abstract);
    assert_eq!(recs[0].label.as_deref(), Some("base"));
    assert_eq!(recs[0].def_name, None);
    assert_eq!(recs[1].def_name.as_deref(), Some("RS_Gun"));
    assert_eq!(recs[1].parent_name.as_deref(), Some("RS_Base"));
    assert_eq!(recs[1].class.as_deref(), Some("RS.Thing"));
    assert_eq!(recs[1].may_require.as_deref(), Some("rs.mod"));
    assert_eq!(recs[1].label.as_deref(), Some("R&D gun"));
    assert_eq!(recs[2].def_type, "RecipeDef");
    let off = recs[1].offset as usize;
    assert!(xml.as_bytes()[off..].starts_with(b"<ThingDef ParentName"));
}

#[test]
fn truncated_and_dtd_files_report_how_they_ended() {
    let cut = index_file_detailed(
        b"<Defs><ThingDef><defName>RS_A</defName></ThingDef><ThingDef><defName>RS_B",
    );
    assert_eq!(cut.records.len(), 2);
    assert_eq!(cut.records[1].def_name.as_deref(), Some("RS_B"));
    assert!(cut.error.is_some());
    let dtd = index_file_detailed(b"<!DOCTYPE x><Defs><ThingDef/></Defs>");
    assert_eq!(dtd.error.unwrap().code(), "xml.dtd-rejected");
    let bad = index_file_detailed(b"<Defs><ThingDef a=\"1\" a=\"2\"/><x></Defs>");
    assert!(!bad.records.is_empty());
    assert!(index_file(b"").is_empty());
    assert!(index_file(b"plain text").is_empty());
}

#[test]
fn a_root_that_is_not_defs_is_still_indexed_and_reported() {
    let d = index_file_detailed(b"<Other><ThingDef><defName>a</defName></ThingDef></Other>");
    assert_eq!(d.root_tag.as_deref(), Some("Other"));
    assert_eq!(d.records.len(), 1);
}

/// What a record must say according to the tree reader.
fn expected(node: &Node) -> DefIndexRecord {
    let text_of = |tag: &str| {
        node.child(tag)
            .map(|n| n.text_content().trim().to_owned())
            .filter(|s| !s.is_empty())
    };
    DefIndexRecord {
        def_type: node.tag.clone(),
        class: node.attr("Class").map(str::to_owned),
        def_name: text_of("defName"),
        name: node.attr("Name").map(str::to_owned),
        parent_name: node.attr("ParentName").map(str::to_owned),
        is_abstract: node
            .attr("Abstract")
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("true")),
        label: text_of("label"),
        may_require: node.attr("MayRequire").map(str::to_owned),
        offset: 0,
    }
}

fn text_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_ .&<>'\"\u{e9}\u{4e2d}-]{1,16}"
}

fn name_strategy() -> impl Strategy<Value = String> {
    "[A-Za-z][A-Za-z0-9_]{0,10}"
}

fn def_strategy() -> impl Strategy<Value = Node> {
    (
        prop::sample::select(vec!["ThingDef", "RecipeDef", "StatDef", "RS_CustomDef"]),
        prop::option::of(name_strategy()),
        prop::option::of(name_strategy()),
        prop::option::of(prop::sample::select(vec![
            "true", "True", "false", " TRUE ",
        ])),
        prop::option::of(text_strategy()),
        prop::option::of(text_strategy()),
        prop::collection::vec((name_strategy(), text_strategy()), 0..4),
        prop::option::of(name_strategy()),
    )
        .prop_map(|(tag, name, parent, abs, def_name, label, extra, class)| {
            let mut n = Node::new(tag);
            if let Some(v) = name {
                n.set_attr("Name", v);
            }
            if let Some(v) = parent {
                n.set_attr("ParentName", v);
            }
            if let Some(v) = abs {
                n.set_attr("Abstract", v);
            }
            if let Some(v) = class {
                n.set_attr("Class", v);
            }
            if let Some(v) = def_name {
                n.push_child(Node::with_text("defName", v));
            }
            for (tag, text) in extra {
                n.push_child(Node::with_text(format!("x{tag}"), text));
            }
            if let Some(v) = label {
                n.push_child(Node::with_text("label", v));
            }
            n
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn the_index_agrees_with_a_full_parse_on_generated_corpora(defs in prop::collection::vec(def_strategy(), 0..12), crlf in any::<bool>(), bom in any::<bool>(), tabs in any::<bool>()) {
        let opts = RenderOpts {
            eol: if crlf { rimstudio_xml::render::LineEnding::Crlf } else { rimstudio_xml::render::LineEnding::Lf },
            bom,
            indent: if tabs { rimstudio_xml::render::Indent::Tab } else { rimstudio_xml::render::Indent::Spaces(2) },
            ..RenderOpts::default()
        };
        let text = render_defs_file(&defs, &opts);
        let tree = parse_document(text.as_bytes(), ParseMode::Game).unwrap();
        let tops: Vec<&Node> = tree.root.elements().collect();
        let recs = index_file(text.as_bytes());
        prop_assert_eq!(recs.len(), tops.len());
        for (rec, node) in recs.iter().zip(&tops) {
            let mut want = expected(node);
            want.offset = rec.offset;
            prop_assert_eq!(rec, &want);
            let open = format!("<{}", node.tag);
            prop_assert!(text.as_bytes()[rec.offset as usize..].starts_with(open.as_bytes()));
        }
    }
}
