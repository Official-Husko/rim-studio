//! Naming the archetype of an existing weapon from its tags, verbs and tools.
//!
//! The rules are data (`deriveRules` of the taxonomy): an ordered list of conditions on the weapon's
//! numbers read against the install's medians, so they hold on any install. The descriptors follow from the
//! numbers: the action from the burst and the tags, the calibre from the damage class (the real calibre of a
//! vanilla gun is unknown), the handling from the mass and the rate of fire from the cycle time. The
//! validation harness uses this to compare proposals with the real weapons, and the clone flow can use it
//! to offer the archetype of a weapon.

use std::collections::BTreeMap;

use crate::model::{ArchetypeMode, Descriptors, ItemKind, RateOfFire};
use crate::ranged::cycle_time;
use crate::reader::WeaponDetail;

use super::basis::{MeleeMedians, RangedMedians};
use super::data::{ArchetypeRef, Condition, Taxonomy};
use super::gun::{shape as gun_shape, shape_cycle};
use super::resolve::resolve;

/// The archetype of a weapon and the descriptors that describe it.
#[derive(Debug, Clone, PartialEq)]
pub struct Derived {
    /// The archetype id.
    pub archetype: String,
    /// The descriptors.
    pub descriptors: Descriptors,
}

fn holds(c: &Condition, stats: &BTreeMap<&str, f64>) -> bool {
    let Some(v) = stats.get(c.stat.as_str()) else {
        return false;
    };
    c.min.is_none_or(|m| *v >= m) && c.max.is_none_or(|m| *v <= m)
}

fn first_rule<'a>(
    rules: &'a [super::data::DeriveRule],
    stats: &BTreeMap<&str, f64>,
) -> Option<&'a str> {
    rules
        .iter()
        .find(|r| r.when.iter().all(|c| holds(c, stats)))
        .map(|r| r.archetype.as_str())
}

fn ratio(value: f64, median: f64) -> f64 {
    if median > 0.0 { value / median } else { 1.0 }
}

fn nearest<'a>(ids: impl Iterator<Item = (&'a String, f64)>, wanted_ln: f64) -> Option<String> {
    ids.min_by(|a, b| {
        (wanted_ln - a.1.ln())
            .abs()
            .total_cmp(&(wanted_ln - b.1.ln()).abs())
    })
    .map(|(id, _)| id.clone())
}

/// Derives the archetype of a gun or bow.
#[must_use]
pub fn derive_ranged(tax: &Taxonomy, w: &WeaponDetail, m: &RangedMedians) -> Option<Derived> {
    let r = w.ranged.as_ref()?;
    let profile = r.profile();
    let tier = w.tech_level.map_or(2.0, |t| f64::from(t.index()));
    let mut stats: BTreeMap<&str, f64> = BTreeMap::new();
    stats.insert("tier", tier);
    stats.insert("burst", f64::from(r.burst_count));
    stats.insert("damage_ratio", ratio(r.damage, m.damage.value));
    stats.insert("range_ratio", ratio(r.range, m.range.value));
    stats.insert("warmup_ratio", ratio(r.warmup, m.warmup.value));
    stats.insert("cooldown_ratio", ratio(r.cooldown, m.cooldown.value));
    let mass = w.stats.get("mass").copied().unwrap_or(m.mass.value);
    stats.insert("mass_ratio", ratio(mass, m.mass.value));
    let id = first_rule(&tax.derive_rules.ranged, &stats)?.to_owned();
    let arch = tax.find(&id)?;
    let a = arch.archetype;
    let shape = a.ranged.as_ref()?;

    // The action: from the tags, then from the burst and the warmup.
    let has_tag = |part: &str| {
        w.weapon_tags
            .iter()
            .any(|t| t.to_lowercase().contains(&part.to_lowercase()))
    };
    let allowed = |x: &str| a.actions.iter().any(|y| y == x);
    let counts = a.burst?;
    let action = if has_tag("pump") && allowed("pump") {
        "pump".to_owned()
    } else if has_tag("lever") && allowed("lever") {
        "lever".to_owned()
    } else if r.burst_count <= 1 {
        if allowed("draw") {
            "draw".to_owned()
        } else if allowed("bolt") && stats.get("warmup_ratio").copied().unwrap_or(1.0) >= 1.3 {
            "bolt".to_owned()
        } else if allowed("semi") {
            "semi".to_owned()
        } else {
            a.default_action.clone()?
        }
    } else if allowed("full-auto")
        && (!allowed("burst")
            || (f64::from(r.burst_count) - f64::from(counts.auto)).abs()
                < (f64::from(r.burst_count) - f64::from(counts.burst)).abs())
    {
        "full-auto".to_owned()
    } else if allowed("burst") {
        "burst".to_owned()
    } else {
        a.default_action.clone()?
    };

    let base = a.default_calibre.as_deref().and_then(|d| tax.calibre(d))?;
    let calibre = nearest(
        a.calibres
            .iter()
            .filter_map(|id| tax.calibre(id).map(|c| (id, c.damage / base.damage))),
        (r.damage / (m.damage.value * shape.damage)).ln(),
    )?;
    let cal_mass = tax.calibre(&calibre).map_or(1.0, |c| c.mass / base.mass);
    let handling = nearest(
        a.handlings
            .iter()
            .filter_map(|id| tax.handling(id).map(|h| (id, h.mass))),
        (mass / (m.mass.value * shape.mass * cal_mass)).ln(),
    )?;

    // The rate of fire: the class whose shape cycle is nearest to the real cycle.
    let real_cycle = cycle_time(&profile.cycle_input()).ok()?;
    let mut best: Option<(f64, String)> = None;
    for rof in &a.rof_classes {
        let d = Descriptors {
            action: Some(action.clone()),
            rof: Some(RateOfFire::Class(rof.clone())),
            calibre: Some(calibre.clone()),
            handling: Some(handling.clone()),
            ..Descriptors::default()
        };
        let resolved = resolve(tax, &arch, &d, ArchetypeMode::Vanilla, None).ok()?;
        let (s, _) = gun_shape(&arch, &resolved, m, tax).ok()?;
        let cycle = shape_cycle(&s, &tax.ladders.exponents).ok()?;
        let miss = (real_cycle / cycle).ln().abs();
        if best.as_ref().is_none_or(|(e, _)| miss < *e) {
            best = Some((miss, rof.clone()));
        }
    }
    Some(Derived {
        archetype: id,
        descriptors: Descriptors {
            action: Some(action),
            rof: best.map(|(_, id)| RateOfFire::Class(id)),
            calibre: Some(calibre),
            handling: Some(handling),
            tier: w.tech_level,
            ..Descriptors::default()
        },
    })
}

/// Derives the archetype of a melee weapon.
#[must_use]
pub fn derive_melee(tax: &Taxonomy, w: &WeaponDetail, m: &MeleeMedians) -> Option<Derived> {
    let caps = |name: &str| {
        w.tools
            .iter()
            .any(|t| t.capacities.iter().any(|c| c.eq_ignore_ascii_case(name)))
    };
    let only_blunt = w.tools.iter().all(|t| {
        t.capacities
            .iter()
            .all(|c| matches!(c.as_str(), "Blunt" | "Poke" | "Demolish"))
    });
    let mut stats: BTreeMap<&str, f64> = BTreeMap::new();
    stats.insert("tools", w.tools.len() as f64);
    stats.insert("has_cut", f64::from(u8::from(caps("Cut"))));
    stats.insert("has_stab", f64::from(u8::from(caps("Stab"))));
    stats.insert("only_blunt", f64::from(u8::from(only_blunt)));
    stats.insert("tier", w.tech_level.map_or(1.0, |t| f64::from(t.index())));
    let cooldown = w.stats.get("cooldown").copied().unwrap_or(m.cooldown.value);
    let mass = w.stats.get("mass").copied().unwrap_or(m.mass.value);
    stats.insert("cooldown_ratio", ratio(cooldown, m.cooldown.value));
    stats.insert("mass_ratio", ratio(mass, m.mass.value));
    let id = first_rule(&tax.derive_rules.melee, &stats)?.to_owned();
    let arch: ArchetypeRef<'_> = tax.find(&id)?;
    if arch.kind != ItemKind::Melee {
        return None;
    }
    let a = arch.archetype;
    let shape = a.melee.as_ref()?;
    let handling = nearest(
        a.handlings
            .iter()
            .filter_map(|id| tax.handling(id).map(|h| (id, h.mass))),
        (mass / (m.mass.value * shape.mass)).ln(),
    )?;
    // The swing speed: the class whose tempo is nearest to the real cooldown, read on the slowest tool.
    let slowest = shape
        .tools
        .iter()
        .map(|t| t.cooldown)
        .fold(0.0_f64, f64::max)
        .max(1e-9);
    let h = tax.handling(&handling)?;
    let expo = tax.ladders.exponents.rof_cooldown;
    let mut best: Option<(f64, String)> = None;
    for rof in &a.rof_classes {
        let rate = tax.rof_class(rof)?.rate;
        let predicted = m.cooldown.value * slowest * shape.cooldown * h.cooldown / rate.powf(expo);
        let miss = (cooldown / predicted).ln().abs();
        if best.as_ref().is_none_or(|(e, _)| miss < *e) {
            best = Some((miss, rof.clone()));
        }
    }
    Some(Derived {
        archetype: id,
        descriptors: Descriptors {
            rof: best.map(|(_, id)| RateOfFire::Class(id)),
            handling: Some(handling),
            tier: w.tech_level,
            ..Descriptors::default()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_condition_needs_its_stat() {
        let c = Condition {
            stat: "burst".into(),
            min: Some(2.0),
            max: None,
        };
        let mut s = BTreeMap::new();
        assert!(!holds(&c, &s));
        s.insert("burst", 3.0);
        assert!(holds(&c, &s));
        s.insert("burst", 1.0);
        assert!(!holds(&c, &s));
    }
}
