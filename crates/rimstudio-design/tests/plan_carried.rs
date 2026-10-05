//! Tests of the fields a clone carries: recipe, comps, raw fields, own projectile, omitted defaults and
//! accepted missing fields, through the vanilla write plan.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeMap;

use common::{assert_golden, ranged_spec};
use proptest::prelude::*;
use rimstudio_core::diag::Severity;
use rimstudio_core::tree::Node;
use rimstudio_design::model::{
    Draft, ExtraMeleeDamage, ProjectileChoice, ProjectileSpec, RecipeSpec, Sourced,
    SurpriseAttackSpec, TechLevel, ToolSpec, ValueSource,
};
use rimstudio_design::plan::{ProjectLayout, WritePlan, export_vanilla_plan};
use rimstudio_design::validation::{codes, validate_vanilla};

fn raw(tag: &str, text: &str) -> Node {
    Node::with_text(tag, text)
}

fn comp(class: &str, field: &str, text: &str) -> Node {
    let mut li = Node::new("li");
    let _ = li.set_attr("Class", class);
    li.push_child(raw(field, text));
    li
}

/// A ranged spec that carries every kind of field a clone carries.
fn carried_spec() -> rimstudio_design::model::DesignSpec {
    let mut s = ranged_spec();
    s.parent.as_mut().unwrap().inherited_tech_level = Some(TechLevel::Industrial);
    s.sound_interact = Some("RS_Click".into());
    s.recipe = Some(RecipeSpec {
        skill_requirements: BTreeMap::from([("RS_Crafting".to_owned(), 5)]),
        display_priority: Some(415.0),
        recipe_users: vec!["RS_Bench".into()],
        unfinished_thing_def: Some("RS_Unfinished".into()),
        work_skill: Some("RS_Crafting".into()),
        extra: vec![raw("soundWorking", "RS_Working")],
        attrs: BTreeMap::new(),
    });
    s.equipped_stat_offsets = BTreeMap::from([("MoveSpeed".to_owned(), -0.25)]);
    s.comps = vec![comp("RS_CompArt", "nameMaker", "RS_Namer")];
    s.ui_icon_path = Some("RS/Icon".into());
    s.ui_icon_scale = Some(1.2);
    s.draw_size = Some("(1.5,1.5)".into());
    s.graphic_color = Some("(0.8,0.8,0.8)".into());
    s.graphic_extra = vec![raw("shaderType", "CutoutComplex")];
    s.extra_fields = vec![raw("relicChance", "2"), raw("smeltable", "true")];
    s.extra_attrs = BTreeMap::from([("MayRequire".to_owned(), "rs.dlc".to_owned())]);
    s.tools[0].extra_melee_damages = vec![ExtraMeleeDamage {
        def: "Stun".into(),
        amount: Some(14.0),
        chance: Some(0.5),
    }];
    s.tools[0].surprise_attack = Some(SurpriseAttackSpec {
        extra_melee_damages: vec![ExtraMeleeDamage {
            def: "Stun".into(),
            amount: Some(14.0),
            chance: None,
        }],
    });
    s.tools[0].extra = vec![raw("labelUsedInLogging", "false")];
    let r = s.ranged.as_mut().unwrap();
    r.forced_miss_radius = Some(0.5);
    r.verb_extra = vec![raw("aimingChargeMote", "RS_Mote")];
    r.projectile = Some(ProjectileChoice::Inline(ProjectileSpec {
        def_name: "RS_Bullet_TestRifle".into(),
        label: "test bullet".into(),
        parent: Some("RS_BaseBullet".into()),
        texture_path: Some("Things/RS/Bullet".into()),
        speed: Some(Sourced::typed(50.0)),
        extra: vec![raw("explosionRadius", "1.9")],
        graphic_extra: vec![raw("drawSize", "(0.5,0.5)")],
        thing_extra: vec![raw("thingClass", "RS_Bullet")],
        omit_defaults: vec!["damageDef".into(), "graphicClass".into()],
        copied_from: Some("RS_Shot".into()),
        ..ProjectileSpec::default()
    }));
    s
}

fn plan_of(spec: &rimstudio_design::model::DesignSpec) -> WritePlan {
    export_vanilla_plan(spec, &ProjectLayout::default())
}

fn defs(plan: &WritePlan) -> Vec<Node> {
    plan.files[0]
        .tree
        .as_ref()
        .unwrap()
        .elements()
        .cloned()
        .collect()
}

/// The weapon definition of a plan (the last def of the file).
fn weapon(plan: &WritePlan) -> Node {
    defs(plan).pop().unwrap()
}

fn projectile(plan: &WritePlan) -> Node {
    defs(plan).remove(0)
}

fn tags(node: &Node) -> Vec<&str> {
    node.elements().map(|e| e.tag.as_str()).collect()
}

#[test]
fn golden_carried_plan() {
    let plan = plan_of(&carried_spec());
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    let mut text = serde_json::to_string_pretty(&plan).unwrap();
    text.push('\n');
    assert_golden("ranged_carried.json", &text);
}

#[test]
fn every_modelled_field_is_written_in_the_documented_order() {
    let plan = plan_of(&carried_spec());
    let w = weapon(&plan);
    assert_eq!(w.attr("ParentName"), Some("RS_BaseGun"));
    assert_eq!(w.attr("MayRequire"), Some("rs.dlc"));
    assert_eq!(
        tags(&w),
        [
            "defName",
            "label",
            "description",
            "graphicData",
            "uiIconPath",
            "uiIconScale",
            "statBases",
            "equippedStatOffsets",
            "costList",
            "recipeMaker",
            "soundInteract",
            "verbs",
            "tools",
            "weaponTags",
            "comps",
            "relicChance",
            "smeltable"
        ]
    );
    assert_eq!(w.child_text("soundInteract"), Some("RS_Click"));
    assert_eq!(w.child_text("uiIconScale"), Some("1.2"));
    let offsets = w.child("equippedStatOffsets").unwrap();
    assert_eq!(offsets.child_text("MoveSpeed"), Some("-0.25"));
    let graphic = w.child("graphicData").unwrap();
    assert_eq!(
        tags(graphic),
        ["texPath", "graphicClass", "drawSize", "color", "shaderType"]
    );
    let recipe = w.child("recipeMaker").unwrap();
    assert_eq!(
        tags(recipe),
        [
            "researchPrerequisite",
            "skillRequirements",
            "displayPriority",
            "recipeUsers",
            "unfinishedThingDef",
            "workSkill",
            "soundWorking"
        ]
    );
    assert_eq!(
        recipe
            .child("skillRequirements")
            .unwrap()
            .child_text("RS_Crafting"),
        Some("5")
    );
    assert_eq!(recipe.child_text("displayPriority"), Some("415"));
    let comps = w.child("comps").unwrap();
    assert_eq!(
        comps.elements().next().unwrap().attr("Class"),
        Some("RS_CompArt")
    );
}

#[test]
fn the_verb_and_the_tool_carry_their_extras() {
    let plan = plan_of(&carried_spec());
    let w = weapon(&plan);
    let verb = w.child("verbs").unwrap().elements().next().unwrap();
    assert_eq!(verb.child_text("forcedMissRadius"), Some("0.5"));
    assert_eq!(tags(verb).last(), Some(&"aimingChargeMote"));
    let tool = w.child("tools").unwrap().elements().next().unwrap();
    assert_eq!(
        tags(tool),
        [
            "label",
            "capacities",
            "power",
            "cooldownTime",
            "extraMeleeDamages",
            "surpriseAttack",
            "labelUsedInLogging"
        ]
    );
    let damage = tool
        .child("extraMeleeDamages")
        .unwrap()
        .elements()
        .next()
        .unwrap();
    assert_eq!(damage.child_text("def"), Some("Stun"));
    assert_eq!(damage.child_text("amount"), Some("14"));
    assert_eq!(damage.child_text("chance"), Some("0.5"));
    let surprise = tool.child("surpriseAttack").unwrap();
    assert!(surprise.child("extraMeleeDamages").is_some());
}

#[test]
fn the_own_projectile_is_written_before_the_weapon_with_its_extras_and_without_omitted_fields() {
    let plan = plan_of(&carried_spec());
    assert_eq!(plan.files.len(), 1);
    let p = projectile(&plan);
    assert_eq!(p.attr("ParentName"), Some("RS_BaseBullet"));
    assert_eq!(p.child_text("defName"), Some("RS_Bullet_TestRifle"));
    let props = p.child("projectile").unwrap();
    assert!(props.child("damageDef").is_none(), "the parent supplies it");
    assert_eq!(props.child_text("damageAmountBase"), Some("11"));
    assert_eq!(
        tags(props),
        ["damageAmountBase", "speed", "explosionRadius"]
    );
    let graphic = p.child("graphicData").unwrap();
    assert_eq!(tags(graphic), ["texPath", "drawSize"]);
    assert_eq!(tags(&p).last(), Some(&"thingClass"));
    let w = weapon(&plan);
    let verb = w.child("verbs").unwrap().elements().next().unwrap();
    assert_eq!(
        verb.child_text("defaultProjectile"),
        Some("RS_Bullet_TestRifle")
    );
}

#[test]
fn the_tech_level_is_not_written_when_the_parent_supplies_it() {
    let mut s = carried_spec();
    assert!(weapon(&plan_of(&s)).child("techLevel").is_none());
    // another level than the parent's is written
    s.tech_level = Some(TechLevel::Spacer);
    assert_eq!(weapon(&plan_of(&s)).child_text("techLevel"), Some("Spacer"));
    // a spec without the parent's value writes it
    s.tech_level = Some(TechLevel::Industrial);
    s.parent.as_mut().unwrap().inherited_tech_level = None;
    assert_eq!(
        weapon(&plan_of(&s)).child_text("techLevel"),
        Some("Industrial")
    );
}

#[test]
fn a_stat_the_parent_supplies_is_written_only_when_it_changed_or_was_typed() {
    let mut s = carried_spec();
    s.parent
        .as_mut()
        .unwrap()
        .inherited_stats
        .extend([("Mass".to_owned(), 3.3), ("WorkToMake".to_owned(), 9100.0)]);
    // mass is typed in the fixture, so it is written; the work value is answered and equal, so it is not
    let stats = weapon(&plan_of(&s)).child("statBases").cloned().unwrap();
    assert_eq!(stats.child_text("Mass"), Some("3.3"));
    assert!(stats.child("WorkToMake").is_none());
    // a changed value is written
    s.work_to_make = Some(Sourced::new(9500.0, ValueSource::Anchor));
    let stats = weapon(&plan_of(&s)).child("statBases").cloned().unwrap();
    assert_eq!(stats.child_text("WorkToMake"), Some("9500"));
}

#[test]
fn defaults_the_source_does_not_write_are_left_out() {
    let mut s = carried_spec();
    s.omit_defaults = vec![
        "hasStandardCommand".into(),
        "verbClass".into(),
        "graphicClass".into(),
    ];
    let w = weapon(&plan_of(&s));
    let verb = w.child("verbs").unwrap().elements().next().unwrap();
    assert!(verb.child("hasStandardCommand").is_none());
    assert!(verb.child("verbClass").is_none());
    assert!(
        w.child("graphicData")
            .unwrap()
            .child("graphicClass")
            .is_none()
    );
    // a class that is set is still written
    s.graphic_class = Some("Graphic_Multi".into());
    let w = weapon(&plan_of(&s));
    assert_eq!(
        w.child("graphicData").unwrap().child_text("graphicClass"),
        Some("Graphic_Multi")
    );
}

#[test]
fn lists_the_source_resets_are_written_with_inherit_false() {
    let mut s = carried_spec();
    s.inherit_reset = vec![
        "comps".into(),
        "tools".into(),
        "verbs".into(),
        "recipeUsers".into(),
    ];
    let w = weapon(&plan_of(&s));
    for tag in ["comps", "tools", "verbs"] {
        assert_eq!(
            w.child(tag).unwrap().attr("Inherit"),
            Some("False"),
            "{tag}"
        );
    }
    let users = w
        .child("recipeMaker")
        .unwrap()
        .child("recipeUsers")
        .unwrap();
    assert_eq!(users.attr("Inherit"), Some("False"));
    assert!(w.child("weaponTags").unwrap().attr("Inherit").is_none());
}

#[test]
fn a_recipe_element_keeps_its_attributes_and_an_empty_one_is_still_written() {
    let mut s = carried_spec();
    s.research_prerequisite = None;
    s.recipe = Some(RecipeSpec {
        attrs: BTreeMap::from([("IsNull".to_owned(), "True".to_owned())]),
        ..RecipeSpec::default()
    });
    let w = weapon(&plan_of(&s));
    let recipe = w.child("recipeMaker").unwrap();
    assert_eq!(recipe.attr("IsNull"), Some("True"));
    assert!(recipe.elements().next().is_none());
    s.recipe = None;
    assert!(weapon(&plan_of(&s)).child("recipeMaker").is_none());
}

#[test]
fn a_parent_texture_keeps_the_reserved_path_out() {
    let mut s = carried_spec();
    s.texture_path = None;
    s.draw_size = None;
    s.graphic_color = None;
    s.graphic_extra.clear();
    s.omit_defaults = vec!["texPath".into()];
    let plan = plan_of(&s);
    assert!(weapon(&plan).child("graphicData").is_none());
    assert!(
        !plan
            .diagnostics
            .iter()
            .any(|d| d.code.as_str() == "design.texture-reserved")
    );
    // without the marker the reserved path is chosen and reported
    s.omit_defaults.clear();
    let plan = plan_of(&s);
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code.as_str() == "design.texture-reserved")
    );
}

#[test]
fn a_texture_that_is_not_the_reserved_one_is_reported_for_the_weapon_and_the_projectile() {
    let plan = plan_of(&carried_spec());
    let shared: Vec<_> = plan
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == codes::TEXTURE_SHARED.code)
        .collect();
    assert_eq!(shared.len(), 2, "{:?}", plan.diagnostics);
    assert!(shared.iter().all(|d| d.severity == Severity::Info));
    assert!(shared[0].message.contains("Things/RS/TestRifle"));
    assert!(
        shared[0]
            .message
            .contains("Textures/Things/Item/Equipment/WeaponRanged/RS_TestRifle.png")
    );
    assert!(
        shared[1]
            .message
            .contains("Textures/Things/Projectile/RS_Bullet_TestRifle.png")
    );
}

#[test]
fn a_required_field_the_source_lacks_is_accepted_and_reported_as_information() {
    let mut s = carried_spec();
    s.cost_list.clear();
    s.work_to_make = None;
    assert!(plan_of(&s).has_errors());
    s.accepted_missing = vec!["/costList".into(), "/workToMake".into()];
    let plan = plan_of(&s);
    assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
    let accepted: Vec<_> = plan
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == codes::ACCEPTED_MISSING.code)
        .collect();
    assert_eq!(accepted.len(), 2);
    assert!(accepted.iter().all(|d| d.severity == Severity::Info));
    let w = weapon(&plan);
    assert!(w.child("costList").is_none());
    assert!(w.child("statBases").unwrap().child("WorkToMake").is_none());
}

#[test]
fn an_accepted_pointer_never_hides_another_missing_field() {
    let mut s = carried_spec();
    s.mass = None;
    s.accepted_missing = vec!["/workToMake".into()];
    assert!(
        validate_vanilla(&s)
            .iter()
            .any(|d| d.code.as_str() == codes::REQUIRED_MISSING.code)
    );
}

fn error_codes(spec: &rimstudio_design::model::DesignSpec) -> Vec<String> {
    validate_vanilla(spec)
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.code.as_str().to_owned())
        .collect()
}

#[test]
fn a_carried_field_that_repeats_a_modelled_one_is_an_error() {
    let mut s = carried_spec();
    assert!(error_codes(&s).is_empty());
    s.extra_fields.push(raw("statBases", ""));
    assert_eq!(error_codes(&s), [codes::EXTRA_FIELD_CONFLICT.code]);
    let plan = plan_of(&s);
    assert!(plan.files.is_empty(), "an error plan has no files");
    let mut s = carried_spec();
    s.ranged
        .as_mut()
        .unwrap()
        .verb_extra
        .push(raw("range", "1"));
    assert_eq!(error_codes(&s), [codes::EXTRA_FIELD_CONFLICT.code]);
    let mut s = carried_spec();
    s.tools[0].extra.push(raw("power", "1"));
    assert_eq!(error_codes(&s), [codes::EXTRA_FIELD_CONFLICT.code]);
    let mut s = carried_spec();
    s.graphic_extra.push(raw("texPath", "x"));
    assert_eq!(error_codes(&s), [codes::EXTRA_FIELD_CONFLICT.code]);
}

#[test]
fn a_carried_node_that_is_not_valid_xml_or_not_a_list_entry_is_an_error() {
    let mut s = carried_spec();
    s.extra_fields.push(Node::new("1bad name"));
    assert_eq!(error_codes(&s), [codes::EXTRA_FIELD_INVALID.code]);
    let mut s = carried_spec();
    s.comps.push(raw("notAnEntry", "x"));
    assert_eq!(error_codes(&s), [codes::EXTRA_FIELD_INVALID.code]);
    let mut s = carried_spec();
    s.other_verbs.push(raw("li", "x"));
    s.other_verbs.push(raw("verb", "x"));
    assert_eq!(error_codes(&s), [codes::EXTRA_FIELD_INVALID.code]);
}

#[test]
fn bad_numbers_and_names_of_carried_fields_are_errors() {
    let mut s = carried_spec();
    s.ui_icon_scale = Some(0.0);
    assert_eq!(error_codes(&s), [codes::VALUE_INVALID.code]);
    let mut s = carried_spec();
    s.equipped_stat_offsets.insert("Bad Name".into(), 1.0);
    assert_eq!(error_codes(&s), [codes::NAME_INVALID.code]);
    let mut s = carried_spec();
    s.inherit_reset.push("category".into());
    assert_eq!(error_codes(&s), [codes::VALUE_INVALID.code]);
    let mut s = carried_spec();
    s.ranged.as_mut().unwrap().forced_miss_radius = Some(-1.0);
    assert_eq!(error_codes(&s), [codes::VALUE_INVALID.code]);
}

#[test]
fn carried_text_with_a_combat_extended_class_never_reaches_a_vanilla_file() {
    let mut s = carried_spec();
    s.comps
        .push(comp("CombatExtended.CompProperties_AmmoUser", "x", "1"));
    let plan = plan_of(&s);
    assert!(plan.has_errors());
    assert!(plan.files.is_empty());
}

#[test]
fn the_plan_of_a_carried_spec_is_deterministic() {
    let a = serde_json::to_string(&plan_of(&carried_spec())).unwrap();
    let b = serde_json::to_string(&plan_of(&carried_spec())).unwrap();
    assert_eq!(a, b);
}

#[test]
fn a_spec_without_carried_fields_writes_none_of_them() {
    let w = weapon(&plan_of(&ranged_spec()));
    for tag in [
        "soundInteract",
        "comps",
        "equippedStatOffsets",
        "uiIconPath",
        "uiIconScale",
    ] {
        assert!(w.child(tag).is_none(), "{tag}");
    }
    assert_eq!(
        w.child("recipeMaker").map(tags),
        Some(vec!["researchPrerequisite"])
    );
    let _ = ToolSpec::default();
}

// ---------------------------------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------------------------------

fn raw_strategy() -> impl Strategy<Value = Node> {
    let name = "[a-zA-Z][a-zA-Z0-9]{0,8}".prop_filter("not a modelled field", |n| {
        !rimstudio_design::model::MODELLED_THING_FIELDS.contains(&n.as_str())
    });
    let text = "[a-zA-Z0-9 .,_-]{0,16}";
    (
        name,
        text,
        proptest::option::of(("[A-Za-z]{1,6}", "[a-z0-9]{0,6}")),
    )
        .prop_map(|(tag, text, attr)| {
            let mut node = Node::with_text(tag, text.trim());
            if let Some((n, v)) = attr {
                let _ = node.set_attr(n, v);
            }
            node
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn carried_raw_fields_survive_a_draft_save_load_and_the_plan(
        extras in proptest::collection::vec(raw_strategy(), 0..6),
        comps in proptest::collection::vec(raw_strategy(), 0..4),
    ) {
        let mut spec = carried_spec();
        spec.extra_fields = extras.clone();
        spec.comps = comps
            .iter()
            .map(|n| {
                let mut li = Node::new("li");
                li.push_child(n.clone());
                li
            })
            .collect();
        let draft = Draft::new(spec);
        let text = serde_json::to_string(&draft).unwrap();
        let back: Draft = serde_json::from_str(&text).unwrap();
        prop_assert_eq!(&back, &draft);
        let plan = plan_of(&back.spec);
        prop_assert!(!plan.has_errors(), "{:?}", plan.diagnostics);
        let w = weapon(&plan);
        let written: Vec<Node> = w.elements().skip(w.elements().count() - extras.len()).cloned().collect();
        prop_assert_eq!(written, extras);
        let written_comps: Vec<Node> = w
            .child("comps")
            .map(|c| c.elements().cloned().collect())
            .unwrap_or_default();
        prop_assert_eq!(written_comps, back.spec.comps.clone());
    }
}
