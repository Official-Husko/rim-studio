//! Flow C of the designer over a fictional install: clone and adjust (IT-012), the diff against the source
//! and the structure defaults of a new weapon. Every name starts with `RS_` and every number is invented.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;
mod common_project;

use std::sync::Arc;

use camino::Utf8PathBuf;
use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_design::model::{
    CalibrationMode, CostEntry, DesignSpec, Draft, ProjectileChoice, ScalarField, ValueSource,
};
use rimstudio_io::roots::DataRoots;
use rimstudio_ipc_types::designer::{
    DesignerCloneDiffRequest, DesignerCloneRequest, DesignerDraftListRequest,
    DesignerPreviewRequest, DesignerStructureDefaultsRequest, DraftDto, ReadoutDeltaDto,
};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_testing::install_tree::InstallBuilder;
use rimstudio_toolkit::designer::dto::{draft_from_dto, draft_to_dto};
use rimstudio_toolkit::designer::{
    Ctx, clone_diff, clone_draft, draft_list, draft_load, export_plan, preview, structure_defaults,
};
use rimstudio_toolkit::error::ToolkitError;

const PROJECT: &str = "p-clone";

fn abstract_base(name: &str, stats: &[(&str, &str)]) -> Node {
    NodeBuilder::new("ThingDef")
        .attr("Name", name)
        .attr("Abstract", "True")
        .text_elem("category", "Item")
        .when(!stats.is_empty(), |b| {
            b.elem("statBases", |s| {
                stats.iter().fold(s, |s, (k, v)| s.text_elem(*k, *v))
            })
        })
        .build()
}

fn with_parent(mut node: Node, parent: &str) -> Node {
    node.set_attr("ParentName", parent);
    node
}

/// A gun of the fixture family that inherits two stats from an abstract base and declares one itself.
fn parent_gun() -> Node {
    let mut gun = with_parent(
        common::gun("RS_ParentGun", "RS_Shot05", 5, "Industrial", "RS_Rifle"),
        "RS_BaseGun",
    );
    if let Some(stats) = gun.child_mut("statBases") {
        stats.push_child(Node::with_text("RS_Own", "7"));
    }
    gun.push_child(Node::with_text("description", "a fictional rifle"));
    gun.push_child(Node::with_text("soundInteract", "RS_Click"));
    gun
}

/// A melee weapon made of stuff.
fn stuffed_blade() -> Node {
    let mut blade = with_parent(
        common::melee("RS_StuffBlade", 9, "Industrial", "Cut"),
        "RS_BaseMelee",
    );
    blade.push_child(
        NodeBuilder::new("stuffCategories")
            .li("RS_Metallic")
            .build(),
    );
    blade.push_child(Node::with_text("costStuffCount", "60"));
    blade
}

/// A gun that carries a conversion of the fictional Combat Extended in its own verb.
fn converted_gun() -> Node {
    let mut gun = common::gun("RS_ConvGun", "RS_Shot02", 2, "Industrial", "RS_Rifle");
    if let Some(verbs) = gun.child_mut("verbs")
        && let Some(li) = verbs.elements_mut().next()
    {
        li.set_attr("Class", "CombatExtended.VerbPropertiesCE");
    }
    gun
}

fn fixture() -> common::Fixture {
    let mut defs = common::core_defs_n(14, 8);
    defs.push(abstract_base(
        "RS_BaseGun",
        &[("RS_Flammability", "0.5"), ("RS_Durability", "80")],
    ));
    defs.push(abstract_base("RS_BaseMelee", &[]));
    defs.push(parent_gun());
    defs.push(stuffed_blade());
    defs.push(converted_gun());
    let install = InstallBuilder::new()
        .core_defs_file("RS_Core.xml", defs)
        .mod_folder(common::extra_mod())
        .mod_folder(common::ce_mod())
        .build_temp()
        .unwrap();
    let session = common::open_session(&install, false);
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let roots = DataRoots::under_base(&base);
    roots.ensure_all().unwrap();
    let clock = FakeClock::new(FakeClock::DEFAULT_START_MS);
    let ctx = Ctx::new(session.clone(), &roots, Arc::new(clock.clone())).unwrap();
    common::Fixture {
        install,
        session,
        tmp,
        roots,
        clock,
        ctx,
    }
}

fn request(source: &str, name: &str) -> DesignerCloneRequest {
    DesignerCloneRequest {
        project_id: PROJECT.into(),
        source: source.into(),
        def_name: name.into(),
        label: None,
        mod_prefix: None,
    }
}

fn cloned(f: &common::Fixture, source: &str, name: &str) -> Draft {
    let response = clone_draft(&f.ctx, request(source, name)).unwrap();
    draft_from_dto(&response.entry.draft).unwrap()
}

fn edit(draft: &Draft, change: impl FnOnce(&mut DesignSpec)) -> DraftDto {
    let mut next = draft.clone();
    change(&mut next.spec);
    draft_to_dto(&next).unwrap()
}

fn diff_of(
    f: &common::Fixture,
    draft: DraftDto,
) -> rimstudio_ipc_types::designer::DesignerCloneDiffResponse {
    clone_diff(&f.ctx, DesignerCloneDiffRequest { draft }).unwrap()
}

fn readout<'a>(
    diff: &'a rimstudio_ipc_types::designer::DesignerCloneDiffResponse,
    key: &str,
) -> &'a ReadoutDeltaDto {
    diff.readouts
        .iter()
        .find(|r| r.key == key)
        .unwrap_or_else(|| panic!("no readout {key}"))
}

// ---------------------------------------------------------------------------------------------------
// clone
// ---------------------------------------------------------------------------------------------------

#[test]
fn a_ranged_clone_copies_every_field_of_the_source_as_anchor_values() {
    let f = fixture();
    let draft = cloned(&f, "RS_ParentGun", "RS_CloneGun");
    let spec = &draft.spec;
    assert_eq!(spec.identity.def_name, "RS_CloneGun");
    assert_eq!(spec.identity.label, "rs clone gun");
    assert_eq!(spec.identity.description, "a fictional rifle");
    assert_eq!(spec.parent.as_ref().unwrap().def_name, "RS_BaseGun");
    assert_eq!(spec.tech_level.map(|t| t.xml_name()), Some("Industrial"));
    assert_eq!(spec.mass.unwrap().value, 3.0);
    assert_eq!(spec.mass.unwrap().source, ValueSource::Anchor);
    assert_eq!(spec.work_to_make.unwrap().value, 9500.0);
    assert_eq!(spec.cost_list, vec![CostEntry::new("RS_Steel", 30.0)]);
    assert_eq!(spec.weapon_tags, vec!["RS_Rifle"]);
    let r = spec.ranged.as_ref().unwrap();
    assert_eq!(r.damage.unwrap().value, 13.0);
    assert_eq!(r.damage.unwrap().source, ValueSource::Anchor);
    assert_eq!(r.range.unwrap().value, 25.5);
    assert!((r.warmup.unwrap().value - 1.15).abs() < 1e-9);
    assert_eq!(r.cooldown.unwrap().value, 1.5);
    assert_eq!(r.accuracy.touch.unwrap().value, 0.7);
    assert!(r.accuracy.long.is_some());
    assert_eq!(
        r.projectile,
        Some(ProjectileChoice::Reference("RS_Shot05".into()))
    );
    assert_eq!(spec.tools.len(), 1);
    assert_eq!(spec.tools[0].label, "grip");
    assert_eq!(spec.tools[0].capacities, vec!["Blunt"]);
    assert_eq!(spec.tools[0].power.unwrap().value, 8.0);
    assert_eq!(spec.tools[0].cooldown_time.unwrap().value, 2.0);
    assert!(spec.role.is_some());
}

#[test]
fn inherited_stats_stay_inherited_and_only_declared_ones_are_copied() {
    let f = fixture();
    let draft = cloned(&f, "RS_ParentGun", "RS_CloneGun");
    let spec = &draft.spec;
    assert_eq!(
        spec.extra_stats.keys().collect::<Vec<_>>(),
        vec!["RS_Own"],
        "only the stat the def declares itself is an own stat"
    );
    let inherited = &spec.parent.as_ref().unwrap().inherited_stats;
    assert_eq!(inherited.get("RS_Flammability"), Some(&0.5));
    assert_eq!(inherited.get("RS_Durability"), Some(&80.0));
}

#[test]
fn a_clone_records_the_source_as_first_anchor_and_in_the_draft_and_is_anchored() {
    let f = fixture();
    let draft = cloned(&f, "RS_ParentGun", "RS_CloneGun");
    assert_eq!(draft.cloned_from.as_deref(), Some("RS_ParentGun"));
    assert_eq!(draft.anchors.first().unwrap().def_name, "RS_ParentGun");
    assert_eq!(
        draft.anchors.first().unwrap().label.as_deref(),
        Some("rs_parentgun")
    );
    assert_eq!(draft.calibration, CalibrationMode::Anchored);
}

#[test]
fn a_melee_clone_copies_tools_stuff_and_parent() {
    let f = fixture();
    let draft = cloned(&f, "RS_StuffBlade", "RS_CloneBlade");
    let spec = &draft.spec;
    assert_eq!(spec.kind, rimstudio_design::model::ItemKind::Melee);
    assert!(spec.ranged.is_none());
    assert_eq!(spec.parent.as_ref().unwrap().def_name, "RS_BaseMelee");
    let stuff = spec.stuff.as_ref().unwrap();
    assert_eq!(stuff.categories, vec!["RS_Metallic"]);
    assert_eq!(stuff.count.unwrap().value, 60.0);
    assert_eq!(stuff.count.unwrap().source, ValueSource::Anchor);
    assert_eq!(spec.tools.len(), 1);
    assert_eq!(spec.tools[0].capacities, vec!["Cut"]);
    assert_eq!(spec.tools[0].power.unwrap().value, 7.0 + 1.1 * 9.0);
    assert_eq!(spec.weapon_tags, vec!["RS_Melee"]);
    assert_eq!(draft.cloned_from.as_deref(), Some("RS_StuffBlade"));
}

#[test]
fn a_clone_is_vanilla_even_when_combat_extended_is_part_of_the_setup() {
    let f = fixture();
    // a second session with the fictional Combat Extended next to the vanilla main one
    let ce = common::open_session(&f.install, true);
    let ctx = f.ctx.with_ce_session(ce);
    assert!(ctx.require_engine().unwrap().ce_available());
    let response = clone_draft(&ctx, request("RS_ParentGun", "RS_CloneGun")).unwrap();
    let draft = draft_from_dto(&response.entry.draft).unwrap();
    assert!(draft.spec.ce.is_none(), "the CE toggle stays off");
    assert!(!draft.ce_patch_enabled());
    let preview = preview(
        &ctx,
        DesignerPreviewRequest {
            draft: response.entry.draft,
        },
    )
    .unwrap();
    assert!(preview.readouts.iter().all(|r| !r.key.starts_with("ce-")));
}

#[test]
fn a_source_with_a_combat_extended_conversion_is_refused_with_a_plain_reason() {
    let f = fixture();
    let err = clone_draft(&f.ctx, request("RS_ConvGun", "RS_CloneGun")).unwrap_err();
    assert_eq!(err.code(), "designer.invalid-draft");
    let text = err.to_string();
    assert!(
        text.contains("Combat Extended") && text.contains("RS_ConvGun"),
        "{text}"
    );
    assert!(
        draft_list(
            &f.ctx,
            DesignerDraftListRequest {
                project_id: PROJECT.into()
            }
        )
        .unwrap()
        .drafts
        .is_empty(),
        "nothing is stored for a refused clone"
    );
}

#[test]
fn a_source_that_is_not_loaded_says_so() {
    let f = fixture();
    let err = clone_draft(&f.ctx, request("RS_NoSuchGun", "RS_CloneGun")).unwrap_err();
    assert!(
        matches!(err, ToolkitError::ReferenceUnavailable { .. }),
        "{err}"
    );
    assert!(err.to_string().contains("RS_NoSuchGun"));
}

#[test]
fn a_def_that_is_not_a_weapon_cannot_be_cloned() {
    let f = fixture();
    let err = clone_draft(&f.ctx, request("RS_ArmorA", "RS_CloneGun")).unwrap_err();
    assert_eq!(err.code(), "design.invalid-input");
}

#[test]
fn without_a_game_install_a_clone_reports_no_reference_data() {
    let (_tmp, ctx, _clock) = common::offline();
    let err = clone_draft(&ctx, request("RS_ParentGun", "RS_CloneGun")).unwrap_err();
    assert_eq!(err.code(), "designer.reference-unavailable");
}

#[test]
fn invalid_new_names_are_refused_and_nothing_is_stored() {
    let f = fixture();
    for (name, why) in [
        ("", "an empty name"),
        ("RS Clone Gun", "a space"),
        ("RS_Clone/Gun", "a slash"),
        ("RS_Gun01", "a name a loaded def already uses"),
        ("RS_ParentGun", "the source's own name"),
    ] {
        let err = clone_draft(&f.ctx, request("RS_ParentGun", name)).unwrap_err();
        assert_eq!(err.code(), "designer.invalid-draft", "{why}: {err}");
        assert!(err.to_string().contains("new name"), "{why}: {err}");
    }
    let listed = draft_list(
        &f.ctx,
        DesignerDraftListRequest {
            project_id: PROJECT.into(),
        },
    )
    .unwrap();
    assert!(listed.drafts.is_empty());
}

#[test]
fn a_bad_project_id_is_refused() {
    let f = fixture();
    let mut req = request("RS_ParentGun", "RS_CloneGun");
    req.project_id = "Not Valid".into();
    assert_eq!(
        clone_draft(&f.ctx, req).unwrap_err().code(),
        "designer.invalid-draft"
    );
}

#[test]
fn the_mod_prefix_is_applied_and_recorded() {
    let f = fixture();
    let mut req = request("RS_ParentGun", "CloneGun");
    req.mod_prefix = Some("RSM".into());
    let draft = draft_from_dto(&clone_draft(&f.ctx, req).unwrap().entry.draft).unwrap();
    assert_eq!(draft.spec.identity.def_name, "RSM_CloneGun");
    assert_eq!(draft.spec.identity.mod_prefix, "RSM");
    assert_eq!(draft.spec.identity.label, "clone gun");
    // a name that already has the prefix is kept as it is
    let mut again = request("RS_ParentGun", "RSM_Other");
    again.mod_prefix = Some("RSM".into());
    let second = draft_from_dto(&clone_draft(&f.ctx, again).unwrap().entry.draft).unwrap();
    assert_eq!(second.spec.identity.def_name, "RSM_Other");
}

#[test]
fn an_explicit_label_is_used() {
    let f = fixture();
    let mut req = request("RS_ParentGun", "RS_CloneGun");
    req.label = Some("  Fancy Rifle ".into());
    let draft = draft_from_dto(&clone_draft(&f.ctx, req).unwrap().entry.draft).unwrap();
    assert_eq!(draft.spec.identity.label, "Fancy Rifle");
}

#[test]
fn the_clone_is_stored_and_reopens_identical() {
    let f = fixture();
    let response = clone_draft(&f.ctx, request("RS_ParentGun", "RS_CloneGun")).unwrap();
    let loaded = draft_load(&f.ctx, PROJECT, &response.entry.id).unwrap();
    assert_eq!(loaded.draft, response.entry.draft);
    assert_eq!(loaded.def_name, "RS_CloneGun");
    assert_eq!(loaded.draft.cloned_from.as_deref(), Some("RS_ParentGun"));
    let listed = draft_list(
        &f.ctx,
        DesignerDraftListRequest {
            project_id: PROJECT.into(),
        },
    )
    .unwrap();
    assert_eq!(listed.drafts.len(), 1);
}

#[test]
fn clones_of_the_same_source_are_byte_identical_apart_from_the_draft_id() {
    let f = fixture();
    let a = clone_draft(&f.ctx, request("RS_ParentGun", "RS_CloneGun")).unwrap();
    let b = clone_draft(&f.ctx, request("RS_ParentGun", "RS_CloneGun")).unwrap();
    assert_ne!(a.entry.id, b.entry.id);
    assert_eq!(
        serde_json::to_string(&a.entry.draft).unwrap(),
        serde_json::to_string(&b.entry.draft).unwrap()
    );
    assert_eq!(a.notes, b.notes);
}

#[test]
fn the_notes_name_left_behind_fields_and_the_shared_projectile() {
    let f = fixture();
    let response = clone_draft(&f.ctx, request("RS_ParentGun", "RS_CloneGun")).unwrap();
    let all = response.notes.join("\n");
    assert!(all.contains("soundInteract"), "{all}");
    assert!(
        all.contains("RS_Shot05") && all.contains("RS_ParentGun"),
        "{all}"
    );
}

#[test]
fn a_clone_never_changes_the_source_def() {
    let f = fixture();
    let before = f.session.snapshot();
    let source_before = before.get("ThingDef", "RS_ParentGun").unwrap().node.clone();
    let _ = cloned(&f, "RS_ParentGun", "RS_CloneGun");
    let after = f.session.snapshot();
    assert_eq!(
        after.get("ThingDef", "RS_ParentGun").unwrap().node,
        source_before
    );
}

// ---------------------------------------------------------------------------------------------------
// diff
// ---------------------------------------------------------------------------------------------------

#[test]
fn an_untouched_clone_has_no_changes_and_zero_deltas() {
    let f = fixture();
    let draft = cloned(&f, "RS_ParentGun", "RS_CloneGun");
    let diff = diff_of(&f, draft_to_dto(&draft).unwrap());
    assert_eq!(diff.source, "RS_ParentGun");
    assert!(diff.changes.is_empty(), "{:?}", diff.changes);
    assert!(diff.notes.is_empty());
    assert!(!diff.readouts.is_empty());
    assert!(
        diff.readouts
            .iter()
            .all(|r| r.delta.is_none_or(|d| d.abs() < 1e-12)),
        "{:?}",
        diff.readouts
    );
}

#[test]
fn raising_the_damage_by_two_lists_exactly_that_field_and_moves_dps_not_cycle_time() {
    let f = fixture();
    let draft = cloned(&f, "RS_ParentGun", "RS_CloneGun");
    let edited = edit(&draft, |spec| {
        spec.offer(ScalarField::Damage, 15.0, ValueSource::Typed);
    });
    let diff = diff_of(&f, edited);
    let fields: Vec<&str> = diff.changes.iter().map(|c| c.field.as_str()).collect();
    assert_eq!(fields, vec!["/ranged/damage"]);
    let change = &diff.changes[0];
    assert_eq!(change.old, Some(serde_json::json!(13.0)));
    assert_eq!(change.new, Some(serde_json::json!(15.0)));
    assert_eq!(change.label, "ranged damage");
    let dps = readout(&diff, "dps");
    assert!(dps.delta.unwrap() > 0.0);
    let expected = dps.old.unwrap() / 13.0 * 15.0;
    assert!((dps.new.unwrap() - expected).abs() < 1e-9);
    assert_eq!(readout(&diff, "cycle-time").delta, Some(0.0));
    // the shared projectile keeps the written damage as it is, and the diff says so
    assert!(
        diff.notes.iter().any(|n| n.contains("RS_Shot05")),
        "{:?}",
        diff.notes
    );
}

#[test]
fn several_edits_list_exactly_the_edited_fields_with_their_old_and_new_values() {
    let f = fixture();
    let draft = cloned(&f, "RS_ParentGun", "RS_CloneGun");
    let edited = edit(&draft, |spec| {
        spec.offer(ScalarField::Cooldown, 2.0, ValueSource::Typed);
        spec.offer(ScalarField::Range, 30.0, ValueSource::Typed);
        spec.weapon_tags.push("RS_Extra".into());
        spec.cost_list = vec![CostEntry::new("RS_Steel", 40.0)];
        spec.identity.label = "another label".into();
    });
    let diff = diff_of(&f, edited);
    let fields: Vec<&str> = diff.changes.iter().map(|c| c.field.as_str()).collect();
    assert_eq!(
        fields,
        vec![
            "/costList/0/count",
            "/ranged/range",
            "/ranged/cooldown",
            "/weaponTags/1"
        ]
    );
    let tag = diff
        .changes
        .iter()
        .find(|c| c.field == "/weaponTags/1")
        .unwrap();
    assert!(tag.old.is_none());
    assert_eq!(tag.new, Some(serde_json::json!("RS_Extra")));
    assert!(readout(&diff, "cycle-time").delta.unwrap() > 0.0);
    assert!(readout(&diff, "dps").delta.unwrap() < 0.0);
}

#[test]
fn a_typed_value_equal_to_the_source_is_not_a_change() {
    let f = fixture();
    let draft = cloned(&f, "RS_ParentGun", "RS_CloneGun");
    let edited = edit(&draft, |spec| {
        spec.offer(ScalarField::Damage, 13.0, ValueSource::Typed);
        spec.offer(ScalarField::Mass, 3.0, ValueSource::Typed);
    });
    assert!(diff_of(&f, edited).changes.is_empty());
}

#[test]
fn a_melee_edit_moves_the_panel_and_in_fight_readouts() {
    let f = fixture();
    let draft = cloned(&f, "RS_StuffBlade", "RS_CloneBlade");
    let edited = edit(&draft, |spec| {
        spec.offer(ScalarField::ToolPower(0), 20.0, ValueSource::Typed);
    });
    let diff = diff_of(&f, edited);
    let fields: Vec<&str> = diff.changes.iter().map(|c| c.field.as_str()).collect();
    assert_eq!(fields, vec!["/tools/0/power"]);
    assert_eq!(
        diff.changes[0].label,
        "tools 1 power".replace("tools", "tool")
    );
    assert!(readout(&diff, "melee-dps").delta.unwrap() > 0.0);
    assert!(readout(&diff, "melee-fight-dps").delta.unwrap() > 0.0);
    assert_eq!(readout(&diff, "melee-swing-cooldown").delta, Some(0.0));
    assert!(diff.notes.is_empty(), "a melee clone shares no projectile");
}

#[test]
fn the_diff_ignores_the_combat_extended_block_of_the_draft() {
    let f = fixture();
    let draft = cloned(&f, "RS_ParentGun", "RS_CloneGun");
    let edited = edit(&draft, |spec| {
        spec.ce = Some(rimstudio_design::model::CePatchSpec {
            bulk: Some(rimstudio_design::model::Sourced::typed(5.5)),
            ..rimstudio_design::model::CePatchSpec::default()
        });
    });
    let diff = diff_of(&f, edited);
    assert!(diff.changes.is_empty());
    assert!(diff.readouts.iter().all(|r| !r.key.starts_with("ce-")));
}

#[test]
fn a_draft_that_is_not_a_clone_has_nothing_to_compare() {
    let f = fixture();
    let err = clone_diff(
        &f.ctx,
        DesignerCloneDiffRequest {
            draft: common::dto(common::ranged_spec()),
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.invalid-draft");
    assert!(err.to_string().contains("not cloned"));
}

#[test]
fn a_draft_whose_source_is_gone_reports_no_reference_data() {
    let f = fixture();
    let mut draft = cloned(&f, "RS_ParentGun", "RS_CloneGun");
    draft.cloned_from = Some("RS_Vanished".into());
    let err = clone_diff(
        &f.ctx,
        DesignerCloneDiffRequest {
            draft: draft_to_dto(&draft).unwrap(),
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.reference-unavailable");
}

#[test]
fn the_diff_is_deterministic() {
    let f = fixture();
    let draft = cloned(&f, "RS_ParentGun", "RS_CloneGun");
    let edited = edit(&draft, |spec| {
        spec.offer(ScalarField::Damage, 16.0, ValueSource::Typed);
        spec.offer(ScalarField::Warmup, 0.9, ValueSource::Typed);
    });
    let a = diff_of(&f, edited.clone());
    let b = diff_of(&f, edited);
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
}

// ---------------------------------------------------------------------------------------------------
// structure defaults
// ---------------------------------------------------------------------------------------------------

/// A new ranged design whose numbers equal those of `RS_ParentGun`, with no structure at all.
fn bare_ranged() -> DesignSpec {
    let mut spec = DesignSpec::new_ranged("RS_NewRifle", "new rifle");
    spec.tech_level = Some(rimstudio_design::model::TechLevel::Industrial);
    spec.role = Some("RS_Rifle".into());
    for (field, value) in [
        (ScalarField::Damage, 13.0),
        (ScalarField::Warmup, 1.15),
        (ScalarField::Cooldown, 1.5),
        (ScalarField::Range, 25.5),
        (ScalarField::Mass, 3.0),
    ] {
        spec.offer(field, value, ValueSource::Suggested);
    }
    spec
}

#[test]
fn the_structure_of_a_new_gun_comes_from_the_nearest_reference_weapon() {
    let f = fixture();
    let response = structure_defaults(
        &f.ctx,
        DesignerStructureDefaultsRequest {
            draft: common::dto(bare_ranged()),
        },
    )
    .unwrap();
    assert_eq!(
        response.reference.as_ref().unwrap().def_name,
        "RS_ParentGun"
    );
    assert_eq!(
        response.filled,
        vec!["/parent", "/ranged/projectile", "/costList"]
    );
    let spec = draft_from_dto(&response.draft).unwrap().spec;
    assert_eq!(spec.parent.as_ref().unwrap().def_name, "RS_BaseGun");
    assert_eq!(
        spec.ranged.as_ref().unwrap().projectile,
        Some(ProjectileChoice::Reference("RS_Shot05".into()))
    );
    assert_eq!(spec.cost_list, vec![CostEntry::new("RS_Steel", 30.0)]);
    assert!(spec.ce.is_none());
    let note = response.notes.join(" ");
    assert!(
        note.contains("suggestions") && note.contains("RS_ParentGun"),
        "{note}"
    );
    assert!(note.contains("not typed"), "{note}");
}

#[test]
fn structure_that_is_already_set_is_never_replaced() {
    let f = fixture();
    let mut spec = bare_ranged();
    spec.parent = Some(rimstudio_design::model::ParentRef::named("RS_MyBase"));
    spec.cost_list = vec![CostEntry::new("RS_Part", 2.0)];
    if let Some(r) = spec.ranged.as_mut() {
        r.projectile = Some(ProjectileChoice::Reference("RS_Shot00".into()));
    }
    let response = structure_defaults(
        &f.ctx,
        DesignerStructureDefaultsRequest {
            draft: common::dto(spec),
        },
    )
    .unwrap();
    assert!(response.filled.is_empty());
    let out = draft_from_dto(&response.draft).unwrap().spec;
    assert_eq!(out.parent.unwrap().def_name, "RS_MyBase");
    assert_eq!(out.cost_list, vec![CostEntry::new("RS_Part", 2.0)]);
    assert!(response.notes.join(" ").contains("nothing to fill"));
}

#[test]
fn a_new_melee_weapon_gets_a_parent_cost_list_or_stuff_from_the_nearest_blade() {
    let f = fixture();
    let mut spec = DesignSpec::new_melee("RS_NewBlade", "new blade");
    spec.tech_level = Some(rimstudio_design::model::TechLevel::Industrial);
    spec.tools = vec![
        rimstudio_design::model::ToolSpec::new("head", &["Cut"]).with_numbers(
            7.0 + 1.1 * 9.0,
            1.8 + 0.9,
            ValueSource::Suggested,
        ),
    ];
    spec.offer(ScalarField::Mass, 0.8 + 1.8, ValueSource::Suggested);
    let response = structure_defaults(
        &f.ctx,
        DesignerStructureDefaultsRequest {
            draft: common::dto(spec),
        },
    )
    .unwrap();
    assert_eq!(
        response.reference.as_ref().unwrap().def_name,
        "RS_StuffBlade"
    );
    assert_eq!(response.filled, vec!["/parent", "/stuff"]);
    let out = draft_from_dto(&response.draft).unwrap().spec;
    let stuff = out.stuff.unwrap();
    assert_eq!(stuff.categories, vec!["RS_Metallic"]);
    assert_eq!(stuff.count.unwrap().value, 60.0);
    assert_eq!(
        stuff.count.unwrap().source,
        ValueSource::Suggested,
        "a copied number is a suggestion, never typed"
    );
}

#[test]
fn a_draft_without_numbers_gets_no_structure_and_says_why() {
    let f = fixture();
    let response = structure_defaults(
        &f.ctx,
        DesignerStructureDefaultsRequest {
            draft: common::dto(DesignSpec::new_ranged("RS_NewRifle", "new rifle")),
        },
    )
    .unwrap();
    assert!(response.reference.is_none());
    assert!(response.filled.is_empty());
    assert!(response.notes.join(" ").contains("no number"));
}

#[test]
fn structure_defaults_are_deterministic_and_leave_the_numbers_alone() {
    let f = fixture();
    let req = || DesignerStructureDefaultsRequest {
        draft: common::dto(bare_ranged()),
    };
    let a = structure_defaults(&f.ctx, req()).unwrap();
    let b = structure_defaults(&f.ctx, req()).unwrap();
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
    let input = draft_from_dto(&req().draft).unwrap().spec;
    let output = draft_from_dto(&a.draft).unwrap().spec;
    assert_eq!(
        input.ranged.as_ref().unwrap().damage,
        output.ranged.as_ref().unwrap().damage
    );
    assert_eq!(input.mass, output.mass);
}

#[test]
fn without_a_game_install_the_structure_cannot_be_suggested() {
    let (_tmp, ctx, _clock) = common::offline();
    let err = structure_defaults(
        &ctx,
        DesignerStructureDefaultsRequest {
            draft: common::dto(bare_ranged()),
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.reference-unavailable");
}

// ---------------------------------------------------------------------------------------------------
// planning a clone
// ---------------------------------------------------------------------------------------------------

fn plan_of(f: &common::Fixture, draft: &Draft) -> rimstudio_ipc_types::designer::WritePlanDto {
    let project = common_project::project(f, &[]);
    export_plan(&f.ctx, common_project::request(&project, &draft.spec)).unwrap()
}

#[test]
fn a_ranged_clone_plans_one_vanilla_file_with_the_structure_of_its_source() {
    let f = fixture();
    let draft = cloned(&f, "RS_ParentGun", "RS_CloneGun");
    let plan = plan_of(&f, &draft);
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    assert_eq!(plan.files.len(), 1);
    let file = &plan.files[0];
    assert_eq!(file.path, "Defs/Weapons/RS_CloneGun.xml");
    let text = &file.rendered;
    assert!(text.contains("ParentName=\"RS_BaseGun\""), "{text}");
    assert!(
        text.contains("<defaultProjectile>RS_Shot05</defaultProjectile>"),
        "{text}"
    );
    assert!(text.contains("<RS_Steel>30</RS_Steel>"), "{text}");
    assert!(text.contains("<RS_Own>7</RS_Own>"), "{text}");
    assert!(
        !text.contains("RS_Flammability") && !text.contains("RS_Durability"),
        "inherited stats are not repeated: {text}"
    );
    assert!(!text.contains("CombatExtended"), "{text}");
}

#[test]
fn a_melee_clone_plans_with_its_stuff_and_tools() {
    let f = fixture();
    let draft = cloned(&f, "RS_StuffBlade", "RS_CloneBlade");
    let plan = plan_of(&f, &draft);
    assert!(!plan.has_errors, "{:?}", plan.diagnostics);
    let text = &plan.files[0].rendered;
    assert!(
        text.contains("<costStuffCount>60</costStuffCount>"),
        "{text}"
    );
    assert!(text.contains("<li>RS_Metallic</li>"), "{text}");
    assert!(text.contains("<li>Cut</li>"), "{text}");
}

#[test]
fn a_plan_of_a_clone_is_deterministic() {
    let f = fixture();
    let draft = cloned(&f, "RS_ParentGun", "RS_CloneGun");
    let a = plan_of(&f, &draft);
    let b = plan_of(&f, &draft);
    assert_eq!(a.plan_id, b.plan_id);
}

#[test]
fn a_source_that_cannot_be_crafted_says_what_the_clone_still_needs() {
    let f = fixture();
    // the fixture blades have work to make but neither a cost list nor stuff
    let response = clone_draft(&f.ctx, request("RS_Blade01", "RS_CloneBlade")).unwrap();
    let notes = response.notes.join("\n");
    assert!(notes.contains("no cost list and no stuff"), "{notes}");
    assert!(!notes.contains("no work to make"), "{notes}");
    let crafted = clone_draft(&f.ctx, request("RS_StuffBlade", "RS_CloneBlade2")).unwrap();
    assert!(!crafted.notes.join("\n").contains("no cost list"));
}
