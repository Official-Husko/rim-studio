//! The solver for guns and bows.
//!
//! 1. The shape: every stat is the install's median times the ratios of the archetype, the action, the
//!    calibre, the handling and the rate of fire. The shape alone holds the character of the weapon (a
//!    sniper reaches far, aims long and hits hard; a submachine gun cycles fast and hits light).
//! 2. The scale: the strength index of the shape is compared with the strength the balance target asks for,
//!    and one scale factor `s` is found by bisection. Damage grows with `s` and the warmup and cooldown
//!    shrink with it, in fixed shares, so the weapon gets stronger by hitting harder and cycling a little
//!    faster, never by changing its range or accuracy profile.
//! 3. The rounding: values are rounded the way the install writes them, and damage is polished by one point
//!    when that lands the strength closer to the target.

use crate::error::{DesignError, DesignResult};
use crate::model::ScalarField;
use crate::ranged::{
    AccuracyBands, DEFAULT_AP_PER_DAMAGE, RangedProfile, ReferenceArmor, cycle_time, strength_p4,
};

use super::basis::{Basis, Med, RangedMedians, round_significant, round_to};
use super::data::{ActionMode, ArchetypeRef, Exponents, RangedShape, Taxonomy};
use super::proposal::{FactorTerm, ProposedValue};
use super::resolve::Resolved;

/// The bounds of the strength scale (natural logarithm).
const LN_SCALE_LIMIT: f64 = 3.0;
/// A penetration within this share of the implied one is left implied.
const AP_IMPLIED_TOLERANCE: f64 = 0.02;

/// The unscaled shape of a gun.
#[derive(Debug, Clone)]
pub struct GunShape {
    /// Damage before the strength scale.
    pub damage: f64,
    /// Range.
    pub range: f64,
    /// Warmup before the strength scale.
    pub warmup: f64,
    /// Cooldown before the strength scale.
    pub cooldown: f64,
    /// Shots per burst.
    pub burst: u32,
    /// Ticks between burst shots.
    pub ticks: f64,
    /// Accuracy at touch, short, medium and long range.
    pub accuracy: [f64; 4],
    /// Armor penetration against the implied one.
    pub ap_factor: f64,
    /// Mass before the strength scale.
    pub mass: f64,
    /// Work to make before the strength scale.
    pub work: f64,
}

/// The ratios that went into the shape, kept for the explanations.
#[derive(Debug, Clone)]
pub struct Terms {
    arch: String,
    action: Option<(String, f64, f64, f64)>,
    calibre: (String, f64, f64, f64, f64),
    handling: (String, f64, f64, f64, f64, f64),
    rof: (String, f64),
}

fn burst_of(base: u32, rate: f64, exps: &Exponents, max: u32) -> u32 {
    if base <= 1 {
        return 1;
    }
    let scaled = (f64::from(base) * rate.powf(exps.rof_burst)).round();
    (scaled.max(2.0) as u32).min(max.max(2))
}

/// Builds the unscaled shape.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] for an archetype without a ranged shape.
pub fn shape(
    arch: &ArchetypeRef<'_>,
    resolved: &Resolved<'_>,
    m: &RangedMedians,
    tax: &Taxonomy,
) -> DesignResult<(GunShape, Terms)> {
    let a = arch.archetype;
    let s = a
        .ranged
        .as_ref()
        .ok_or_else(|| DesignError::invalid("archetype", "not a ranged archetype"))?;
    let exps = &tax.ladders.exponents;
    let c = &resolved.calibre;
    let h = resolved.handling;
    let (act_warm, act_cool, act_ticks, mode) = resolved
        .action
        .map_or((1.0, 1.0, 1.0, ActionMode::Single), |x| {
            (x.warmup, x.cooldown, x.ticks, x.mode)
        });
    let counts = a
        .burst
        .unwrap_or(super::data::BurstCounts { burst: 1, auto: 1 });
    let base_burst = match mode {
        ActionMode::Single => 1,
        ActionMode::Burst => counts.burst,
        ActionMode::Auto => counts.auto,
    };
    let burst = burst_of(
        base_burst,
        resolved.rate,
        exps,
        base_burst.saturating_mul(3),
    );
    let rate = resolved.rate;
    let shape = GunShape {
        damage: m.damage.value * s.damage * c.damage,
        range: m.range.value * s.range * c.range * h.range,
        warmup: m.warmup.value * s.warmup * act_warm * h.warmup,
        cooldown: m.cooldown.value * s.cooldown * act_cool * c.cooldown * h.cooldown
            / rate.powf(exps.rof_cooldown),
        burst,
        ticks: (m.ticks.value * s.ticks * act_ticks / rate.powf(exps.rof_ticks))
            .round()
            .max(2.0),
        accuracy: [
            m.accuracy[0].value * s.accuracy[0] * h.touch,
            m.accuracy[1].value * s.accuracy[1],
            m.accuracy[2].value * s.accuracy[2],
            m.accuracy[3].value * s.accuracy[3] * h.long,
        ]
        .map(|v| v.clamp(0.05, 0.98)),
        ap_factor: s.ap * c.ap,
        mass: m.mass.value * s.mass * c.mass * h.mass,
        work: m.work.value * s.work,
    };
    let terms = Terms {
        arch: a.label.clone(),
        action: resolved
            .action
            .map(|x| (x.label.clone(), act_warm, act_cool, act_ticks)),
        calibre: (c.label.clone(), c.damage, c.range, c.mass, c.cooldown),
        handling: (
            format!("{} handling", resolved.handling.label.to_lowercase()),
            h.mass,
            h.warmup,
            h.range,
            h.touch,
            h.long,
        ),
        rof: (format!("{} rate of fire", resolved.rof_id), rate),
    };
    Ok((shape, terms))
}

/// The profile of the shape at strength scale `s`, before rounding.
#[must_use]
pub fn profile_at(shape: &GunShape, exps: &Exponents, s: f64) -> RangedProfile {
    let damage = shape.damage * s.powf(exps.damage_scale);
    RangedProfile {
        damage,
        armor_penetration: shape.ap_factor * DEFAULT_AP_PER_DAMAGE * damage,
        warmup: shape.warmup * s.powf(-exps.warmup_scale),
        cooldown: shape.cooldown * s.powf(-exps.cooldown_scale),
        burst_count: shape.burst,
        ticks_between_shots: shape.ticks,
        range: shape.range,
        accuracy: Some(AccuracyBands {
            touch: shape.accuracy[0],
            short: shape.accuracy[1],
            medium: shape.accuracy[2],
            long: shape.accuracy[3],
        }),
    }
}

/// The strength index of a profile against the reference armor.
///
/// # Errors
///
/// [`DesignError`] from the index (a degenerate cycle) or when there are no reference armor layers.
pub fn strength_of(profile: &RangedProfile, armor: &[f64]) -> DesignResult<f64> {
    if armor.is_empty() {
        return Err(DesignError::EmptyInput {
            what: "reference armor layers",
        });
    }
    strength_p4(
        profile,
        &ReferenceArmor {
            sharp_ratings: armor,
        },
    )
}

/// Finds the scale at which an increasing function reaches the target: `(scale, clamped)`.
///
/// The function is evaluated at the bounds first; a target outside them returns the nearest bound with
/// `clamped` set.
pub fn solve_scale(f: impl Fn(f64) -> DesignResult<f64>, target: f64) -> DesignResult<(f64, bool)> {
    let lo = f((-LN_SCALE_LIMIT).exp())?;
    let hi = f(LN_SCALE_LIMIT.exp())?;
    if target <= lo {
        return Ok(((-LN_SCALE_LIMIT).exp(), target < lo * 0.999));
    }
    if target >= hi {
        return Ok((LN_SCALE_LIMIT.exp(), target > hi * 1.001));
    }
    let (mut a, mut b) = (-LN_SCALE_LIMIT, LN_SCALE_LIMIT);
    for _ in 0..60 {
        let mid = 0.5 * (a + b);
        if f(mid.exp())? < target {
            a = mid;
        } else {
            b = mid;
        }
    }
    Ok((((a + b) * 0.5).exp(), false))
}

/// The rounded numbers of a solved gun.
#[derive(Debug, Clone)]
pub struct GunNumbers {
    /// The rounded profile.
    pub profile: RangedProfile,
    /// True when the penetration is written explicitly.
    pub explicit_ap: bool,
    /// The mass.
    pub mass: f64,
    /// The work to make.
    pub work: f64,
    /// The scale that was applied.
    pub scale: f64,
    /// True when the strength target was out of reach.
    pub clamped: bool,
    /// The strength of the rounded profile.
    pub achieved: f64,
}

fn round_profile(
    shape: &GunShape,
    exps: &Exponents,
    s: f64,
    damage: f64,
    range_fraction: Option<f64>,
) -> (RangedProfile, bool) {
    let raw = profile_at(shape, exps, s);
    let explicit = (shape.ap_factor - 1.0).abs() > AP_IMPLIED_TOLERANCE;
    let ap = if explicit {
        round_to(shape.ap_factor * DEFAULT_AP_PER_DAMAGE * damage, 0.005)
    } else {
        DEFAULT_AP_PER_DAMAGE * damage
    };
    let range = match range_fraction {
        Some(f) => (raw.range - f).round() + f,
        None => round_to(raw.range, 0.5),
    };
    (
        RangedProfile {
            damage,
            armor_penetration: ap,
            warmup: round_to(raw.warmup, 0.05).max(0.05),
            cooldown: round_to(raw.cooldown, 0.05).max(0.05),
            burst_count: raw.burst_count,
            ticks_between_shots: raw.ticks_between_shots,
            range: range.max(1.0),
            accuracy: raw.accuracy.map(|a| AccuracyBands {
                touch: round_to(a.touch, 0.01),
                short: round_to(a.short, 0.01),
                medium: round_to(a.medium, 0.01),
                long: round_to(a.long, 0.01),
            }),
        },
        explicit,
    )
}

/// Solves the gun: finds the scale, rounds, polishes the damage.
///
/// # Errors
///
/// [`DesignError`] from the strength index.
pub fn solve(
    shape: &GunShape,
    exps: &Exponents,
    basis: &Basis<'_>,
    target: f64,
    m: &RangedMedians,
    range_fraction: Option<f64>,
    strength_work_ratio: f64,
) -> DesignResult<GunNumbers> {
    let (scale, clamped) = solve_scale(
        |s| strength_of(&profile_at(shape, exps, s), basis.armor),
        target,
    )?;
    let raw_damage = shape.damage * scale.powf(exps.damage_scale);
    let mut best: Option<(f64, RangedProfile, bool, f64)> = None;
    for candidate in [raw_damage.floor(), raw_damage.ceil()] {
        let damage = candidate.max(1.0);
        let (profile, explicit) = round_profile(shape, exps, scale, damage, range_fraction);
        let strength = strength_of(&profile, basis.armor)?;
        let better = best
            .as_ref()
            .is_none_or(|(e, ..)| (strength / target).ln().abs() < *e);
        if better {
            best = Some(((strength / target).ln().abs(), profile, explicit, strength));
        }
    }
    let (_, profile, explicit, achieved) = best.ok_or(DesignError::Degenerate {
        what: "no damage candidate",
    })?;
    let mass_raw = shape.mass * scale.powf(exps.mass_coupling);
    let mass = if mass_raw < 2.0 {
        round_to(mass_raw, 0.05)
    } else if mass_raw < 10.0 {
        round_to(mass_raw, 0.1)
    } else {
        round_to(mass_raw, 0.5)
    };
    let work = round_significant(shape.work * strength_work_ratio.powf(exps.work_coupling), 2);
    let _ = m;
    Ok(GunNumbers {
        profile,
        explicit_ap: explicit,
        mass: mass.max(0.05),
        work: work.max(1.0),
        scale,
        clamped,
        achieved,
    })
}

fn reason(stat: &str, value: f64, med: Option<Med>, terms: &[FactorTerm]) -> String {
    let mut text = format!("{stat} {}", crate::model::format_number(value));
    match med {
        Some(m) => text.push_str(&format!(
            ": the install's median is {} ({} reference weapons)",
            crate::model::format_number((m.value * 100.0).round() / 100.0),
            m.n
        )),
        None => text.push(':'),
    }
    let parts: Vec<String> = terms
        .iter()
        .filter(|t| (t.factor.ln()).abs() > 0.005)
        .map(|t| format!("{} x{:.2}", t.label, t.factor))
        .collect();
    if !parts.is_empty() {
        text.push_str(if med.is_some() { ". " } else { " " });
        text.push_str(&parts.join(", "));
    }
    text.push('.');
    text
}

/// The proposed values of a solved gun, with their reasons, in display order.
#[must_use]
pub fn values(
    numbers: &GunNumbers,
    shape_ratios: &RangedShape,
    terms: &Terms,
    m: &RangedMedians,
    exps: &Exponents,
) -> Vec<ProposedValue> {
    let s = numbers.scale;
    let p = &numbers.profile;
    let scale_term = |label: &str, e: f64| FactorTerm::new(label, s.powf(e));
    let arch = |f: f64| FactorTerm::new(terms.arch.clone(), f);
    let action_term = |pick: fn(&(String, f64, f64, f64)) -> f64| {
        terms
            .action
            .as_ref()
            .map(|a| FactorTerm::new(a.0.clone(), pick(a)))
    };
    let (cal, cal_damage, cal_range, cal_mass, cal_cool) = (
        terms.calibre.0.clone(),
        terms.calibre.1,
        terms.calibre.2,
        terms.calibre.3,
        terms.calibre.4,
    );
    let (hl, h_mass, h_warm, h_range, h_touch, h_long) = (
        terms.handling.0.clone(),
        terms.handling.1,
        terms.handling.2,
        terms.handling.3,
        terms.handling.4,
        terms.handling.5,
    );
    let mut out = Vec::new();
    let mut push = |field: ScalarField,
                    stat: &str,
                    label: &str,
                    value: f64,
                    write: bool,
                    med: Option<Med>,
                    t: Vec<FactorTerm>| {
        out.push(ProposedValue {
            field,
            stat: stat.to_owned(),
            value,
            write,
            reason: reason(label, value, med, &t),
            median: med.map(|x| x.value),
            median_n: med.map(|x| x.n),
            terms: t,
        });
    };
    push(
        ScalarField::Damage,
        "damage",
        "damage",
        p.damage,
        true,
        Some(m.damage),
        vec![
            arch(shape_ratios.damage),
            FactorTerm::new(cal.clone(), cal_damage),
            scale_term("strength scale", exps.damage_scale),
        ],
    );
    push(
        ScalarField::ArmorPenetration,
        "ap",
        "armor penetration",
        p.armor_penetration,
        numbers.explicit_ap,
        None,
        vec![FactorTerm::new(
            format!("{} against the implied 0.015 per damage point", terms.arch),
            if p.damage > 0.0 {
                p.armor_penetration / (DEFAULT_AP_PER_DAMAGE * p.damage)
            } else {
                1.0
            },
        )],
    );
    push(
        ScalarField::Range,
        "range",
        "range",
        p.range,
        true,
        Some(m.range),
        vec![
            arch(shape_ratios.range),
            FactorTerm::new(cal.clone(), cal_range),
            FactorTerm::new(hl.clone(), h_range),
        ],
    );
    push(
        ScalarField::BurstCount,
        "burst",
        "burst count",
        f64::from(p.burst_count),
        p.burst_count > 1,
        None,
        vec![terms.rof_term()],
    );
    push(
        ScalarField::TicksBetweenBurstShots,
        "ticks_between",
        "ticks between burst shots",
        p.ticks_between_shots,
        p.burst_count > 1,
        Some(m.ticks),
        [Some(arch(shape_ratios.ticks)), action_term(|a| a.3)]
            .into_iter()
            .flatten()
            .collect(),
    );
    push(
        ScalarField::Warmup,
        "warmup",
        "warmup",
        p.warmup,
        true,
        Some(m.warmup),
        [
            Some(arch(shape_ratios.warmup)),
            action_term(|a| a.1),
            Some(FactorTerm::new(hl.clone(), h_warm)),
            Some(scale_term("strength scale", -exps.warmup_scale)),
        ]
        .into_iter()
        .flatten()
        .collect(),
    );
    push(
        ScalarField::Cooldown,
        "cooldown",
        "cooldown",
        p.cooldown,
        true,
        Some(m.cooldown),
        [
            Some(arch(shape_ratios.cooldown)),
            action_term(|a| a.2),
            Some(FactorTerm::new(cal.clone(), cal_cool)),
            Some(terms.rof_term_cooldown(exps)),
            Some(scale_term("strength scale", -exps.cooldown_scale)),
        ]
        .into_iter()
        .flatten()
        .collect(),
    );
    if let Some(a) = p.accuracy {
        for (i, (field, stat, label, value)) in [
            (
                ScalarField::AccuracyTouch,
                "touch",
                "touch accuracy",
                a.touch,
            ),
            (
                ScalarField::AccuracyShort,
                "short",
                "short accuracy",
                a.short,
            ),
            (
                ScalarField::AccuracyMedium,
                "medium",
                "medium accuracy",
                a.medium,
            ),
            (ScalarField::AccuracyLong, "long", "long accuracy", a.long),
        ]
        .into_iter()
        .enumerate()
        {
            let handling = match i {
                0 => Some(FactorTerm::new(hl.clone(), h_touch)),
                3 => Some(FactorTerm::new(hl.clone(), h_long)),
                _ => None,
            };
            push(
                field,
                stat,
                label,
                value,
                true,
                Some(m.accuracy[i]),
                [Some(arch(shape_ratios.accuracy[i])), handling]
                    .into_iter()
                    .flatten()
                    .collect(),
            );
        }
    }
    push(
        ScalarField::Mass,
        "mass",
        "mass",
        numbers.mass,
        true,
        Some(m.mass),
        vec![
            arch(shape_ratios.mass),
            FactorTerm::new(cal, cal_mass),
            FactorTerm::new(hl, h_mass),
            scale_term("strength scale", exps.mass_coupling),
        ],
    );
    push(
        ScalarField::WorkToMake,
        "work",
        "work to make",
        numbers.work,
        true,
        Some(m.work),
        vec![arch(shape_ratios.work)],
    );
    let action = terms
        .action
        .as_ref()
        .map_or("this action", |a| a.0.as_str())
        .to_lowercase();
    for v in &mut out {
        match v.field {
            ScalarField::BurstCount if p.burst_count > 1 => {
                v.reason = format!(
                    "burst count {}: {action} fires {} shots per cycle, scaled by the {} (x{:.2}).",
                    p.burst_count, p.burst_count, terms.rof.0, terms.rof.1
                );
            }
            ScalarField::BurstCount => {
                v.reason = format!(
                    "burst count 1: {action} fires one shot per cycle, so no burst is written."
                );
            }
            ScalarField::ArmorPenetration if !numbers.explicit_ap => {
                v.reason = format!(
                    "armor penetration {}: left to the game, which implies 0.015 per damage point.",
                    crate::model::format_number(v.value)
                );
            }
            ScalarField::TicksBetweenBurstShots if p.burst_count <= 1 => {
                v.reason = "ticks between burst shots: a single shot weapon has no burst, so none is written.".to_owned();
            }
            _ => {}
        }
    }
    out
}

impl Terms {
    fn rof_term(&self) -> FactorTerm {
        FactorTerm::new(self.rof.0.clone(), self.rof.1)
    }

    fn rof_term_cooldown(&self, exps: &Exponents) -> FactorTerm {
        FactorTerm::new(self.rof.0.clone(), self.rof.1.powf(-exps.rof_cooldown))
    }
}

/// The cycle time of the shape at scale 1, for deriving a rate of fire class from a real weapon.
///
/// # Errors
///
/// [`DesignError`] from the cycle formula.
pub fn shape_cycle(shape: &GunShape, exps: &Exponents) -> DesignResult<f64> {
    cycle_time(&profile_at(shape, exps, 1.0).cycle_input())
}
