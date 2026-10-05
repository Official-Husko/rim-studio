//! Fictional defs for the reader tests: names start with `RS_` and every number is invented. The helpers
//! build node trees, run them through the real def engine and return the load output.

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::sync::Arc;

use rimstudio_core::ids::{FileId, ModIdx};
use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_defs::{
    DefFile, FileContent, LoadInput, LoadOutput, ModEntry, TypeInfo, TypeTable, load,
};

/// The type table of the fictional set; `extra` adds `(full name, base)` entries.
pub(crate) fn types(extra: &[(&str, &str)]) -> Arc<TypeTable> {
    let mut list = vec![
        (
            "Verse.Def".to_owned(),
            TypeInfo::with_base("Verse.Editable"),
        ),
        (
            "Verse.ThingDef".to_owned(),
            TypeInfo::with_base("Verse.Def"),
        ),
        ("Verse.StatDef".to_owned(), TypeInfo::with_base("Verse.Def")),
        (
            "Verse.DamageDef".to_owned(),
            TypeInfo::with_base("Verse.Def"),
        ),
    ];
    for (name, base) in extra {
        list.push(((*name).to_owned(), TypeInfo::with_base(*base)));
    }
    Arc::new(TypeTable::new(list).unwrap())
}

/// Loads the given def nodes as one mod, in the given order.
pub(crate) fn load_defs(defs: Vec<Node>, table: &Arc<TypeTable>) -> LoadOutput {
    load_mods(vec![("rs.test", defs)], table)
}

/// Loads several mods in order, each a `(package id, defs)` pair.
pub(crate) fn load_mods(mods: Vec<(&str, Vec<Node>)>, table: &Arc<TypeTable>) -> LoadOutput {
    let entries: Vec<ModEntry> = mods
        .iter()
        .enumerate()
        .map(|(i, (id, _))| {
            ModEntry::new(ModIdx(u32::try_from(i).unwrap()), *id, format!("name {id}"))
        })
        .collect();
    let mut input = LoadInput::new(entries, table.clone());
    for (i, (_, defs)) in mods.into_iter().enumerate() {
        let mut root = Node::new("Defs");
        for d in defs {
            root.push_child(d);
        }
        input.def_files.push(DefFile {
            mod_idx: ModIdx(u32::try_from(i).unwrap()),
            file: FileId(u32::try_from(i).unwrap()),
            rel_path: format!("Defs/RS_{i}.xml"),
            content: FileContent::parsed(root),
        });
    }
    load(input)
}

/// A stat definition.
pub(crate) fn stat_def(name: &str, default: f64) -> Node {
    NodeBuilder::new("StatDef")
        .text_elem("defName", name)
        .text_elem("defaultBaseValue", default.to_string())
        .build()
}

/// A damage definition with a default damage and penetration.
pub(crate) fn damage_def(name: &str, damage: f64, ap: f64) -> Node {
    NodeBuilder::new("DamageDef")
        .text_elem("defName", name)
        .text_elem("defaultDamage", damage.to_string())
        .text_elem("defaultArmorPenetration", ap.to_string())
        .text_elem("armorCategory", "RS_Sharp")
        .build()
}

/// A projectile def.
pub(crate) fn projectile(name: &str, damage: f64, speed: f64, explosion: Option<f64>) -> Node {
    NodeBuilder::new("ThingDef")
        .text_elem("defName", name)
        .elem("projectile", |p| {
            p.text_elem("damageDef", "RS_Damage")
                .text_elem("damageAmountBase", damage.to_string())
                .text_elem("speed", speed.to_string())
                .when(explosion.is_some(), |p| {
                    p.text_elem("explosionRadius", explosion.unwrap_or(0.0).to_string())
                })
        })
        .build()
}

/// The numbers of a fictional gun.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GunNumbers {
    pub(crate) mass: f64,
    pub(crate) range: f64,
    pub(crate) warmup: f64,
    pub(crate) cooldown: f64,
    pub(crate) burst: u32,
    pub(crate) tier: &'static str,
}

impl Default for GunNumbers {
    fn default() -> Self {
        Self {
            mass: 3.0,
            range: 26.0,
            warmup: 1.2,
            cooldown: 1.6,
            burst: 1,
            tier: "Industrial",
        }
    }
}

/// A fictional gun that fires `projectile`.
pub(crate) fn gun(name: &str, projectile: &str, n: GunNumbers, tags: &[&str]) -> Node {
    NodeBuilder::new("ThingDef")
        .text_elem("defName", name)
        .text_elem("label", name.to_lowercase())
        .text_elem("category", "Item")
        .text_elem("techLevel", n.tier)
        .elem("statBases", |s| {
            s.text_elem("Mass", n.mass.to_string())
                .text_elem("RangedWeapon_Cooldown", n.cooldown.to_string())
                .text_elem("AccuracyTouch", "0.7")
                .text_elem("AccuracyShort", "0.75")
                .text_elem("AccuracyMedium", "0.6")
                .text_elem("AccuracyLong", "0.45")
                .text_elem("WorkToMake", "9000")
        })
        .elem("costList", |c| c.text_elem("RS_Steel", "30"))
        .elem("verbs", |v| {
            v.elem("li", |li| {
                li.text_elem("verbClass", "Verb_Shoot")
                    .text_elem("defaultProjectile", projectile)
                    .text_elem("warmupTime", n.warmup.to_string())
                    .text_elem("range", n.range.to_string())
                    .when(n.burst > 1, |li| {
                        li.text_elem("burstShotCount", n.burst.to_string())
                            .text_elem("ticksBetweenBurstShots", "8")
                    })
                    .text_elem("soundCast", "RS_Shot")
            })
        })
        .elem("tools", |t| {
            t.elem("li", |li| {
                li.text_elem("label", "grip")
                    .elem("capacities", |c| c.li("Blunt"))
                    .text_elem("power", "8")
                    .text_elem("cooldownTime", "2")
            })
        })
        .elem("weaponTags", |w| {
            if tags.is_empty() {
                w.li("RS_Gun")
            } else {
                w.li_each(tags.iter().copied())
            }
        })
        .build()
}

/// A fictional melee weapon with the given `(label, capacity, power, cooldown)` tools.
pub(crate) fn melee(name: &str, tier: &str, mass: f64, tools: &[(&str, &str, f64, f64)]) -> Node {
    NodeBuilder::new("ThingDef")
        .text_elem("defName", name)
        .text_elem("label", name.to_lowercase())
        .text_elem("category", "Item")
        .text_elem("techLevel", tier)
        .elem("statBases", |s| {
            s.text_elem("Mass", mass.to_string())
                .text_elem("WorkToMake", "4000")
        })
        .elem("weaponTags", |w| w.li("RS_Melee"))
        .elem("tools", |t| {
            tools.iter().fold(t, |t, (label, cap, power, cooldown)| {
                t.elem("li", |li| {
                    li.text_elem("label", *label)
                        .elem("capacities", |c| c.li(*cap))
                        .text_elem("power", power.to_string())
                        .text_elem("cooldownTime", cooldown.to_string())
                })
            })
        })
        .build()
}

/// A fixed armor item with a sharp rating.
pub(crate) fn armor(name: &str, sharp: f64) -> Node {
    NodeBuilder::new("ThingDef")
        .text_elem("defName", name)
        .text_elem("category", "Item")
        .elem("apparel", |a| a.elem("layers", |l| l.li("Shell")))
        .elem("statBases", |s| {
            s.text_elem("ArmorRating_Sharp", sharp.to_string())
        })
        .build()
}

/// A material with factors for the melee multiplier stats.
pub(crate) fn material(name: &str, category: &str, damage_factor: f64) -> Node {
    NodeBuilder::new("ThingDef")
        .text_elem("defName", name)
        .elem("stuffProps", |p| {
            p.elem("categories", |c| c.li(category))
                .elem("statFactors", |f| {
                    f.text_elem("MeleeWeapon_DamageMultiplier", damage_factor.to_string())
                })
        })
        .build()
}

/// The shared base: stat and damage definitions, a projectile and three armor items.
pub(crate) fn base_defs() -> Vec<Node> {
    vec![
        stat_def("Mass", 0.0),
        stat_def("RangedWeapon_Cooldown", 0.0),
        stat_def("MeleeWeapon_DamageMultiplier", 1.0),
        damage_def("RS_Damage", 10.0, 0.2),
        projectile("RS_Shot1", 10.0, 55.0, None),
        projectile("RS_Shot2", 14.0, 60.0, None),
        projectile("RS_Boom", 20.0, 40.0, Some(1.9)),
        armor("RS_ArmorA", 0.3),
        armor("RS_ArmorB", 0.6),
        armor("RS_ArmorC", 0.9),
    ]
}

/// A material whose sharp and blunt damage multipliers are its own stats, the way the game stores them.
pub(crate) fn material_with_multipliers(
    name: &str,
    category: &str,
    sharp: f64,
    blunt: f64,
) -> Node {
    NodeBuilder::new("ThingDef")
        .text_elem("defName", name)
        .elem("stuffProps", |p| p.elem("categories", |c| c.li(category)))
        .elem("statBases", |s| {
            s.text_elem("SharpDamageMultiplier", sharp.to_string())
                .text_elem("BluntDamageMultiplier", blunt.to_string())
        })
        .build()
}
