//! Tests of the patch generator over the fictional conversion set of the reader tests, run through the real
//! def engine (the dry run) so that the generated operations are applied, not only inspected.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use rimstudio_core::tree::Node;
use rimstudio_defs::DefRecord;

use super::*;
use crate::ce::patchgen::convert::family_key;
use crate::ce::reader::fixtures_tests::{Build, load_set, rows};
use crate::ce::reader::{CeModel, CeReadOptions, read_conversions_with};
use crate::model::{
    CePatchSpec, CeToolPenetration, RangedInputs, Sourced, TechLevel, ToolSpec, ValueSource,
};
use crate::plan::ProjectLayout;
use crate::reader::fixtures_tests::{self as vanilla, GunNumbers};

fn model() -> CeModel {
    let rows = rows();
    let ce = load_set(&rows, Build::default());
    let plain = load_set(
        &rows,
        Build {
            ce: false,
            ..Build::default()
        },
    );
    read_conversions_with(
        &ce.databases,
        &CeReadOptions {
            vanilla: Some(&plain.databases),
            ..CeReadOptions::default()
        },
    )
}

fn gun_node(name: &str) -> Node {
    vanilla::gun(
        name,
        "RS_Shot1",
        GunNumbers {
            mass: 2.4,
            range: 30.0,
            warmup: 1.1,
            cooldown: 1.4,
            burst: 3,
            tier: "Industrial",
        },
        &["RS_Gun"],
    )
}

fn gun_spec(name: &str) -> DesignSpec {
    let mut s = DesignSpec::new_ranged(name, "test gun");
    s.tech_level = Some(TechLevel::Industrial);
    s.mass = Some(Sourced::typed(2.4));
    s.ranged = Some(RangedInputs {
        range: Some(Sourced::typed(30.0)),
        warmup: Some(Sourced::typed(1.1)),
        cooldown: Some(Sourced::typed(1.4)),
        burst_count: Some(Sourced::typed(3)),
        ticks_between_burst_shots: Some(Sourced::typed(8.0)),
        sound_cast: Some("RS_Shot".into()),
        ..RangedInputs::default()
    });
    s.tools = vec![ToolSpec::new("grip", &["Blunt"]).with_numbers(8.0, 2.0, ValueSource::Typed)];
    s.ce = Some(CePatchSpec {
        ammo_set: Some("RS_AmmoSetA".into()),
        default_projectile: Some("RS_CeBullet1".into()),
        weapon_tag_class: Some("CE_AI_RS_Rifle".into()),
        magazine_size: Some(Sourced::typed(24)),
        reload_time: Some(Sourced::typed(3.5)),
        bulk: Some(Sourced::typed(5.5)),
        sway_factor: Some(Sourced::typed(1.25)),
        shot_spread: Some(Sourced::typed(0.09)),
        // The fictional library has one melee conversion: too few to estimate a penetration, so the
        // block gives it (an estimate that cannot be measured is never written).
        tool_penetration: vec![CeToolPenetration {
            tool: "grip".into(),
            sharp: None,
            blunt: Some(Sourced::typed(3.0)),
        }],
        ..CePatchSpec::default()
    });
    s
}

fn scratch_defs(extra: Node) -> Vec<Node> {
    let mut defs = vanilla::base_defs();
    defs.push(extra);
    defs
}

fn record(node: &Node) -> DefRecord {
    DefRecord {
        seq: 0,
        tag: "ThingDef".into(),
        type_name: "Verse.ThingDef".into(),
        def_name: node.child_text("defName").unwrap().to_owned(),
        origin: None,
        node: node.clone(),
        parents: Vec::new(),
        patched_by: Vec::new(),
    }
}

fn class_count(node: &Node, list: &str, class: &str) -> usize {
    node.child(list)
        .map(|l| {
            l.children_named("li")
                .filter(|li| li.attr("Class") == Some(class))
                .count()
        })
        .unwrap_or(0)
}

#[test]
fn a_gun_patch_applies_in_the_engine_and_converts_the_def() {
    let model = model();
    let node = gun_node("RS_NewGun");
    let spec = gun_spec("RS_NewGun");
    let patch = gun_patch(&spec, &model, &Container::from_node(&node)).unwrap();
    assert_eq!(patch.mode, PatchMode::New);
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    assert_eq!(
        patch.operations[0].attr("Class"),
        Some(model.classes.make_gun_op.as_str())
    );
    let run = dry_apply(&scratch_defs(node), &[patch.patch_root()], &model);
    assert!(run.is_clean(), "{:?}", run.diagnostics);
    let def = run.def("ThingDef", "RS_NewGun").unwrap();
    // The converted verb replaced the shoot verb, the comps and the tag are there, accuracy stats are gone.
    assert_eq!(class_count(def, "verbs", &model.classes.verb_properties), 1);
    assert_eq!(def.child("verbs").unwrap().children_named("li").count(), 1);
    assert_eq!(class_count(def, "comps", &model.classes.ammo_user), 1);
    assert_eq!(class_count(def, "comps", &model.classes.fire_modes), 1);
    let stats = def.child("statBases").unwrap();
    assert!(stats.child("AccuracyTouch").is_none());
    assert_eq!(stats.child_text("Bulk"), Some("5.5"));
    let tags: Vec<String> = def
        .child("weaponTags")
        .unwrap()
        .children_named("li")
        .map(Node::text_content)
        .collect();
    assert_eq!(tags, ["RS_Gun", "CE_AI_RS_Rifle"]);
    // The tools were replaced by converted tools with a blunt penetration.
    let tools = def.child("tools").unwrap();
    let tool = tools.children_named("li").next().unwrap();
    assert_eq!(tool.attr("Class"), Some(model.classes.tool.as_str()));
    assert!(tool.child("armorPenetrationBlunt").is_some());
}

#[test]
fn numbers_not_in_the_block_are_derived_and_listed() {
    let model = model();
    let node = gun_node("RS_NewGun");
    let patch = gun_patch(&gun_spec("RS_NewGun"), &model, &Container::from_node(&node)).unwrap();
    let fields: Vec<&str> = patch.derived.iter().map(|d| d.field.as_str()).collect();
    for expected in ["mass", "range", "warmup"] {
        assert!(fields.contains(&expected), "{fields:?}");
    }
    // Nothing the block held is reported as derived.
    assert!(!fields.contains(&"ce.bulk"));
}

#[test]
fn a_def_without_stat_bases_gets_a_patch_that_applies() {
    let model = model();
    let mut node = vanilla::melee("RS_Bare", "Medieval", 1.4, &[("blade", "Cut", 18.0, 2.2)]);
    node.remove_child("statBases");
    let mut spec = DesignSpec::new_melee("RS_Bare", "bare");
    spec.tech_level = Some(TechLevel::Medieval);
    spec.tools = vec![ToolSpec::new("blade", &["Cut"]).with_numbers(18.0, 2.2, ValueSource::Typed)];
    spec.ce = Some(CePatchSpec {
        bulk: Some(Sourced::typed(4.0)),
        melee_crit_chance: Some(Sourced::typed(0.1)),
        melee_parry_chance: Some(Sourced::typed(0.2)),
        melee_dodge_chance: Some(Sourced::typed(-0.05)),
        tool_penetration: vec![CeToolPenetration {
            tool: "blade".into(),
            sharp: Some(Sourced::typed(0.6)),
            blunt: Some(Sourced::typed(0.9)),
        }],
        ..CePatchSpec::default()
    });
    let container = Container::from_node(&node);
    assert!(!container.has("statBases"));
    let patch = melee_patch(&spec, &model, &container).unwrap();
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    // The containers are ensured, never a Replace on a missing path.
    assert!(
        patch.operations[0]
            .child_text("xpath")
            .unwrap()
            .ends_with("/statBases")
    );
    let run = dry_apply(&scratch_defs(node.clone()), &[patch.patch_root()], &model);
    assert!(run.is_clean(), "{:?}", run.diagnostics);
    let def = run.def("ThingDef", "RS_Bare").unwrap();
    assert_eq!(
        def.child("statBases").unwrap().child_text("Bulk"),
        Some("4")
    );
    assert_eq!(
        def.child("equippedStatOffsets")
            .unwrap()
            .child_text("MeleeCritChance"),
        Some("0.1")
    );
    // Claiming that statBases exists when the def has none makes the engine report the failing operation.
    let mut wrong = Container::unknown();
    wrong.containers.insert("statBases".into(), vec![]);
    wrong
        .containers
        .insert("equippedStatOffsets".into(), vec![]);
    wrong.containers.insert("tools".into(), vec![]);
    let bad = melee_patch(&spec, &model, &wrong).unwrap();
    let run = dry_apply(&scratch_defs(node), &[bad.patch_root()], &model);
    assert!(!run.is_clean());
    assert_eq!(run.diagnostics[0].code.as_str(), "ce.dry-run-failed");
    assert!(run.diagnostics[0].message.contains("statBases"));
}

#[test]
fn an_existing_conversion_is_updated_field_by_field_and_never_converted_twice() {
    let model = model();
    let node = gun_node("RS_NewGun");
    let spec = gun_spec("RS_NewGun");
    let first = gun_patch(&spec, &model, &Container::from_node(&node)).unwrap();
    let run = dry_apply(&scratch_defs(node.clone()), &[first.patch_root()], &model);
    let converted = run.def("ThingDef", "RS_NewGun").unwrap().clone();
    let existing = ExistingConversion::from_def(
        &record(&converted),
        &model.classes,
        ConversionSource::Foreign(Some("ModPatches/x.xml".into())),
    )
    .unwrap();
    assert!(existing.markers.is_conversion());
    let container = Container::from_node(&converted).with_existing(existing);
    let mut changed = spec.clone();
    if let Some(ce) = changed.ce.as_mut() {
        ce.bulk = Some(Sourced::typed(7.0));
        ce.reload_time = Some(Sourced::typed(2.5));
        ce.recoil_amount = Some(Sourced::typed(1.9));
    }
    let update = gun_patch(&changed, &model, &container).unwrap();
    assert_eq!(update.mode, PatchMode::Update);
    assert!(!update.has_errors(), "{:?}", update.diagnostics);
    let class = model.classes.make_gun_op.clone();
    assert!(
        update
            .operations
            .iter()
            .all(|op| op.attr("Class") != Some(class.as_str()))
    );
    assert_eq!(update.operations.len(), 3, "{:?}", update.operations);
    assert!(
        update
            .diagnostics
            .iter()
            .any(|d| d.code.as_str() == "ce.already-converted")
    );
    // A foreign source is never rewritten and the user is told about the order.
    assert!(
        update
            .diagnostics
            .iter()
            .any(|d| d.code.as_str() == "ce.update-load-after")
    );
    let run = dry_apply(
        &scratch_defs(node),
        &[first.patch_root(), update.patch_root()],
        &model,
    );
    assert!(run.is_clean(), "{:?}", run.diagnostics);
    let def = run.def("ThingDef", "RS_NewGun").unwrap();
    assert_eq!(
        def.child("statBases").unwrap().child_text("Bulk"),
        Some("7")
    );
    assert_eq!(class_count(def, "verbs", &model.classes.verb_properties), 1);
    assert_eq!(class_count(def, "comps", &model.classes.ammo_user), 1);
    let comp = def
        .child("comps")
        .unwrap()
        .children_named("li")
        .find(|li| li.attr("Class") == Some(model.classes.ammo_user.as_str()))
        .unwrap();
    assert_eq!(comp.child_text("reloadTime"), Some("2.5"));
    // The recoil was absent from the conversion, so it was added to the verb entry.
    let verb = def
        .child("verbs")
        .unwrap()
        .children_named("li")
        .next()
        .unwrap();
    assert_eq!(verb.child_text("recoilAmount"), Some("1.9"));
}

#[test]
fn updating_with_the_same_values_writes_nothing() {
    let model = model();
    let node = gun_node("RS_NewGun");
    let spec = gun_spec("RS_NewGun");
    let first = gun_patch(&spec, &model, &Container::from_node(&node)).unwrap();
    let run = dry_apply(&scratch_defs(node), &[first.patch_root()], &model);
    let converted = run.def("ThingDef", "RS_NewGun").unwrap().clone();
    let existing = ExistingConversion::from_def(
        &record(&converted),
        &model.classes,
        ConversionSource::RimStudio(None),
    )
    .unwrap();
    let container = Container::from_node(&converted).with_existing(existing);
    let update = gun_patch(&spec, &model, &container).unwrap();
    assert!(update.operations.is_empty());
    assert!(
        update
            .diagnostics
            .iter()
            .any(|d| d.code.as_str() == "ce.update-nothing")
    );
}

#[test]
fn the_toggle_off_produces_nothing_even_with_ce_present() {
    let model = model();
    let node = gun_node("RS_NewGun");
    let mut spec = gun_spec("RS_NewGun");
    spec.ce = None;
    let patch = gun_patch(&spec, &model, &Container::from_node(&node)).unwrap();
    assert_eq!(patch.mode, PatchMode::Off);
    assert!(patch.operations.is_empty() && patch.diagnostics.is_empty());
    let plan = export_ce_plan(&spec, &model, &ProjectLayout::default());
    assert!(plan.files.is_empty() && plan.diagnostics.is_empty());
}

#[test]
fn apparel_is_deferred() {
    let err = apparel_patch(&gun_spec("RS_X"), &model(), &Container::unknown()).unwrap_err();
    assert_eq!(err.code(), "design.deferred");
}

#[test]
fn an_absent_model_gives_no_patch_and_a_reason() {
    let absent = CeModel::absent("not installed");
    let spec = gun_spec("RS_NewGun");
    let patch = gun_patch(&spec, &absent, &Container::unknown()).unwrap();
    assert!(patch.operations.is_empty());
    assert_eq!(patch.diagnostics[0].code.as_str(), "design.ce-absent");
    let plan = export_ce_plan(&spec, &absent, &ProjectLayout::default());
    assert!(plan.files.is_empty());
    assert_eq!(plan.diagnostics[0].code.as_str(), "design.ce-absent");
}

#[test]
fn unresolved_references_block_the_patch() {
    let model = model();
    let node = gun_node("RS_NewGun");
    let mut spec = gun_spec("RS_NewGun");
    spec.ce.as_mut().unwrap().ammo_set = Some("RS_NoSuchSet".into());
    let patch = gun_patch(&spec, &model, &Container::from_node(&node)).unwrap();
    assert!(patch.has_errors());
    assert!(patch.operations.is_empty());
    let mut spec = gun_spec("RS_NewGun");
    spec.ce.as_mut().unwrap().default_projectile = Some("RS_CeBullet2".into());
    let patch = gun_patch(&spec, &model, &Container::from_node(&node)).unwrap();
    assert!(
        patch.has_errors(),
        "a projectile outside the set must block"
    );
}

// ---------------------------------------------------------------------------------------------------------
// Flow D

fn plain_load() -> rimstudio_defs::LoadOutput {
    load_set(
        &rows(),
        Build {
            ce: false,
            ..Build::default()
        },
    )
}

#[test]
fn the_scan_lists_statuses() {
    let model = model();
    let rows = rows();
    let ce = load_set(&rows, Build::default());
    let mut project = crate::ce::reader::fixtures_tests::vanilla_defs(&rows);
    let mut abstract_gun = gun_node("RS_Base");
    abstract_gun.set_attr("Abstract", "True");
    project.push(abstract_gun);
    project.push(gun_node("RS_Missing"));
    // A grenade like weapon: a one use verb.
    let mut grenade = gun_node("RS_Grenade");
    if let Some(v) = grenade.child_mut("verbs").and_then(|v| v.child_mut("li")) {
        v.set_child_text("verbClass", "Verb_ShootOneUse");
    }
    project.push(grenade.clone());
    let mut with_grenade = vanilla::base_defs();
    with_grenade.push(grenade);
    let plain = crate::reader::fixtures_tests::load_defs(with_grenade, &vanilla::types(&[]));
    // Against the CE load every gun is converted.
    let converted = scan(&project, &ce.databases, &model);
    let gun0 = converted.iter().find(|c| c.def == "RS_CeGun00").unwrap();
    assert_eq!(gun0.status, ConvertStatus::AlreadyCe);
    assert_eq!(gun0.kind, Some(ItemKind::Ranged));
    let base = converted.iter().find(|c| c.def == "RS_Base").unwrap();
    assert_eq!(base.status, ConvertStatus::TargetNotFound);
    let missing = converted.iter().find(|c| c.def == "RS_Missing").unwrap();
    assert_eq!(missing.status, ConvertStatus::TargetNotFound);
    // Against the plain load the guns are not converted and the one use weapon is unsupported.
    let plain_all = plain_load();
    let listed = scan(&project, &plain_all.databases, &model);
    let gun0 = listed.iter().find(|c| c.def == "RS_CeGun00").unwrap();
    assert_eq!(gun0.status, ConvertStatus::NotConverted);
    let edge = listed.iter().find(|c| c.def == "RS_CeEdge").unwrap();
    assert_eq!(edge.kind, Some(ItemKind::Melee));
    let names: Vec<&str> = listed.iter().map(|c| c.def.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "candidates are sorted by def name");
    let g = scan(&project, &plain.databases, &model)
        .into_iter()
        .find(|c| c.def == "RS_Grenade")
        .unwrap();
    assert_eq!(g.status, ConvertStatus::UnsupportedKind);
}

#[test]
fn weapons_of_one_caliber_and_class_share_a_family_and_others_do_not() {
    let a = gun_node("RS_FamA");
    let b = gun_node("RS_FamB");
    assert_eq!(
        family_key(&a, ItemKind::Ranged),
        family_key(&b, ItemKind::Ranged)
    );
    assert_eq!(family_key(&a, ItemKind::Ranged), "ranged/RS_Gun/RS_Shot1");
    let mut other = gun_node("RS_FamC");
    if let Some(v) = other.child_mut("verbs").and_then(|v| v.child_mut("li")) {
        v.set_child_text("defaultProjectile", "RS_Shot2");
    }
    assert_ne!(
        family_key(&a, ItemKind::Ranged),
        family_key(&other, ItemKind::Ranged)
    );
    let blade = untagged_blade("RS_FamBlade", true);
    assert_eq!(family_key(&blade, ItemKind::Melee), "melee/untagged");
    let listed = scan(
        &[a.clone(), b.clone()],
        &load_set(
            &rows(),
            Build {
                ce: false,
                ..Build::default()
            },
        )
        .databases,
        &model(),
    );
    assert!(
        listed
            .iter()
            .all(|c| c.family.is_empty() || c.family.starts_with("ranged/"))
    );
}

/// A melee weapon of the fictional set that carries no weapon tags at all.
fn untagged_blade(name: &str, primary_in_weapon_category: bool) -> Node {
    use rimstudio_core::tree::NodeBuilder;
    let mut b = NodeBuilder::new("ThingDef")
        .text_elem("defName", name)
        .text_elem("label", name.to_lowercase())
        .text_elem("category", "Item")
        .text_elem("techLevel", "Industrial")
        .elem("tools", |t| {
            t.elem("li", |li| {
                li.text_elem("label", "edge")
                    .elem("capacities", |c| c.li("Cut"))
                    .text_elem("power", "9")
                    .text_elem("cooldownTime", "2")
            })
        });
    if primary_in_weapon_category {
        b = b
            .text_elem("equipmentType", "Primary")
            .elem("thingCategories", |c| c.li("RS_WeaponsMelee"));
    }
    b.build()
}

#[test]
fn a_melee_weapon_without_weapon_tags_is_listed_when_the_pools_would_count_it() {
    let model = model();
    let tagless = untagged_blade("RS_TaglessBlade", true);
    let plain_item = untagged_blade("RS_ToolCarrier", false);
    let mut defs = vanilla::base_defs();
    defs.push(tagless.clone());
    defs.push(plain_item.clone());
    let plain = crate::reader::fixtures_tests::load_defs(defs, &vanilla::types(&[]));
    let listed = scan(&[tagless, plain_item], &plain.databases, &model);
    let blade = listed.iter().find(|c| c.def == "RS_TaglessBlade").unwrap();
    assert_eq!(blade.kind, Some(ItemKind::Melee));
    assert_eq!(blade.status, ConvertStatus::NotConverted);
    assert!(
        listed.iter().all(|c| c.def != "RS_ToolCarrier"),
        "an item that merely carries tools is no weapon"
    );
}

fn convert_env<'a>(
    dbs: &'a rimstudio_defs::DefDatabases,
    model: &'a CeModel,
    project: &'a [Node],
    state: &'a CeProjectState,
    layout: &'a ProjectLayout,
    reader: &'a crate::reader::options::ReaderOptions,
    source: &'a ConversionSource,
) -> ConvertEnv<'a> {
    ConvertEnv {
        dbs,
        model,
        layout,
        reader,
        project,
        state,
        source,
    }
}

#[test]
fn the_automatic_mode_asks_for_what_it_cannot_derive_and_converts_when_answered() {
    let model = model();
    let rows = rows();
    let plain = plain_load();
    let project = crate::ce::reader::fixtures_tests::vanilla_defs(&rows);
    let layout = ProjectLayout::default();
    let state = CeProjectState::default();
    let reader = crate::reader::options::ReaderOptions::default();
    let source = ConversionSource::Unknown;
    let env = convert_env(
        &plain.databases,
        &model,
        &project,
        &state,
        &layout,
        &reader,
        &source,
    );
    let candidate = scan(&project, &plain.databases, &model)
        .into_iter()
        .find(|c| c.def == "RS_CeGun03")
        .unwrap();
    let first = convert(&candidate, &ConvertAnswers::default(), &env);
    assert!(first.plan.files.is_empty());
    for field in [
        "/ce/ammoSet",
        "/ce/weaponTagClass",
        "/ce/oneHanded",
        "/ce/beltFed",
    ] {
        assert!(first.asks.asks(field), "{field}: {:?}", first.asks);
    }
    // The numbers the model can predict are not asked.
    assert!(!first.asks.asks("/ce/bulk"));
    assert!(!first.derived.is_empty());
    // One melee conversion is too few to estimate a penetration: the question carries no reference guess.
    assert!(first.asks.asks("/ce/toolPenetration/grip/blunt"));

    let answers = ConvertAnswers {
        tool_penetration: vec![CeToolPenetration {
            tool: "grip".into(),
            sharp: None,
            blunt: Some(Sourced::typed(2.5)),
        }],
        ammo_set: Some("RS_AmmoSetA".into()),
        weapon_tag_class: Some("CE_AI_RS_Rifle".into()),
        one_handed: Some(false),
        belt_fed: Some(false),
        ..ConvertAnswers::default()
    };
    let done = convert(&candidate, &answers, &env);
    assert!(done.asks.is_empty(), "{:?}", done.asks);
    assert!(!done.update);
    assert!(!done.plan.has_errors(), "{:?}", done.plan.diagnostics);
    let paths = done.plan.paths();
    assert_eq!(paths, ["CE/Patches/Weapons_Ranged.xml", "LoadFolders.xml"]);
    // The default projectile is the first of the ammo set.
    assert_eq!(
        done.spec
            .as_ref()
            .and_then(|s| s.ce.as_ref())
            .and_then(|c| c.default_projectile.clone()),
        Some("RS_CeBullet1".into())
    );
    // The plan applies to the existing definition without error, which is never edited.
    let file = done.plan.file("CE/Patches/Weapons_Ranged.xml").unwrap();
    let run = dry_apply(
        &scratch_defs_all(&project),
        &[file.tree.clone().unwrap()],
        &model,
    );
    assert!(run.is_clean(), "{:?}", run.diagnostics);
    let def = run.def("ThingDef", "RS_CeGun03").unwrap();
    assert_eq!(class_count(def, "verbs", &model.classes.verb_properties), 1);
}

fn scratch_defs_all(project: &[Node]) -> Vec<Node> {
    project.to_vec()
}

#[test]
fn an_already_converted_candidate_is_updated_and_the_def_is_not_touched() {
    let model = model();
    let rows = rows();
    let ce = load_set(&rows, Build::default());
    let project = crate::ce::reader::fixtures_tests::vanilla_defs(&rows);
    let layout = ProjectLayout::default();
    let state = CeProjectState::default();
    let reader = crate::reader::options::ReaderOptions::default();
    let source = ConversionSource::Foreign(Some("ModPatches/RS.xml".into()));
    let env = convert_env(
        &ce.databases,
        &model,
        &project,
        &state,
        &layout,
        &reader,
        &source,
    );
    let candidate = scan(&project, &ce.databases, &model)
        .into_iter()
        .find(|c| c.def == "RS_CeGun01")
        .unwrap();
    assert_eq!(candidate.status, ConvertStatus::AlreadyCe);
    let answers = ConvertAnswers {
        overrides: CePatchSpec {
            magazine_size: Some(Sourced::typed(17)),
            ..CePatchSpec::default()
        },
        ..ConvertAnswers::default()
    };
    let out = convert(&candidate, &answers, &env);
    assert!(out.update);
    assert!(out.asks.is_empty());
    assert!(!out.plan.has_errors(), "{:?}", out.plan.diagnostics);
    let file = out
        .plan
        .file("CE/Patches/Weapons_Ranged_Update.xml")
        .expect("update mode writes its own file");
    let ops: Vec<&Node> = file.tree.as_ref().unwrap().elements().collect();
    assert_eq!(ops.len(), 1);
    assert!(
        ops[0]
            .child_text("xpath")
            .unwrap()
            .ends_with("/magazineSize")
    );
    assert!(out.plan.paths().contains(&"LoadFolders.xml"));
}

#[test]
fn convert_refuses_unsupported_candidates() {
    let model = model();
    let plain = plain_load();
    let layout = ProjectLayout::default();
    let state = CeProjectState::default();
    let reader = crate::reader::options::ReaderOptions::default();
    let source = ConversionSource::Unknown;
    let env = convert_env(
        &plain.databases,
        &model,
        &[],
        &state,
        &layout,
        &reader,
        &source,
    );
    let candidate = ConvertCandidate {
        def: "RS_Grenade".into(),
        label: String::new(),
        kind: Some(ItemKind::Ranged),
        status: ConvertStatus::UnsupportedKind,
        reason: "a one use weapon".into(),
        family: String::new(),
    };
    let out = convert(&candidate, &ConvertAnswers::default(), &env);
    assert!(out.plan.files.is_empty());
    assert_eq!(out.plan.diagnostics.len(), 1);
}

// ---------------------------------------------------------------------------------------------------------
// the simulation hook

#[test]
fn the_simulation_reports_parameters_it_does_not_apply_and_still_converts() {
    let model = model();
    let node = gun_node("RS_Platform");
    let spec = gun_spec("RS_Platform");
    let mut patch = gun_patch(&spec, &model, &Container::from_node(&node)).unwrap();
    // A weapon platform parameter is parsed but not applied by the merge.
    patch.operations[0].push_child(Node::with_text("isWeaponPlatform", "true"));
    let run = dry_apply(&scratch_defs(node), &[patch.patch_root()], &model);
    assert!(run.is_clean(), "{:?}", run.diagnostics);
    assert!(
        run.diagnostics
            .iter()
            .any(|d| d.code.as_str() == simulate::SIMULATION_UNSUPPORTED)
    );
    let def = run.def("ThingDef", "RS_Platform").unwrap();
    assert_eq!(class_count(def, "verbs", &model.classes.verb_properties), 1);
}

#[test]
fn the_simulation_reports_a_conversion_of_a_def_that_does_not_exist() {
    let model = model();
    let node = gun_node("RS_Other");
    let spec = gun_spec("RS_Absent");
    let patch = gun_patch(&spec, &model, &Container::unknown()).unwrap();
    let run = dry_apply(&scratch_defs(node), &[patch.patch_root()], &model);
    assert!(!run.is_clean());
    assert_eq!(run.failed, 2);
    assert!(
        run.diagnostics
            .iter()
            .all(|d| d.code.as_str() == "ce.dry-run-failed")
    );
}

#[test]
fn the_registered_simulation_has_the_class_of_the_conversion_operation() {
    use rimstudio_defs::CustomPatchOp;
    let sim = MakeGunCeSimulation::new(&model().classes);
    assert_eq!(sim.class(), model().classes.make_gun_op);
}

#[test]
fn a_rimstudio_owned_source_is_updated_without_the_load_after_hint() {
    let model = model();
    let node = gun_node("RS_Own");
    let spec = gun_spec("RS_Own");
    let first = gun_patch(&spec, &model, &Container::from_node(&node)).unwrap();
    let run = dry_apply(&scratch_defs(node), &[first.patch_root()], &model);
    let converted = run.def("ThingDef", "RS_Own").unwrap().clone();
    let existing = ExistingConversion::from_def(
        &record(&converted),
        &model.classes,
        ConversionSource::RimStudio(Some("CE/Patches/Weapons_Ranged.xml".into())),
    )
    .unwrap();
    let container = Container::from_node(&converted).with_existing(existing);
    let mut changed = spec;
    changed.ce.as_mut().unwrap().sway_factor = Some(Sourced::typed(1.9));
    let update = gun_patch(&changed, &model, &container).unwrap();
    assert_eq!(update.operations.len(), 1);
    assert!(
        update
            .diagnostics
            .iter()
            .all(|d| d.code.as_str() != "ce.update-load-after")
    );
    assert!(matches!(
        update.source,
        Some(ConversionSource::RimStudio(_))
    ));
}

/// A hand written conversion of the usual shape: a sequence that starts with the Combat Extended mod gate.
fn hand_written_sequence(gate_name: &str, def_name: &str) -> Node {
    let classes = crate::ce::reader::CeClassNames::default();
    let gate = Node::builder("li")
        .attr("Class", "CombatExtended.PatchOperationFindMod")
        .text_elem("modName", gate_name)
        .build();
    let convert = Node::builder("li")
        .attr("Class", classes.make_gun_op)
        .text_elem("defName", def_name)
        .elem("statBases", |s| s.text_elem("Bulk", "7"))
        .build();
    let mut operations = Node::new("operations");
    operations.push_child(gate);
    operations.push_child(convert);
    let mut sequence = Node::new("Operation");
    sequence.set_attr("Class", "PatchOperationSequence");
    sequence.push_child(Node::with_text("success", "Always"));
    sequence.push_child(operations);
    let mut patch = Node::new("Patch");
    patch.push_child(sequence);
    patch
}

#[test]
fn a_hand_written_conversion_behind_the_combat_extended_mod_gate_is_applied() {
    let model = model();
    let node = gun_node("RS_HandGun");
    let patch = hand_written_sequence("Combat Extended", "RS_HandGun");
    let run = dry_apply(&scratch_defs(node), &[patch], &model);
    let def = run.def("ThingDef", "RS_HandGun").unwrap();
    assert_eq!(
        def.child("statBases").unwrap().child_text("Bulk"),
        Some("7"),
        "the gate must find the active mod so that the conversion runs"
    );
}

#[test]
fn the_combat_extended_mod_gate_compares_the_exact_name_and_fails_for_other_names() {
    let model = model();
    for name in [
        "combat extended",
        " Combat Extended",
        "ceteam.combatextended",
        "",
    ] {
        let node = gun_node("RS_HandGun");
        let patch = hand_written_sequence(name, "RS_HandGun");
        let run = dry_apply(&scratch_defs(node), &[patch], &model);
        let def = run.def("ThingDef", "RS_HandGun").unwrap();
        assert_eq!(
            def.child("statBases").unwrap().child_text("Bulk"),
            None,
            "the gate with the name {name:?} must not match"
        );
    }
}

#[test]
fn every_derived_number_of_a_plan_is_reported_as_an_info_diagnostic() {
    let model = model();
    let spec = gun_spec("RS_NewGun");
    let patch = gun_patch(&spec, &model, &Container::unknown()).unwrap();
    assert!(!patch.derived.is_empty(), "the fixture must derive numbers");
    let plan = export_ce_plan(&spec, &model, &ProjectLayout::default());
    let reported: Vec<&str> = plan
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "ce.derived-value")
        .filter_map(|d| d.args.get("field").map(String::as_str))
        .collect();
    assert_eq!(reported.len(), patch.derived.len());
    for d in &patch.derived {
        assert!(reported.contains(&d.field.as_str()), "{}", d.field);
    }
}

/// The fictional library with a stat that varies wildly between conversions, unrelated to anything.
fn noisy_model(stat: &str, scale: f64) -> CeModel {
    let mut model = model();
    let noise = [0.4, 3.1, 0.9, 2.2, 0.5, 2.9, 1.1, 0.3, 2.6, 1.7, 0.6, 3.3];
    for (g, n) in model.guns.iter_mut().zip(noise) {
        g.stats.insert(stat.to_owned(), n * scale);
    }
    model
}

fn answered() -> ConvertAnswers {
    ConvertAnswers {
        ammo_set: Some("RS_AmmoSetA".into()),
        weapon_tag_class: Some("CE_AI_RS_Rifle".into()),
        one_handed: Some(false),
        belt_fed: Some(false),
        ..ConvertAnswers::default()
    }
}

#[test]
fn a_number_rated_unreliable_is_asked_with_its_reason_and_a_reference_guess() {
    let model = noisy_model("magazine", 10.0);
    let mut spec = gun_spec("RS_NewGun");
    spec.ce = None;
    let (block, asks, derived) = derive_ce_block(&spec, &model, &answered());
    // The magazine is asked, never written: the block holds nothing for it.
    assert!(block.magazine_size.is_none());
    let ask = asks
        .items
        .iter()
        .find(|a| a.field == "/ce/magazineSize")
        .expect("the magazine is asked");
    let reason = ask.reason.as_deref().unwrap_or_default();
    assert!(reason.contains("unreliable"), "{reason}");
    assert!(ask.suggestion.is_some_and(|s| s > 0.0));
    // A stat the library can measure well is still derived, with its rating recorded.
    assert!(!asks.asks("/ce/bulk"));
    let reload = derived.iter().find(|d| d.field == "ce.reloadTime").unwrap();
    assert!(reload.rating.is_some_and(|r| r.is_usable()));
    assert!(derived.iter().all(|d| d.field != "ce.magazineSize"));
}

#[test]
fn an_answer_to_a_rejected_estimate_is_used_as_given() {
    let model = noisy_model("magazine", 10.0);
    let mut spec = gun_spec("RS_NewGun");
    spec.ce = None;
    let mut answers = answered();
    answers.overrides.magazine_size = Some(Sourced::typed(17));
    let (block, asks, _) = derive_ce_block(&spec, &model, &answers);
    assert_eq!(block.magazine_size.map(|m| m.value), Some(17));
    assert!(!asks.asks("/ce/magazineSize"));
}

#[test]
fn a_stat_no_conversion_can_supply_is_asked_with_the_reason() {
    let mut model = model();
    for g in &mut model.guns {
        g.stats.remove("reload");
    }
    let mut spec = gun_spec("RS_NewGun");
    spec.ce = None;
    let (_, asks, _) = derive_ce_block(&spec, &model, &answered());
    let ask = asks
        .items
        .iter()
        .find(|a| a.field == "/ce/reloadTime")
        .unwrap();
    assert!(ask.suggestion.is_none());
    let reason = ask.reason.as_deref().unwrap_or_default();
    assert!(reason.contains("no estimate"), "{reason}");
}

#[test]
fn a_rejected_range_keeps_the_vanilla_number_and_the_plan_says_why() {
    let model = noisy_model("range", 20.0);
    let spec = gun_spec("RS_NewGun");
    let patch = gun_patch(&spec, &model, &Container::unknown()).unwrap();
    let range = patch.derived.iter().find(|d| d.field == "range").unwrap();
    assert_eq!(range.origin, ValueOrigin::Vanilla);
    assert!((range.value - 30.0).abs() < 1e-9);
    assert_eq!(
        range.rating,
        Some(crate::ce::classes::Reliability::Unreliable)
    );
    let plan = export_ce_plan(&spec, &model, &ProjectLayout::default());
    let said = plan
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "ce.derived-value")
        .any(|d| {
            d.args.get("field").map(String::as_str) == Some("range")
                && d.message.contains("rated unreliable")
        });
    assert!(said, "{:?}", plan.diagnostics);
}
