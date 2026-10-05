//! Class statistics over the user's Combat Extended conversions.
//!
//! The conversions found by [`crate::ce::reader`] become a [`crate::classes::Pool`]: one reference item per
//! converted gun (or melee weapon) whose stats are the converted numbers, plus, for items with a vanilla twin,
//! `vanilla.<stat>` (the twin's number) and `ratio.<stat>` (converted over vanilla). Because everything is a
//! named stat of a pool item, the calibration statistics apply unchanged: [`ClassStats::derive`] summarises
//! every stat per class (median, percentiles, spread, flatness) and every stat widens through the fallback
//! chain on its own (role and tier, role, group and tier, tier, group, all) when the class is sparse (IT-006).
//!
//! On top of that, [`predict_conversion`] estimates the Combat Extended numbers of a vanilla design (the
//! "convert an item" flow). Per stat it chooses between four predictors by leave-one-out error over the
//! class members that have a twin, preferring the simpler one on ties:
//!
//! 1. identity: the converted value equals the vanilla value (mass, as the research found);
//! 2. median: the class median of the converted stat (class constants: spread, sway, reload, sights);
//! 3. ratio: the vanilla value times the median converted over vanilla ratio of the class (range);
//! 4. elastic: a power law fitted on the class's twin pairs (bulk from mass, warmup, cooldown).
//!
//! Each prediction carries an error band from its own leave-one-out residuals ([`crate::baseline::Band`]), the
//! class it is based on and a note when the class had to be widened. Nothing here is a table: all numbers
//! are derived from the conversions that the reader handed over.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::reader::{CeGun, CeMelee, CeModel};
use crate::baseline::{Band, band_from_residuals};
use crate::classes::numeric::{log_log_fit, median};
use crate::classes::{
    ChainLevel, ClassKey, ClassStats as BaseStats, Cohort, DeriveOptions, ItemKind, MIN_POOL, Pool,
    PoolOptions, ReferenceItem, StatSummary,
};
use crate::error::{DesignError, DesignResult};
use crate::model::{DEFAULT_TICKS_BETWEEN_SHOTS, TechLevel};
use crate::ranged::{CycleInput, cycle_time};

pub mod estimate;

#[cfg(test)]
mod tests;

pub use estimate::{
    EstimateOptions, Example, ExampleSet, Profile, Reliability, estimate_conversion, leave_one_out,
    profile_of,
};

/// Prefix of the stat that holds the vanilla twin's number.
pub const VANILLA_PREFIX: &str = "vanilla.";
/// Prefix of the stat that holds the converted over vanilla ratio.
pub const RATIO_PREFIX: &str = "ratio.";

/// The definition of the Combat Extended strength proxies (replaceable data, section 4 of the balance math
/// specification): ranged `damage x pellets x burst / cycle x sqrt(range / reference) x (1 + w x sharp
/// penetration)`, melee `mean tool power / mean tool cooldown`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StrengthDefinition {
    /// Weight of the sharp penetration in the ranged proxy.
    pub penetration_weight: f64,
    /// The range that gives a factor of 1 in the ranged proxy.
    pub range_reference: f64,
}

impl Default for StrengthDefinition {
    fn default() -> Self {
        Self {
            penetration_weight: 0.1,
            range_reference: 25.0,
        }
    }
}

/// Options of the class statistics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CeClassOptions {
    /// Options of the calibration statistics.
    pub derive: DeriveOptions,
    /// The strength proxies.
    pub strength: StrengthDefinition,
    /// Smallest number of items a class needs before it is trusted.
    pub min_pool: usize,
    /// Stats that are whole numbers; their predictions are rounded to at least 1.
    pub integer_stats: Vec<String>,
}

impl Default for CeClassOptions {
    fn default() -> Self {
        Self {
            derive: DeriveOptions {
                mass_stat: Some("mass".to_owned()),
                ..DeriveOptions::default()
            },
            strength: StrengthDefinition::default(),
            min_pool: MIN_POOL,
            integer_stats: vec!["burst".to_owned(), "magazine".to_owned()],
        }
    }
}

fn get(stats: &BTreeMap<String, f64>, name: &str) -> Option<f64> {
    stats.get(name).copied()
}

/// The ranged strength proxy of a converted gun, `None` when a needed number is missing or degenerate.
#[must_use]
pub fn gun_strength(stats: &BTreeMap<String, f64>, def: &StrengthDefinition) -> Option<f64> {
    let damage = get(stats, "damage")?;
    let range = get(stats, "range")?;
    let burst = get(stats, "burst").unwrap_or(1.0);
    let cycle = cycle_time(&CycleInput {
        warmup: get(stats, "warmup")?,
        cooldown: get(stats, "cooldown")?,
        burst_count: if burst >= 1.0 && burst <= f64::from(u32::MAX) {
            burst.floor() as u32
        } else {
            1
        },
        ticks_between_shots: get(stats, "ticks_between").unwrap_or(DEFAULT_TICKS_BETWEEN_SHOTS),
    })
    .ok()?;
    if range <= 0.0 || def.range_reference <= 0.0 || damage < 0.0 {
        return None;
    }
    let value = damage * get(stats, "pellets").unwrap_or(1.0) * burst.max(1.0) / cycle
        * (range / def.range_reference).sqrt()
        * (1.0 + def.penetration_weight * get(stats, "ap_sharp").unwrap_or(0.0));
    (value.is_finite() && value > 0.0).then_some(value)
}

/// The melee strength proxy: mean tool power over mean tool cooldown.
#[must_use]
pub fn melee_strength_proxy(stats: &BTreeMap<String, f64>) -> Option<f64> {
    let power = get(stats, "tool_power")?;
    let cooldown = get(stats, "tool_cooldown")?;
    let value = power / cooldown;
    (cooldown > 0.0 && value.is_finite() && value > 0.0).then_some(value)
}

fn twin_stats_into(
    stats: &mut BTreeMap<String, f64>,
    twin: Option<&BTreeMap<String, f64>>,
    names: &[&str],
) {
    let Some(twin) = twin else { return };
    for name in names {
        let Some(v) = twin.get(*name).copied().filter(|v| v.is_finite()) else {
            continue;
        };
        stats.insert(format!("{VANILLA_PREFIX}{name}"), v);
        if let Some(c) = stats.get(*name).copied()
            && v > 0.0
            && c > 0.0
        {
            stats.insert(format!("{RATIO_PREFIX}{name}"), c / v);
        }
    }
}

fn gun_item(g: &CeGun, def: &StrengthDefinition) -> ReferenceItem {
    let mut item = ReferenceItem::new(g.def_name.clone(), ItemKind::Ranged);
    item.labels.push(g.label.clone());
    item.labels.extend(g.ce_tags.iter().cloned());
    item.tier = g.tech_level.map(TechLevel::index);
    item.role.clone_from(&g.ai_class);
    item.group = Some(
        if g.stats.get("burst").copied().unwrap_or(1.0) > 1.0 {
            "burst"
        } else {
            "single"
        }
        .to_owned(),
    );
    item.stats.clone_from(&g.stats);
    twin_stats_into(
        &mut item.stats,
        g.twin.as_ref(),
        &[
            "mass", "range", "warmup", "cooldown", "burst", "damage", "speed",
        ],
    );
    item.strength = gun_strength(&g.stats, def);
    item
}

fn melee_item(m: &CeMelee) -> ReferenceItem {
    let mut item = ReferenceItem::new(m.def_name.clone(), ItemKind::Melee);
    item.labels.push(m.label.clone());
    item.tier = m.tech_level.map(TechLevel::index);
    item.stats.clone_from(&m.stats);
    twin_stats_into(
        &mut item.stats,
        m.twin.as_ref(),
        &["mass", "tool_power", "tool_cooldown"],
    );
    item.strength = melee_strength_proxy(&m.stats);
    item
}

/// The reference items of the converted weapons of one kind. Weapons with an exclusion reason (turret guns,
/// launchers, grenades) are left out.
///
/// # Errors
///
/// [`DesignError::InvalidInput`] for the apparel kind (not built in this release).
pub fn reference_items(
    model: &CeModel,
    kind: ItemKind,
    def: &StrengthDefinition,
) -> DesignResult<Vec<ReferenceItem>> {
    match kind {
        ItemKind::Ranged => Ok(model
            .guns
            .iter()
            .filter(|g| g.excluded.is_none())
            .map(|g| gun_item(g, def))
            .collect()),
        ItemKind::Melee => Ok(model
            .melee
            .iter()
            .filter(|m| m.excluded.is_none())
            .map(melee_item)
            .collect()),
        ItemKind::Apparel => Err(DesignError::invalid(
            "kind",
            "apparel pools are not built in this release",
        )),
    }
}

/// The pool of the converted weapons of one kind. Tier comes from the tech level, role from the AI class tag
/// for guns, group from the burst length; missing axes are derived by the pool.
///
/// # Errors
///
/// See [`reference_items`].
pub fn build_pool(model: &CeModel, kind: ItemKind, opts: &CeClassOptions) -> DesignResult<Pool> {
    let items = reference_items(model, kind, &opts.strength)?;
    let options = PoolOptions {
        derive_tier: true,
        tier_labels: TechLevel::ALL
            .into_iter()
            .map(|t| (t.index(), t.xml_name().to_owned()))
            .collect(),
    };
    Ok(Pool::build(kind, &items, &options))
}

/// The class key of a design: its tier, role and group.
#[must_use]
pub fn class_key(tier: Option<TechLevel>, role: Option<&str>, group: Option<&str>) -> ClassKey {
    ClassKey {
        role: role.map(str::to_owned),
        tier: tier.map(TechLevel::index),
        group: group.map(str::to_owned),
    }
}

/// A stat that is flat within the class: every item has about the same value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassConstant {
    /// Stat name.
    pub stat: String,
    /// The class median.
    pub value: f64,
    /// Items behind the value.
    pub n: usize,
    /// The chain level the value was drawn from.
    pub level: ChainLevel,
}

/// The statistics of one class of converted weapons.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassStats {
    /// The calibration statistics of the class: every stat of the pool, ratios included.
    pub base: BaseStats,
    /// The converted stats that are flat within their class, by stat name.
    pub constants: Vec<ClassConstant>,
}

impl ClassStats {
    /// Derives the statistics of the class `key` from `pool`. Each stat is drawn from the first chain level
    /// that has enough items carrying it, so a sparse class widens stat by stat.
    #[must_use]
    pub fn derive(pool: &Pool, key: &ClassKey, opts: &CeClassOptions) -> Self {
        let base = BaseStats::derive(pool, key, &opts.derive);
        let constants = base
            .stats
            .values()
            .filter(|s| is_converted_stat(&s.stat) && s.flat)
            .map(|s| ClassConstant {
                stat: s.stat.clone(),
                value: s.median,
                n: s.n,
                level: s.level,
            })
            .collect();
        Self { base, constants }
    }

    /// The summary of a converted stat.
    #[must_use]
    pub fn stat(&self, name: &str) -> Option<&StatSummary> {
        self.base.stat(name)
    }

    /// The summary of the converted over vanilla ratio of a stat (items with a twin only).
    #[must_use]
    pub fn ratio(&self, name: &str) -> Option<&StatSummary> {
        self.base.stat(&format!("{RATIO_PREFIX}{name}"))
    }

    /// The class constant of a stat, when the stat is flat in its class.
    #[must_use]
    pub fn constant(&self, name: &str) -> Option<&ClassConstant> {
        self.constants.iter().find(|c| c.stat == name)
    }

    /// True when the class is small enough that bands are rough.
    #[must_use]
    pub fn is_rough(&self) -> bool {
        self.base.is_rough()
    }

    /// A note when the class had to be widened, naming the level used (IT-006).
    #[must_use]
    pub fn fallback_note(&self) -> Option<String> {
        (self.base.level != self.base.requested_level).then(|| self.base.explanation.clone())
    }
}

pub(crate) fn is_converted_stat(name: &str) -> bool {
    !name.starts_with(VANILLA_PREFIX) && !name.starts_with(RATIO_PREFIX) && name != "strength"
}

// ---------------------------------------------------------------------------------------------------------
// Conversion predictors
// ---------------------------------------------------------------------------------------------------------

/// The predictor chosen for a converted stat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConversionPredictor {
    /// The converted value equals the vanilla value.
    Identity,
    /// The class median of the converted stat.
    Median,
    /// The vanilla value times the class's median ratio.
    Ratio,
    /// A power law fitted on the class's twin pairs.
    Elastic,
}

/// The estimate of one converted stat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatPrediction {
    /// The estimated value.
    pub value: f64,
    /// The predictor that produced it.
    pub predictor: ConversionPredictor,
    /// The vanilla stat that drove it, when one did.
    pub driver: Option<String>,
    /// The error band of the predictor (from its leave-one-out residuals).
    pub band: Band,
    /// Items behind the estimate.
    pub n: usize,
    /// The chain level the items came from.
    pub level: ChainLevel,
    /// Median relative error of the predictor in leave-one-out, when it could be measured.
    pub loo_error: Option<f64>,
    /// How far the number can be trusted, from its leave one out error. A generator writes only usable
    /// ratings and asks the rest.
    #[serde(default)]
    pub reliability: Reliability,
    /// Converted weapons that carry the stat at all (the pool the class was drawn from).
    #[serde(default)]
    pub pool: usize,
}

impl StatPrediction {
    /// The name of the predictor form that produced the number (`identity`, `median`, `ratio`, `elastic`).
    #[must_use]
    pub fn predictor_name(&self) -> &'static str {
        match self.predictor {
            ConversionPredictor::Identity => "identity",
            ConversionPredictor::Median => "median",
            ConversionPredictor::Ratio => "ratio",
            ConversionPredictor::Elastic => "elastic",
        }
    }

    /// True when the rating allows a generator to write the number.
    #[must_use]
    pub fn is_usable(&self) -> bool {
        self.value.is_finite() && self.reliability.is_usable()
    }

    /// Plain words about the accuracy, for readouts and questions: "typical error about 24 percent over
    /// 17 conversions" or "not measured".
    #[must_use]
    pub fn accuracy_note(&self) -> String {
        match self.loo_error {
            Some(e) if self.reliability != Reliability::Unmeasured => format!(
                "rated {}: the typical error was about {:.0} percent when each of {} converted weapons was predicted from the others",
                self.reliability.label(),
                100.0 * e,
                self.pool
            ),
            _ => format!(
                "rated {}: {} converted weapons are too few to measure an error",
                self.reliability.label(),
                self.pool
            ),
        }
    }
}

/// The estimates for a conversion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversionPrediction {
    /// "based on N items of ..." for the class of the key.
    pub class_label: String,
    /// The chain level of the class itself.
    pub level: ChainLevel,
    /// The estimates by converted stat.
    pub stats: BTreeMap<String, StatPrediction>,
    /// Plain words about widened classes and missing twins.
    pub notes: Vec<String>,
    /// Stats that were not estimated at all, with the reason (too few converted weapons carry them).
    #[serde(default)]
    pub withheld: BTreeMap<String, String>,
}

/// The vanilla stat that drives the prediction of a converted stat.
#[must_use]
pub fn driver_of(stat: &str) -> Option<&'static str> {
    match stat {
        "mass" => Some("mass"),
        "range" => Some("range"),
        "warmup" => Some("warmup"),
        "cooldown" => Some("cooldown"),
        "burst" => Some("burst"),
        "bulk" => Some("mass"),
        "tool_power" => Some("tool_power"),
        "tool_cooldown" => Some("tool_cooldown"),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
struct Obs {
    x: f64,
    y: f64,
}

fn fit_predict(kind: ConversionPredictor, train: &[Obs], x: f64) -> Option<f64> {
    let ys: Vec<f64> = train.iter().map(|o| o.y).collect();
    let value = match kind {
        ConversionPredictor::Identity => x,
        ConversionPredictor::Median => median(&ys)?,
        ConversionPredictor::Ratio => {
            let ratios: Vec<f64> = train
                .iter()
                .filter(|o| o.x > 0.0 && o.y > 0.0)
                .map(|o| o.y / o.x)
                .collect();
            x * median(&ratios)?
        }
        ConversionPredictor::Elastic => {
            let xs: Vec<f64> = train.iter().map(|o| o.x).collect();
            let fit = log_log_fit(&xs, &ys)?;
            if x <= 0.0 {
                return None;
            }
            fit.predict(x.ln()).exp()
        }
    };
    value.is_finite().then_some(value)
}

/// Leave-one-out relative errors and log deviations of a predictor over the observations.
fn loo(kind: ConversionPredictor, obs: &[Obs]) -> (Vec<f64>, Vec<f64>) {
    let mut rel = Vec::new();
    let mut dev = Vec::new();
    for (j, held) in obs.iter().enumerate() {
        let train: Vec<Obs> = obs
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != j)
            .map(|(_, o)| *o)
            .collect();
        let Some(p) = fit_predict(kind, &train, held.x) else {
            continue;
        };
        if held.y > 0.0 && p > 0.0 {
            rel.push((p - held.y).abs() / held.y);
            dev.push((p / held.y).ln().abs());
        }
    }
    (rel, dev)
}

/// The first chain level with at least `min` items that carry every stat in `required` (the last level is
/// returned even when smaller).
fn select(
    pool: &Pool,
    key: &ClassKey,
    required: &[String],
    min: usize,
) -> (ChainLevel, Vec<usize>) {
    let eligible: Vec<usize> = pool
        .items
        .iter()
        .enumerate()
        .filter(|(_, item)| required.iter().all(|r| item.stat(r).is_some()))
        .map(|(i, _)| i)
        .collect();
    let requested = ChainLevel::most_specific(key);
    let mut started = false;
    for level in ChainLevel::CHAIN {
        if level == requested {
            started = true;
        }
        if !started || !level.applies(key) {
            continue;
        }
        let members: Vec<usize> = eligible
            .iter()
            .copied()
            .filter(|i| pool.items.get(*i).is_some_and(|it| level.matches(key, it)))
            .collect();
        if members.len() >= min || level == ChainLevel::All {
            return (level, members);
        }
    }
    (ChainLevel::All, eligible)
}

fn predict_stat(
    pool: &Pool,
    key: &ClassKey,
    stat: &str,
    vanilla: &BTreeMap<String, f64>,
    opts: &CeClassOptions,
) -> Option<StatPrediction> {
    let driver = driver_of(stat);
    let x = driver
        .and_then(|d| vanilla.get(d).copied())
        .filter(|v| v.is_finite() && *v > 0.0);
    let mut required = vec![stat.to_owned()];
    if let (Some(d), Some(_)) = (driver, x) {
        required.push(format!("{VANILLA_PREFIX}{d}"));
    }
    let (level, members) = select(pool, key, &required, opts.min_pool);
    let obs: Vec<Obs> = members
        .iter()
        .filter_map(|i| pool.items.get(*i))
        .filter_map(|item| {
            let y = item.stat(stat)?;
            let x = match driver {
                Some(d) if x.is_some() => item.stat(&format!("{VANILLA_PREFIX}{d}"))?,
                _ => 0.0,
            };
            Some(Obs { x, y })
        })
        .collect();
    if obs.is_empty() {
        return None;
    }
    let mut candidates = vec![ConversionPredictor::Identity];
    candidates.retain(|_| x.is_some());
    candidates.push(ConversionPredictor::Median);
    if x.is_some() {
        candidates.push(ConversionPredictor::Ratio);
        candidates.push(ConversionPredictor::Elastic);
    }
    // Identity is only a candidate for stats the vanilla driver measures in the same unit.
    if driver != Some(stat) {
        candidates.retain(|c| *c != ConversionPredictor::Identity);
    }
    let measured = obs.len() >= opts.min_pool;
    let mut best: Option<(ConversionPredictor, f64, Vec<f64>)> = None;
    if measured {
        for kind in &candidates {
            let (rel, dev) = loo(*kind, &obs);
            let Some(err) = median(&rel) else { continue };
            if best.as_ref().is_none_or(|(_, e, _)| err < *e - 1e-12) {
                best = Some((*kind, err, dev));
            }
        }
    }
    let (kind, loo_error, dev) = match best {
        Some((k, e, d)) => (k, Some(e), d),
        None => (candidates.first().copied()?, None, Vec::new()),
    };
    let mut value = fit_predict(kind, &obs, x.unwrap_or(0.0))
        .or_else(|| fit_predict(ConversionPredictor::Median, &obs, 0.0))?;
    if opts.integer_stats.iter().any(|s| s == stat) {
        value = value.round().max(1.0);
    }
    Some(StatPrediction {
        value,
        predictor: kind,
        driver: driver.filter(|_| x.is_some()).map(str::to_owned),
        band: if dev.len() >= 3 {
            band_from_residuals(&dev, 0.0)
        } else {
            Band::fallback()
        },
        n: obs.len(),
        level,
        loo_error,
        reliability: Reliability::Unmeasured,
        pool: obs.len(),
    })
}

/// Estimates the converted numbers of a vanilla design in the class `key`.
///
/// `vanilla` holds the design's own numbers under the names `mass`, `range`, `warmup`, `cooldown`, `burst`
/// (guns) or `mass`, `tool_power`, `tool_cooldown` (melee). Stats without a vanilla driver are class medians.
/// Without any twin pair in the pool the driven stats fall back to the class median too, and the notes say so.
#[must_use]
pub fn predict_conversion(
    pool: &Pool,
    key: &ClassKey,
    vanilla: &BTreeMap<String, f64>,
    opts: &CeClassOptions,
) -> ConversionPrediction {
    let cohort = pool.cohort(key, None, None, opts.min_pool);
    let class_label = pool.explain(key, &cohort);
    let mut notes = Vec::new();
    if cohort.widened() {
        notes.push(format!("the class was widened: {class_label}"));
    }
    let twins = pool
        .items
        .iter()
        .filter(|i| i.stats.keys().any(|k| k.starts_with(RATIO_PREFIX)))
        .count();
    if twins == 0 {
        notes.push(
            "no converted weapon has a vanilla twin: every stat is a class median".to_owned(),
        );
    }
    let mut stats = BTreeMap::new();
    for name in pool.stat_names() {
        if !is_converted_stat(&name) {
            continue;
        }
        let Some(p) = predict_stat(pool, key, &name, vanilla, opts) else {
            continue;
        };
        if p.level != cohort.level && p.level != ChainLevel::All {
            notes.push(format!(
                "{name} is based on the wider level {}",
                p.level.as_str()
            ));
        }
        stats.insert(name, p);
    }
    ConversionPrediction {
        class_label,
        level: cohort.level,
        stats,
        notes,
        withheld: BTreeMap::new(),
    }
}

/// The cohort of the strength distribution of a class, for callers that only need the label.
#[must_use]
pub fn class_cohort(pool: &Pool, key: &ClassKey, opts: &CeClassOptions) -> Cohort {
    pool.cohort(key, None, None, opts.min_pool)
}
