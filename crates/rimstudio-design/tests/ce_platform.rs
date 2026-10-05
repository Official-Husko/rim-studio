//! Weapon platforms and under barrel units through the public API of the patch generator: golden node
//! trees, the gating rule over the plan (IT-052), the lint rules CEP040 to CEP046, and the rule that the
//! vanilla definition never learns about any of it (D-085). Every name is fictional.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod common_ce;

use std::collections::BTreeSet;

use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_design::ce::lint::{self, LintContext};
use rimstudio_design::ce::patchgen::platform::under_barrel_li;
use rimstudio_design::ce::patchgen::{
    Container, PatchMode, export_ce_plan, gate_violations, gun_patch,
};
use rimstudio_design::ce::reader::CeClassNames;
use rimstudio_design::model::{
    CeAttachmentLink, CeGraphicPart, CeStatEntry, CeUnderBarrel, CeUnderBarrelFireModes,
    DesignSpec, Sourced,
};
use rimstudio_design::plan::{ProjectLayout, export_vanilla_plan};

use common::ranged_spec;
use common_ce::{CLASS_TAG, ce_model, op, patch};

const ABILITY: &str = "CompProperties_EquippableAbilityReloadable";

fn unit() -> CeUnderBarrel {
    CeUnderBarrel {
        standard_label: Some("switch to rifle".into()),
        under_barrel_label: Some("switch to spray".into()),
        ammo_set: Some("RS_AmmoSetOther".into()),
        default_projectile: Some("RS_Bullet_Other".into()),
        magazine_size: Some(Sourced::typed(6)),
        reload_time: Some(Sourced::typed(5.0)),
        recoil_amount: Some(Sourced::typed(0.4)),
        warmup_time: Some(Sourced::typed(1.1)),
        range: Some(Sourced::typed(12.0)),
        min_range: Some(1.0),
        burst_shot_count: Some(5),
        ticks_between_burst_shots: Some(3),
        sound_cast: Some("RS_Spray".into()),
        muzzle_flash_scale: Some(0.0),
        verb_extra: vec![
            NodeBuilder::new("targetParams")
                .text_elem("canTargetLocations", "true")
                .build(),
        ],
        fire_modes: CeUnderBarrelFireModes {
            ai_use_burst_mode: Some(false),
            ai_aim_mode: Some("AimedShot".into()),
            aimed_burst_shot_count: Some(5),
            no_single_shot: true,
        },
        ..CeUnderBarrel::default()
    }
}

fn link() -> CeAttachmentLink {
    CeAttachmentLink {
        attachment: "RS_Scope".into(),
        draw_scale: Some("(1.2,1.2)".into()),
        draw_offset: Some("(0.1,0)".into()),
        stat_offsets: vec![CeStatEntry {
            stat: "RS_Spread".into(),
            value: -0.02,
        }],
        stat_multipliers: vec![CeStatEntry {
            stat: "RS_Sway".into(),
            value: 0.9,
        }],
        stat_replacers: Vec::new(),
    }
}

fn part() -> CeGraphicPart {
    CeGraphicPart {
        part_graphic: Some(
            NodeBuilder::new("partGraphicData")
                .text_elem("texPath", "Things/RS/Stock")
                .text_elem("graphicClass", "Graphic_Single")
                .build(),
        ),
        outline_graphic: None,
        slot_tags: vec!["RS_Stock".into()],
    }
}

/// The raw target: a def whose own components include the vanilla ability component.
fn target() -> Node {
    NodeBuilder::new("ThingDef")
        .text_elem("defName", "RS_TestRifle")
        .elem("comps", |c| {
            c.attr("Inherit", "False")
                .elem("li", |li| li.attr("Class", "RS_CompSlot"))
                .elem("li", |li| {
                    li.attr("Class", ABILITY)
                        .text_elem("abilityDef", "RS_Burner")
                })
        })
        .build()
}

fn spec() -> DesignSpec {
    let mut s = ranged_spec();
    let ce = s.ce.get_or_insert_with(common::ranged_ce);
    ce.weapon_tag_class = Some(CLASS_TAG.into());
    ce.under_barrel = Some(unit());
    ce.is_weapon_platform = true;
    ce.attachment_links = vec![link()];
    ce.default_graphic_parts = vec![part()];
    s
}

fn json(node: &Node) -> String {
    let mut text = serde_json::to_string_pretty(node).unwrap();
    text.push('\n');
    text
}

#[test]
fn the_platform_and_unit_patch_matches_its_golden_tree() {
    let spec = spec();
    let patch = gun_patch(&spec, &ce_model(), &Container::from_node(&target())).unwrap();
    assert_eq!(patch.mode, PatchMode::New);
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    common::assert_golden("ce_platform_patch.json", &json(&patch.patch_root()));
}

#[test]
fn the_unit_alone_on_a_def_without_an_ability_matches_its_golden_tree() {
    let mut spec = spec();
    {
        let ce = spec.ce.as_mut().unwrap();
        ce.is_weapon_platform = false;
        ce.attachment_links.clear();
        ce.default_graphic_parts.clear();
        ce.under_barrel = Some(CeUnderBarrel::default());
    }
    let plain = NodeBuilder::new("ThingDef")
        .text_elem("defName", "RS_TestRifle")
        .build();
    let patch = gun_patch(&spec, &ce_model(), &Container::from_node(&plain)).unwrap();
    assert!(!patch.has_errors(), "{:?}", patch.diagnostics);
    common::assert_golden(
        "ce_under_barrel_slot_patch.json",
        &json(&patch.patch_root()),
    );
}

#[test]
fn the_golden_has_the_documented_shape() {
    let spec = spec();
    let model = ce_model();
    let patch = gun_patch(&spec, &model, &Container::from_node(&target())).unwrap();
    let conversion = patch.gun_conversion(&model.classes).unwrap();
    assert_eq!(conversion.child_text("isWeaponPlatform"), Some("true"));
    assert_eq!(
        conversion
            .child("attachmentLinks")
            .unwrap()
            .children_named("li")
            .count(),
        1
    );
    assert_eq!(
        conversion
            .child("defaultGraphicParts")
            .unwrap()
            .children_named("li")
            .count(),
        1
    );
    // The unit is one guarded operation that replaces the ability component.
    let guarded = patch
        .operations
        .iter()
        .find(|o| {
            o.child("nomatch")
                .and_then(|n| n.child_text("xpath"))
                .is_some_and(|x| x.contains(ABILITY))
        })
        .expect("the replace of the ability component");
    assert_eq!(guarded.attr("Class"), Some("PatchOperationConditional"));
    let value = guarded.child("nomatch").unwrap().child("value").unwrap();
    assert_eq!(value.children_named("li").count(), 2, "unit and equippable");
}

#[test]
fn the_plan_keeps_every_combat_extended_class_in_the_gated_folder() {
    let spec = spec();
    let layout = ProjectLayout::default();
    let plan = export_ce_plan(&spec, &ce_model(), &layout);
    assert!(!plan.files.is_empty());
    assert_eq!(gate_violations(&plan, &layout), Vec::<String>::new());
}

#[test]
fn the_vanilla_definition_never_learns_about_platforms_or_units() {
    let with = export_vanilla_plan(&spec(), &ProjectLayout::default());
    let mut plain = spec();
    plain.ce = None;
    let without = export_vanilla_plan(&plain, &ProjectLayout::default());
    let text = |p: &rimstudio_design::plan::WritePlan| {
        p.files
            .iter()
            .map(|f| format!("{:?}", f))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let a = text(&with);
    assert!(
        !a.contains("UnderBarrel") && !a.contains("WeaponPlatform"),
        "{a}"
    );
    assert_eq!(a, text(&without));
}

#[test]
fn no_choice_without_the_toggle_means_no_operations() {
    let mut s = spec();
    s.ce = None;
    let patch = gun_patch(&s, &ce_model(), &Container::from_node(&target())).unwrap();
    assert_eq!(patch.mode, PatchMode::Off);
    assert!(patch.operations.is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// lint

fn findings(file: Node, known: &[&str]) -> Vec<String> {
    let ctx = LintContext {
        known_defs: known
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<BTreeSet<_>>(),
        ..LintContext::default()
    };
    lint::run(&[file], &ce_model(), &ctx)
        .into_iter()
        .map(|d| d.code.as_str().to_owned())
        .filter(|c| c.starts_with("ce.cep04"))
        .collect()
}

fn unit_op(class_xpath_comp: &str, li: Node, with_equippable: bool) -> Node {
    let mut value = NodeBuilder::new("value").child(li);
    if with_equippable {
        value = value.elem("li", |l| l.text_elem("compClass", "CompEquippable"));
    }
    op("PatchOperationReplace")
        .text_elem("xpath", class_xpath_comp)
        .child(value.build())
        .build()
}

fn li_of(unit: &CeUnderBarrel) -> Node {
    under_barrel_li(&CeClassNames::default(), unit)
}

const ABILITY_XPATH: &str = "Defs/ThingDef[defName=\"RS_X\"]/comps/li[@Class=\"CompProperties_EquippableAbilityReloadable\"]";

#[test]
fn a_complete_generated_patch_raises_none_of_the_rules() {
    let spec = spec();
    let patch = gun_patch(&spec, &ce_model(), &Container::from_node(&target())).unwrap();
    assert_eq!(
        findings(patch.patch_root(), &["RS_Scope"]),
        Vec::<String>::new()
    );
}

#[test]
fn cep040_names_a_unit_without_an_ammo_set_or_projectile() {
    let mut u = unit();
    u.ammo_set = None;
    u.default_projectile = None;
    let f = findings(patch(vec![unit_op(ABILITY_XPATH, li_of(&u), true)]), &[]);
    assert_eq!(
        f.iter()
            .filter(|c| *c == "ce.cep040-under-barrel-incomplete")
            .count(),
        2,
        "{f:?}"
    );
    // The bare slot form and a complete unit are fine.
    let f = findings(
        patch(vec![unit_op(
            ABILITY_XPATH,
            li_of(&CeUnderBarrel::default()),
            true,
        )]),
        &[],
    );
    assert!(f.is_empty(), "{f:?}");
}

#[test]
fn cep040_does_not_ask_a_shared_ammo_holder_for_an_ammo_set() {
    let mut u = unit();
    u.one_ammo_holder = true;
    u.ammo_set = None;
    let f = findings(patch(vec![unit_op(ABILITY_XPATH, li_of(&u), true)]), &[]);
    assert!(f.is_empty(), "{f:?}");
}

#[test]
fn cep041_and_cep042_check_the_ammo_set_and_the_projectile() {
    let mut u = unit();
    u.ammo_set = Some("RS_NoSuchSet".into());
    let f = findings(patch(vec![unit_op(ABILITY_XPATH, li_of(&u), true)]), &[]);
    assert_eq!(f, ["ce.cep041-under-barrel-ammoset-unresolved"]);
    // A set another mod defines is known through the project's defs.
    let f = findings(
        patch(vec![unit_op(ABILITY_XPATH, li_of(&u), true)]),
        &["RS_NoSuchSet"],
    );
    assert!(f.is_empty(), "{f:?}");
    let mut u = unit();
    u.default_projectile = Some("RS_Bullet_CE".into());
    let f = findings(patch(vec![unit_op(ABILITY_XPATH, li_of(&u), true)]), &[]);
    assert_eq!(f, ["ce.cep042-under-barrel-projectile-not-in-set"]);
}

#[test]
fn cep043_wants_the_equippable_component_back_after_replacing_an_ability() {
    let f = findings(
        patch(vec![unit_op(ABILITY_XPATH, li_of(&unit()), false)]),
        &[],
    );
    assert_eq!(f, ["ce.cep043-equippable-comp-missing"]);
    let add = op("PatchOperationAdd")
        .text_elem("xpath", "Defs/ThingDef[defName=\"RS_X\"]/comps")
        .child(NodeBuilder::new("value").child(li_of(&unit())).build())
        .build();
    assert!(findings(patch(vec![add]), &[]).is_empty());
}

fn platform_op(params: Vec<Node>) -> Node {
    let mut b = op(&CeClassNames::default().make_gun_op).text_elem("defName", "RS_X");
    for p in params {
        b = b.child(p);
    }
    b.build()
}

#[test]
fn cep044_cep045_and_cep046_check_the_platform_parameters() {
    let flag = || Node::with_text("isWeaponPlatform", "true");
    // CEP046: a platform with nothing to fit.
    let f = findings(patch(vec![platform_op(vec![flag()])]), &[]);
    assert_eq!(f, ["ce.cep046-platform-empty"]);
    // CEP044: a link without an attachment.
    let empty_link = NodeBuilder::new("attachmentLinks")
        .elem("li", |li| li.text_elem("drawScale", "(1,1)"))
        .build();
    let f = findings(patch(vec![platform_op(vec![flag(), empty_link])]), &[]);
    assert_eq!(f, ["ce.cep044-attachment-link-empty"]);
    // CEP045: an unknown attachment, only when something is known to compare with.
    let named = |name: &str| {
        NodeBuilder::new("attachmentLinks")
            .elem("li", |li| li.text_elem("attachment", name))
            .build()
    };
    let f = findings(
        patch(vec![platform_op(vec![flag(), named("RS_Other")])]),
        &["RS_Scope"],
    );
    assert_eq!(f, ["ce.cep045-attachment-unknown"]);
    let f = findings(
        patch(vec![platform_op(vec![flag(), named("RS_Scope")])]),
        &["RS_Scope"],
    );
    assert!(f.is_empty(), "{f:?}");
    let f = findings(
        patch(vec![platform_op(vec![flag(), named("RS_Other")])]),
        &[],
    );
    assert!(f.is_empty(), "nothing to compare with: {f:?}");
}

#[test]
fn a_platform_only_conversion_is_not_reported_as_an_incomplete_gun_conversion() {
    let links = NodeBuilder::new("attachmentLinks")
        .elem("li", |li| li.text_elem("attachment", "RS_Scope"))
        .build();
    let all: Vec<String> = lint::run(
        &[patch(vec![platform_op(vec![
            Node::with_text("isWeaponPlatform", "true"),
            links,
        ])])],
        &ce_model(),
        &LintContext::default(),
    )
    .into_iter()
    .map(|d| d.code.as_str().to_owned())
    .collect();
    assert!(
        !all.iter()
            .any(|c| c.contains("cep008") || c.contains("cep007")),
        "{all:?}"
    );
}
