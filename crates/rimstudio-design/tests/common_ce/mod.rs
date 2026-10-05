//! A fictional Combat Extended model for the patch generator and lint tests. Every name starts with `RS_`
//! and every number is invented by a simple formula; nothing here comes from an install.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_design::ce::reader::{
    AmmoSetInfo, AmmoType, CeClassNames, CeGun, CeMelee, CeModel, CeNames, CeToolRow,
    ProjectileInfo,
};
use rimstudio_design::model::TechLevel;

/// The fictional AI class tag of the guns (also the class tag of the shared spec fixtures).
pub(crate) const CLASS_TAG: &str = "RS_CE_Class";

fn map(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
    pairs.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
}

fn gun(i: u32) -> CeGun {
    let f = f64::from(i);
    let mass = 1.5 + 0.4 * f;
    let range = 15.0 + 2.5 * f;
    let stats = map(&[
        ("mass", mass),
        ("bulk", 2.0 * mass + 0.5),
        ("spread", 0.07),
        ("sway", 1.3),
        ("sights", 1.0),
        ("cooldown", 0.4),
        ("recoil", 1.5),
        ("warmup", 0.5 + 0.05 * f),
        ("range", range * 1.8),
        ("burst", 1.0),
        ("ticks_between", 6.0),
        ("magazine", 30.0),
        ("reload", 4.0),
        ("damage", 9.0),
        ("ap_sharp", 2.5),
        ("ap_blunt", 14.0),
        ("speed", 120.0),
        ("pellets", 1.0),
    ]);
    let twin = map(&[
        ("mass", mass),
        ("range", range),
        ("warmup", 0.8 + 0.1 * f),
        ("cooldown", 1.0 + 0.1 * f),
        ("burst", 1.0),
        ("damage", 10.0),
        ("speed", 55.0),
    ]);
    CeGun {
        def_name: format!("RS_CeGun{i:02}"),
        label: format!("ce gun {i}"),
        mod_idx: None,
        tech_level: Some(if i < 6 {
            TechLevel::Industrial
        } else {
            TechLevel::Spacer
        }),
        weapon_tags: vec!["RS_Gun".into(), CLASS_TAG.into()],
        ce_tags: vec![CLASS_TAG.into()],
        ai_class: Some(CLASS_TAG.into()),
        ammo_set: Some("RS_AmmoSet".into()),
        default_projectile: Some("RS_Bullet_CE".into()),
        reload_one_at_a_time: false,
        ai_aim_mode: Some("AimedShot".into()),
        stats,
        twin: Some(twin),
        twin_tags: vec!["RS_Gun".into()],
        excluded: None,
    }
}

fn melee(i: u32) -> CeMelee {
    let f = f64::from(i);
    let power = 10.0 + 2.0 * f;
    let tool = |label: &str, capacity: &str, p: f64, sharp: f64, blunt: f64| CeToolRow {
        label: label.into(),
        capacities: vec![capacity.into()],
        power: Some(p),
        cooldown: Some(2.0),
        ap_sharp: (sharp > 0.0).then_some(sharp),
        ap_blunt: Some(blunt),
        chance_factor: None,
    };
    CeMelee {
        def_name: format!("RS_CeBlade{i:02}"),
        label: format!("ce blade {i}"),
        mod_idx: None,
        tech_level: Some(TechLevel::Medieval),
        weapon_tags: vec!["RS_Melee".into()],
        tools: vec![
            tool("blade", "Cut", power, power * 0.05, power * 0.25),
            tool("handle", "Blunt", 8.0, 0.0, 2.0),
        ],
        stats: map(&[
            ("mass", 1.0 + 0.1 * f),
            ("bulk", 5.0 + 0.2 * f),
            ("counter_parry", 0.2),
            ("crit", 0.1),
            ("parry", 0.2),
            ("dodge", -0.05),
            ("tool_power", power),
            ("tool_cooldown", 2.0),
            ("ap_sharp_ratio", 0.05),
            ("ap_blunt_ratio", 0.25),
        ]),
        twin: Some(map(&[
            ("mass", 1.0 + 0.1 * f),
            ("tool_power", power),
            ("tool_cooldown", 2.0),
        ])),
        twin_tags: vec!["RS_Melee".into()],
        excluded: None,
    }
}

fn ammo_set(name: &str, ammo: &str, projectile: &str) -> AmmoSetInfo {
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

/// A present model with twelve guns, six melee weapons, two ammo sets and the fictional class tag.
pub(crate) fn ce_model() -> CeModel {
    let mut model = CeModel::absent("placeholder");
    model.absent = None;
    model.diagnostics.clear();
    model.names = Some(CeNames {
        package_id: "RS.CombatExt".into(),
        name: "RS Combat Ext".into(),
    });
    model.classes = CeClassNames::default();
    model.ammo_sets = vec![
        ammo_set("RS_AmmoSet", "RS_Ammo", "RS_Bullet_CE"),
        ammo_set("RS_AmmoSetOther", "RS_AmmoOther", "RS_Bullet_Other"),
    ];
    model.weapon_tags = vec![CLASS_TAG.into(), "RS_CE_OneHanded".into()];
    model.ai_class_tags = vec![CLASS_TAG.into()];
    model.guns = (0..12).map(gun).collect();
    model.melee = (0..6).map(melee).collect();
    model.probe_def = Some("RS_AmmoSet".into());
    model
}

/// A model of the same install without conversions to learn from (no predictors).
pub(crate) fn ce_model_without_pools() -> CeModel {
    let mut model = ce_model();
    model.guns.clear();
    model.melee.clear();
    model
}

/// An `Operation` element of a plain class.
pub(crate) fn op(class: &str) -> NodeBuilder {
    NodeBuilder::new("Operation").attr("Class", class)
}

/// A `Patch` root holding the operations.
pub(crate) fn patch(ops: Vec<Node>) -> Node {
    let mut root = Node::new("Patch");
    for o in ops {
        root.push_child(o);
    }
    root
}

/// A complete, valid gun conversion operation of the fictional model.
pub(crate) fn make_gun(def: &str) -> NodeBuilder {
    let classes = CeClassNames::default();
    op(&classes.make_gun_op)
        .text_elem("defName", def)
        .elem("statBases", |s| {
            s.text_elem("Bulk", "6").text_elem("ShotSpread", "0.1")
        })
        .elem("Properties", |p| {
            p.text_elem("verbClass", &classes.shoot_verb)
                .text_elem("hasStandardCommand", "true")
                .text_elem("defaultProjectile", "RS_Bullet_CE")
                .text_elem("range", "30")
        })
        .elem("AmmoUser", |a| {
            a.text_elem("magazineSize", "30")
                .text_elem("reloadTime", "4")
                .text_elem("ammoSet", "RS_AmmoSet")
        })
        .elem("FireModes", |f| f.text_elem("aiAimMode", "AimedShot"))
        .elem("weaponTags", |t| t.li(CLASS_TAG))
}
