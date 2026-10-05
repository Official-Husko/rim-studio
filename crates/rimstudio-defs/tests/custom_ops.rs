//! Custom patch classes through the whole pipeline: the Combat Extended settings conditional as a
//! registered handler (settings supplied as data), and `MakeGunCECompatible` kept as an unknown
//! operation whose raw parameters stay readable. All names and numbers are fictional.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use rimstudio_core::ids::{FileId, ModIdx};
use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_defs::{
    CustomRegistry, DefFile, FileContent, LoadInput, ModEntry, PatchFile, SettingsConditional,
    TypeInfo, TypeTable, diag_codes, load,
};

const CE_SETTINGS_CONDITIONAL_CLASS: &str = "CombatExtended.PatchOperationSettingsConditional";
const MAKE_GUN_CLASS: &str = "CombatExtended.PatchOperationMakeGunCECompatible";

fn types() -> Arc<TypeTable> {
    Arc::new(
        TypeTable::new(vec![
            ("Verse.Def".into(), TypeInfo::with_base("Verse.Editable")),
            ("Verse.RS_ThingDef".into(), TypeInfo::with_base("Verse.Def")),
        ])
        .unwrap(),
    )
}

fn input(patches: Vec<NodeBuilder>, registry: CustomRegistry) -> LoadInput {
    let mut i = LoadInput::new(
        vec![
            ModEntry::new(ModIdx(0), "ludeon.rimworld", "RS Core"),
            ModEntry::new(ModIdx(1), "rs.combat", "RS Combat"),
        ],
        types(),
    );
    i.def_files = vec![DefFile {
        mod_idx: ModIdx(0),
        file: FileId(0),
        rel_path: "Defs/a.xml".into(),
        content: FileContent::parsed(
            NodeBuilder::new("Defs")
                .child(
                    NodeBuilder::new("RS_ThingDef")
                        .text_elem("defName", "RS_TestRifle")
                        .text_elem("label", "test rifle"),
                )
                .build(),
        ),
    }];
    i.patch_files = vec![PatchFile {
        mod_idx: ModIdx(1),
        file: FileId(1),
        rel_path: "Patches/p.xml".into(),
        content: FileContent::parsed(NodeBuilder::new("Patch").children(patches).build()),
    }];
    i.custom_ops = registry;
    i
}

fn set_label(label: &str) -> NodeBuilder {
    NodeBuilder::new("Operation")
        .attr("Class", "PatchOperationReplace")
        .text_elem("xpath", "Defs/RS_ThingDef[defName=\"RS_TestRifle\"]/label")
        .child(NodeBuilder::new("value").text_elem("label", label))
}

fn conditional(setting: &str, on: &str, off: &str) -> NodeBuilder {
    NodeBuilder::new("Operation")
        .attr("Class", CE_SETTINGS_CONDITIONAL_CLASS)
        .text_elem("settingName", setting)
        .child(rename(set_label(on), "match"))
        .child(rename(set_label(off), "nomatch"))
}

fn rename(b: NodeBuilder, tag: &str) -> NodeBuilder {
    let n: Node = b.build();
    NodeBuilder::new(tag)
        .attrs(n.attrs.iter().cloned())
        .children(n.children)
}

fn registry(flag: bool) -> CustomRegistry {
    let mut r = CustomRegistry::new();
    r.register(SettingsConditional::new(
        CE_SETTINGS_CONDITIONAL_CLASS,
        BTreeMap::from([("rsFlag".to_owned(), flag)]),
    ));
    r
}

#[test]
fn the_settings_conditional_picks_the_branch_of_the_supplied_value() {
    for (flag, want) in [(true, "on"), (false, "off")] {
        let out = load(input(
            vec![conditional("rsFlag", "on", "off")],
            registry(flag),
        ));
        assert_eq!(out.patch_report.results(), vec![true]);
        let d = out.get("RS_ThingDef", "RS_TestRifle").unwrap();
        assert_eq!(d.node.child_text("label"), Some(want));
        assert_eq!(out.diagnostics.count(&diag_codes::PATCH_UNKNOWN_CLASS), 0);
        assert_eq!(d.patched_by.len(), 1);
        assert_eq!(d.patched_by[0].class, CE_SETTINGS_CONDITIONAL_CLASS);
    }
}

#[test]
fn a_missing_setting_is_reported_and_the_operation_fails() {
    let out = load(input(
        vec![conditional("rsOther", "on", "off")],
        registry(true),
    ));
    assert_eq!(out.patch_report.results(), vec![false]);
    assert_eq!(out.diagnostics.count(&diag_codes::PATCH_SETTING_MISSING), 1);
    let d = out.get("RS_ThingDef", "RS_TestRifle").unwrap();
    assert_eq!(d.node.child_text("label"), Some("test rifle"));
}

#[test]
fn without_a_handler_the_settings_conditional_stays_unknown() {
    let out = load(input(
        vec![conditional("rsFlag", "on", "off")],
        CustomRegistry::new(),
    ));
    assert_eq!(out.patch_report.results(), vec![false]);
    assert_eq!(out.diagnostics.count(&diag_codes::PATCH_UNKNOWN_CLASS), 1);
    assert_eq!(out.diagnostics.count(&diag_codes::PATCH_BASE_CLASS), 1);
    assert!(out.patch_report.events[0].raw.is_some());
}

#[test]
fn make_gun_compatible_stays_unknown_with_its_raw_parameters() {
    let op = NodeBuilder::new("Operation")
        .attr("Class", MAKE_GUN_CLASS)
        .text_elem("defName", "RS_TestRifle")
        .child(
            NodeBuilder::new("statBases")
                .text_elem("RS_Bulk", "7")
                .text_elem("RS_Recoil", "1.5"),
        )
        .child(NodeBuilder::new("FireModes").text_elem("aiUseBurstMode", "true"));
    // with a handler registered for another class the operation is still unknown
    let out = load(input(vec![op], registry(true)));
    assert_eq!(out.patch_report.results(), vec![false]);
    let event = &out.patch_report.events[0];
    assert_eq!(event.class, MAKE_GUN_CLASS);
    let raw = event.raw.as_ref().unwrap();
    assert_eq!(raw.child_text("defName"), Some("RS_TestRifle"));
    assert_eq!(
        raw.find("statBases/RS_Bulk")
            .map(Node::text_content)
            .as_deref(),
        Some("7")
    );
    assert_eq!(
        raw.find("FireModes/aiUseBurstMode")
            .map(Node::text_content)
            .as_deref(),
        Some("true")
    );
    // the def itself is untouched (the conversion is not an XML edit)
    let d = out.get("RS_ThingDef", "RS_TestRifle").unwrap();
    assert_eq!(d.node.children.len(), 2);
}

#[test]
fn an_unreadable_patch_file_is_reported_and_skipped() {
    let mut i = input(vec![set_label("patched")], CustomRegistry::new());
    i.patch_files.push(PatchFile {
        mod_idx: ModIdx(1),
        file: FileId(2),
        rel_path: "Patches/broken.xml".into(),
        content: FileContent::failed("unexpected end of file"),
    });
    let out = load(i);
    assert_eq!(out.patch_report.results(), vec![true]);
    assert_eq!(out.diagnostics.count(&diag_codes::PATCH_FILE_UNREADABLE), 1);
    assert_eq!(out.diagnostics.count(&diag_codes::XML_PARSE_ERROR), 1);
}

#[test]
fn patches_apply_mod_by_mod_in_load_order_whatever_the_file_order() {
    let mut i = input(
        vec![set_label("from the second mod")],
        CustomRegistry::new(),
    );
    // a patch file of the first mod listed after the second mod's file still runs first
    i.patch_files.push(PatchFile {
        mod_idx: ModIdx(0),
        file: FileId(3),
        rel_path: "Patches/core.xml".into(),
        content: FileContent::parsed(
            NodeBuilder::new("Patch")
                .child(set_label("from the first mod"))
                .build(),
        ),
    });
    let out = load(i);
    let d = out.get("RS_ThingDef", "RS_TestRifle").unwrap();
    assert_eq!(d.node.child_text("label"), Some("from the second mod"));
    assert_eq!(out.patch_report.events[0].mod_idx, ModIdx(0));
    assert_eq!(d.patched_by.len(), 2);
    assert_eq!(d.patched_by[0].mod_idx, ModIdx(0));
}
