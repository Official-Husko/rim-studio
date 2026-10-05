//! The optional additions of the Combat Extended block: the ammo comp extras and the recoil pattern, the
//! explicit tool plan, the tool children that conversions drop, accepted companion tags and raw nodes.
//! Everything is fictional; the real install is compared by `real_ce_fidelity`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod common_ce;

use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_design::ce::patchgen::extras::{OWNED_CHILDREN, suggest_tool_plan};
use rimstudio_design::ce::patchgen::{
    Container, GeneratedPatch, PatchMode, dry_apply, gun_patch, melee_patch,
};
use rimstudio_design::ce::reader::extras::{ExtrasLibrary, WeaponExtras};
use rimstudio_design::ce::reader::{CeClassNames, CeModel, CeToolRow};
use rimstudio_design::ce::suggest::options::{
    EXTRA_TAGS, RECOIL_PATTERN, RELOAD_ONE_AT_A_TIME, TOOL_PLAN, accept_options, suggest_options,
};
use rimstudio_design::model::{CePatchSpec, CeToolPlan, DesignSpec, ToolSpec, ValueSource};

use common::{melee_ce, melee_spec, ranged_ce, ranged_spec};
use common_ce::{CLASS_TAG, ce_model};

fn ranged() -> DesignSpec {
    let mut ce = ranged_ce();
    ce.weapon_tag_class = Some(CLASS_TAG.into());
    let mut spec = ranged_spec();
    spec.ce = Some(ce);
    spec
}

fn melee() -> DesignSpec {
    let mut spec = melee_spec();
    spec.ce = Some(melee_ce());
    spec
}

fn ce_mut(spec: &mut DesignSpec) -> &mut CePatchSpec {
    spec.ce.as_mut().unwrap()
}

fn conversion(patch: &GeneratedPatch) -> Node {
    patch
        .gun_conversion(&CeClassNames::default())
        .expect("a gun conversion")
        .clone()
}

fn codes(patch: &GeneratedPatch) -> Vec<String> {
    patch
        .diagnostics
        .iter()
        .map(|d| d.code.as_str().to_owned())
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// ammo comp extras and the recoil pattern

#[test]
fn the_ammo_extras_and_the_recoil_pattern_are_written_when_the_block_gives_them() {
    let mut spec = ranged();
    {
        let ce = ce_mut(&mut spec);
        ce.reload_one_at_a_time = Some(true);
        ce.recoil_pattern = Some("Mounted".into());
        ce.ammo_gen_per_mag = Some(rimstudio_design::model::Sourced::new(3, ValueSource::Typed));
    }
    let patch = gun_patch(&spec, &ce_model(), &Container::from_spec(&spec)).unwrap();
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    let op = conversion(&patch);
    let ammo = op.child("AmmoUser").unwrap();
    assert_eq!(ammo.child_text("reloadOneAtATime"), Some("true"));
    assert_eq!(ammo.child_text("AmmoGenPerMagOverride"), Some("3"));
    assert_eq!(
        op.child("Properties").unwrap().child_text("recoilPattern"),
        Some("Mounted")
    );
}

#[test]
fn nothing_of_the_extras_is_written_without_the_block_asking() {
    let spec = ranged();
    let patch = gun_patch(&spec, &ce_model(), &Container::from_spec(&spec)).unwrap();
    let op = conversion(&patch);
    let ammo = op.child("AmmoUser").unwrap();
    assert!(ammo.child("reloadOneAtATime").is_none());
    assert!(ammo.child("AmmoGenPerMagOverride").is_none());
    assert!(
        op.child("Properties")
            .unwrap()
            .child("recoilPattern")
            .is_none()
    );
}

// ---------------------------------------------------------------------------------------------------------
// extra tags

#[test]
fn accepted_tags_are_written_once_next_to_the_class_tag() {
    let mut spec = ranged();
    ce_mut(&mut spec).extra_tags = vec!["RS_Gun".into(), CLASS_TAG.into(), "RS_Gun".into()];
    let patch = gun_patch(&spec, &ce_model(), &Container::from_spec(&spec)).unwrap();
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    let tags: Vec<String> = conversion(&patch)
        .child("weaponTags")
        .unwrap()
        .children_named("li")
        .map(Node::text_content)
        .collect();
    assert_eq!(tags, [CLASS_TAG, "RS_Gun"]);
}

#[test]
fn a_melee_weapon_gets_its_accepted_tags_as_guarded_entries() {
    let mut spec = melee();
    ce_mut(&mut spec).extra_tags = vec!["RS_Melee".into()];
    let patch = melee_patch(&spec, &ce_model(), &Container::from_spec(&spec)).unwrap();
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    let xml = format!("{:?}", patch.operations);
    assert!(xml.contains("weaponTags/li[.=\\\"RS_Melee\\\"]"), "{xml}");
}

// ---------------------------------------------------------------------------------------------------------
// raw nodes

fn draw_extension() -> Node {
    let mut ext = Node::new("modExtensions");
    ext.push_child(
        NodeBuilder::new("li")
            .attr("Class", "RS.GunDrawExtension")
            .text_elem("DrawSize", "(1.2,1.2)")
            .build(),
    );
    ext
}

fn blade_def() -> Node {
    let mut node = NodeBuilder::new("ThingDef")
        .text_elem("defName", "RS_TestBlade")
        .build();
    node.push_child(Node::new("statBases"));
    node
}

#[test]
fn raw_nodes_are_merged_once_and_applied_twice_change_nothing() {
    let model = ce_model();
    let mut spec = melee();
    ce_mut(&mut spec).raw_extras = vec![
        draw_extension(),
        Node::with_text("generateCommonality", "0.5"),
    ];
    let node = blade_def();
    let patch = melee_patch(&spec, &model, &Container::from_node(&node)).unwrap();
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    let once = dry_apply(std::slice::from_ref(&node), &[patch.patch_root()], &model);
    let twice = dry_apply(&[node], &[patch.patch_root(), patch.patch_root()], &model);
    assert!(twice.is_clean(), "{:?}", twice.diagnostics);
    let def = once.def("ThingDef", "RS_TestBlade").unwrap();
    assert_eq!(
        def.child("modExtensions")
            .unwrap()
            .children_named("li")
            .count(),
        1
    );
    assert_eq!(def.child_text("generateCommonality"), Some("0.5"));
    assert_eq!(
        once.def("ThingDef", "RS_TestBlade"),
        twice.def("ThingDef", "RS_TestBlade")
    );
}

#[test]
fn a_raw_node_the_conversion_owns_is_refused() {
    for owned in OWNED_CHILDREN {
        let mut spec = melee();
        ce_mut(&mut spec).raw_extras = vec![Node::new(owned)];
        let patch = melee_patch(&spec, &ce_model(), &Container::from_spec(&spec)).unwrap();
        assert_eq!(patch.mode, PatchMode::Off, "{owned}");
        assert!(
            codes(&patch).contains(&"ce.raw-extra-invalid".to_owned()),
            "{owned}"
        );
    }
}

#[test]
fn raw_nodes_are_checked_for_their_shape() {
    let mut bad_name = Node::new("modExtensions");
    bad_name.tag = "not valid".into();
    let mut unguardable = Node::new("modExtensions");
    unguardable.push_child(NodeBuilder::new("li").text_elem("DrawSize", "1").build());
    for raw in [
        vec![bad_name],
        vec![unguardable],
        vec![draw_extension(), draw_extension()],
    ] {
        let mut spec = melee();
        ce_mut(&mut spec).raw_extras = raw;
        let patch = melee_patch(&spec, &ce_model(), &Container::from_spec(&spec)).unwrap();
        assert_eq!(patch.mode, PatchMode::Off);
        assert!(codes(&patch).contains(&"ce.raw-extra-invalid".to_owned()));
    }
}

// ---------------------------------------------------------------------------------------------------------
// the tool plan

#[test]
fn a_tool_plan_replaces_the_derived_tools_entry_by_entry() {
    let mut spec = melee();
    ce_mut(&mut spec).tool_plan = vec![
        CeToolPlan {
            label: "edge".into(),
            from: Some("blade".into()),
            capacities: Some(vec!["Cut".into()]),
            power: Some(21.0),
            ..CeToolPlan::default()
        },
        CeToolPlan::keep("handle"),
    ];
    let patch = melee_patch(&spec, &ce_model(), &Container::from_spec(&spec)).unwrap();
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    let xml = format!("{:?}", patch.operations);
    assert!(xml.contains("edge") && !xml.contains("\"blade\""), "{xml}");
}

#[test]
fn a_tool_plan_entry_without_a_start_needs_every_field() {
    let mut spec = melee();
    ce_mut(&mut spec).tool_plan = vec![CeToolPlan {
        label: "claw".into(),
        capacities: Some(vec!["Scratch".into()]),
        ..CeToolPlan::default()
    }];
    let patch = melee_patch(&spec, &ce_model(), &Container::from_spec(&spec)).unwrap();
    assert_eq!(patch.mode, PatchMode::Off);
    let codes = codes(&patch);
    assert!(
        codes.contains(&"ce.tool-plan-invalid".to_owned()),
        "{codes:?}"
    );
}

#[test]
fn a_tool_plan_refuses_an_empty_label_and_allows_a_repeated_one() {
    // Combat Extended's own conversions repeat a label for tools with other capacities.
    let mut spec = melee();
    ce_mut(&mut spec).tool_plan = vec![
        CeToolPlan::keep("blade"),
        CeToolPlan {
            label: "blade".into(),
            from: Some("handle".into()),
            ..CeToolPlan::default()
        },
    ];
    let patch = melee_patch(&spec, &ce_model(), &Container::from_spec(&spec)).unwrap();
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    ce_mut(&mut spec).tool_plan = vec![CeToolPlan::keep(" ")];
    let patch = melee_patch(&spec, &ce_model(), &Container::from_spec(&spec)).unwrap();
    assert!(codes(&patch).contains(&"ce.tool-plan-invalid".to_owned()));
}

// ---------------------------------------------------------------------------------------------------------
// tool children that conversions drop

fn model_that_drops_a_tool_field() -> CeModel {
    let mut model = ce_model();
    model.extras = ExtrasLibrary {
        guns: Vec::new(),
        melee: model
            .melee
            .iter()
            .map(|m| WeaponExtras {
                def_name: m.def_name.clone(),
                tool_fields: Vec::new(),
                twin_tool_fields: vec!["labelUsedInLogging".into()],
                recoil_pattern: None,
            })
            .collect(),
    };
    model
}

fn tool_names(patch: &GeneratedPatch) -> String {
    format!("{:?}", patch.operations)
}

#[test]
fn a_tool_child_that_conversions_consistently_drop_is_removed() {
    let mut spec = melee();
    spec.tools
        .iter_mut()
        .for_each(|t| t.extra = vec![Node::with_text("labelUsedInLogging", "false")]);
    let patch = melee_patch(
        &spec,
        &model_that_drops_a_tool_field(),
        &Container::from_spec(&spec),
    )
    .unwrap();
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    assert!(!tool_names(&patch).contains("labelUsedInLogging"));
    assert!(
        patch
            .diagnostics
            .iter()
            .any(|d| d.code.as_str() == "ce.derived-value"
                && d.message.contains("labelUsedInLogging"))
    );
    // a model whose conversions do not drop it keeps it, and so does the opt out
    let patch = melee_patch(&spec, &ce_model(), &Container::from_spec(&spec)).unwrap();
    assert!(tool_names(&patch).contains("labelUsedInLogging"));
    ce_mut(&mut spec).keep_tool_fields = vec!["labelUsedInLogging".into()];
    let patch = melee_patch(
        &spec,
        &model_that_drops_a_tool_field(),
        &Container::from_spec(&spec),
    )
    .unwrap();
    assert!(tool_names(&patch).contains("labelUsedInLogging"));
}

// ---------------------------------------------------------------------------------------------------------
// suggestions

fn row(label: &str, capacities: &[&str]) -> CeToolRow {
    CeToolRow {
        label: label.into(),
        capacities: capacities.iter().map(|c| (*c).to_owned()).collect(),
        power: Some(8.0),
        cooldown: Some(1.5),
        ap_sharp: None,
        ap_blunt: Some(2.0),
        chance_factor: None,
        linked_body_parts_group: None,
    }
}

/// Every converted gun carries a companion tag and loads one round at a time; the recoil pattern is written
/// by all of them.
fn model_with_habits() -> CeModel {
    let mut model = ce_model();
    for g in &mut model.guns {
        g.weapon_tags.push("RS_Sidearm".into());
        g.reload_one_at_a_time = true;
        g.tools = vec![
            row("stock", &["Blunt"]),
            row("barrel", &["Blunt"]),
            row("muzzle", &["Poke"]),
        ];
    }
    model.extras.guns = model
        .guns
        .iter()
        .map(|g| WeaponExtras {
            def_name: g.def_name.clone(),
            recoil_pattern: Some("Mounted".into()),
            ..WeaponExtras::default()
        })
        .collect();
    model
}

#[test]
fn the_habits_of_the_conversions_are_offered_as_options() {
    let spec = ranged();
    let model = model_with_habits();
    let options = suggest_options(&spec, &model, spec.ce.as_ref().unwrap());
    let ids: Vec<&str> = options.iter().map(|o| o.id.as_str()).collect();
    assert!(ids.contains(&EXTRA_TAGS), "{ids:?}");
    assert!(ids.contains(&RELOAD_ONE_AT_A_TIME), "{ids:?}");
    assert!(ids.contains(&RECOIL_PATTERN), "{ids:?}");
    // without a class there is nothing to base the gun options on
    let mut bare = ranged();
    ce_mut(&mut bare).weapon_tag_class = None;
    assert!(suggest_options(&bare, &model, bare.ce.as_ref().unwrap()).is_empty());
}

#[test]
fn accepting_options_fills_empty_fields_and_never_overwrites() {
    let mut spec = ranged();
    ce_mut(&mut spec).recoil_pattern = Some("Regular".into());
    let model = model_with_habits();
    let out = accept_options(&spec, &model, None);
    let ce = out.spec.ce.as_ref().unwrap();
    assert_eq!(ce.recoil_pattern.as_deref(), Some("Regular"));
    assert_eq!(ce.extra_tags, ["RS_Sidearm"]);
    assert_eq!(ce.reload_one_at_a_time, Some(true));
    assert!(out.accepted.contains(&EXTRA_TAGS.to_owned()));
    assert!(!out.accepted.contains(&RECOIL_PATTERN.to_owned()));
    // a named option that does not exist is reported, not invented
    let out = accept_options(&spec, &model, Some(&["nonsense".to_owned()]));
    assert_eq!(out.spec, spec);
    assert_eq!(out.skipped.len(), 1);
    // a design without a block is returned as it is
    let mut off = ranged();
    off.ce = None;
    assert_eq!(accept_options(&off, &model, None).spec, off);
}

#[test]
fn the_suggested_tool_plan_is_a_plan_the_generator_accepts() {
    let mut spec = ranged();
    spec.tools = vec![
        ToolSpec::new("stock", &["Blunt"]).with_numbers(9.0, 2.0, ValueSource::Typed),
        ToolSpec::new("barrel", &["Blunt", "Poke"]).with_numbers(7.0, 2.5, ValueSource::Typed),
    ];
    let model = model_with_habits();
    let options = suggest_options(&spec, &model, spec.ce.as_ref().unwrap());
    assert!(options.iter().any(|o| o.id == TOOL_PLAN));
    let accepted = accept_options(&spec, &model, Some(&[TOOL_PLAN.to_owned()])).spec;
    let patch = gun_patch(&accepted, &model, &Container::from_spec(&accepted)).unwrap();
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    assert!(
        !codes(&patch).contains(&"ce.tool-restructure-suggested".to_owned()),
        "a plan is a decision, it needs no hint"
    );
    // a design that needs no restructuring has no suggestion
    let mut plain = ranged();
    plain.tools =
        vec![ToolSpec::new("stock", &["Blunt"]).with_numbers(9.0, 2.0, ValueSource::Typed)];
    assert!(suggest_tool_plan(&plain, plain.ce.as_ref().unwrap(), &model).is_none());
}

// ---------------------------------------------------------------------------------------------------------
// update mode

#[test]
fn update_mode_sets_the_new_fields_and_the_tags_with_guards() {
    use rimstudio_design::ce::patchgen::{ConversionSource, ExistingConversion};
    use rimstudio_design::ce::reader::CeMarkers;
    let model = ce_model();
    let spec = ranged();
    let current = ExistingConversion {
        markers: CeMarkers {
            verb: true,
            tool: true,
            ammo_comp: true,
            fire_modes: true,
        },
        block: ranged_ce(),
        source: ConversionSource::Foreign(Some("ModPatches/RS.xml".into())),
        verb_fields: ["verbClass", "defaultProjectile", "range"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        ammo_fields: ["magazineSize", "reloadTime", "ammoSet"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        weapon_tags: vec![CLASS_TAG.into()],
    };
    let mut changed = spec.clone();
    {
        let ce = ce_mut(&mut changed);
        ce.reload_one_at_a_time = Some(true);
        ce.recoil_pattern = Some("Mounted".into());
        ce.extra_tags = vec!["RS_Gun".into()];
        ce.raw_extras = vec![draw_extension()];
    }
    let container = Container::from_spec(&spec).with_existing(current);
    let patch = gun_patch(&changed, &model, &container).unwrap();
    assert_eq!(patch.mode, PatchMode::Update);
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    let xml = format!("{:?}", patch.operations);
    for needle in [
        "reloadOneAtATime",
        "recoilPattern",
        "weaponTags/li[.=",
        "modExtensions",
    ] {
        assert!(xml.contains(needle), "{needle}");
    }
    // nothing is changed when the block holds what the conversion has
    let same = gun_patch(&spec, &model, &container).unwrap();
    assert!(same.operations.is_empty(), "{:?}", same.operations);
}
