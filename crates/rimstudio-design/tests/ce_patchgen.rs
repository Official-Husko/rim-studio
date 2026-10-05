//! The Combat Extended patch generator through its public API: golden node trees, the gating rule over every
//! plan (IT-052), `LoadFolders.xml` cases (IT-054), the name and package id split (IT-055), update mode
//! (IT-057), determinism and tree validity.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod common_ce;

use proptest::prelude::*;
use rimstudio_core::tree::Node;
use rimstudio_design::ce::lint::{self, LintContext};
use rimstudio_design::ce::patchgen::{
    CeProjectState, Container, ConversionSource, ExistingConversion, PatchMode, apparel_patch,
    ce_name, ce_package_id, dry_apply, export_ce_plan, export_ce_plan_with, find_mod_fallback,
    gate_violations, gun_patch, melee_patch,
};
use rimstudio_design::ce::reader::{CeClassNames, CeMarkers};
use rimstudio_design::model::{CePatchSpec, DesignSpec, Sourced};
use rimstudio_design::plan::{FileAction, FileKind, ProjectLayout, WritePlan, export_vanilla_plan};

use common::{melee_ce, melee_spec, ranged_ce, ranged_spec};
use common_ce::{CLASS_TAG, ce_model, ce_model_without_pools};

fn with_ce(mut spec: DesignSpec, ce: CePatchSpec) -> DesignSpec {
    spec.ce = Some(ce);
    spec
}

fn ranged_with_ce() -> DesignSpec {
    let mut ce = ranged_ce();
    ce.ammo_set = Some("RS_AmmoSet".into());
    ce.default_projectile = Some("RS_Bullet_CE".into());
    ce.weapon_tag_class = Some(CLASS_TAG.into());
    with_ce(ranged_spec(), ce)
}

fn melee_with_ce() -> DesignSpec {
    with_ce(melee_spec(), melee_ce())
}

fn json(node: &Node) -> String {
    let mut text = serde_json::to_string_pretty(node).unwrap();
    text.push('\n');
    text
}

// ---------------------------------------------------------------------------------------------------------
// golden node trees

#[test]
fn the_ranged_patch_matches_its_golden_tree() {
    let spec = ranged_with_ce();
    let patch = gun_patch(&spec, &ce_model(), &Container::from_spec(&spec)).unwrap();
    assert_eq!(patch.mode, PatchMode::New);
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    common::assert_golden("ce_ranged_patch.json", &json(&patch.patch_root()));
}

#[test]
fn the_melee_patch_matches_its_golden_tree() {
    let spec = melee_with_ce();
    let patch = melee_patch(&spec, &ce_model(), &Container::from_spec(&spec)).unwrap();
    assert_eq!(patch.mode, PatchMode::New);
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    common::assert_golden("ce_melee_patch.json", &json(&patch.patch_root()));
}

#[test]
fn the_ranged_golden_has_the_documented_shape() {
    let spec = ranged_with_ce();
    let model = ce_model();
    let patch = gun_patch(&spec, &model, &Container::from_spec(&spec)).unwrap();
    let make = &patch.operations[0];
    assert_eq!(make.attr("Class"), Some(model.classes.make_gun_op.as_str()));
    let tags: Vec<&str> = make.elements().map(|e| e.tag.as_str()).collect();
    assert_eq!(
        tags,
        [
            "defName",
            "statBases",
            "Properties",
            "AmmoUser",
            "FireModes",
            "weaponTags"
        ]
    );
    let props = make.child("Properties").unwrap();
    assert_eq!(
        props.child_text("verbClass"),
        Some(model.classes.shoot_verb.as_str())
    );
    assert_eq!(props.child_text("hasStandardCommand"), Some("true"));
    assert_eq!(props.child_text("defaultProjectile"), Some("RS_Bullet_CE"));
    // The conversion comes first, the tools replacement second, nothing else.
    assert_eq!(patch.operations.len(), 2);
    assert_eq!(
        patch.operations[1].attr("Class"),
        Some("PatchOperationReplace")
    );
}

// ---------------------------------------------------------------------------------------------------------
// IT-052 and CP-020: gating and the toggle

fn plans() -> Vec<(DesignSpec, WritePlan)> {
    let layouts = [
        ProjectLayout::default(),
        ProjectLayout::with_version_folder("1.6"),
    ];
    let mut out = Vec::new();
    for spec in [ranged_with_ce(), melee_with_ce()] {
        for layout in &layouts {
            out.push((spec.clone(), export_ce_plan(&spec, &ce_model(), layout)));
        }
    }
    out
}

#[test]
fn no_file_outside_the_gated_folder_holds_a_ce_class() {
    for (spec, plan) in plans() {
        assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
        assert!(!plan.files.is_empty());
        for layout in [
            ProjectLayout::default(),
            ProjectLayout::with_version_folder("1.6"),
        ] {
            // Either layout may have produced the plan; the rule holds for the one that did.
            let violations = gate_violations(&plan, &layout);
            let dir = layout.in_version(&layout.ce_folder);
            let belongs = plan
                .files
                .iter()
                .filter(|f| f.kind == FileKind::CePatch)
                .all(|f| f.path.starts_with(&format!("{dir}/")));
            if belongs {
                assert!(violations.is_empty(), "{violations:?}");
            }
        }
        for f in &plan.files {
            match f.kind {
                FileKind::CePatch => assert!(
                    f.path.contains("Compat/CombatExtended/Patches/"),
                    "{}",
                    f.path
                ),
                FileKind::LoadFolders => assert!(!f.contains_text("CombatExtended.")),
                other => panic!("unexpected file kind {other:?}"),
            }
        }
        // The vanilla plan of the same spec holds no CE class at all.
        let vanilla = export_vanilla_plan(&spec, &ProjectLayout::default());
        assert!(!vanilla.has_errors());
        assert!(!vanilla.contains_text("CombatExtended"));
    }
}

#[test]
fn the_ce_plan_has_the_patch_file_and_load_folders_only() {
    let plan = export_ce_plan(&ranged_with_ce(), &ce_model(), &ProjectLayout::default());
    assert_eq!(
        plan.paths(),
        [
            "Compat/CombatExtended/Patches/Weapons_Ranged.xml",
            "LoadFolders.xml"
        ]
    );
    let file = plan
        .file("Compat/CombatExtended/Patches/Weapons_Ranged.xml")
        .unwrap();
    assert_eq!(file.action, FileAction::Create);
    assert_eq!(file.tree.as_ref().unwrap().tag, "Patch");
    assert!(file.sections[0].comment.contains("generated by RimStudio"));
    let melee = export_ce_plan(&melee_with_ce(), &ce_model(), &ProjectLayout::default());
    assert_eq!(
        melee.paths(),
        [
            "Compat/CombatExtended/Patches/Weapons_Melee.xml",
            "LoadFolders.xml"
        ]
    );
}

#[test]
fn the_toggle_decides_and_the_vanilla_plan_does_not_change() {
    let layout = ProjectLayout::default();
    let off = ranged_spec();
    assert!(off.ce.is_none());
    let plan = export_ce_plan(&off, &ce_model(), &layout);
    assert!(plan.files.is_empty() && plan.diagnostics.is_empty());
    let on = ranged_with_ce();
    assert!(!export_ce_plan(&on, &ce_model(), &layout).files.is_empty());
    assert_eq!(
        export_vanilla_plan(&off, &layout),
        export_vanilla_plan(&on, &layout),
        "the vanilla definition is the same with the toggle off and on"
    );
}

#[test]
fn without_combat_extended_nothing_is_generated() {
    let absent = rimstudio_design::ce::reader::CeModel::absent("not installed");
    let plan = export_ce_plan(&ranged_with_ce(), &absent, &ProjectLayout::default());
    assert!(plan.files.is_empty());
    assert_eq!(plan.diagnostics[0].code.as_str(), "design.ce-absent");
    for (spec, kind) in [(ranged_with_ce(), "gun"), (melee_with_ce(), "melee")] {
        let patch = if kind == "gun" {
            gun_patch(&spec, &absent, &Container::unknown())
        } else {
            melee_patch(&spec, &absent, &Container::unknown())
        }
        .unwrap();
        assert!(patch.operations.is_empty());
    }
}

#[test]
fn apparel_is_deferred_with_a_stable_code() {
    let err = apparel_patch(&ranged_with_ce(), &ce_model(), &Container::unknown()).unwrap_err();
    assert_eq!(err.code(), "design.deferred");
}

// ---------------------------------------------------------------------------------------------------------
// IT-054: LoadFolders cases

fn load_folders_of(plan: &WritePlan) -> &rimstudio_design::plan::PlannedFile {
    plan.file("LoadFolders.xml").unwrap()
}

#[test]
fn load_folders_is_created_when_the_project_has_none() {
    let plan = export_ce_plan(&ranged_with_ce(), &ce_model(), &ProjectLayout::default());
    let file = load_folders_of(&plan);
    assert_eq!(file.action, FileAction::Create);
    common::assert_golden(
        "ce_load_folders_none.json",
        &json(file.tree.as_ref().unwrap()),
    );
    let block = file.tree.as_ref().unwrap().child("v1.6").unwrap();
    let gated = block
        .children_named("li")
        .find(|e| e.attr("IfModActive").is_some())
        .unwrap();
    // The gate is the lower case package id, never the mod name.
    assert_eq!(gated.attr("IfModActive"), Some("rs.combatext"));
    assert_eq!(gated.text_content(), "Compat/CombatExtended");
}

#[test]
fn load_folders_is_edited_when_it_exists_without_the_gate() {
    let existing = rimstudio_core::tree::NodeBuilder::new("loadFolders")
        .elem("v1.5", |b| b.li("/").li("1.5"))
        .elem("default", |b| b.li("/"))
        .build();
    let state = CeProjectState {
        load_folders: Some(existing.clone()),
        supported_versions: vec!["1.5".into()],
        ..CeProjectState::default()
    };
    let plan = export_ce_plan_with(
        &ranged_with_ce(),
        &ce_model(),
        &ProjectLayout::default(),
        &state,
    );
    let file = load_folders_of(&plan);
    assert_eq!(file.action, FileAction::UpdateRegion);
    let tree = file.tree.as_ref().unwrap();
    // The old block is untouched, the new block copies it and adds the gate.
    assert_eq!(tree.child("v1.5"), existing.child("v1.5"));
    assert_eq!(tree.child("v1.6").unwrap().children_named("li").count(), 3);
    common::assert_golden("ce_load_folders_existing.json", &json(tree));
    // The About editor is asked for the version and for loadAfter.
    let hints: Vec<&str> = plan
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "ce.about-suggestion")
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(hints.len(), 2, "{hints:?}");
    assert!(hints.iter().any(|h| h.contains("supportedVersions")));
}

#[test]
fn load_folders_with_the_gate_is_unchanged() {
    let first = export_ce_plan(&ranged_with_ce(), &ce_model(), &ProjectLayout::default());
    let tree = load_folders_of(&first).tree.clone().unwrap();
    let state = CeProjectState {
        load_folders: Some(tree.clone()),
        ..CeProjectState::default()
    };
    let plan = export_ce_plan_with(
        &ranged_with_ce(),
        &ce_model(),
        &ProjectLayout::default(),
        &state,
    );
    let file = load_folders_of(&plan);
    assert_eq!(file.action, FileAction::Unchanged);
    assert_eq!(file.tree.as_ref().unwrap(), &tree);
}

// ---------------------------------------------------------------------------------------------------------
// IT-055: the mod name and the package id are two things

#[test]
fn the_name_and_the_package_id_are_distinct_and_a_find_mod_with_an_id_is_flagged() {
    let model = ce_model();
    let names = model.names.clone().unwrap();
    assert_ne!(names.name, names.package_id);
    let find = |entry: &str| {
        let mut mods = Node::new("mods");
        mods.push_child(Node::with_text("li", entry));
        let mut inner = Node::new("match");
        inner.set_attr("Class", "PatchOperationAdd");
        let op = common_ce::op("PatchOperationFindMod")
            .child(mods)
            .child(inner)
            .build();
        common_ce::patch(vec![op])
    };
    let ctx = LintContext::default();
    let bad = lint::run(&[find(&names.package_id)], &model, &ctx);
    assert!(
        bad.iter()
            .any(|d| d.code.as_str() == "ce.cep001-findmod-looks-like-packageid")
    );
    let good = lint::run(&[find(&names.name)], &model, &ctx);
    assert!(
        good.iter()
            .all(|d| d.code.as_str() != "ce.cep001-findmod-looks-like-packageid")
    );
    // The generated gate uses the package id in lower case.
    let plan = export_ce_plan(&ranged_with_ce(), &model, &ProjectLayout::default());
    assert!(load_folders_of(&plan).contains_text("rs.combatext"));
    assert!(!load_folders_of(&plan).contains_text(&names.name));
}

// ---------------------------------------------------------------------------------------------------------
// IT-057: an already converted target

fn existing(block: CePatchSpec) -> ExistingConversion {
    ExistingConversion {
        markers: CeMarkers {
            verb: true,
            tool: true,
            ammo_comp: true,
            fire_modes: true,
        },
        block,
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
    }
}

#[test]
fn an_already_converted_target_gets_update_operations_and_never_a_second_make_gun() {
    let model = ce_model();
    let spec = ranged_with_ce();
    let current = existing(ranged_ce());
    let mut changed = spec.clone();
    if let Some(ce) = changed.ce.as_mut() {
        ce.magazine_size = Some(Sourced::typed(45));
        ce.sway_factor = Some(Sourced::typed(1.5));
    }
    let mut container = Container::from_spec(&spec).with_existing(current);
    container
        .containers
        .insert("statBases".into(), vec!["Bulk".into(), "SwayFactor".into()]);
    let patch = gun_patch(&changed, &model, &container).unwrap();
    assert_eq!(patch.mode, PatchMode::Update);
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    assert_eq!(patch.operations.len(), 2);
    let class = CeClassNames::default().make_gun_op;
    for op in &patch.operations {
        assert_eq!(op.attr("Class"), Some("PatchOperationReplace"));
        assert_ne!(op.attr("Class"), Some(class.as_str()));
    }
    let xpaths: Vec<&str> = patch
        .operations
        .iter()
        .map(|o| o.child_text("xpath").unwrap())
        .collect();
    assert!(xpaths[0].ends_with("/statBases/SwayFactor"), "{xpaths:?}");
    assert!(xpaths[1].ends_with("/magazineSize"), "{xpaths:?}");
    // The plan names its own file and still carries the gate.
    let state = CeProjectState {
        container: Some(container),
        ..CeProjectState::default()
    };
    let plan = export_ce_plan_with(&changed, &model, &ProjectLayout::default(), &state);
    assert_eq!(
        plan.paths(),
        [
            "Compat/CombatExtended/Patches/Weapons_Ranged_Update.xml",
            "LoadFolders.xml"
        ]
    );
    assert!(!plan.contains_text(&class));
}

#[test]
fn update_mode_for_melee_replaces_single_fields() {
    let model = ce_model();
    let spec = melee_with_ce();
    let mut container = Container::from_spec(&spec).with_existing(existing(melee_ce()));
    container
        .containers
        .insert("statBases".into(), vec!["Bulk".into()]);
    let mut changed = spec.clone();
    if let Some(ce) = changed.ce.as_mut() {
        ce.bulk = Some(Sourced::typed(9.0));
        ce.tool_penetration[1].sharp = Some(Sourced::typed(0.8));
    }
    let patch = melee_patch(&changed, &model, &container).unwrap();
    assert_eq!(patch.mode, PatchMode::Update);
    assert_eq!(patch.operations.len(), 2, "{:?}", patch.operations);
    let xpaths: Vec<&str> = patch
        .operations
        .iter()
        .map(|o| o.child_text("xpath").unwrap())
        .collect();
    assert!(xpaths[0].ends_with("/statBases/Bulk"));
    assert!(
        xpaths[1].ends_with("tools/li[label=\"blade\"]/armorPenetrationSharp"),
        "{xpaths:?}"
    );
}

// ---------------------------------------------------------------------------------------------------------
// the generated files pass the lint and apply in the engine

#[test]
fn generated_patches_lint_clean_and_apply() {
    let model = ce_model();
    for spec in [ranged_with_ce(), melee_with_ce()] {
        let layout = ProjectLayout::default();
        let plan = export_ce_plan(&spec, &model, &layout);
        let patch_file = plan
            .files
            .iter()
            .find(|f| f.kind == FileKind::CePatch)
            .unwrap();
        let load_folders = load_folders_of(&plan).tree.clone().unwrap();
        let ctx = LintContext {
            paths: vec![patch_file.path.clone()],
            load_folders: Some(load_folders),
            ..LintContext::default()
        };
        let findings = lint::run(&[patch_file.tree.clone().unwrap()], &model, &ctx);
        let bad: Vec<_> = findings
            .iter()
            .filter(|d| {
                matches!(
                    d.severity,
                    rimstudio_core::diag::Severity::Error | rimstudio_core::diag::Severity::Warning
                )
            })
            .map(|d| d.code.as_str().to_owned())
            .collect();
        assert!(bad.is_empty(), "{bad:?}");
    }
}

#[test]
fn a_dry_apply_of_the_generated_melee_patch_succeeds_on_a_def_with_all_containers() {
    let spec = melee_with_ce();
    let model = ce_model();
    let vanilla = export_vanilla_plan(&spec, &ProjectLayout::default());
    let def = vanilla.files[0]
        .tree
        .as_ref()
        .unwrap()
        .elements()
        .next()
        .unwrap()
        .clone();
    let patch = melee_patch(&spec, &model, &Container::from_node(&def)).unwrap();
    let run = dry_apply(&[def], &[patch.patch_root()], &model);
    assert!(run.is_clean(), "{:?}", run.diagnostics);
    let patched = run.def("ThingDef", "RS_TestBlade").unwrap();
    let tool = patched
        .child("tools")
        .unwrap()
        .children_named("li")
        .next()
        .unwrap();
    assert_eq!(tool.attr("Class"), Some(model.classes.tool.as_str()));
}

// ---------------------------------------------------------------------------------------------------------
// determinism and tree validity

#[test]
fn the_same_input_gives_the_same_plan() {
    for _ in 0..3 {
        let a = export_ce_plan(&ranged_with_ce(), &ce_model(), &ProjectLayout::default());
        let b = export_ce_plan(&ranged_with_ce(), &ce_model(), &ProjectLayout::default());
        assert_eq!(a, b);
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }
}

#[test]
fn a_model_without_pools_still_generates_from_the_block() {
    let mut spec = ranged_with_ce();
    let model = ce_model_without_pools();
    // Without a ratio to learn from, a tool needs its penetration from the block (else the patch is refused).
    let refused = gun_patch(&spec, &model, &Container::from_spec(&spec)).unwrap();
    assert!(refused.has_errors() && refused.operations.is_empty());
    if let Some(ce) = spec.ce.as_mut() {
        ce.tool_penetration = vec![rimstudio_design::model::CeToolPenetration {
            tool: "grip".into(),
            sharp: None,
            blunt: Some(Sourced::typed(0.5)),
        }];
    }
    let patch = gun_patch(&spec, &model, &Container::from_spec(&spec)).unwrap();
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    // Mass, range and warmup fall back to the vanilla design (identity) and are listed as such.
    assert!(patch.derived.iter().all(|d| matches!(
        d.origin,
        rimstudio_design::ce::patchgen::ValueOrigin::Vanilla
    )));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn generated_trees_are_valid_and_survive_a_json_round_trip(
        bulk in 0.1f64..40.0,
        sway in 0.1f64..4.0,
        spread in 0.01f64..1.0,
        magazine in 1u32..200,
        reload in 0.5f64..12.0,
        crit in -0.5f64..0.5,
    ) {
        let mut ranged = ranged_with_ce();
        if let Some(ce) = ranged.ce.as_mut() {
            ce.bulk = Some(Sourced::typed(bulk));
            ce.sway_factor = Some(Sourced::typed(sway));
            ce.shot_spread = Some(Sourced::typed(spread));
            ce.magazine_size = Some(Sourced::typed(magazine));
            ce.reload_time = Some(Sourced::typed(reload));
        }
        let mut melee = melee_with_ce();
        if let Some(ce) = melee.ce.as_mut() {
            ce.bulk = Some(Sourced::typed(bulk));
            ce.melee_crit_chance = Some(Sourced::typed(crit));
        }
        for spec in [ranged, melee] {
            let model = ce_model();
            let plan = export_ce_plan(&spec, &model, &ProjectLayout::default());
            prop_assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
            for file in &plan.files {
                let tree = file.tree.as_ref().unwrap();
                prop_assert!(tree.validate().is_ok());
                let text = serde_json::to_string(tree).unwrap();
                let back: Node = serde_json::from_str(&text).unwrap();
                prop_assert_eq!(&back, tree);
            }
            // Same input, same output.
            prop_assert_eq!(&plan, &export_ce_plan(&spec, &model, &ProjectLayout::default()));
            // Every written number parses back to the value that was given.
            let file = plan.files.iter().find(|f| f.kind == FileKind::CePatch).unwrap();
            let text = serde_json::to_string(file.tree.as_ref().unwrap()).unwrap();
            prop_assert!(text.contains(&rimstudio_design::model::format_number(bulk)));
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// the FindMod fallback for a mod that must not use LoadFolders.xml

#[test]
fn the_find_mod_fallback_uses_the_exact_name_and_refuses_the_gun_conversion() {
    let model = ce_model();
    assert_eq!(ce_name(&model), "RS Combat Ext");
    assert_eq!(ce_package_id(&model), "rs.combatext");
    let spec = melee_with_ce();
    let patch = melee_patch(&spec, &model, &Container::from_spec(&spec)).unwrap();
    let wrapped = find_mod_fallback(&patch, &model);
    assert_eq!(wrapped.operations.len(), patch.operations.len());
    for op in &wrapped.operations {
        assert_eq!(op.attr("Class"), Some("PatchOperationFindMod"));
        assert_eq!(
            op.child("mods").unwrap().child_text("li"),
            Some("RS Combat Ext")
        );
        assert!(op.child("match").is_some());
    }
    // The name passes the FindMod rules of the lint.
    let findings = lint::run(&[wrapped.patch_root()], &model, &LintContext::default());
    for rule in ["CEP001", "CEP002", "CEP003"] {
        assert!(
            findings
                .iter()
                .all(|d| d.args.get("ruleId").is_none_or(|r| r != rule)),
            "{rule}"
        );
    }
    // Applied in the engine, where a mod with that name is active, it converts the def.
    let vanilla = export_vanilla_plan(&spec, &ProjectLayout::default());
    let def = vanilla.files[0]
        .tree
        .as_ref()
        .unwrap()
        .elements()
        .next()
        .unwrap()
        .clone();
    let run = dry_apply(&[def], &[wrapped.patch_root()], &model);
    assert!(run.is_clean(), "{:?}", run.diagnostics);
    let converted = run.def("ThingDef", "RS_TestBlade").unwrap();
    assert!(converted.child("equippedStatOffsets").is_some());
    // A gun conversion is a Combat Extended class and cannot be gated by FindMod.
    let gun = ranged_with_ce();
    let gun_patch = gun_patch(&gun, &model, &Container::from_spec(&gun)).unwrap();
    let refused = find_mod_fallback(&gun_patch, &model);
    assert!(refused.operations.is_empty());
    assert!(refused.has_errors());
}
