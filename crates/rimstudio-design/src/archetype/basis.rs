//! What the solver reads from the install: the medians of the pool, the strength to aim at, the role
//! that matches an archetype and the conventions the install writes its numbers with.
//!
//! Everything here is computed at run time from the reference pool and the reference weapons of the user's
//! install; nothing is stored. With few members the calibration fallback chain applies and the proposal says so.

use std::collections::BTreeMap;

use crate::baseline::{BaselineInput, Model, StrengthInput};
use crate::classes::numeric::{log_linear_fit, mean, median};
use crate::classes::{ChainLevel, ClassKey, ItemKind as PoolKind, Pool, PoolItem};
use crate::error::{DesignError, DesignResult};
use crate::melee::{MeleeModifiers, enumerate_attacks, strength as melee_strength};
use crate::model::ItemKind;
use crate::reader::WeaponDetail;

use super::costs::CostBasis;

/// A median with the number of reference weapons behind it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Med {
    /// The median.
    pub value: f64,
    /// The number of reference weapons that carry the stat.
    pub n: usize,
}

/// Everything the solver reads from the install.
#[derive(Debug, Clone, Copy)]
pub struct Basis<'a> {
    /// The fitted baseline model of the pool of the kind (its pool is the reference pool).
    pub model: &'a Model,
    /// The reference weapons of the install, for tools, tags and classes (may be empty).
    pub weapons: &'a [WeaponDetail],
    /// The sharp ratings of the reference armor layers of the ranged strength index.
    pub armor: &'a [f64],
    /// The cost lists and unit prices of the reference weapons (may be absent).
    pub costs: Option<&'a CostBasis>,
}

impl Basis<'_> {
    /// The reference pool.
    #[must_use]
    pub fn pool(&self) -> &Pool {
        &self.model.pool
    }
}

/// The pool kind of an item kind.
#[must_use]
pub fn pool_kind(kind: ItemKind) -> PoolKind {
    match kind {
        ItemKind::Melee => PoolKind::Melee,
        _ => PoolKind::Ranged,
    }
}

/// The item kind of a pool kind.
#[must_use]
pub fn item_kind(kind: PoolKind) -> ItemKind {
    match kind {
        PoolKind::Melee => ItemKind::Melee,
        _ => ItemKind::Ranged,
    }
}

/// The median of a stat over the pool items that carry it and pass `keep`.
#[must_use]
pub fn median_where(pool: &Pool, stat: &str, keep: impl Fn(&PoolItem) -> bool) -> Option<Med> {
    let values: Vec<f64> = pool
        .items
        .iter()
        .filter(|i| keep(i))
        .filter_map(|i| i.stat(stat))
        .collect();
    median(&values).map(|value| Med {
        value,
        n: values.len(),
    })
}

/// The medians a ranged shape is a ratio of.
#[derive(Debug, Clone, PartialEq)]
pub struct RangedMedians {
    /// Median damage.
    pub damage: Med,
    /// Median range.
    pub range: Med,
    /// Median warmup.
    pub warmup: Med,
    /// Median cooldown.
    pub cooldown: Med,
    /// Median mass.
    pub mass: Med,
    /// Median work to make.
    pub work: Med,
    /// Median ticks between burst shots of the burst weapons.
    pub ticks: Med,
    /// Median accuracy at touch, short, medium and long range.
    pub accuracy: [Med; 4],
}

/// The medians a melee shape is a ratio of.
#[derive(Debug, Clone, PartialEq)]
pub struct MeleeMedians {
    /// Median swing damage of the stat panel.
    pub swing_damage: Med,
    /// Median cooldown of the stat panel.
    pub cooldown: Med,
    /// Median mass.
    pub mass: Med,
    /// Median work to make.
    pub work: Med,
}

/// The median work to make over the weapons that are made at a bench: a work of 1 or less marks a weapon the
/// game prices explicitly and nobody crafts (relic and bladelink weapons), which would drag the median down.
fn need_work(pool: &Pool) -> DesignResult<Med> {
    median_where(pool, "work", |i| i.stat("work").is_some_and(|w| w > 1.0))
        .or_else(|| median_where(pool, "work", |_| true))
        .ok_or(DesignError::EmptyInput {
            what: "reference weapons with a work to make",
        })
}

fn need(pool: &Pool, stat: &'static str) -> DesignResult<Med> {
    median_where(pool, stat, |_| true).ok_or(DesignError::EmptyInput {
        what: "reference weapons with this stat",
    })
}

impl RangedMedians {
    /// Reads the medians of the ranged pool.
    ///
    /// # Errors
    ///
    /// [`DesignError::EmptyInput`] when no reference weapon carries one of the stats. The burst weapons'
    /// ticks fall back to the median of all weapons, and the work to the mass based guess of the caller.
    pub fn read(pool: &Pool) -> DesignResult<Self> {
        let burst = |i: &PoolItem| i.stat("burst").is_some_and(|b| b > 1.5);
        let ticks = median_where(pool, "ticks_between", burst)
            .or_else(|| median_where(pool, "ticks_between", |_| true))
            .ok_or(DesignError::EmptyInput {
                what: "reference weapons with ticks between shots",
            })?;
        Ok(Self {
            damage: need(pool, "damage")?,
            range: need(pool, "range")?,
            warmup: need(pool, "warmup")?,
            cooldown: need(pool, "cooldown")?,
            mass: need(pool, "mass")?,
            work: need_work(pool)?,
            ticks,
            accuracy: [
                need(pool, "touch")?,
                need(pool, "short")?,
                need(pool, "medium")?,
                need(pool, "long")?,
            ],
        })
    }
}

impl MeleeMedians {
    /// Reads the medians of the melee pool.
    ///
    /// # Errors
    ///
    /// [`DesignError::EmptyInput`] when no reference weapon carries one of the stats.
    pub fn read(pool: &Pool) -> DesignResult<Self> {
        Ok(Self {
            swing_damage: need(pool, "swing_damage")?,
            cooldown: need(pool, "cooldown")?,
            mass: need(pool, "mass")?,
            work: need_work(pool)?,
        })
    }
}

/// The role of the pool that matches an archetype: the first hint that the pool has, compared ignoring case.
#[must_use]
pub fn match_role(pool: &Pool, hints: &[String]) -> Option<String> {
    let roles = pool.roles();
    hints.iter().find_map(|hint| {
        roles
            .iter()
            .find(|(role, _)| role.eq_ignore_ascii_case(hint))
            .map(|(role, _)| role.clone())
    })
}

/// The fractional part the install writes ranges with (the share of the reference ranges that agree is at
/// least 70 percent), or `None` when there is no such habit.
#[must_use]
pub fn range_fraction(pool: &Pool) -> Option<f64> {
    let mut counts: BTreeMap<i64, usize> = BTreeMap::new();
    let mut total = 0usize;
    for item in &pool.items {
        if let Some(r) = item.stat("range") {
            let frac = ((r - r.floor()) * 100.0).round() as i64;
            *counts.entry(frac).or_insert(0) += 1;
            total += 1;
        }
    }
    let (frac, n) = counts.into_iter().max_by_key(|(_, n)| *n)?;
    (total >= 3 && n * 10 >= total * 7).then(|| frac as f64 / 100.0)
}

/// How the strength to aim at was found.
#[derive(Debug, Clone, PartialEq)]
pub struct StrengthTarget {
    /// The strength index to aim at.
    pub strength: f64,
    /// The percentile of the class asked for.
    pub percentile: f64,
    /// The class label of the baseline model.
    pub class_label: String,
    /// The number of items of the class.
    pub class_n: usize,
    /// The chain level of the class.
    pub level: ChainLevel,
    /// Remarks about widening and tier shifts.
    pub notes: Vec<String>,
}

/// The strength index that stands for percentile `p` of the class `key` selects.
///
/// The class comes from the calibration fallback chain (role and tier, role, tier, all). When the chain had to
/// widen past the tier and the pool holds several tiers, the target moves along the fitted trend of the
/// strength with the tier, so a weapon of a tier the install has no peers in still lands higher than the
/// middle of everything. `shift_ln` is the archetype's own shift: how strong the archetype typically is against the middle of its class
/// (an anti materiel rifle against the other weapons of its role, a short bow against the other bows).
///
/// # Errors
///
/// [`DesignError`] when the pool is empty or the percentile is not inside the unit interval.
pub fn strength_target(
    model: &Model,
    key: &ClassKey,
    p: f64,
    shift_ln: f64,
) -> DesignResult<StrengthTarget> {
    let est = model.estimate(&BaselineInput {
        key: key.clone(),
        strength: Some(StrengthInput::Percentile { p }),
        constraints: BTreeMap::new(),
    })?;
    let mut ln = est.ln_strength;
    let mut notes = Vec::new();
    let tier_level = matches!(
        est.level,
        ChainLevel::RoleTier | ChainLevel::GroupTier | ChainLevel::Tier
    );
    if !tier_level && let Some(tier) = key.tier {
        let pool = &model.pool;
        let xs: Vec<f64> = pool.items.iter().map(|i| f64::from(i.tier)).collect();
        let ys: Vec<f64> = pool.items.iter().map(|i| i.strength).collect();
        let distinct = pool.tiers().len();
        let cohort = pool.cohort(key, None, None, crate::classes::MIN_POOL);
        let cohort_tiers: Vec<f64> = cohort
            .members
            .iter()
            .filter_map(|m| pool.item(*m))
            .map(|i| f64::from(i.tier))
            .collect();
        if distinct >= 2
            && let (Some(fit), Some(centre)) = (log_linear_fit(&xs, &ys), mean(&cohort_tiers))
        {
            let slope = fit.slope.clamp(0.0, 0.8);
            let shift = slope * (f64::from(tier) - centre);
            if shift.abs() > 1e-9 {
                ln += shift;
                notes.push(format!(
                    "the class was widened past the tier ({}): the target follows the trend of strength with tier ({:+.0} percent)",
                    pool.tier_text(tier),
                    (shift.exp() - 1.0) * 100.0
                ));
            }
        }
    }
    if shift_ln != 0.0 {
        ln += shift_ln;
        notes.push(format!(
            "the archetype is typically {:+.0} percent in strength against the middle of its class",
            (shift_ln.exp() - 1.0) * 100.0
        ));
    }
    Ok(StrengthTarget {
        strength: ln.exp(),
        percentile: p,
        class_label: est.class_label,
        class_n: est.class_n,
        level: est.level,
        notes,
    })
}

/// How the strength of a pool item with the stuff of the reference weapon differs from the strength of the
/// same tools with unit modifiers: the median of the ratio over the reference melee weapons that are
/// stuffed. 1 when there is none. The melee solver builds tools without stuff factors and divides the
/// target by this bias.
#[must_use]
pub fn melee_bias(weapons: &[WeaponDetail]) -> f64 {
    let ratios: Vec<f64> = weapons
        .iter()
        .filter(|w| item_kind(w.kind) == ItemKind::Melee && !w.stuff_categories.is_empty())
        .filter_map(|w| {
            let pooled = w.strength?;
            let tools: Vec<crate::melee::MeleeTool> = w
                .tools
                .iter()
                .filter_map(|t| {
                    Some(crate::melee::MeleeTool {
                        label: t.label.clone(),
                        capacities: t
                            .capacities
                            .iter()
                            .map(|c| crate::melee::MeleeCapacity::named(c))
                            .collect(),
                        power: t.power?,
                        cooldown: t.cooldown?,
                        armor_penetration: t.armor_penetration,
                        chance_factor: t.chance_factor,
                    })
                })
                .collect();
            let attacks = enumerate_attacks(&tools, &MeleeModifiers::default()).ok()?;
            let plain = melee_strength(&attacks).ok()?;
            (plain > 0.0 && pooled > 0.0).then_some(pooled / plain)
        })
        .collect();
    median(&ratios)
        .filter(|r| r.is_finite() && *r > 0.0)
        .unwrap_or(1.0)
}

/// Rounds `value` to the nearest multiple of `step`.
#[must_use]
pub fn round_to(value: f64, step: f64) -> f64 {
    if step <= 0.0 || !value.is_finite() {
        return value;
    }
    let r = (value / step).round() * step;
    // Keep the decimals the step has: 0.05 steps give 1.35, not 1.3500000000000001.
    let decimals = (-step.log10()).ceil().max(0.0) as i32;
    let scale = 10f64.powi(decimals + 1);
    (r * scale).round() / scale
}

/// Rounds to `digits` significant digits.
#[must_use]
pub fn round_significant(value: f64, digits: i32) -> f64 {
    if !value.is_finite() || value == 0.0 {
        return value;
    }
    let magnitude = value.abs().log10().floor() as i32;
    let step = 10f64.powi(magnitude - digits + 1);
    (value / step).round() * step
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_keeps_the_decimals_of_the_step() {
        assert_eq!(round_to(1.3677, 0.05), 1.35);
        assert_eq!(round_to(1.4, 0.05), 1.4);
        assert_eq!(round_to(7.26, 0.1), 7.3);
        assert_eq!(round_to(12.4, 1.0), 12.0);
        assert_eq!(round_significant(12345.0, 2), 12000.0);
        assert_eq!(round_significant(4560.0, 2), 4600.0);
    }
}
