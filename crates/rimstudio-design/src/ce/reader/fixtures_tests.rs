//! A fictional Combat Extended reference set for the tests. Every name starts with `RS_` and every number is
//! invented. The set is a vanilla mod with guns, projectiles and melee weapons, and a Combat Extended mod
//! that converts them with the gun conversion operation and plain patch operations, so the tests run the
//! real def engine with the registry of [`crate::ce::reader::custom_registry`].

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::collections::BTreeMap;
use std::sync::Arc;

use rimstudio_core::ids::{FileId, ModIdx};
use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_defs::{
    DefFile, FileContent, LoadInput, LoadOutput, ModEntry, PatchFile, TypeTable, load,
};

use crate::ce::reader::{CeClassNames, custom_registry, with_ce_types};
use crate::reader::fixtures_tests::{self as vanilla, GunNumbers};

/// The CE side numbers of a fictional gun.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CeNumbers {
    pub(crate) mass: f64,
    pub(crate) bulk: f64,
    pub(crate) range: f64,
    pub(crate) warmup: f64,
    pub(crate) cooldown: f64,
    pub(crate) spread: f64,
    pub(crate) sway: f64,
    pub(crate) magazine: u32,
    pub(crate) reload: f64,
    pub(crate) burst: u32,
}

/// One fictional gun with both sides.
#[derive(Debug, Clone)]
pub(crate) struct GunRow {
    pub(crate) name: String,
    pub(crate) role: &'static str,
    pub(crate) vanilla: GunNumbers,
    pub(crate) ce: CeNumbers,
}

/// Twelve guns of two roles. CE mass equals the vanilla mass, CE range is a fixed multiple of the vanilla
/// range, spread and sway are class constants, bulk follows mass.
pub(crate) fn rows() -> Vec<GunRow> {
    let mut out = Vec::new();
    for i in 0..12u32 {
        let f = f64::from(i);
        let rifle = i % 3 != 2;
        let mass = 1.5 + 0.4 * f;
        let range = 15.0 + 2.5 * f;
        let burst = if rifle && i % 2 == 0 { 3 } else { 1 };
        out.push(GunRow {
            name: format!("RS_CeGun{i:02}"),
            role: if rifle {
                "CE_AI_RS_Rifle"
            } else {
                "CE_AI_RS_Pistol"
            },
            vanilla: GunNumbers {
                mass,
                range,
                warmup: 0.8 + 0.1 * f,
                cooldown: 1.0 + 0.12 * f,
                burst,
                tier: if i < 6 { "Industrial" } else { "Spacer" },
            },
            ce: CeNumbers {
                mass,
                bulk: 2.0 * mass,
                range: range * 1.8,
                warmup: 0.5 + 0.05 * f,
                cooldown: if rifle { 0.4 } else { 0.5 },
                spread: if rifle { 0.07 } else { 0.15 },
                sway: if rifle { 1.3 } else { 1.1 },
                magazine: if rifle { 30 } else { 8 },
                reload: 4.0,
                burst,
            },
        });
    }
    out
}

/// The CE patch for one gun: the gun conversion operation.
pub(crate) fn makegun_op(g: &GunRow) -> Node {
    NodeBuilder::new("Operation")
        .attr("Class", CeClassNames::default().make_gun_op)
        .text_elem("defName", &g.name)
        .elem("statBases", |s| {
            s.text_elem("Mass", g.ce.mass.to_string())
                .text_elem("Bulk", g.ce.bulk.to_string())
                .text_elem("ShotSpread", g.ce.spread.to_string())
                .text_elem("SwayFactor", g.ce.sway.to_string())
                .text_elem("SightsEfficiency", "1")
                .text_elem("RangedWeapon_Cooldown", g.ce.cooldown.to_string())
        })
        .elem("Properties", |p| {
            p.text_elem("verbClass", CeClassNames::default().shoot_verb)
                .text_elem("hasStandardCommand", "true")
                .text_elem("defaultProjectile", "RS_CeBullet1")
                .text_elem("warmupTime", g.ce.warmup.to_string())
                .text_elem("range", g.ce.range.to_string())
                .text_elem("burstShotCount", g.ce.burst.to_string())
                .text_elem("ticksBetweenBurstShots", "6")
                .text_elem("recoilAmount", "1.4")
        })
        .elem("AmmoUser", |a| {
            a.text_elem("magazineSize", g.ce.magazine.to_string())
                .text_elem("reloadTime", g.ce.reload.to_string())
                .text_elem("ammoSet", "RS_AmmoSetA")
        })
        .elem("FireModes", |f| f.text_elem("aiAimMode", "AimedShot"))
        .elem("weaponTags", |t| t.li(g.role))
        .build()
}

/// Defs of the CE mod: two ammo sets, their projectiles and one gun preset.
pub(crate) fn ce_defs() -> Vec<Node> {
    let ammo_set = |name: &str, ammo: &str, projectile: &str| {
        NodeBuilder::new("CombatExtended.AmmoSetDef")
            .text_elem("defName", name)
            .elem("ammoTypes", |t| t.text_elem(ammo, projectile))
            .build()
    };
    let bullet = |name: &str, damage: f64, sharp: f64, blunt: f64, speed: f64| {
        NodeBuilder::new("ThingDef")
            .text_elem("defName", name)
            .elem("projectile", |p| {
                p.text_elem("damageAmountBase", damage.to_string())
                    .text_elem("armorPenetrationSharp", sharp.to_string())
                    .text_elem("armorPenetrationBlunt", blunt.to_string())
                    .text_elem("speed", speed.to_string())
            })
            .build()
    };
    vec![
        ammo_set("RS_AmmoSetA", "RS_AmmoA1", "RS_CeBullet1"),
        ammo_set("RS_AmmoSetB", "RS_AmmoB1", "RS_CeBullet2"),
        bullet("RS_CeBullet1", 9.0, 2.5, 14.0, 120.0),
        bullet("RS_CeBullet2", 13.0, 4.0, 22.0, 140.0),
        gun_preset(),
    ]
}

/// A fictional gun preset in the shape of the install's preset defs.
pub(crate) fn gun_preset() -> Node {
    NodeBuilder::new("CombatExtended.GunPatcherPresetDef")
        .text_elem("defName", "RS_PresetRifle")
        .elem("names", |n| n.li("rsrifle").li("rscarbine"))
        .elem("tags", |n| n.li("RS_Handgun"))
        .text_elem("DiscardDesignations", "true")
        .text_elem("DetermineCaliber", "true")
        .text_elem("setCaliber", "RS_AmmoSetA")
        .text_elem("RangeRange", "20~40")
        .text_elem("WarmupRange", "0.5~1.5")
        .text_elem("damageRange", "5~15")
        .text_elem("projSpeedRange", "50~90")
        .elem("rangeCurve", |c| {
            c.elem("points", |p| p.li("20,30").li("40,70"))
        })
        .elem("warmupCurve", |c| c.elem("points", |p| p.li("1,0.9")))
        .elem("cooldownCurve", |c| c.elem("points", |p| p.li("1.5,0.4")))
        .text_elem("Mass", "3")
        .text_elem("Bulk", "6")
        .text_elem("Spread", "0.08")
        .text_elem("Sway", "1.2")
        .text_elem("ReloadTime", "4")
        .text_elem("AmmoCapacity", "25")
        .elem("gunStats", |g| {
            g.text_elem("burstShotCount", "3")
                .text_elem("recoilAmount", "1.5")
                .text_elem("range", "40")
                .text_elem("warmupTime", "0.9")
        })
        .elem("MiscOtherStats", |m| m.text_elem("SightsEfficiency", "0.8"))
        .elem("CaliberRanges", |c| {
            c.elem("li", |li| {
                li.text_elem("DamageRange", "5~12")
                    .text_elem("SpeedRange", "50~80")
                    .text_elem("AmmoSet", "RS_AmmoSetA")
            })
        })
        .elem("specialGuns", |s| {
            s.elem("li", |li| {
                li.elem("names", |n| n.li("rsspecial"))
                    .text_elem("magCap", "12")
                    .text_elem("mass", "2.5")
            })
        })
        .build()
}

/// The vanilla mod's defs: base stat and damage defs, projectiles, guns, a few melee weapons.
pub(crate) fn vanilla_defs(rows: &[GunRow]) -> Vec<Node> {
    let mut defs = vanilla::base_defs();
    for g in rows {
        defs.push(vanilla::gun(
            &g.name,
            "RS_Shot1",
            g.vanilla,
            &["RS_Handgun"],
        ));
    }
    defs.push(vanilla::melee(
        "RS_CeEdge",
        "Medieval",
        1.4,
        &[("blade", "Cut", 18.0, 2.2), ("handle", "Blunt", 9.0, 2.0)],
    ));
    defs
}

/// The type table of the set, with the Combat Extended def types added when `with_ce`.
pub(crate) fn table(with_ce: bool) -> Arc<TypeTable> {
    let base = vanilla::types(&[]);
    if with_ce {
        Arc::new(with_ce_types(&base, &CeClassNames::default()).unwrap())
    } else {
        base
    }
}

/// The melee patch of the CE mod: replaces the tools, adds a bulk stat and offsets.
pub(crate) fn melee_patches() -> Vec<Node> {
    let class = |c: &str| ("Class", c.to_owned());
    let (k, v) = class("PatchOperationReplace");
    let replace = NodeBuilder::new("Operation")
        .attr(k, v)
        .text_elem("xpath", "Defs/ThingDef[defName=\"RS_CeEdge\"]/tools")
        .elem("value", |value| {
            value.elem("tools", |t| {
                t.elem("li", |li| {
                    li.attr("Class", CeClassNames::default().tool)
                        .text_elem("label", "blade")
                        .elem("capacities", |c| c.li("Cut"))
                        .text_elem("power", "20")
                        .text_elem("cooldownTime", "1.8")
                        .text_elem("armorPenetrationSharp", "0.6")
                        .text_elem("armorPenetrationBlunt", "1.2")
                })
            })
        })
        .build();
    let (k, v) = class("PatchOperationAdd");
    let add_bulk = NodeBuilder::new("Operation")
        .attr(k, v)
        .text_elem("xpath", "Defs/ThingDef[defName=\"RS_CeEdge\"]/statBases")
        .elem("value", |value| value.text_elem("Bulk", "6.5"))
        .build();
    vec![replace, add_bulk]
}

/// What to include in a load.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Build {
    /// Include the Combat Extended mod.
    pub(crate) ce: bool,
    /// Register the gun conversion operation (otherwise it stays an unknown class).
    pub(crate) register_op: bool,
    /// Add the Combat Extended def types to the type table.
    pub(crate) ce_types: bool,
    /// Include the conversion patch file.
    pub(crate) patches: bool,
}

impl Default for Build {
    fn default() -> Self {
        Self {
            ce: true,
            register_op: true,
            ce_types: true,
            patches: true,
        }
    }
}

/// Loads the fictional set.
pub(crate) fn load_set(rows: &[GunRow], build: Build) -> LoadOutput {
    let mut mods = vec![ModEntry::new(ModIdx(0), "rs.vanilla", "RS Vanilla")];
    if build.ce {
        mods.push(ModEntry::new(
            ModIdx(1),
            "CETeam.CombatExtended",
            "RS Combat Extended",
        ));
    }
    let mut input = LoadInput::new(mods, table(build.ce_types));
    let mut root = Node::new("Defs");
    for d in vanilla_defs(rows) {
        root.push_child(d);
    }
    input.def_files.push(DefFile {
        mod_idx: ModIdx(0),
        file: FileId(0),
        rel_path: "Defs/RS_Vanilla.xml".into(),
        content: FileContent::parsed(root),
    });
    if build.ce {
        let mut ce_root = Node::new("Defs");
        for d in ce_defs() {
            ce_root.push_child(d);
        }
        input.def_files.push(DefFile {
            mod_idx: ModIdx(1),
            file: FileId(1),
            rel_path: "Defs/RS_Ce.xml".into(),
            content: FileContent::parsed(ce_root),
        });
        if build.patches {
            let mut patch = Node::new("Patch");
            for g in rows {
                patch.push_child(makegun_op(g));
            }
            for op in melee_patches() {
                patch.push_child(op);
            }
            input.patch_files.push(PatchFile {
                mod_idx: ModIdx(1),
                file: FileId(2),
                rel_path: "Patches/RS_Ce.xml".into(),
                content: FileContent::parsed(patch),
            });
        }
        if build.register_op {
            input.custom_ops = custom_registry(&CeClassNames::default(), BTreeMap::new());
        }
    }
    load(input)
}
