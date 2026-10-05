//! Reading the auto-patcher presets from the user's Combat Extended defs.
//!
//! When the install carries the gun and apparel preset defs, their fields become [`GunPreset`] and
//! [`ApparelPreset`] values (the data the formulas of [`crate::ce::formulas`] work on). Nothing is bundled:
//! without the defs the lists are empty.

use rimstudio_core::tree::Node;
use rimstudio_defs::DefDatabases;

use crate::ce::formulas::{ApparelPreset, CaliberRange, Curve, FloatRange, GunPreset, SpecialGun};
use crate::reader::access::{
    child_bool, child_number, child_text, list_items, list_texts, number_map, parse_number,
};

fn range_of(node: &Node, tag: &str) -> Option<FloatRange> {
    let text = child_text(node, tag)?;
    let (a, b) = text.split_once('~')?;
    Some(FloatRange::new(parse_number(a)?, parse_number(b)?))
}

fn curve_of(node: &Node, tag: &str) -> Option<Curve> {
    let points = node.child(tag)?.child("points")?;
    let mut out = Vec::new();
    for li in points.children_named("li") {
        let text = li.text_content();
        let (x, y) = text.split_once(',')?;
        out.push((parse_number(x)?, parse_number(y)?));
    }
    Curve::new(out).ok()
}

fn special_gun(li: &Node) -> SpecialGun {
    SpecialGun {
        names: list_texts(li, "names"),
        caliber: child_text(li, "caliber"),
        mag_cap: child_number(li, "magCap"),
        reload_time: child_number(li, "reloadTime"),
        mass: child_number(li, "mass"),
        bulk: child_number(li, "bulk"),
        stats: number_map(li, "stats"),
    }
}

fn caliber_range(li: &Node) -> Option<CaliberRange> {
    Some(CaliberRange {
        damage_range: range_of(li, "DamageRange")?,
        speed_range: range_of(li, "SpeedRange")?,
        ammo_set: child_text(li, "AmmoSet")?,
    })
}

/// Parses one gun preset def node.
#[must_use]
pub fn gun_preset(def_name: &str, node: &Node) -> GunPreset {
    let zero = FloatRange::new(0.0, 0.0);
    let stats = number_map(node, "gunStats");
    let misc = number_map(node, "MiscOtherStats");
    GunPreset {
        def_name: def_name.to_owned(),
        names: list_texts(node, "names"),
        tags: list_texts(node, "tags"),
        discard_designations: child_bool(node, "DiscardDesignations").unwrap_or(false),
        special_guns: list_items(node, "specialGuns")
            .into_iter()
            .map(special_gun)
            .collect(),
        warmup_range: range_of(node, "WarmupRange").unwrap_or(zero),
        range_range: range_of(node, "RangeRange").unwrap_or(zero),
        damage_range: range_of(node, "damageRange").unwrap_or(zero),
        proj_speed_range: range_of(node, "projSpeedRange").unwrap_or(zero),
        caliber_ranges: list_items(node, "CaliberRanges")
            .into_iter()
            .filter_map(caliber_range)
            .collect(),
        determine_caliber: child_bool(node, "DetermineCaliber").unwrap_or(false),
        set_caliber: child_text(node, "setCaliber").unwrap_or_default(),
        range_curve: curve_of(node, "rangeCurve"),
        warmup_curve: curve_of(node, "warmupCurve"),
        cooldown_curve: curve_of(node, "cooldownCurve"),
        mass_curve: curve_of(node, "MassCurve"),
        flat_range: stats.get("range").copied().unwrap_or(0.0),
        flat_warmup: stats.get("warmupTime").copied().unwrap_or(0.0),
        cooldown_time: child_number(node, "CooldownTime").unwrap_or(0.0),
        mass: child_number(node, "Mass").unwrap_or(0.0),
        bulk: child_number(node, "Bulk").unwrap_or(0.0),
        spread: child_number(node, "Spread").unwrap_or(0.0),
        sway: child_number(node, "Sway").unwrap_or(0.0),
        sights_efficiency: misc.get("SightsEfficiency").copied(),
        recoil_amount: stats.get("recoilAmount").copied(),
        burst_shot_count: stats.get("burstShotCount").copied().unwrap_or(1.0),
        ticks_between_burst_shots: stats.get("ticksBetweenBurstShots").copied(),
        ammo_capacity: child_number(node, "AmmoCapacity").unwrap_or(0.0),
        reload_time: child_number(node, "ReloadTime").unwrap_or(0.0),
    }
}

/// Parses one apparel preset def node.
#[must_use]
pub fn apparel_preset(def_name: &str, node: &Node) -> ApparelPreset {
    ApparelPreset {
        def_name: def_name.to_owned(),
        needed_layers: list_texts(node, "neededLayers"),
        needed_groups: list_texts(node, "neededGroups"),
        vanilla_armor_rating_range: range_of(node, "vanillaArmorRatingRange")
            .unwrap_or_else(|| FloatRange::new(0.0, 0.0)),
        bulk: child_number(node, "Bulk").unwrap_or(0.0),
        bulk_worn: child_number(node, "BulkWorn").unwrap_or(0.0),
        mass: child_number(node, "Mass"),
        sharp_curve: curve_of(node, "ArmorCurveSharp"),
        blunt_curve: curve_of(node, "ArmorCurveBlunt"),
        static_sharp: child_number(node, "ArmorStaticSharp").unwrap_or(0.0),
        static_blunt: child_number(node, "ArmorStaticBlunt").unwrap_or(0.0),
        has_partial_armor: node
            .child("partialStats")
            .is_some_and(|p| p.children_named("li").next().is_some()),
    }
}

/// Reads every gun preset def of the database `db_type`, in database (load) order. An unknown type gives an
/// empty list.
#[must_use]
pub fn read_gun_presets(dbs: &DefDatabases, db_type: &str) -> Vec<GunPreset> {
    dbs.database(db_type)
        .map(|view| {
            view.iter()
                .map(|d| gun_preset(&d.def_name, &d.node))
                .collect()
        })
        .unwrap_or_default()
}

/// Reads every apparel preset def of the database `db_type`, in load order.
#[must_use]
pub fn read_apparel_presets(dbs: &DefDatabases, db_type: &str) -> Vec<ApparelPreset> {
    dbs.database(db_type)
        .map(|view| {
            view.iter()
                .map(|d| apparel_preset(&d.def_name, &d.node))
                .collect()
        })
        .unwrap_or_default()
}
