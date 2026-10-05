//! The Combat Extended conversion of bows and crossbows through the public API: detection, the shape of the
//! patch (a golden tree), idempotence, update mode, the habits learned from converted bows, the convert flow
//! derivation and the lint rules CEP030 to CEP033. Everything is fictional; the real install is compared by
//! `real_ce_fidelity`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod common_ce;

use std::collections::BTreeMap;

use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_design::ce::lint::{self, LintContext};
use rimstudio_design::ce::patchgen::bow::{
    ammo_gen_habit, bow_ammo_sets, bow_prediction, bow_tag_candidates, bow_tag_habit,
};
use rimstudio_design::ce::patchgen::{
    AskKind, Container, ConversionSource, ConvertAnswers, ExistingConversion, PatchMode,
    derive_ce_block, dry_apply, gun_patch, is_bow_spec,
};
use rimstudio_design::ce::reader::{
    AmmoSetInfo, AmmoType, CeClassNames, CeGun, CeModel, CeToolRow, ProjectileInfo,
};
use rimstudio_design::model::{
    CePatchSpec, CeToolPenetration, DesignSpec, ProjectileChoice, RangedInputs, Sourced, TechLevel,
    ToolSpec, ValueSource,
};
use rimstudio_design::validation::validate_ce_patch;

use common_ce::ce_model;

const BOW_TAG: &str = "CE_RS_Bow";
const ARROWS: &str = "RS_ArrowSet";

fn map(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
    pairs.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
}

fn bow(i: u32, tag: &str, ammo_gen: Option<f64>) -> CeGun {
    let f = f64::from(i);
    let mass = 1.0 + 0.2 * f;
    CeGun {
        def_name: format!("RS_CeBow{i:02}"),
        label: format!("ce bow {i}"),
        mod_idx: None,
        tech_level: Some(TechLevel::Neolithic),
        weapon_tags: vec!["RS_NeoRanged".into(), tag.into()],
        ce_tags: vec![tag.into()],
        ai_class: None,
        ammo_set: Some(ARROWS.into()),
        default_projectile: Some("RS_Arrow_CE".into()),
        reload_one_at_a_time: false,
        ai_aim_mode: None,
        use_burst_mode: None,
        aimed_burst: None,
        tools: vec![CeToolRow {
            label: String::new(),
            capacities: vec!["Blunt".into()],
            power: Some(7.0),
            cooldown: Some(1.6),
            ap_sharp: None,
            ap_blunt: Some(0.65),
            chance_factor: None,
            linked_body_parts_group: None,
        }],
        bow: true,
        ammo_gen_per_mag: ammo_gen,
        stats: map(&[
            ("mass", mass),
            ("bulk", 2.0 * mass + 1.0),
            ("spread", 1.0),
            ("sway", 2.0),
            ("sights", 0.8),
            ("cooldown", 1.0),
            ("warmup", 1.0 + 0.1 * f),
            ("range", 12.0 + 3.0 * f),
            ("burst", 1.0),
            ("damage", 12.0),
        ]),
        twin: Some(map(&[
            ("mass", mass),
            ("range", 20.0 + 4.0 * f),
            ("warmup", 1.5 + 0.1 * f),
            ("cooldown", 1.4),
            ("burst", 1.0),
            ("damage", 12.0),
        ])),
        twin_tags: vec!["RS_NeoRanged".into()],
        excluded: None,
    }
}

fn arrow_set(name: &str, ammo: &str, projectile: &str) -> AmmoSetInfo {
    AmmoSetInfo {
        def_name: name.into(),
        similar_to: None,
        ammo_types: vec![AmmoType {
            ammo: ammo.into(),
            projectile: projectile.into(),
            info: ProjectileInfo::default(),
        }],
    }
}

/// The fictional model with `n` converted bows, an arrow set they use, another set of bolts and the bow tag.
fn bow_model(n: u32) -> CeModel {
    let mut model = ce_model();
    model
        .ammo_sets
        .push(arrow_set(ARROWS, "RS_Ammo_Arrow", "RS_Arrow_CE"));
    model
        .ammo_sets
        .push(arrow_set("RS_BoltSet", "RS_Ammo_Bolt", "RS_Bolt_CE"));
    model.weapon_tags.push(BOW_TAG.into());
    model
        .guns
        .extend((0..n).map(|i| bow(i, BOW_TAG, Some(5.0))));
    model
}

fn bow_spec(name: &str) -> DesignSpec {
    let mut s = DesignSpec::new_ranged(name, "test bow");
    s.tech_level = Some(TechLevel::Neolithic);
    s.role = Some("bow".into());
    s.mass = Some(Sourced::typed(1.2));
    s.weapon_tags = vec!["RS_NeoRanged".into()];
    s.ranged = Some(RangedInputs {
        range: Some(Sourced::typed(24.0)),
        warmup: Some(Sourced::typed(1.2)),
        cooldown: Some(Sourced::typed(1.2)),
        sound_cast: Some("RS_BowSound".into()),
        projectile: Some(ProjectileChoice::Reference("RS_Arrow".into())),
        ..RangedInputs::default()
    });
    s.tools =
        vec![ToolSpec::new("limb", &["Blunt", "Poke"]).with_numbers(9.0, 2.0, ValueSource::Typed)];
    s.ce = Some(CePatchSpec {
        ammo_set: Some(ARROWS.into()),
        default_projectile: Some("RS_Arrow_CE".into()),
        weapon_tag_class: Some(BOW_TAG.into()),
        bulk: Some(Sourced::typed(3.0)),
        sway_factor: Some(Sourced::typed(2.0)),
        shot_spread: Some(Sourced::typed(1.0)),
        sights_efficiency: Some(Sourced::typed(0.8)),
        tool_penetration: vec![CeToolPenetration {
            tool: "limb".into(),
            sharp: None,
            blunt: Some(Sourced::typed(0.65)),
        }],
        ..CePatchSpec::default()
    });
    s
}

fn raw_bow(name: &str) -> Node {
    NodeBuilder::new("ThingDef")
        .text_elem("defName", name)
        .elem("statBases", |s| {
            s.text_elem("Mass", "1.2").text_elem("AccuracyTouch", "0.6")
        })
        .elem("verbs", |v| {
            v.child(
                NodeBuilder::new("li")
                    .text_elem("verbClass", "Verb_Shoot")
                    .text_elem("defaultProjectile", "RS_Arrow")
                    .text_elem("range", "24")
                    .build(),
            )
        })
        .elem("tools", |t| {
            t.child(
                NodeBuilder::new("li")
                    .text_elem("label", "limb")
                    .elem("capacities", |c| c.li("Blunt"))
                    .build(),
            )
        })
        .build()
}

fn classes() -> CeClassNames {
    CeClassNames::default()
}

fn conversion(patch: &rimstudio_design::ce::patchgen::GeneratedPatch) -> &Node {
    patch.gun_conversion(&classes()).expect("a conversion")
}

fn count_class(def: &Node, list: &str, class: &str) -> usize {
    def.child(list)
        .map(|l| {
            l.children_named("li")
                .filter(|li| li.attr("Class") == Some(class))
                .count()
        })
        .unwrap_or(0)
}

fn json(node: &Node) -> String {
    let mut text = serde_json::to_string_pretty(node).unwrap();
    text.push('\n');
    text
}

// ---------------------------------------------------------------------------------------------------------
// detection

#[test]
fn a_bow_is_told_by_role_tag_class_or_projectile_and_the_block_can_overrule() {
    let mut by_role = bow_spec("RS_A");
    by_role.ce = None;
    assert!(is_bow_spec(&by_role));
    let mut by_tag = bow_spec("RS_B");
    by_tag.role = Some("rifle".into());
    by_tag.weapon_tags = vec!["RS_Crossbow".into()];
    assert!(is_bow_spec(&by_tag));
    let mut by_class = bow_spec("RS_C");
    by_class.role = None;
    by_class.weapon_tags.clear();
    by_class.weapon_classes = vec!["RS_ShortBow".into()];
    assert!(is_bow_spec(&by_class));
    let mut by_projectile = bow_spec("RS_D");
    by_projectile.role = None;
    by_projectile.weapon_tags.clear();
    assert!(is_bow_spec(&by_projectile), "the projectile is RS_Arrow");
    by_projectile.ranged.as_mut().unwrap().projectile =
        Some(ProjectileChoice::Reference("RS_Bullet".into()));
    assert!(!is_bow_spec(&by_projectile));
    // the block decides when it says so
    by_projectile.ce.as_mut().unwrap().bow = Some(true);
    assert!(is_bow_spec(&by_projectile));
    let mut forced_gun = bow_spec("RS_E");
    forced_gun.ce.as_mut().unwrap().bow = Some(false);
    assert!(!is_bow_spec(&forced_gun));
    assert!(!is_bow_spec(&DesignSpec::new_melee("RS_M", "m")));
}

// ---------------------------------------------------------------------------------------------------------
// the patch

#[test]
fn the_bow_patch_matches_its_golden_tree() {
    let spec = bow_spec("RS_TestBow");
    let patch = gun_patch(
        &spec,
        &bow_model(6),
        &Container::from_node(&raw_bow("RS_TestBow")),
    )
    .unwrap();
    assert_eq!(patch.mode, PatchMode::New);
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    common::assert_golden("ce_bow_patch.json", &json(&patch.patch_root()));
}

#[test]
fn the_bow_conversion_has_no_magazine_recoil_or_fire_mode_and_forbids_run_and_gun() {
    let spec = bow_spec("RS_TestBow");
    let model = bow_model(6);
    let patch = gun_patch(&spec, &model, &Container::from_node(&raw_bow("RS_TestBow"))).unwrap();
    let make = conversion(&patch);
    let names: Vec<&str> = make.elements().map(|e| e.tag.as_str()).collect();
    assert_eq!(
        names,
        [
            "defName",
            "statBases",
            "Properties",
            "AmmoUser",
            "FireModes",
            "weaponTags",
            "AllowWithRunAndGun"
        ]
    );
    let ammo = make.child("AmmoUser").unwrap();
    assert_eq!(ammo.child_text("ammoSet"), Some(ARROWS));
    assert!(ammo.child("magazineSize").is_none());
    assert!(ammo.child("reloadTime").is_none());
    // the block has no spawn count: the one the six library bows agree on is written and reported
    assert_eq!(ammo.child_text("AmmoGenPerMagOverride"), Some("5"));
    assert!(
        patch
            .derived
            .iter()
            .any(|d| d.field == "ammoGenPerMagOverride")
    );
    assert_eq!(make.child("FireModes").unwrap().elements().count(), 0);
    let props = make.child("Properties").unwrap();
    assert!(props.child("recoilAmount").is_none());
    assert!(props.child("burstShotCount").is_none());
    assert_eq!(props.child_text("soundCast"), Some("RS_BowSound"));
    assert_eq!(make.child_text("AllowWithRunAndGun"), Some("false"));
    assert_eq!(
        make.child("weaponTags")
            .unwrap()
            .children_named("li")
            .count(),
        1
    );
}

#[test]
fn the_mass_of_a_bow_is_written_only_when_it_differs_from_the_vanilla_mass() {
    let model = bow_model(0);
    let mut spec = bow_spec("RS_TestBow");
    spec.ce.as_mut().unwrap().mass = Some(Sourced::typed(1.2));
    let patch = gun_patch(&spec, &model, &Container::unknown()).unwrap();
    assert!(
        conversion(&patch)
            .child("statBases")
            .unwrap()
            .child("Mass")
            .is_none()
    );
    spec.ce.as_mut().unwrap().mass = Some(Sourced::typed(1.5));
    let patch = gun_patch(&spec, &model, &Container::unknown()).unwrap();
    assert_eq!(
        conversion(&patch)
            .child("statBases")
            .unwrap()
            .child_text("Mass"),
        Some("1.5")
    );
}

#[test]
fn a_magazine_a_reload_and_a_spawn_count_are_written_when_the_block_gives_them() {
    let mut spec = bow_spec("RS_TestBow");
    {
        let ce = spec.ce.as_mut().unwrap();
        ce.magazine_size = Some(Sourced::typed(1));
        ce.reload_time = Some(Sourced::typed(4.0));
        ce.ammo_gen_per_mag = Some(Sourced::typed(2));
        ce.recoil_amount = Some(Sourced::typed(0.5));
    }
    let patch = gun_patch(&spec, &bow_model(0), &Container::unknown()).unwrap();
    let make = conversion(&patch);
    let ammo = make.child("AmmoUser").unwrap();
    assert_eq!(ammo.child_text("magazineSize"), Some("1"));
    assert_eq!(ammo.child_text("reloadTime"), Some("4"));
    assert_eq!(ammo.child_text("AmmoGenPerMagOverride"), Some("2"));
    assert_eq!(
        make.child("Properties").unwrap().child_text("recoilAmount"),
        Some("0.5")
    );
}

#[test]
fn run_and_gun_is_written_only_to_forbid_it() {
    let mut spec = bow_spec("RS_TestBow");
    spec.ce.as_mut().unwrap().allow_with_run_and_gun = Some(true);
    let patch = gun_patch(&spec, &bow_model(0), &Container::unknown()).unwrap();
    assert!(conversion(&patch).child("AllowWithRunAndGun").is_none());
    spec.ce.as_mut().unwrap().allow_with_run_and_gun = Some(false);
    let patch = gun_patch(&spec, &bow_model(0), &Container::unknown()).unwrap();
    assert_eq!(
        conversion(&patch).child_text("AllowWithRunAndGun"),
        Some("false")
    );
}

#[test]
fn the_one_handed_mark_joins_the_bow_tag() {
    let model = bow_model(0);
    let mut spec = bow_spec("RS_TestBow");
    spec.ce.as_mut().unwrap().one_handed = true;
    let patch = gun_patch(&spec, &model, &Container::unknown()).unwrap();
    let tags: Vec<String> = conversion(&patch)
        .child("weaponTags")
        .unwrap()
        .children_named("li")
        .map(Node::text_content)
        .collect();
    assert_eq!(tags, [BOW_TAG, "RS_CE_OneHanded"]);
}

#[test]
fn a_bow_that_the_block_keeps_on_the_gun_conversion_needs_a_magazine() {
    let mut spec = bow_spec("RS_TestBow");
    spec.ce.as_mut().unwrap().bow = Some(false);
    let patch = gun_patch(&spec, &bow_model(0), &Container::unknown()).unwrap();
    assert!(patch.has_errors());
    assert!(
        patch
            .diagnostics
            .iter()
            .any(|d| d.message.contains("magazine size"))
    );
}

#[test]
fn a_bow_block_without_magazine_and_reload_validates() {
    let spec = bow_spec("RS_TestBow");
    assert!(
        validate_ce_patch(&spec).is_empty(),
        "{:?}",
        validate_ce_patch(&spec)
    );
    let mut incomplete = spec;
    incomplete.ce.as_mut().unwrap().ammo_set = None;
    incomplete.ce.as_mut().unwrap().bulk = None;
    let findings = validate_ce_patch(&incomplete);
    assert!(findings.iter().any(|d| d.message.contains("ammo set")));
    assert!(findings.iter().any(|d| d.message.contains("bulk")));
}

#[test]
fn a_bow_patch_names_an_ammo_set_the_model_does_not_know_as_an_error() {
    let mut spec = bow_spec("RS_TestBow");
    spec.ce.as_mut().unwrap().ammo_set = Some("RS_NoSuchSet".into());
    let patch = gun_patch(&spec, &bow_model(0), &Container::unknown()).unwrap();
    assert!(patch.has_errors());
    assert!(patch.operations.is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// applying: once, twice, inherited verbs, update

#[test]
fn a_bow_patch_applies_and_applied_twice_converts_once() {
    let model = bow_model(6);
    let spec = bow_spec("RS_TestBow");
    let node = raw_bow("RS_TestBow");
    let patch = gun_patch(&spec, &model, &Container::from_node(&node)).unwrap();
    let once = dry_apply(std::slice::from_ref(&node), &[patch.patch_root()], &model);
    let twice = dry_apply(&[node], &[patch.patch_root(), patch.patch_root()], &model);
    assert!(once.is_clean(), "{:?}", once.diagnostics);
    assert!(twice.is_clean(), "{:?}", twice.diagnostics);
    let a = once.def("ThingDef", "RS_TestBow").unwrap();
    let b = twice.def("ThingDef", "RS_TestBow").unwrap();
    assert_eq!(a, b);
    let c = classes();
    assert_eq!(count_class(b, "verbs", &c.verb_properties), 1);
    assert_eq!(b.child("verbs").unwrap().children_named("li").count(), 1);
    assert_eq!(count_class(b, "comps", &c.ammo_user), 1);
    assert_eq!(count_class(b, "comps", &c.fire_modes), 1);
    assert_eq!(b.child("tools").unwrap().children_named("li").count(), 1);
    // accuracy stats are gone, the converted stats are there, the mass is untouched
    let stats = b.child("statBases").unwrap();
    assert!(stats.child("AccuracyTouch").is_none());
    assert_eq!(stats.child_text("Bulk"), Some("3"));
    assert_eq!(stats.child_text("Mass"), Some("1.2"));
}

#[test]
fn a_bow_that_inherits_its_verbs_starts_a_list_of_its_own() {
    let spec = bow_spec("RS_Child");
    let mut node = raw_bow("RS_Child");
    node.set_attr("ParentName", "RS_Parent");
    node.children
        .retain(|c| !matches!(c, rimstudio_core::tree::Child::Element(e) if e.tag == "verbs"));
    let patch = gun_patch(&spec, &bow_model(0), &Container::from_node(&node)).unwrap();
    let first = &patch.operations[0];
    assert!(first.child_text("xpath").unwrap().ends_with("/verbs"));
}

#[test]
fn an_existing_bow_conversion_gets_update_operations_never_a_second_conversion() {
    let model = bow_model(6);
    let spec = bow_spec("RS_TestBow");
    let node = raw_bow("RS_TestBow");
    let patch = gun_patch(&spec, &model, &Container::from_node(&node)).unwrap();
    let converted = dry_apply(std::slice::from_ref(&node), &[patch.patch_root()], &model);
    let def = converted
        .defs
        .iter()
        .find(|d| d.child_text("defName") == Some("RS_TestBow"))
        .cloned()
        .unwrap();
    let record = rimstudio_defs::DefRecord {
        seq: 0,
        tag: "ThingDef".into(),
        type_name: "Verse.ThingDef".into(),
        def_name: "RS_TestBow".into(),
        origin: None,
        node: def,
        parents: Vec::new(),
        patched_by: Vec::new(),
    };
    let existing =
        ExistingConversion::from_def(&record, &classes(), ConversionSource::RimStudio(None))
            .unwrap();
    assert_eq!(existing.block.ammo_gen_per_mag.map(|a| a.value), Some(5));
    let container = Container::from_node(&node).with_existing(existing);
    // the same design changes nothing
    let same = gun_patch(&spec, &model, &container).unwrap();
    assert_eq!(same.mode, PatchMode::Update);
    assert!(same.operations.is_empty(), "{:?}", same.operations);
    // a new spawn count and a new bulk each become one operation
    let mut changed = spec;
    changed.ce.as_mut().unwrap().ammo_gen_per_mag = Some(Sourced::typed(8));
    changed.ce.as_mut().unwrap().bulk = Some(Sourced::typed(3.5));
    let update = gun_patch(&changed, &model, &container).unwrap();
    assert_eq!(update.mode, PatchMode::Update);
    assert!(update.gun_conversion(&classes()).is_none());
    let xpaths: Vec<String> = update
        .operations
        .iter()
        .filter_map(|o| o.child_text("xpath").map(str::to_owned))
        .collect();
    assert!(
        xpaths.iter().any(|x| x.ends_with("AmmoGenPerMagOverride")),
        "{xpaths:?}"
    );
    assert!(
        xpaths.iter().any(|x| x.ends_with("statBases/Bulk")),
        "{xpaths:?}"
    );
}

// ---------------------------------------------------------------------------------------------------------
// what the converted bows teach

#[test]
fn bow_estimates_come_from_the_bows_and_not_from_the_guns() {
    let spec = bow_spec("RS_TestBow");
    assert!(
        bow_prediction(&spec, &ce_model()).is_none(),
        "twelve guns, no bow"
    );
    let prediction = bow_prediction(&spec, &bow_model(6)).unwrap();
    let sway = prediction.stats.get("sway").unwrap();
    // every library bow has the same sway, a gun class would say 1.3
    assert!((sway.value - 2.0).abs() < 1e-9, "{}", sway.value);
}

#[test]
fn the_bow_tag_and_the_spawn_count_are_learned_only_when_enough_bows_agree() {
    assert_eq!(bow_tag_habit(&bow_model(3)).as_deref(), Some(BOW_TAG));
    assert_eq!(ammo_gen_habit(&bow_model(3)), Some(5));
    assert_eq!(bow_tag_habit(&bow_model(1)), None);
    assert_eq!(ammo_gen_habit(&bow_model(1)), None);
    assert_eq!(bow_tag_habit(&ce_model()), None);
    // two tags, one bow each: no winner
    let mut split = bow_model(0);
    split.guns.push(bow(0, "CE_RS_Bow", None));
    split.guns.push(bow(1, "CE_RS_Crossbow", None));
    assert_eq!(bow_tag_habit(&split), None);
    assert_eq!(bow_tag_candidates(&split).len(), 2);
    // a minority tag does not win
    let mut mixed = bow_model(0);
    mixed.guns.extend((0..2).map(|i| bow(i, "CE_RS_Bow", None)));
    mixed
        .guns
        .extend((2..5).map(|i| bow(i, "CE_RS_Other", None)));
    assert_eq!(bow_tag_habit(&mixed).as_deref(), Some("CE_RS_Other"));
    assert_eq!(ammo_gen_habit(&mixed), None);
    // the one handed mark and the AI class are not bow tags
    let mut odd = bow_model(3);
    for g in &mut odd.guns {
        if g.bow {
            g.ce_tags.push("CE_RS_OneHanded".into());
            g.ce_tags.push("CE_AI_RS_Class".into());
        }
    }
    assert_eq!(bow_tag_candidates(&odd), [(BOW_TAG.to_owned(), 3)]);
}

#[test]
fn the_arrow_sets_come_first_and_every_set_is_listed_when_none_reads_as_arrows() {
    let model = bow_model(2);
    assert_eq!(bow_ammo_sets(&model), [ARROWS, "RS_BoltSet"]);
    let mut other = ce_model();
    other.ammo_sets = vec![
        arrow_set("RS_SetB", "RS_Ammo_B", "RS_Proj_B"),
        arrow_set("RS_SetA", "RS_Ammo_A", "RS_Proj_A"),
    ];
    assert_eq!(bow_ammo_sets(&other), ["RS_SetA", "RS_SetB"]);
}

// ---------------------------------------------------------------------------------------------------------
// the convert flow

fn twin_spec() -> DesignSpec {
    let mut spec = bow_spec("RS_TwinBow");
    spec.ce = None;
    spec
}

#[test]
fn the_derivation_asks_for_the_arrow_set_and_never_for_a_magazine() {
    let (block, asks, _) = derive_ce_block(&twin_spec(), &bow_model(0), &ConvertAnswers::default());
    let ammo = asks
        .items
        .iter()
        .find(|a| a.field == "/ce/ammoSet")
        .unwrap();
    assert_eq!(ammo.kind, AskKind::Choice);
    assert_eq!(ammo.options.first().map(String::as_str), Some(ARROWS));
    for field in [
        "/ce/magazineSize",
        "/ce/reloadTime",
        "/ce/oneHanded",
        "/ce/beltFed",
        "/ce/recoilAmount",
    ] {
        assert!(!asks.asks(field), "{field}");
    }
    assert_eq!(block.bow, Some(true));
    // no converted bow, no estimate: the numbers are asked with the reason
    for field in ["/ce/bulk", "/ce/swayFactor", "/ce/shotSpread"] {
        let ask = asks.items.iter().find(|a| a.field == field).unwrap();
        assert!(ask.reason.is_some(), "{field}");
    }
    assert!(asks.asks("/ce/weaponTagClass"));
}

#[test]
fn with_converted_bows_the_derivation_decides_the_tag_the_projectile_and_the_spawn_count() {
    let answers = ConvertAnswers {
        ammo_set: Some(ARROWS.into()),
        ..ConvertAnswers::default()
    };
    let (block, asks, derived) = derive_ce_block(&twin_spec(), &bow_model(6), &answers);
    assert_eq!(block.weapon_tag_class.as_deref(), Some(BOW_TAG));
    assert_eq!(block.default_projectile.as_deref(), Some("RS_Arrow_CE"));
    assert_eq!(block.ammo_gen_per_mag.map(|a| a.value), Some(5));
    assert!(block.magazine_size.is_none() && block.reload_time.is_none());
    assert!(!asks.asks("/ce/weaponTagClass"));
    assert!(!asks.asks("/ce/ammoSet"));
    // six bows are enough to measure: sway is constant in the library and is derived
    assert!(!asks.asks("/ce/swayFactor"), "{asks:?}");
    assert_eq!(block.sway_factor.map(|s| s.value), Some(2.0));
    assert!(derived.iter().any(|d| d.field == "ce.swayFactor"));
}

#[test]
fn an_answer_or_an_override_wins_over_the_derivation() {
    let answers = ConvertAnswers {
        ammo_set: Some("RS_BoltSet".into()),
        weapon_tag_class: Some("RS_Mine".into()),
        overrides: CePatchSpec {
            ammo_gen_per_mag: Some(Sourced::answered(12)),
            allow_with_run_and_gun: Some(true),
            sway_factor: Some(Sourced::answered(3.0)),
            ..CePatchSpec::default()
        },
        ..ConvertAnswers::default()
    };
    let (block, _, _) = derive_ce_block(&twin_spec(), &bow_model(6), &answers);
    assert_eq!(block.weapon_tag_class.as_deref(), Some("RS_Mine"));
    assert_eq!(block.default_projectile.as_deref(), Some("RS_Bolt_CE"));
    assert_eq!(block.ammo_gen_per_mag.map(|a| a.value), Some(12));
    assert_eq!(block.allow_with_run_and_gun, Some(true));
    assert_eq!(block.sway_factor.map(|s| s.value), Some(3.0));
}

#[test]
fn the_override_can_keep_a_bow_shaped_weapon_on_the_gun_conversion() {
    let answers = ConvertAnswers {
        overrides: CePatchSpec {
            bow: Some(false),
            ..CePatchSpec::default()
        },
        ..ConvertAnswers::default()
    };
    let (block, asks, _) = derive_ce_block(&twin_spec(), &bow_model(0), &answers);
    assert_eq!(block.bow, Some(false));
    // the gun conversion needs a magazine, which the twelve library guns let it estimate
    assert!(block.magazine_size.is_some() && block.reload_time.is_some());
    assert!(asks.asks("/ce/oneHanded"));
}

// ---------------------------------------------------------------------------------------------------------
// lint

fn lint_codes(op: Node, model: &CeModel) -> Vec<String> {
    let mut root = Node::new("Patch");
    root.push_child(op);
    let load_folders = NodeBuilder::new("loadFolders")
        .elem("v1.6", |b| {
            b.li("/").child({
                let mut e = Node::with_text("li", "CE");
                e.set_attr("IfModActive", "RS.CombatExt");
                e
            })
        })
        .build();
    let ctx = LintContext {
        paths: vec!["CE/Patches/Bow.xml".to_owned()],
        load_folders: Some(load_folders),
        ..LintContext::default()
    };
    lint::run(&[root], model, &ctx)
        .into_iter()
        .map(|d| d.code.as_str().to_owned())
        .collect()
}

fn make_bow_op(set: &str, with_run_and_gun: bool, with_count: bool, modes: bool) -> Node {
    let c = classes();
    let mut b = common_ce::op(&c.make_gun_op)
        .text_elem("defName", "RS_HandBow")
        .elem("statBases", |s| {
            s.text_elem("Bulk", "3").text_elem("ShotSpread", "1")
        })
        .elem("Properties", |p| {
            p.text_elem("verbClass", &c.shoot_verb)
                .text_elem("hasStandardCommand", "true")
                .text_elem("defaultProjectile", "RS_Arrow_CE")
        })
        .elem("AmmoUser", |a| {
            let a = a.text_elem("ammoSet", set);
            if with_count {
                a.text_elem("AmmoGenPerMagOverride", "5")
            } else {
                a
            }
        });
    b = if modes {
        b.elem("FireModes", |f| f.text_elem("aiAimMode", "Snapshot"))
    } else {
        b.empty_elem("FireModes")
    };
    b = b.elem("weaponTags", |t| t.li(BOW_TAG));
    if with_run_and_gun {
        b = b.text_elem("AllowWithRunAndGun", "false");
    }
    common_ce::op("PatchOperationConditional")
        .text_elem("xpath", "Defs/ThingDef[defName=\"RS_HandBow\"]/comps")
        .child({
            let mut n = b.build();
            n.tag = "nomatch".into();
            n
        })
        .build()
}

#[test]
fn a_complete_bow_conversion_lints_clean_and_needs_no_magazine() {
    let codes = lint_codes(make_bow_op(ARROWS, true, true, false), &bow_model(2));
    assert!(codes.iter().all(|c| c == "ce.not-checked"), "{codes:?}");
}

#[test]
fn cep030_names_a_bow_with_an_ammo_set_that_serves_no_arrows() {
    let mut model = bow_model(2);
    model
        .ammo_sets
        .push(arrow_set("RS_PelletSet", "RS_Ammo_Pellet", "RS_Pellet_CE"));
    let codes = lint_codes(make_bow_op("RS_PelletSet", true, true, false), &model);
    assert!(
        codes.contains(&"ce.cep030-bow-ammo-not-arrows".to_owned()),
        "{codes:?}"
    );
    let ok = lint_codes(make_bow_op("RS_BoltSet", true, true, false), &model);
    assert!(!ok.contains(&"ce.cep030-bow-ammo-not-arrows".to_owned()));
}

#[test]
fn cep031_hints_at_fire_modes_on_a_bow() {
    let codes = lint_codes(make_bow_op(ARROWS, true, true, true), &bow_model(2));
    assert!(
        codes.contains(&"ce.cep031-bow-fire-modes".to_owned()),
        "{codes:?}"
    );
}

#[test]
fn cep032_hints_at_a_bow_that_may_be_fired_while_running() {
    let codes = lint_codes(make_bow_op(ARROWS, false, true, false), &bow_model(2));
    assert!(
        codes.contains(&"ce.cep032-bow-run-and-gun".to_owned()),
        "{codes:?}"
    );
}

#[test]
fn cep033_hints_at_a_bow_without_a_spawn_count() {
    let codes = lint_codes(make_bow_op(ARROWS, true, false, false), &bow_model(2));
    assert!(
        codes.contains(&"ce.cep033-bow-spawn-count".to_owned()),
        "{codes:?}"
    );
}

#[test]
fn a_gun_conversion_is_not_checked_by_the_bow_rules() {
    let model = bow_model(2);
    let mut root = Node::new("Patch");
    root.push_child(
        common_ce::op("PatchOperationConditional")
            .text_elem("xpath", "Defs/ThingDef[defName=\"RS_G\"]/comps")
            .child({
                let mut n = common_ce::make_gun("RS_G").build();
                n.tag = "nomatch".into();
                n
            })
            .build(),
    );
    let codes: Vec<String> = lint::run(&[root], &model, &LintContext::default())
        .into_iter()
        .map(|d| d.code.as_str().to_owned())
        .collect();
    assert!(
        codes.iter().all(|c| !c.starts_with("ce.cep03")),
        "{codes:?}"
    );
}
