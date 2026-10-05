//! Tests of the reference reader over fictional defs.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::fixtures_tests::{
    GunNumbers, base_defs, gun, load_defs, material, material_with_multipliers, melee, projectile,
    types,
};
use super::*;
use crate::classes::ItemKind;
use crate::model::{ProjectileChoice, TechLevel, ValueSource};
use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_defs::LoadOutput;
use std::collections::BTreeSet;

fn guns() -> Vec<Node> {
    let n = |mass, range, warmup, cooldown, burst, tier| GunNumbers {
        mass,
        range,
        warmup,
        cooldown,
        burst,
        tier,
    };
    vec![
        gun(
            "RS_GunA",
            "RS_Shot1",
            n(2.0, 20.0, 0.8, 1.2, 1, "Industrial"),
            &["RS_Handgun"],
        ),
        gun(
            "RS_GunB",
            "RS_Shot1",
            n(3.5, 28.0, 1.1, 1.5, 3, "Industrial"),
            &["RS_Handgun", "RS_Auto"],
        ),
        gun(
            "RS_GunC",
            "RS_Shot2",
            n(4.0, 32.0, 1.4, 1.9, 1, "Spacer"),
            &["RS_Handgun"],
        ),
        gun(
            "RS_GunD",
            "RS_Shot2",
            n(5.5, 40.0, 1.8, 2.4, 1, "Spacer"),
            &["RS_Long"],
        ),
        gun(
            "RS_GunE",
            "RS_Shot1",
            n(1.5, 16.0, 0.5, 0.9, 1, "Neolithic"),
            &["RS_Handgun"],
        ),
    ]
}

fn melee_weapons() -> Vec<Node> {
    vec![
        melee(
            "RS_Edge",
            "Medieval",
            1.4,
            &[("blade", "Cut", 18.0, 2.2), ("handle", "Blunt", 9.0, 2.0)],
        ),
        melee("RS_Spike", "Medieval", 1.8, &[("point", "Stab", 15.0, 2.0)]),
        melee("RS_Club", "Neolithic", 2.4, &[("head", "Blunt", 11.0, 2.0)]),
        melee(
            "RS_Big",
            "Industrial",
            3.0,
            &[("blade", "Cut", 24.0, 2.6), ("pommel", "Blunt", 10.0, 2.0)],
        ),
    ]
}

fn load_all() -> LoadOutput {
    let mut defs = base_defs();
    defs.extend(guns());
    defs.extend(melee_weapons());
    load_defs(defs, &types(&[]))
}

#[test]
fn ranged_weapons_are_read_with_resolved_numbers() {
    let out = load_all();
    let set = read_reference_set(&out.databases, &ReaderOptions::default()).unwrap();
    let b = set.weapon("RS_GunB").expect("RS_GunB is read");
    assert_eq!(b.kind, ItemKind::Ranged);
    assert_eq!(b.tech_level, Some(TechLevel::Industrial));
    let r = b.ranged.as_ref().unwrap();
    assert_eq!(r.burst_count, 3);
    assert!((r.range - 28.0).abs() < 1e-12);
    assert!((r.damage - 10.0).abs() < 1e-12);
    assert!((r.cooldown - 1.5).abs() < 1e-12);
    assert!((r.ticks_between_shots - 8.0).abs() < 1e-12);
    assert!(r.accuracy.is_some());
    assert!(b.strength.is_some_and(|s| s > 0.0));
    assert_eq!(b.stats.get("mass"), Some(&3.5));
    assert!(b.stats.contains_key("dps"));
    // The armor piercing of a projectile that sets its damage is 0.015 per damage point.
    assert!((r.armor_penetration - 0.15).abs() < 1e-12);
    let a = set.weapon("RS_GunA").unwrap().ranged.clone().unwrap();
    assert_eq!(
        a.burst_count, 1,
        "no burstShotCount reads as the game default"
    );
}

#[test]
fn strength_is_the_p4_index_of_the_profile() {
    let out = load_all();
    let set = read_reference_set(&out.databases, &ReaderOptions::default()).unwrap();
    let w = set.weapon("RS_GunC").unwrap();
    let expected = crate::ranged::strength_p4(
        &w.ranged.as_ref().unwrap().profile(),
        &crate::ranged::ReferenceArmor {
            sharp_ratings: &set.armor.ratings,
        },
    )
    .unwrap();
    assert!((w.strength.unwrap() - expected).abs() < 1e-12);
}

#[test]
fn candidates_that_do_not_belong_in_the_pools_are_listed_with_a_reason() {
    use crate::reader::SkipReason;
    let mut defs = base_defs();
    defs.extend(guns());
    let mut hidden = gun("RS_Hidden", "RS_Shot1", GunNumbers::default(), &[]);
    hidden.push_child(Node::with_text("menuHidden", "true"));
    let boom = gun("RS_Thrower", "RS_Boom", GunNumbers::default(), &[]);
    let dup = gun("RS_GunA_Unique", "RS_Shot1", GunNumbers::default(), &[]);
    let tagged = gun(
        "RS_Turretish",
        "RS_Shot1",
        GunNumbers::default(),
        &["RS_TurretGun"],
    );
    let mut modded = gun("RS_Modded", "RS_Shot1", GunNumbers::default(), &[]);
    if let Some(verbs) = modded.child_mut("verbs") {
        for li in verbs.elements_mut() {
            li.set_attr("Class", "RS_Mod.VerbProps");
        }
    }
    defs.extend([hidden, boom, dup, tagged, modded]);
    let out = load_defs(defs, &types(&[]));
    let set = read_reference_set(&out.databases, &ReaderOptions::default()).unwrap();
    let reason = |name: &str| {
        set.skipped
            .iter()
            .find(|s| s.def_name == name)
            .map(|s| s.reason.clone())
    };
    assert_eq!(reason("RS_Hidden"), Some(SkipReason::MenuHidden));
    assert_eq!(reason("RS_Thrower"), Some(SkipReason::Explosive));
    assert_eq!(
        reason("RS_GunA_Unique"),
        Some(SkipReason::NameSuffix("_Unique".into()))
    );
    assert_eq!(
        reason("RS_Turretish"),
        Some(SkipReason::Tag("RS_TurretGun".into()))
    );
    assert_eq!(
        reason("RS_Modded"),
        Some(SkipReason::CustomClass("RS_Mod.VerbProps".into()))
    );
    assert!(set.weapon("RS_Hidden").is_none());
    assert_eq!(set.items(ItemKind::Ranged).len(), 5);
}

#[test]
fn a_def_that_is_not_a_weapon_is_neither_read_nor_listed() {
    let out = load_all();
    let set = read_reference_set(&out.databases, &ReaderOptions::default()).unwrap();
    for name in ["RS_ArmorA", "RS_Shot1", "RS_Steel"] {
        assert!(set.weapon(name).is_none());
        assert!(set.skipped.iter().all(|s| s.def_name != name));
    }
}

#[test]
fn melee_weapons_get_roles_from_the_default_rules() {
    let out = load_all();
    let set = read_reference_set(&out.databases, &ReaderOptions::default()).unwrap();
    let role = |n: &str| set.weapon(n).and_then(|w| w.role.clone());
    assert_eq!(role("RS_Edge").as_deref(), Some("blade"));
    assert_eq!(role("RS_Spike").as_deref(), Some("piercing"));
    assert_eq!(role("RS_Club").as_deref(), Some("blunt"));
    let edge = set.weapon("RS_Edge").unwrap();
    assert!(edge.strength.is_some_and(|s| s > 0.0));
    assert!(edge.stats.contains_key("swing_damage"));
    assert_eq!(edge.tools.len(), 2);
}

#[test]
fn the_reference_stuff_scales_a_stuffed_melee_weapon_through_the_stat_pipeline() {
    let mut defs = base_defs();
    let mut stuffed = melee(
        "RS_Stuffed",
        "Medieval",
        1.5,
        &[("blade", "Cut", 20.0, 2.0)],
    );
    stuffed.push_child(Node::new("stuffCategories"));
    if let Some(c) = stuffed.child_mut("stuffCategories") {
        c.push_child(Node::with_text("li", "RS_Metallic"));
    }
    defs.extend([stuffed, material("RS_Alloy", "RS_Metallic", 1.5)]);
    let out = load_defs(defs, &types(&[]));
    let plain = read_reference_set(&out.databases, &ReaderOptions::default()).unwrap();
    let opts = ReaderOptions {
        reference_stuff: Some("RS_Alloy".into()),
        ..ReaderOptions::default()
    };
    let with = read_reference_set(&out.databases, &opts).unwrap();
    let a = plain.weapon("RS_Stuffed").unwrap().stats["swing_damage"];
    let b = with.weapon("RS_Stuffed").unwrap().stats["swing_damage"];
    assert!((a - 20.0).abs() < 1e-9);
    assert!((b - 30.0).abs() < 1e-9);
    let missing = ReaderOptions {
        reference_stuff: Some("RS_Nothing".into()),
        ..ReaderOptions::default()
    };
    let none = read_reference_set(&out.databases, &missing).unwrap();
    assert!(
        none.diagnostics
            .iter()
            .any(|d| d.code.as_str() == "reader.stuff-missing")
    );
}

#[test]
fn the_reference_armor_is_derived_from_the_apparel_or_falls_back() {
    let out = load_all();
    let set = read_reference_set(&out.databases, &ReaderOptions::default()).unwrap();
    assert!(!set.armor.fallback);
    assert_eq!(set.armor.ratings.len(), 3);
    assert!((set.armor.ratings[0]).abs() < 1e-12);
    assert!((set.armor.ratings[1] - 0.6).abs() < 1e-9);
    assert_eq!(set.armor.derived_from, 3);
    let bare = load_defs(guns(), &types(&[]));
    let fallback = read_reference_set(&bare.databases, &ReaderOptions::default()).unwrap();
    assert!(fallback.armor.fallback);
    assert_eq!(fallback.armor.ratings, FALLBACK_ARMOR_RATINGS.to_vec());
    assert!(
        fallback
            .diagnostics
            .iter()
            .any(|d| d.code.as_str() == "reader.armor-fallback")
    );
    let given = ReaderOptions {
        armor: ArmorSource::Given {
            ratings: vec![0.0, 0.25],
        },
        ..ReaderOptions::default()
    };
    let g = read_reference_set(&out.databases, &given).unwrap();
    assert_eq!(g.armor.ratings, vec![0.0, 0.25]);
}

#[test]
fn pools_are_built_by_kind_with_tiers_from_the_tech_level() {
    let out = load_all();
    let pools = build_pools(&out.databases, &ReaderOptions::default()).unwrap();
    assert_eq!(pools.ranged.len(), 5);
    assert_eq!(pools.melee.len(), 4);
    assert!(pools.ranged.items.iter().all(|i| !i.tier_derived));
    assert_eq!(pools.ranged.tiers(), vec![0, 2, 3]);
    assert_eq!(pools.melee.tiers(), vec![0, 1, 2]);
    assert_eq!(pools.ranged.tier_text(2), "Industrial");
    // Sorted by strength; the burst gun's group is "burst".
    let b = pools
        .ranged
        .items
        .iter()
        .find(|i| i.id == "RS_GunB")
        .unwrap();
    assert_eq!(b.group.as_deref(), Some("burst"));
    let a = pools
        .ranged
        .items
        .iter()
        .find(|i| i.id == "RS_GunA")
        .unwrap();
    assert_eq!(a.group.as_deref(), Some("single"));
    assert!(pools.ranged.excluded.is_empty());
}

#[test]
fn pools_do_not_depend_on_the_order_of_the_defs() {
    let forward = load_all();
    let mut defs = base_defs();
    let mut weapons: Vec<Node> = guns();
    weapons.extend(melee_weapons());
    weapons.reverse();
    defs.extend(weapons);
    let reversed = load_defs(defs, &types(&[]));
    let a = build_pools(&forward.databases, &ReaderOptions::default()).unwrap();
    let b = build_pools(&reversed.databases, &ReaderOptions::default()).unwrap();
    assert_eq!(a.ranged, b.ranged);
    assert_eq!(a.melee, b.melee);
}

#[test]
fn an_unknown_weapon_database_is_an_error() {
    let out = load_all();
    let opts = ReaderOptions {
        db_type: "RS_NotAType".into(),
        ..ReaderOptions::default()
    };
    let err = read_reference_set(&out.databases, &opts).unwrap_err();
    assert_eq!(err.code(), "design.invalid-input");
}

#[test]
fn a_def_becomes_a_vanilla_spec_for_cloning() {
    let out = load_all();
    let opts = ReaderOptions::default();
    let def = out.databases.get("ThingDef", "RS_GunB").unwrap();
    let reading = spec_from_def(def, &out.databases, &opts, ValueSource::Anchor).unwrap();
    assert!(reading.strength.is_some());
    let spec = reading.clone_as("RS_Clone", "clone");
    assert_eq!(spec.identity.def_name, "RS_Clone");
    assert_eq!(spec.kind, crate::model::ItemKind::Ranged);
    assert!(spec.ce.is_none(), "the CE block stays off");
    assert_eq!(spec.tech_level, Some(TechLevel::Industrial));
    assert_eq!(spec.mass.map(|m| m.value), Some(3.5));
    assert_eq!(spec.mass.map(|m| m.source), Some(ValueSource::Anchor));
    assert_eq!(spec.cost_list.len(), 1);
    assert_eq!(spec.weapon_tags, vec!["RS_Handgun", "RS_Auto"]);
    assert_eq!(spec.tools.len(), 1);
    let r = spec.ranged.as_ref().unwrap();
    assert_eq!(r.burst_count.map(|b| b.value), Some(3));
    assert_eq!(r.range.map(|v| v.value), Some(28.0));
    assert_eq!(r.damage.map(|v| v.value), Some(10.0));
    assert_eq!(
        r.projectile,
        Some(ProjectileChoice::Reference("RS_Shot1".into()))
    );
    assert_eq!(r.sound_cast.as_deref(), Some("RS_Shot"));
    assert!(spec.extra_stats.is_empty());
}

#[test]
fn a_melee_def_becomes_a_melee_spec_and_unmapped_stats_are_kept() {
    let mut defs = base_defs();
    let mut blade = melee("RS_Edge", "Medieval", 1.4, &[("blade", "Cut", 18.0, 2.2)]);
    if let Some(s) = blade.child_mut("statBases") {
        s.push_child(Node::with_text("RS_Sharpness", "1.25"));
    }
    defs.push(blade);
    let out = load_defs(defs, &types(&[]));
    let def = out.databases.get("ThingDef", "RS_Edge").unwrap();
    let reading = spec_from_def(
        def,
        &out.databases,
        &ReaderOptions::default(),
        ValueSource::Typed,
    )
    .unwrap();
    let spec = reading.spec;
    assert_eq!(spec.kind, crate::model::ItemKind::Melee);
    assert!(spec.ranged.is_none());
    assert_eq!(spec.tools.len(), 1);
    assert_eq!(spec.tools[0].capacities, vec!["Cut"]);
    assert_eq!(
        spec.extra_stats.get("RS_Sharpness").map(|s| s.value),
        Some(1.25)
    );
    assert_eq!(spec.role.as_deref(), Some("blade"));
}

#[test]
fn a_def_without_verbs_or_tools_cannot_become_a_spec() {
    let out = load_all();
    let def = out.databases.get("ThingDef", "RS_ArmorA").unwrap();
    let err = spec_from_def(
        def,
        &out.databases,
        &ReaderOptions::default(),
        ValueSource::Typed,
    )
    .unwrap_err();
    assert_eq!(err.code(), "design.invalid-input");
}

#[test]
fn a_projectile_that_is_not_loaded_is_noted_and_the_damage_stays_unknown() {
    let mut defs = base_defs();
    defs.push(gun("RS_Lost", "RS_NoSuchShot", GunNumbers::default(), &[]));
    let out = load_defs(defs, &types(&[]));
    let def = out.databases.get("ThingDef", "RS_Lost").unwrap();
    let reading = spec_from_def(
        def,
        &out.databases,
        &ReaderOptions::default(),
        ValueSource::Typed,
    )
    .unwrap();
    assert!(
        reading
            .notes
            .iter()
            .any(|n| n.contains("projectile def is not loaded"))
    );
    assert!(reading.spec.ranged.unwrap().damage.is_none());
    assert!(reading.strength.is_none());
    let set = read_reference_set(&out.databases, &ReaderOptions::default()).unwrap();
    assert!(set.skipped.iter().any(|s| s.def_name == "RS_Lost"));
    let _ = projectile("RS_Unused", 1.0, 1.0, None);
}

fn stuffed_melee(name: &str) -> Node {
    let mut weapon = melee(
        name,
        "Medieval",
        1.5,
        &[("blade", "Cut", 20.0, 2.0), ("head", "Blunt", 10.0, 2.0)],
    );
    weapon.push_child(Node::new("stuffCategories"));
    if let Some(c) = weapon.child_mut("stuffCategories") {
        c.push_child(Node::with_text("li", "RS_Metallic"));
    }
    weapon
}

#[test]
fn a_damage_without_an_armor_category_has_no_penetration() {
    let mut defs = base_defs();
    defs.push(
        NodeBuilder::new("DamageDef")
            .text_elem("defName", "RS_Soft")
            .text_elem("defaultDamage", "10")
            .build(),
    );
    defs.push(
        NodeBuilder::new("ThingDef")
            .text_elem("defName", "RS_SoftShot")
            .elem("projectile", |p| {
                p.text_elem("damageDef", "RS_Soft")
                    .text_elem("damageAmountBase", "10")
                    .text_elem("speed", "50")
            })
            .build(),
    );
    defs.push(gun("RS_SoftGun", "RS_SoftShot", GunNumbers::default(), &[]));
    defs.push(gun("RS_HardGun", "RS_Shot1", GunNumbers::default(), &[]));
    let out = load_defs(defs, &types(&[]));
    let set = read_reference_set(&out.databases, &ReaderOptions::default()).unwrap();
    let ap = |n: &str| {
        set.weapon(n)
            .unwrap()
            .ranged
            .as_ref()
            .unwrap()
            .armor_penetration
    };
    assert_eq!(ap("RS_SoftGun"), 0.0);
    assert!((ap("RS_HardGun") - 0.15).abs() < 1e-12);
}

#[test]
fn the_sharp_and_blunt_multipliers_are_the_stats_of_the_material_itself() {
    let mut defs = base_defs();
    defs.push(stuffed_melee("RS_Mixed"));
    defs.push(material_with_multipliers(
        "RS_Alloy",
        "RS_Metallic",
        1.2,
        0.8,
    ));
    let out = load_defs(defs, &types(&[]));
    let opts = ReaderOptions {
        reference_stuff: Some("RS_Alloy".into()),
        ..ReaderOptions::default()
    };
    let ctx = ReadContext::new(&out.databases, &opts);
    let set = read_reference_set(&out.databases, &opts).unwrap();
    let w = set.weapon("RS_Mixed").unwrap();
    let attacks = ctx.attacks(w).unwrap();
    assert_eq!(attacks.len(), 2);
    assert!(
        (attacks[0].damage - 24.0).abs() < 1e-9,
        "cut uses the sharp multiplier"
    );
    assert!(
        (attacks[1].damage - 8.0).abs() < 1e-9,
        "blunt uses the blunt multiplier"
    );
    // Without a material the multipliers are 1.
    let none = ReaderOptions::default();
    let plain = ReadContext::new(&out.databases, &none);
    let plain_set = read_reference_set(&out.databases, &none).unwrap();
    let base = plain
        .attacks(plain_set.weapon("RS_Mixed").unwrap())
        .unwrap();
    assert!((base[0].damage - 20.0).abs() < 1e-9);
    assert!((base[1].damage - 10.0).abs() < 1e-9);
}

#[test]
fn a_weapon_with_several_verbs_is_read_from_its_first_projectile_verb() {
    let mut defs = base_defs();
    let mut multi = gun("RS_Multi", "RS_Shot1", GunNumbers::default(), &[]);
    // Put a verb without a projectile first and a second shooting verb after the first one.
    if let Some(verbs) = multi.child_mut("verbs") {
        let mut lead = Node::new("li");
        lead.push_child(Node::with_text("verbClass", "RS_NoShot"));
        let mut second = Node::new("li");
        second.push_child(Node::with_text("defaultProjectile", "RS_Shot2"));
        second.push_child(Node::with_text("range", "40"));
        let first_shot = verbs.children_named("li").next().cloned();
        let mut rebuilt = Node::new("verbs");
        rebuilt.push_child(lead);
        if let Some(f) = first_shot {
            rebuilt.push_child(f);
        }
        rebuilt.push_child(second);
        *verbs = rebuilt;
    }
    defs.push(multi);
    let out = load_defs(defs, &types(&[]));
    let set = read_reference_set(&out.databases, &ReaderOptions::default()).unwrap();
    let w = set.weapon("RS_Multi").unwrap();
    let r = w.ranged.as_ref().unwrap();
    assert_eq!(w.kind, ItemKind::Ranged);
    assert_eq!(r.projectile, "RS_Shot1");
    assert!((r.range - 26.0).abs() < 1e-12);
    assert_eq!(w.tools.len(), 1, "the bash tool of a gun is kept as a tool");
    assert!(set.skipped.is_empty());
}

#[test]
fn the_market_value_follows_cost_list_stuff_volume_and_work() {
    let item = NodeBuilder::new("ThingDef")
        .text_elem("defName", "RS_Priced")
        .text_elem("costStuffCount", "40")
        .elem("statBases", |s| s.text_elem("WorkToMake", "3000"))
        .elem("costList", |c| c.text_elem("RS_Part", "2"))
        .build();
    let part = NodeBuilder::new("ThingDef")
        .text_elem("defName", "RS_Part")
        .elem("statBases", |s| s.text_elem("MarketValue", "30"))
        .build();
    let small = NodeBuilder::new("ThingDef")
        .text_elem("defName", "RS_Coin")
        .text_elem("smallVolume", "true")
        .elem("statBases", |s| s.text_elem("MarketValue", "1"))
        .elem("stuffProps", |p| {
            p.elem("categories", |c| c.li("RS_Metallic"))
                .elem("statFactors", |f| f.text_elem("WorkToMake", "2"))
        })
        .build();
    let plain = NodeBuilder::new("ThingDef")
        .text_elem("defName", "RS_Ore")
        .elem("statBases", |s| s.text_elem("MarketValue", "2"))
        .elem("stuffProps", |p| {
            p.elem("categories", |c| c.li("RS_Metallic"))
        })
        .build();
    let fixed = NodeBuilder::new("ThingDef")
        .text_elem("defName", "RS_Fixed")
        .elem("statBases", |s| s.text_elem("MarketValue", "1234"))
        .build();
    let out = load_defs(vec![item, part, small, plain, fixed], &types(&[]));
    let opts = ReaderOptions::default();
    let price = |stuff: Option<&str>| market_value_of(&out.databases, &opts, "RS_Priced", stuff);
    // 2 x 30 + 40 x 2 + 3000 x 0.0036 = 60 + 80 + 10.8
    let ore = price(Some("RS_Ore")).unwrap();
    assert!((ore.ingredients - 60.0).abs() < 1e-9);
    assert!((ore.stuff - 80.0).abs() < 1e-9);
    assert!((ore.work - 10.8).abs() < 1e-9);
    // Small volume: 40 / 0.1 x 1 = 400; the material doubles the work: 6000 x 0.0036 = 21.6.
    let coin = price(Some("RS_Coin")).unwrap();
    assert!((coin.stuff - 400.0).abs() < 1e-9);
    assert!((coin.work - 21.6).abs() < 1e-9);
    assert!((coin.base_value - 481.6).abs() < 1e-9);
    assert_eq!(coin.displayed, 480.0);
    // No material: 40 units at the guess of 2 silver.
    let unknown = price(None).unwrap();
    assert!((unknown.stuff - 80.0).abs() < 1e-9);
    // An explicit market value is returned as written.
    let fixed = market_value_of(&out.databases, &opts, "RS_Fixed", None).unwrap();
    assert_eq!(fixed.base_value, 1234.0);
    assert_eq!(fixed.displayed, 1235.0);
    assert!(market_value_of(&out.databases, &opts, "RS_Missing", None).is_err());
    assert!(price(Some("RS_Missing")).is_err());
}

#[test]
fn a_weapon_without_tags_is_a_candidate_when_it_is_primary_equipment_in_a_weapon_category() {
    let tagless = |name: &str, category: &str, equipment: Option<&str>| {
        let mut n = melee(name, "Medieval", 1.0, &[("blade", "Cut", 10.0, 2.0)]);
        n.remove_child("weaponTags");
        let mut cats = Node::new("thingCategories");
        cats.push_child(Node::with_text("li", category));
        n.push_child(cats);
        if let Some(e) = equipment {
            n.push_child(Node::with_text("equipmentType", e));
        }
        n
    };
    let mut defs = base_defs();
    defs.push(tagless("RS_NoTags", "RS_WeaponsMelee", Some("Primary")));
    defs.push(tagless("RS_Log", "RS_ResourcesRaw", Some("Primary")));
    defs.push(tagless("RS_NotWielded", "RS_WeaponsMelee", None));
    let out = load_defs(defs, &types(&[]));
    let set = read_reference_set(&out.databases, &ReaderOptions::default()).unwrap();
    assert!(
        set.weapon("RS_NoTags").is_some(),
        "tagless weapons are read"
    );
    assert!(
        set.weapon("RS_Log").is_none(),
        "a wieldable log is not a weapon"
    );
    assert!(set.weapon("RS_NotWielded").is_none());
    assert!(set.skipped.is_empty());
}

#[test]
fn the_damage_kind_of_a_capacity_follows_the_maneuver_of_the_install() {
    let mut defs = base_defs();
    defs.push(
        NodeBuilder::new("DamageDef")
            .text_elem("defName", "RS_ClawDamage")
            .text_elem("armorCategory", "Sharp")
            .build(),
    );
    defs.push(
        NodeBuilder::new("ManeuverDef")
            .text_elem("defName", "RS_ClawManeuver")
            .text_elem("requiredCapacity", "RS_Claw")
            .elem("verb", |v| v.text_elem("meleeDamageDef", "RS_ClawDamage"))
            .build(),
    );
    let mut weapon = melee(
        "RS_Claws",
        "Medieval",
        1.0,
        &[("claws", "RS_Claw", 10.0, 2.0)],
    );
    weapon.push_child(Node::new("stuffCategories"));
    if let Some(c) = weapon.child_mut("stuffCategories") {
        c.push_child(Node::with_text("li", "RS_Metallic"));
    }
    defs.push(weapon);
    defs.push(material_with_multipliers(
        "RS_Alloy",
        "RS_Metallic",
        1.5,
        0.5,
    ));
    let table = types(&[("Verse.ManeuverDef", "Verse.Def")]);
    let out = load_defs(defs, &table);
    let opts = ReaderOptions {
        reference_stuff: Some("RS_Alloy".into()),
        ..ReaderOptions::default()
    };
    let ctx = ReadContext::new(&out.databases, &opts);
    let set = read_reference_set(&out.databases, &opts).unwrap();
    let attacks = ctx.attacks(set.weapon("RS_Claws").unwrap()).unwrap();
    // The name alone would read as blunt (0.5); the maneuver says sharp (1.5).
    assert!((attacks[0].damage - 15.0).abs() < 1e-9);
    // Without the maneuver definitions the name rule applies.
    let bare = load_defs(
        {
            let mut d = base_defs();
            d.push(melee(
                "RS_Claws",
                "Medieval",
                1.0,
                &[("claws", "RS_Claw", 10.0, 2.0)],
            ));
            d
        },
        &types(&[]),
    );
    let none = ReaderOptions::default();
    let ctx = ReadContext::new(&bare.databases, &none);
    let set = read_reference_set(&bare.databases, &none).unwrap();
    let attacks = ctx.attacks(set.weapon("RS_Claws").unwrap()).unwrap();
    assert_eq!(attacks[0].kind, crate::melee::DamageKind::Blunt);
}

fn abstract_gun_base() -> Node {
    NodeBuilder::new("ThingDef")
        .attr("Name", "RS_BaseGun")
        .attr("Abstract", "True")
        .text_elem("category", "Item")
        .elem("statBases", |s| {
            s.text_elem("RS_Flammability", "0.5")
                .text_elem("RS_Durability", "80")
                .text_elem("MarketValue", "123")
        })
        .build()
}

fn child_gun(extra_stat: Option<(&str, &str)>) -> Node {
    let mut weapon = gun(
        "RS_ChildGun",
        "RS_Shot1",
        GunNumbers::default(),
        &["RS_Handgun"],
    );
    weapon.set_attr("ParentName", "RS_BaseGun");
    if let (Some((name, value)), Some(stats)) = (extra_stat, weapon.child_mut("statBases")) {
        stats.push_child(Node::with_text(name, value));
    }
    weapon
}

#[test]
fn stats_that_only_the_parent_provides_stay_inherited_in_a_clone() {
    let mut defs = base_defs();
    defs.push(abstract_gun_base());
    defs.push(child_gun(Some(("RS_Own", "7"))));
    let out = load_defs(defs, &types(&[]));
    let def = out.databases.get("ThingDef", "RS_ChildGun").unwrap();
    let own: BTreeSet<String> = ["Mass", "RS_Own"].iter().map(|s| (*s).to_owned()).collect();
    let reading = spec_from_def_inheriting(
        def,
        &out.databases,
        &ReaderOptions::default(),
        ValueSource::Anchor,
        &own,
    )
    .unwrap();
    let spec = reading.spec;
    assert_eq!(
        spec.parent.as_ref().map(|p| p.def_name.as_str()),
        Some("RS_BaseGun")
    );
    let inherited = &spec.parent.as_ref().unwrap().inherited_stats;
    assert_eq!(inherited.get("RS_Flammability"), Some(&0.5));
    assert_eq!(inherited.get("RS_Durability"), Some(&80.0));
    assert_eq!(inherited.get("MarketValue"), Some(&123.0));
    assert_eq!(spec.extra_stats.keys().collect::<Vec<_>>(), vec!["RS_Own"]);
    assert!(
        spec.market_value.is_none(),
        "an inherited price is not copied"
    );
    assert_eq!(
        spec.mass.map(|m| m.value),
        Some(3.0),
        "readout numbers are always copied"
    );
    assert!(spec.ce.is_none());
}

#[test]
fn a_price_the_def_sets_itself_is_kept_by_the_inheriting_reader() {
    let mut defs = base_defs();
    defs.push(abstract_gun_base());
    defs.push(child_gun(Some(("MarketValue", "300"))));
    let out = load_defs(defs, &types(&[]));
    let def = out.databases.get("ThingDef", "RS_ChildGun").unwrap();
    let own: BTreeSet<String> = ["Mass", "MarketValue"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    let spec = spec_from_def_inheriting(
        def,
        &out.databases,
        &ReaderOptions::default(),
        ValueSource::Anchor,
        &own,
    )
    .unwrap()
    .spec;
    assert_eq!(spec.market_value.map(|m| m.value), Some(300.0));
    assert!(
        !spec
            .parent
            .unwrap()
            .inherited_stats
            .contains_key("MarketValue")
    );
}

#[test]
fn a_def_without_a_parent_reads_the_same_with_or_without_the_inheriting_reader() {
    let out = load_all();
    let def = out.databases.get("ThingDef", "RS_GunB").unwrap();
    let opts = ReaderOptions::default();
    let plain = spec_from_def(def, &out.databases, &opts, ValueSource::Anchor).unwrap();
    let inheriting = spec_from_def_inheriting(
        def,
        &out.databases,
        &opts,
        ValueSource::Anchor,
        &BTreeSet::new(),
    )
    .unwrap();
    assert_eq!(plain, inheriting);
}

#[test]
fn fields_the_spec_cannot_hold_are_named_in_the_notes_in_document_order() {
    let mut weapon = gun("RS_Odd", "RS_Shot1", GunNumbers::default(), &[]);
    weapon.push_child(Node::with_text("relicChance", "2"));
    weapon.push_child(Node::with_text("smeltable", "true"));
    if let Some(verbs) = weapon.child_mut("verbs")
        && let Some(li) = verbs.elements_mut().next()
    {
        li.push_child(Node::with_text("aimingChargeMote", "RS_Mote"));
    }
    let mut defs = base_defs();
    defs.push(weapon);
    let out = load_defs(defs, &types(&[]));
    let def = out.databases.get("ThingDef", "RS_Odd").unwrap();
    let reading = spec_from_def(
        def,
        &out.databases,
        &ReaderOptions::default(),
        ValueSource::Anchor,
    )
    .unwrap();
    let note = reading
        .notes
        .iter()
        .find(|n| n.contains("does not carry"))
        .expect("a note names the left behind fields");
    let relic = note.find("relicChance").unwrap();
    let smelt = note.find("smeltable").unwrap();
    let verb = note.find("verbs.aimingChargeMote").unwrap();
    assert!(relic < smelt && smelt < verb, "{note}");
    assert!(!note.contains("soundInteract") && !note.contains("forcedMissRadius"));
    assert!(!note.contains("statBases") && !note.contains("verbs,"));
}

#[test]
fn a_fully_modelled_def_has_no_left_behind_note_beyond_the_category() {
    let out = load_all();
    let def = out.databases.get("ThingDef", "RS_GunA").unwrap();
    let reading = spec_from_def(
        def,
        &out.databases,
        &ReaderOptions::default(),
        ValueSource::Anchor,
    )
    .unwrap();
    let note = reading.notes.iter().find(|n| n.contains("does not carry"));
    assert_eq!(
        note.map(String::as_str),
        Some(
            "fields the designer does not model and a clone does not carry (the parent still supplies what the source inherited from it): category"
        )
    );
}
