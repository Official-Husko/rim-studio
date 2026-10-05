//! A fictional install for the archetype tests: reference weapons with invented numbers, their pools, the
//! baseline models and a cost basis. Nothing here comes from a real game.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use rimstudio_design::archetype::{Basis, CostBasis, CostRow};
use rimstudio_design::baseline::{BaselineConfig, Model};
use rimstudio_design::classes::numeric::SplitMix64;
use rimstudio_design::classes::{ItemKind as PoolKind, Pool, PoolOptions};
use rimstudio_design::melee::{
    MeleeCapacity, MeleeModifiers, MeleeTool, enumerate_attacks, in_fight_dps, stat_panel_dps,
    strength as melee_strength,
};
use rimstudio_design::model::{ItemKind, TechLevel};
use rimstudio_design::ranged::{AccuracyBands, ReferenceArmor, nominal_dps, strength_p4};
use rimstudio_design::reader::{RangedDetail, ToolDetail, WeaponDetail};

pub(crate) const ARMOR: [f64; 3] = [0.0, 0.5, 1.0];

struct GunTemplate {
    role: &'static str,
    tier: TechLevel,
    damage: f64,
    range: f64,
    warmup: f64,
    cooldown: f64,
    burst: u32,
    ticks: f64,
    acc: [f64; 4],
    mass: f64,
    work: f64,
    tags: &'static [&'static str],
    classes: &'static [&'static str],
}

const GUNS: [GunTemplate; 7] = [
    GunTemplate {
        role: "pistol",
        tier: TechLevel::Industrial,
        damage: 10.0,
        range: 22.9,
        warmup: 0.3,
        cooldown: 1.2,
        burst: 1,
        ticks: 15.0,
        acc: [0.8, 0.7, 0.5, 0.35],
        mass: 1.2,
        work: 5000.0,
        tags: &["RsGun", "RsSimple"],
        classes: &["RsRanged", "RsLight"],
    },
    GunTemplate {
        role: "smg",
        tier: TechLevel::Industrial,
        damage: 7.0,
        range: 19.9,
        warmup: 0.6,
        cooldown: 1.0,
        burst: 3,
        ticks: 8.0,
        acc: [0.85, 0.65, 0.4, 0.2],
        mass: 2.5,
        work: 15000.0,
        tags: &["RsGun", "RsAdvanced"],
        classes: &["RsRanged", "RsShort"],
    },
    GunTemplate {
        role: "rifle",
        tier: TechLevel::Industrial,
        damage: 11.0,
        range: 30.9,
        warmup: 1.0,
        cooldown: 1.7,
        burst: 3,
        ticks: 10.0,
        acc: [0.6, 0.7, 0.65, 0.55],
        mass: 3.5,
        work: 40000.0,
        tags: &["RsGun", "RsAdvanced"],
        classes: &["RsRanged"],
    },
    GunTemplate {
        role: "sniper",
        tier: TechLevel::Industrial,
        damage: 24.0,
        range: 44.9,
        warmup: 3.2,
        cooldown: 1.5,
        burst: 1,
        ticks: 15.0,
        acc: [0.5, 0.7, 0.88, 0.9],
        mass: 4.0,
        work: 45000.0,
        tags: &["RsGun"],
        classes: &["RsRanged", "RsLong"],
    },
    GunTemplate {
        role: "shotgun",
        tier: TechLevel::Industrial,
        damage: 18.0,
        range: 15.9,
        warmup: 0.9,
        cooldown: 1.3,
        burst: 1,
        ticks: 15.0,
        acc: [0.8, 0.85, 0.75, 0.6],
        mass: 3.4,
        work: 12000.0,
        tags: &["RsGun", "RsShort"],
        classes: &["RsRanged", "RsShort"],
    },
    GunTemplate {
        role: "heavy",
        tier: TechLevel::Industrial,
        damage: 12.0,
        range: 26.9,
        warmup: 1.8,
        cooldown: 1.6,
        burst: 6,
        ticks: 7.0,
        acc: [0.4, 0.5, 0.35, 0.26],
        mass: 8.5,
        work: 34000.0,
        tags: &["RsGun", "RsAdvanced"],
        classes: &["RsRanged", "RsHeavy"],
    },
    GunTemplate {
        role: "bow",
        tier: TechLevel::Neolithic,
        damage: 12.0,
        range: 23.9,
        warmup: 1.4,
        cooldown: 1.65,
        burst: 1,
        ticks: 15.0,
        acc: [0.75, 0.65, 0.45, 0.25],
        mass: 0.8,
        work: 2400.0,
        tags: &["RsPrimitive"],
        classes: &["RsRanged", "RsLight"],
    },
];

fn gun(index: usize, t: &GunTemplate, jitter: f64, name: String) -> WeaponDetail {
    let j = |v: f64| v * (1.0 + jitter);
    let damage = j(t.damage).round().max(1.0);
    let detail = RangedDetail {
        verb_class: "Verb_Shoot".into(),
        projectile: format!("RS_Bullet{index}"),
        damage,
        speed: Some(55.0),
        armor_penetration: 0.015 * damage,
        range: (j(t.range) - 0.9).round() + 0.9,
        warmup: (j(t.warmup) * 20.0).round() / 20.0,
        cooldown: (j(t.cooldown) * 20.0).round() / 20.0,
        burst_count: t.burst,
        ticks_between_shots: t.ticks,
        accuracy: Some(AccuracyBands {
            touch: t.acc[0],
            short: t.acc[1],
            medium: t.acc[2],
            long: t.acc[3],
        }),
    };
    let profile = detail.profile();
    let strength = strength_p4(
        &profile,
        &ReferenceArmor {
            sharp_ratings: &ARMOR,
        },
    )
    .ok();
    let mut stats = BTreeMap::new();
    for (k, v) in [
        ("damage", detail.damage),
        ("range", detail.range),
        ("warmup", detail.warmup),
        ("cooldown", detail.cooldown),
        ("burst", f64::from(detail.burst_count)),
        ("ticks_between", detail.ticks_between_shots),
        ("ap", detail.armor_penetration),
        ("touch", t.acc[0]),
        ("short", t.acc[1]),
        ("medium", t.acc[2]),
        ("long", t.acc[3]),
        ("mass", j(t.mass)),
        ("work", j(t.work).round()),
    ] {
        stats.insert(k.to_owned(), v);
    }
    stats.insert("dps".into(), nominal_dps(&profile).unwrap());
    WeaponDetail {
        def_name: name.clone(),
        label: name,
        kind: PoolKind::Ranged,
        tech_level: Some(t.tier),
        mod_idx: None,
        weapon_tags: t.tags.iter().map(|s| (*s).to_owned()).collect(),
        weapon_classes: t.classes.iter().map(|s| (*s).to_owned()).collect(),
        stuff_categories: Vec::new(),
        ranged: Some(detail),
        tools: vec![
            ToolDetail {
                label: "stock".into(),
                capacities: vec!["Blunt".into()],
                power: Some(9.0),
                cooldown: Some(2.0),
                armor_penetration: None,
                chance_factor: 1.0,
                linked_body_parts_group: None,
            },
            ToolDetail {
                label: "barrel".into(),
                capacities: vec!["Blunt".into(), "Poke".into()],
                power: Some(9.0),
                cooldown: Some(2.0),
                armor_penetration: None,
                chance_factor: 1.0,
                linked_body_parts_group: None,
            },
        ],
        role: Some(t.role.to_owned()),
        strength,
        stats,
    }
}

struct BladeTemplate {
    role: &'static str,
    tier: TechLevel,
    tools: &'static [(&'static str, &'static [&'static str], f64, f64)],
    mass: f64,
    work: f64,
}

const BLADES: [BladeTemplate; 4] = [
    BladeTemplate {
        role: "blade",
        tier: TechLevel::Medieval,
        tools: &[
            ("handle", &["Blunt"], 9.0, 2.0),
            ("point", &["Stab"], 16.0, 2.0),
            ("edge", &["Cut"], 16.0, 2.0),
        ],
        mass: 0.9,
        work: 12000.0,
    },
    BladeTemplate {
        role: "blade",
        tier: TechLevel::Medieval,
        tools: &[
            ("handle", &["Blunt"], 9.0, 2.0),
            ("point", &["Stab"], 23.0, 2.6),
            ("edge", &["Cut"], 23.0, 2.6),
        ],
        mass: 2.0,
        work: 18000.0,
    },
    BladeTemplate {
        role: "blunt",
        tier: TechLevel::Medieval,
        tools: &[
            ("handle", &["Poke"], 9.0, 2.0),
            ("head", &["Blunt"], 15.7, 2.0),
        ],
        mass: 1.25,
        work: 6000.0,
    },
    BladeTemplate {
        role: "blunt",
        tier: TechLevel::Neolithic,
        tools: &[
            ("handle", &["Poke"], 9.0, 2.0),
            ("head", &["Blunt"], 14.0, 2.0),
        ],
        mass: 2.0,
        work: 1200.0,
    },
];

fn blade(t: &BladeTemplate, jitter: f64, name: String) -> WeaponDetail {
    let tools: Vec<ToolDetail> = t
        .tools
        .iter()
        .map(|(label, caps, power, cooldown)| ToolDetail {
            label: (*label).into(),
            capacities: caps.iter().map(|c| (*c).to_owned()).collect(),
            power: Some((power * (1.0 + jitter)).round()),
            cooldown: Some(*cooldown),
            armor_penetration: None,
            chance_factor: 1.0,
            linked_body_parts_group: None,
        })
        .collect();
    let inputs: Vec<MeleeTool> = tools
        .iter()
        .map(|t| MeleeTool {
            label: t.label.clone(),
            capacities: t
                .capacities
                .iter()
                .map(|c| MeleeCapacity::named(c))
                .collect(),
            power: t.power.unwrap(),
            cooldown: t.cooldown.unwrap(),
            armor_penetration: None,
            chance_factor: 1.0,
        })
        .collect();
    let attacks = enumerate_attacks(&inputs, &MeleeModifiers::default()).unwrap();
    let panel = stat_panel_dps(&attacks).unwrap();
    let fight = in_fight_dps(&attacks).unwrap();
    let mut stats = BTreeMap::new();
    stats.insert("swing_damage".to_owned(), panel.damage);
    stats.insert("cooldown".to_owned(), panel.cooldown);
    stats.insert("dps".to_owned(), panel.dps);
    stats.insert("ap".to_owned(), panel.armor_penetration);
    stats.insert("fight_dps".to_owned(), fight.dps);
    stats.insert("tools".to_owned(), tools.len() as f64);
    stats.insert("mass".to_owned(), t.mass * (1.0 + jitter));
    stats.insert("work".to_owned(), t.work);
    WeaponDetail {
        def_name: name.clone(),
        label: name,
        kind: PoolKind::Melee,
        tech_level: Some(t.tier),
        mod_idx: None,
        weapon_tags: vec!["RsMelee".into()],
        weapon_classes: vec!["RsMelee".into()],
        stuff_categories: vec!["Metallic".into()],
        ranged: None,
        tools,
        role: Some(t.role.to_owned()),
        strength: melee_strength(&attacks).ok(),
        stats,
    }
}

/// The fictional install: reference weapons, pools, models and costs.
pub(crate) struct Install {
    pub weapons: Vec<WeaponDetail>,
    pub ranged: Pool,
    pub melee: Pool,
    pub costs: CostBasis,
    pub ranged_model: Model,
    pub melee_model: Model,
}

impl Install {
    pub(crate) fn basis_ranged(&self) -> Basis<'_> {
        Basis {
            model: &self.ranged_model,
            weapons: &self.weapons,
            armor: &ARMOR,
            costs: Some(&self.costs),
        }
    }

    pub(crate) fn basis_melee(&self) -> Basis<'_> {
        Basis {
            model: &self.melee_model,
            weapons: &self.weapons,
            armor: &ARMOR,
            costs: Some(&self.costs),
        }
    }
}

/// Builds the install: four jittered copies of each template, with a fixed seed.
pub(crate) fn install() -> Install {
    let mut rng = SplitMix64::new(7);
    let mut weapons = Vec::new();
    for (i, t) in GUNS.iter().enumerate() {
        for k in 0..4 {
            let jitter = (rng.next_f64() - 0.5) * 0.16;
            weapons.push(gun(i * 10 + k, t, jitter, format!("RS_{}_{k}", t.role)));
        }
    }
    for (i, t) in BLADES.iter().enumerate() {
        for k in 0..3 {
            let jitter = (rng.next_f64() - 0.5) * 0.12;
            weapons.push(blade(t, jitter, format!("RS_Melee{i}_{k}")));
        }
    }
    let items = |kind: PoolKind| -> Vec<_> {
        weapons
            .iter()
            .filter(|w| w.kind == kind)
            .map(WeaponDetail::reference_item)
            .collect()
    };
    let ranged = Pool::build(
        PoolKind::Ranged,
        &items(PoolKind::Ranged),
        &PoolOptions::default(),
    );
    let melee = Pool::build(
        PoolKind::Melee,
        &items(PoolKind::Melee),
        &PoolOptions::default(),
    );
    let rows: Vec<CostRow> = weapons
        .iter()
        .enumerate()
        .map(|(i, w)| CostRow {
            def_name: w.def_name.clone(),
            kind: if w.kind == PoolKind::Melee {
                ItemKind::Melee
            } else {
                ItemKind::Ranged
            },
            tier: w.tech_level.map(|t| t.index()),
            role: w.role.clone(),
            strength: w.strength,
            cost: if w.kind == PoolKind::Melee {
                BTreeMap::new()
            } else {
                [
                    ("RS_Steel".to_owned(), 20.0 + (i % 5) as f64 * 5.0),
                    ("RS_Part".to_owned(), 2.0 + (i % 3) as f64),
                ]
                .into_iter()
                .collect()
            },
            stuff_categories: w.stuff_categories.clone(),
            stuff_count: (w.kind == PoolKind::Melee).then_some(40.0 + (i % 4) as f64 * 10.0),
        })
        .collect();
    let costs = CostBasis {
        rows,
        unit_values: [("RS_Steel".to_owned(), 1.9), ("RS_Part".to_owned(), 32.0)]
            .into_iter()
            .collect(),
    };
    let ranged_model = Model::fit(ranged.clone(), BaselineConfig::default());
    let melee_model = Model::fit(melee.clone(), BaselineConfig::default());
    Install {
        weapons,
        ranged,
        melee,
        costs,
        ranged_model,
        melee_model,
    }
}
