//! Converted weapons: the user's Combat Extended guns and melee weapons, their numbers, and their vanilla
//! twins.
//!
//! A converted gun is a `ThingDef` whose resolved form carries the converted verb entry, the ammo user
//! component or converted tools (the detector of [`super::names`]). Its numbers are read from the resolved
//! node, so conversions made by the gun conversion operation are seen as the game sees them when the load
//! registered [`super::makegun::MakeGunOp`]. A twin is the same def name in a vanilla load (a second set of
//! databases without Combat Extended); it gives the vanilla values the conversion started from.

use std::collections::BTreeMap;

use rimstudio_core::ids::ModIdx;
use rimstudio_core::tree::Node;
use rimstudio_defs::{DefDatabases, DefRecord};
use serde::{Deserialize, Serialize};

use super::ammo::{AmmoSetInfo, ProjectileInfo};
use super::names::{CeClassNames, CeMarkers, detect_markers};
use crate::classes::numeric::median;
use crate::model::{CePatchSpec, CeToolPenetration, Sourced, TechLevel, ValueSource};
use crate::reader::access::{
    child_bool, child_number, child_text, class_attr, list_items, list_texts, number_map,
};
use crate::reader::options::Exclusions;
use crate::reader::weapons::{SkipReason, WeaponDetail, is_weapon_def, tech_level_named};

/// A converted gun.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CeGun {
    /// The def name.
    pub def_name: String,
    /// The label.
    pub label: String,
    /// The mod that owns the def.
    pub mod_idx: Option<ModIdx>,
    /// The tech level named by the def.
    pub tech_level: Option<TechLevel>,
    /// Every weapon tag.
    pub weapon_tags: Vec<String>,
    /// The tags with the Combat Extended prefix.
    pub ce_tags: Vec<String>,
    /// The first tag with the AI class prefix.
    pub ai_class: Option<String>,
    /// The ammo set of the ammo user component.
    pub ammo_set: Option<String>,
    /// The default projectile of the converted verb.
    pub default_projectile: Option<String>,
    /// Whether the magazine is loaded one round at a time.
    pub reload_one_at_a_time: bool,
    /// The AI aim mode of the fire modes component.
    pub ai_aim_mode: Option<String>,
    /// The `aiUseBurstMode` flag of the fire modes component, when it is written.
    #[serde(default)]
    pub use_burst_mode: Option<bool>,
    /// The `aimedBurstShotCount` of the fire modes component, when it is written.
    #[serde(default)]
    pub aimed_burst: Option<f64>,
    /// The converted tools of the gun (the gun bash).
    #[serde(default)]
    pub tools: Vec<CeToolRow>,
    /// The converted numbers by name (see [`GUN_STATS`]).
    pub stats: BTreeMap<String, f64>,
    /// The numbers of the vanilla twin with the same names, when a vanilla load was given.
    pub twin: Option<BTreeMap<String, f64>>,
    /// The weapon tags of the vanilla twin (empty without a twin): what a vanilla design knows before any
    /// conversion, so that class assignment never sees tags that the conversion added.
    #[serde(default)]
    pub twin_tags: Vec<String>,
    /// Why the gun stays out of the class pools (a turret gun, a launcher, a grenade), when it does.
    pub excluded: Option<SkipReason>,
}

/// One tool of a converted melee weapon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CeToolRow {
    /// The tool label.
    pub label: String,
    /// Capacity def names.
    pub capacities: Vec<String>,
    /// Power.
    pub power: Option<f64>,
    /// Cooldown in seconds.
    pub cooldown: Option<f64>,
    /// Sharp penetration.
    pub ap_sharp: Option<f64>,
    /// Blunt penetration.
    pub ap_blunt: Option<f64>,
    /// Chance factor.
    pub chance_factor: Option<f64>,
    /// The body part group that carries the tool.
    #[serde(default)]
    pub linked_body_parts_group: Option<String>,
}

/// A converted melee weapon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CeMelee {
    /// The def name.
    pub def_name: String,
    /// The label.
    pub label: String,
    /// The mod that owns the def.
    pub mod_idx: Option<ModIdx>,
    /// The tech level named by the def.
    pub tech_level: Option<TechLevel>,
    /// Every weapon tag.
    pub weapon_tags: Vec<String>,
    /// The tools.
    pub tools: Vec<CeToolRow>,
    /// The converted numbers by name (see [`MELEE_STATS`]).
    pub stats: BTreeMap<String, f64>,
    /// The numbers of the vanilla twin with the same names, when a vanilla load was given.
    pub twin: Option<BTreeMap<String, f64>>,
    /// The weapon tags of the vanilla twin (empty without a twin).
    #[serde(default)]
    pub twin_tags: Vec<String>,
    /// Why the weapon stays out of the class pools, when it does.
    pub excluded: Option<SkipReason>,
}

/// The names of the numbers of a [`CeGun`]: `mass`, `bulk`, `spread`, `sway`, `sights`, `cooldown`,
/// `recoil`, `warmup`, `range`, `burst`, `ticks_between`, `magazine`, `reload`, and from the first projectile
/// of the ammo set `damage`, `ap_sharp`, `ap_blunt`, `speed`, `pellets`.
pub const GUN_STATS: [&str; 18] = [
    "mass",
    "bulk",
    "spread",
    "sway",
    "sights",
    "cooldown",
    "recoil",
    "warmup",
    "range",
    "burst",
    "ticks_between",
    "magazine",
    "reload",
    "damage",
    "ap_sharp",
    "ap_blunt",
    "speed",
    "pellets",
];

/// The names of the numbers of a [`CeMelee`]: `mass`, `bulk`, `counter_parry`, `crit`, `parry`, `dodge`,
/// `tool_power` and `tool_cooldown` (means over the tools), `ap_sharp_ratio` and `ap_blunt_ratio` (medians of
/// penetration over power), `tools`.
pub const MELEE_STATS: [&str; 10] = [
    "mass",
    "bulk",
    "counter_parry",
    "crit",
    "parry",
    "dodge",
    "tool_power",
    "tool_cooldown",
    "ap_sharp_ratio",
    "ap_blunt_ratio",
];

fn put(map: &mut BTreeMap<String, f64>, name: &str, value: Option<f64>) {
    if let Some(v) = value.filter(|v| v.is_finite()) {
        map.insert(name.to_owned(), v);
    }
}

fn find_class<'a>(node: &'a Node, list: &str, class: &str) -> Option<&'a Node> {
    list_items(node, list)
        .into_iter()
        .find(|li| class_attr(li).is_some_and(|c| c.eq_ignore_ascii_case(class)))
}

fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

/// Reads a converted gun. `markers` must say the def is a conversion with a converted verb or ammo
/// component. `ammo_sets` and `dbs` resolve the projectile numbers.
#[must_use]
pub fn read_gun(
    def: &DefRecord,
    classes: &CeClassNames,
    ammo_sets: &[AmmoSetInfo],
    dbs: &DefDatabases,
    thing_type: &str,
    exclusions: &Exclusions,
) -> CeGun {
    let node = &def.node;
    let verb = find_class(node, "verbs", &classes.verb_properties);
    let ammo = find_class(node, "comps", &classes.ammo_user);
    let modes = find_class(node, "comps", &classes.fire_modes);
    let weapon_tags = list_texts(node, "weaponTags");
    let stat = number_map(node, "statBases");
    let default_projectile = verb.and_then(|v| child_text(v, "defaultProjectile"));
    let ammo_set = ammo.and_then(|a| child_text(a, "ammoSet"));
    let projectile: ProjectileInfo = ammo_set
        .as_deref()
        .and_then(|s| ammo_sets.iter().find(|a| a.def_name == s))
        .and_then(|set| set.first().map(|f| f.info.clone()))
        .or_else(|| {
            default_projectile
                .as_deref()
                .and_then(|p| dbs.get(thing_type, p))
                .map(|d| ProjectileInfo::from_def(&d.node))
        })
        .unwrap_or_default();
    let mut stats = BTreeMap::new();
    for (name, key) in [
        ("mass", "Mass"),
        ("bulk", "Bulk"),
        ("spread", "ShotSpread"),
        ("sway", "SwayFactor"),
        ("sights", "SightsEfficiency"),
        ("cooldown", "RangedWeapon_Cooldown"),
    ] {
        put(&mut stats, name, stat.get(key).copied());
    }
    if let Some(v) = verb {
        put(&mut stats, "recoil", child_number(v, "recoilAmount"));
        put(&mut stats, "warmup", child_number(v, "warmupTime"));
        put(&mut stats, "range", child_number(v, "range"));
        put(
            &mut stats,
            "burst",
            Some(child_number(v, "burstShotCount").unwrap_or(1.0)),
        );
        put(
            &mut stats,
            "ticks_between",
            child_number(v, "ticksBetweenBurstShots"),
        );
    }
    if let Some(a) = ammo {
        put(&mut stats, "magazine", child_number(a, "magazineSize"));
        put(&mut stats, "reload", child_number(a, "reloadTime"));
    }
    put(&mut stats, "damage", projectile.damage);
    put(&mut stats, "ap_sharp", projectile.ap_sharp);
    put(&mut stats, "ap_blunt", projectile.ap_blunt);
    put(&mut stats, "speed", projectile.speed);
    put(&mut stats, "pellets", projectile.pellets);
    let ce_tags: Vec<String> = weapon_tags
        .iter()
        .filter(|t| t.starts_with(&classes.tag_prefix))
        .cloned()
        .collect();
    let ai_class = weapon_tags
        .iter()
        .find(|t| t.starts_with(&classes.ai_tag_prefix))
        .cloned();
    let verb_class = verb.and_then(|v| child_text(v, "verbClass"));
    let excluded = exclusion_of(
        def,
        exclusions,
        verb_class.as_deref(),
        projectile.explosion_radius.is_some_and(|r| r > 0.0),
        &weapon_tags,
    );
    CeGun {
        def_name: def.def_name.clone(),
        label: child_text(node, "label").unwrap_or_else(|| def.def_name.clone()),
        mod_idx: def.mod_idx(),
        tech_level: child_text(node, "techLevel").and_then(|t| tech_level_named(&t)),
        weapon_tags,
        ce_tags,
        ai_class,
        ammo_set,
        default_projectile,
        reload_one_at_a_time: ammo
            .and_then(|a| child_bool(a, "reloadOneAtATime"))
            .unwrap_or(false),
        ai_aim_mode: modes.and_then(|m| child_text(m, "aiAimMode")),
        use_burst_mode: modes.and_then(|m| child_bool(m, "aiUseBurstMode")),
        aimed_burst: modes.and_then(|m| child_number(m, "aimedBurstShotCount")),
        tools: list_items(node, "tools")
            .into_iter()
            .map(tool_row)
            .collect(),
        stats,
        twin: None,
        twin_tags: Vec::new(),
        excluded,
    }
}

/// Why a converted weapon stays out of the class pools: the same structural rules as the vanilla reader
/// (hidden, destroyed on drop, excluded tags and name suffixes, one use and launcher verbs, explosive
/// projectiles). A modded verb or tool class is of course expected here, so it is not a reason.
#[must_use]
pub fn exclusion_of(
    def: &DefRecord,
    ex: &Exclusions,
    verb_class: Option<&str>,
    explosive: bool,
    tags: &[String],
) -> Option<SkipReason> {
    let node = &def.node;
    if ex.menu_hidden && child_bool(node, "menuHidden") == Some(true) {
        return Some(SkipReason::MenuHidden);
    }
    if ex.destroy_on_drop && child_bool(node, "destroyOnDrop") == Some(true) {
        return Some(SkipReason::DestroyOnDrop);
    }
    if let Some(s) = ex
        .def_name_suffixes
        .iter()
        .find(|s| !s.is_empty() && def.def_name.ends_with(s.as_str()))
    {
        return Some(SkipReason::NameSuffix(s.clone()));
    }
    for tag in tags {
        let lower = tag.to_lowercase();
        if ex
            .tag_parts
            .iter()
            .any(|p| !p.is_empty() && lower.contains(&p.to_lowercase()))
        {
            return Some(SkipReason::Tag(tag.clone()));
        }
    }
    if let Some(class) = verb_class
        && ex
            .verb_class_parts
            .iter()
            .any(|p| !p.is_empty() && class.contains(p.as_str()))
    {
        return Some(SkipReason::VerbClass(class.to_owned()));
    }
    (ex.explosive && explosive).then_some(SkipReason::Explosive)
}

fn tool_row(t: &Node) -> CeToolRow {
    CeToolRow {
        label: child_text(t, "label").unwrap_or_default(),
        capacities: list_texts(t, "capacities"),
        power: child_number(t, "power"),
        cooldown: child_number(t, "cooldownTime"),
        ap_sharp: child_number(t, "armorPenetrationSharp"),
        ap_blunt: child_number(t, "armorPenetrationBlunt"),
        chance_factor: child_number(t, "chanceFactor"),
        linked_body_parts_group: child_text(t, "linkedBodyPartsGroup"),
    }
}

/// True when the def can be a weapon: the rule of the reference pools ([`is_weapon_def`]), so a weapon
/// without weapon tags that is primary equipment under a weapon category counts, and a creature or a
/// building never does.
#[must_use]
pub fn is_item(node: &Node) -> bool {
    is_weapon_def(node)
}

/// Reads a converted melee weapon. `None` for a creature or a building that carries converted tools.
#[must_use]
pub fn read_melee(def: &DefRecord, exclusions: &Exclusions) -> Option<CeMelee> {
    let node = &def.node;
    if !is_item(node) {
        return None;
    }
    let stat = number_map(node, "statBases");
    let offsets = number_map(node, "equippedStatOffsets");
    let tools: Vec<CeToolRow> = list_items(node, "tools")
        .into_iter()
        .map(tool_row)
        .collect();
    let mut stats = BTreeMap::new();
    put(&mut stats, "mass", stat.get("Mass").copied());
    put(&mut stats, "bulk", stat.get("Bulk").copied());
    put(
        &mut stats,
        "counter_parry",
        stat.get("MeleeCounterParryBonus").copied(),
    );
    put(&mut stats, "crit", offsets.get("MeleeCritChance").copied());
    put(
        &mut stats,
        "parry",
        offsets.get("MeleeParryChance").copied(),
    );
    put(
        &mut stats,
        "dodge",
        offsets.get("MeleeDodgeChance").copied(),
    );
    let powers: Vec<f64> = tools.iter().filter_map(|t| t.power).collect();
    let cooldowns: Vec<f64> = tools.iter().filter_map(|t| t.cooldown).collect();
    put(&mut stats, "tool_power", mean(&powers));
    put(&mut stats, "tool_cooldown", mean(&cooldowns));
    let ratio = |pick: fn(&CeToolRow) -> Option<f64>| -> Option<f64> {
        let ratios: Vec<f64> = tools
            .iter()
            .filter_map(|t| {
                let p = t.power.filter(|p| *p > 0.0)?;
                Some(pick(t)? / p)
            })
            .collect();
        median(&ratios)
    };
    put(&mut stats, "ap_sharp_ratio", ratio(|t| t.ap_sharp));
    put(&mut stats, "ap_blunt_ratio", ratio(|t| t.ap_blunt));
    let weapon_tags = list_texts(node, "weaponTags");
    let excluded = exclusion_of(def, exclusions, None, false, &weapon_tags);
    Some(CeMelee {
        def_name: def.def_name.clone(),
        label: child_text(node, "label").unwrap_or_else(|| def.def_name.clone()),
        mod_idx: def.mod_idx(),
        tech_level: child_text(node, "techLevel").and_then(|t| tech_level_named(&t)),
        weapon_tags,
        tools,
        stats,
        twin: None,
        twin_tags: Vec::new(),
        excluded,
    })
}

/// The twin numbers of a vanilla weapon, with the names of [`GUN_STATS`] or [`MELEE_STATS`] that exist on
/// both sides.
#[must_use]
pub fn twin_stats(vanilla: &WeaponDetail) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    for (name, key) in [
        ("mass", "mass"),
        ("range", "range"),
        ("warmup", "warmup"),
        ("cooldown", "cooldown"),
        ("burst", "burst"),
        ("damage", "damage"),
        ("speed", "speed"),
    ] {
        if vanilla.ranged.is_some()
            && let Some(v) = vanilla.stats.get(key)
        {
            out.insert(name.to_owned(), *v);
        }
    }
    if vanilla.ranged.is_none() {
        put(&mut out, "mass", vanilla.stats.get("mass").copied());
        let powers: Vec<f64> = vanilla.tools.iter().filter_map(|t| t.power).collect();
        let cooldowns: Vec<f64> = vanilla.tools.iter().filter_map(|t| t.cooldown).collect();
        put(&mut out, "tool_power", mean(&powers));
        put(&mut out, "tool_cooldown", mean(&cooldowns));
    }
    out
}

/// True when the markers say the def is a converted gun (a converted verb or an ammo component).
#[must_use]
pub fn is_gun(markers: &CeMarkers) -> bool {
    markers.verb || markers.ammo_comp
}

/// Reads the Combat Extended block of an already converted def: the values update mode starts from. The
/// source tag of every number is `source`. `None` when the def carries no conversion marker.
#[must_use]
pub fn ce_block_from_def(
    def: &DefRecord,
    classes: &CeClassNames,
    source: ValueSource,
) -> Option<CePatchSpec> {
    let node = &def.node;
    let markers = detect_markers(node, classes);
    if !markers.is_conversion() {
        return None;
    }
    let stat = number_map(node, "statBases");
    let offsets = number_map(node, "equippedStatOffsets");
    let verb = find_class(node, "verbs", &classes.verb_properties);
    let ammo = find_class(node, "comps", &classes.ammo_user);
    let tags = list_texts(node, "weaponTags");
    let s = |v: Option<f64>| v.map(|v| Sourced::new(v, source));
    let mut block = CePatchSpec {
        ammo_set: ammo.and_then(|a| child_text(a, "ammoSet")),
        default_projectile: verb.and_then(|v| child_text(v, "defaultProjectile")),
        weapon_tag_class: tags
            .iter()
            .find(|t| t.starts_with(&classes.ai_tag_prefix))
            .or_else(|| {
                // A melee conversion has no AI class; its class tag is the first Combat Extended tag that
                // does not say one handed.
                (!is_gun(&markers)).then(|| {
                    tags.iter().find(|t| {
                        t.starts_with(&classes.tag_prefix)
                            && !t.to_lowercase().contains("onehanded")
                    })
                })?
            })
            .cloned(),
        bulk: s(stat.get("Bulk").copied()),
        sway_factor: s(stat.get("SwayFactor").copied()),
        shot_spread: s(stat.get("ShotSpread").copied()),
        sights_efficiency: s(stat.get("SightsEfficiency").copied()),
        cooldown: s(stat.get("RangedWeapon_Cooldown").copied()),
        recoil_amount: s(verb.and_then(|v| child_number(v, "recoilAmount"))),
        reload_time: s(ammo.and_then(|a| child_number(a, "reloadTime"))),
        parry_bonus: s(stat.get("MeleeCounterParryBonus").copied()),
        melee_crit_chance: s(offsets.get("MeleeCritChance").copied()),
        melee_parry_chance: s(offsets.get("MeleeParryChance").copied()),
        melee_dodge_chance: s(offsets.get("MeleeDodgeChance").copied()),
        one_handed: tags.iter().any(|t| t.to_lowercase().contains("onehanded")),
        ..CePatchSpec::default()
    };
    block.magazine_size = ammo
        .and_then(|a| child_number(a, "magazineSize"))
        .filter(|m| *m >= 0.0 && *m <= f64::from(u32::MAX))
        .map(|m| Sourced::new(m.round() as u32, source));
    block.tool_penetration = list_items(node, "tools")
        .into_iter()
        .map(tool_row)
        .map(|t| CeToolPenetration {
            tool: t.label,
            sharp: s(t.ap_sharp),
            blunt: s(t.ap_blunt),
        })
        .collect();
    Some(block)
}
