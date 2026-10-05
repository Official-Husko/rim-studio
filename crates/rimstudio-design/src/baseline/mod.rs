//! The baseline model: estimated stat values with source labels and error bands.
//!
//! For each stat the designer holds three predictors and a rule picks one (section 6 of the math
//! specification): the class median M, a pool quantile Q moved by a rank regression on strength, tier and
//! mass, and an anchor ratio R that scales the values of the nearest reference items. [`Model::fit`] picks
//! the predictor per stat by a nested leave one out inside the pool; [`simple_estimate`] and
//! [`anchor_baseline`] are the two entry points that work from class statistics alone (simple mode and
//! calibrated mode share one code path).
//!
//! Every estimate carries, per stat, the value, the [`Source`] (typed, answer, anchor, class median,
//! quantile or derived), an error [`Band`] and the pool the value came from. Typed values are never
//! overwritten (IT-003). Estimates are non-decreasing in strength for stats with a positive rank slope
//! (non-increasing for a negative one) when produced by the quantile predictor; the anchor predictor is
//! monotone for noise free pools and statistically monotone otherwise.

mod engine;

pub(crate) use engine::band_shift;
pub use engine::{MIN_QUIZ_ITEMS, Model, RuleScore};

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::classes::numeric::{mean, mid_rank_of, quantile_sorted};
use crate::classes::{
    ChainLevel, ClassKey, ClassStats, DeriveOptions, PoolItem, Predictor, StatSummary, StatTraits,
};
use crate::error::{DesignError, DesignResult};

/// Number of nearest anchors blended by the anchor predictor.
pub const K_ANCHORS: usize = 2;
/// Distance added to a role mismatch.
pub const W_ROLE: f64 = 2.0;
/// Distance per tier of difference.
pub const W_TIER: f64 = 0.35;
/// Distance added to a group (fire class) mismatch.
pub const W_GROUP: f64 = 0.8;
/// Offset of the anchor weight `1 / (distance + ANCHOR_EPS)`.
pub const ANCHOR_EPS: f64 = 0.3;
/// Standard deviation of the prior on ln strength used by the comparison dialogue.
pub const PRIOR_SD: f64 = 0.6;
/// Floor of the spread of ln strength used to normalise anchor distances.
pub const MIN_SD_P: f64 = 0.3;
/// A comparison within this relative difference (in ln units) counts as "about the same".
pub const SAME_BAND: f64 = 0.10;
/// Residual factor of the P50 band used when nothing was measured.
pub const DEFAULT_BAND_P50: f64 = 1.5;
/// Residual factor of the P80 band used when nothing was measured.
pub const DEFAULT_BAND_P80: f64 = 2.5;

/// Per stat override of the detected traits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatRule {
    /// Stat name.
    pub name: String,
    /// Force the discrete flag (anchors are copied, never scaled); `None` detects it from the pool.
    #[serde(default)]
    pub discrete: Option<bool>,
    /// Force whole number rounding; `None` detects it from the pool.
    #[serde(default)]
    pub integer: Option<bool>,
    /// The stat is not predictable and is shown as a plain input with the pool range beside it.
    #[serde(default)]
    pub ask: bool,
}

/// A structural identity applied after the predictors (section 6 of the math specification).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Identity {
    /// `stat` is the strength index times `other`, for example melee swing damage equals the in fight
    /// damage per second times the swing cooldown.
    StrengthTimes {
        /// Stat that is derived.
        stat: String,
        /// Stat the strength is multiplied with.
        other: String,
    },
}

/// Constants and options of the baseline model. The defaults are the documented priors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BaselineConfig {
    /// Options of the class statistics (minimum pool, ridges, flat factor, mass stat).
    pub derive: DeriveOptions,
    /// Number of anchors blended by the anchor predictor.
    pub k_anchors: usize,
    /// Distance of a role mismatch.
    pub w_role: f64,
    /// Distance per tier of difference.
    pub w_tier: f64,
    /// Distance of a group mismatch.
    pub w_group: f64,
    /// Offset of the anchor weight.
    pub anchor_eps: f64,
    /// Standard deviation of the strength prior of the comparison dialogue.
    pub prior_sd: f64,
    /// Floor of the ln strength spread.
    pub min_sd_p: f64,
    /// "About the same" band of the comparison dialogue, in ln units.
    pub same_band: f64,
    /// Edge of the "lighter, similar, heavier" buckets in ln units (`ln(1 / 0.85)`).
    pub bucket_edge: f64,
    /// Stat overrides.
    pub rules: Vec<StatRule>,
    /// Structural identities.
    pub identities: Vec<Identity>,
}

impl Default for BaselineConfig {
    fn default() -> Self {
        Self {
            derive: DeriveOptions::default(),
            k_anchors: K_ANCHORS,
            w_role: W_ROLE,
            w_tier: W_TIER,
            w_group: W_GROUP,
            anchor_eps: ANCHOR_EPS,
            prior_sd: PRIOR_SD,
            min_sd_p: MIN_SD_P,
            same_band: SAME_BAND,
            bucket_edge: (1.0f64 / 0.85).ln(),
            rules: Vec::new(),
            identities: Vec::new(),
        }
    }
}

impl BaselineConfig {
    /// The rule for a stat, if any.
    #[must_use]
    pub fn rule(&self, name: &str) -> Option<&StatRule> {
        self.rules.iter().find(|r| r.name == name)
    }

    /// Resolves the traits of a stat: detected traits overridden by an explicit rule.
    #[must_use]
    pub fn resolve_traits(&self, name: &str, detected: StatTraits) -> StatTraits {
        match self.rule(name) {
            Some(r) => StatTraits {
                discrete: r.discrete.unwrap_or(detected.discrete),
                integer: r.integer.unwrap_or(detected.integer),
                ask: r.ask || detected.ask,
            },
            None => detected,
        }
    }
}

/// Where an estimated value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// The user typed the number.
    Typed,
    /// An answer of the quiz (interval, bucket against an anchor) set or restricted the value.
    Answer,
    /// Ratio scaling from the nearest anchors.
    Anchor,
    /// Median of the class.
    ClassMedian,
    /// Quantile of the class distribution.
    Quantile,
    /// Computed from other estimates by a structural identity.
    Derived,
}

impl Source {
    /// Stable lowercase label.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Typed => "typed",
            Self::Answer => "answer",
            Self::Anchor => "anchor",
            Self::ClassMedian => "class-median",
            Self::Quantile => "quantile",
            Self::Derived => "derived",
        }
    }
}

/// The three position strength choice of simple mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StrengthChoice {
    /// Weaker than most of this role and tier (percentile 1/6).
    Weaker,
    /// Typical (percentile 1/2).
    Typical,
    /// Stronger than most (percentile 5/6).
    Stronger,
}

impl StrengthChoice {
    /// The percentile position of the choice.
    #[must_use]
    pub fn percentile(self) -> f64 {
        match self {
            Self::Weaker => 1.0 / 6.0,
            Self::Typical => 0.5,
            Self::Stronger => 5.0 / 6.0,
        }
    }
}

/// What is known about the strength of the new item.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum StrengthInput {
    /// Nothing: the prior (mean ln strength of the peers) at the middle position.
    Unknown,
    /// One of the three simple mode choices.
    Choice {
        /// The choice.
        choice: StrengthChoice,
    },
    /// A percentile position in `[0, 1]` of the class strength distribution.
    Percentile {
        /// Position.
        p: f64,
    },
    /// A strength placed by the comparison dialogue, as `ln` of the strength index.
    Placed {
        /// Natural logarithm of the strength index.
        ln_power: f64,
    },
}

/// Answer to a "compared with the anchor" question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Bucket {
    /// Lighter or smaller than the anchor (below about 85 percent).
    Lower,
    /// About the same.
    Similar,
    /// Heavier or larger than the anchor (above about 118 percent).
    Higher,
}

impl Bucket {
    /// Index into the bucket ratio array.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::Lower => 0,
            Self::Similar => 1,
            Self::Higher => 2,
        }
    }
}

/// A constraint on one stat: the third kind of answer (the others select a pool or place the strength).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum StatConstraint {
    /// A typed number: replaces the prediction and is never re-banded.
    Typed {
        /// The number.
        value: f64,
    },
    /// An interval answer: the value is predicted from class items inside `[lo, hi)`.
    Interval {
        /// Inclusive lower edge, or open.
        lo: Option<f64>,
        /// Exclusive upper edge, or open.
        hi: Option<f64>,
    },
    /// A ratio bucket against a named reference item.
    VsAnchor {
        /// Id of the reference item.
        anchor: String,
        /// The bucket.
        bucket: Bucket,
    },
}

/// Input of [`Model::estimate`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BaselineInput {
    /// Class selectors answered so far.
    pub key: ClassKey,
    /// Strength knowledge.
    pub strength: Option<StrengthInput>,
    /// Constraints on single stats.
    #[serde(default)]
    pub constraints: BTreeMap<String, StatConstraint>,
}

/// Input of [`anchor_baseline`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnchorInput {
    /// Strength knowledge.
    pub strength: StrengthInput,
    /// Constraints on single stats.
    #[serde(default)]
    pub constraints: BTreeMap<String, StatConstraint>,
}

/// Multiplicative error band of a stat: the residual factors that hold half and four fifths of the held
/// out items, plus a shift for stats that can be zero.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Band {
    /// Factor of the P50 band (at least 1).
    pub p50: f64,
    /// Factor of the P80 band (at least the P50 factor).
    pub p80: f64,
    /// Added to value and prediction before the ratio is taken (0 for stats that are always positive).
    #[serde(default)]
    pub shift: f64,
}

impl Band {
    /// Builds a band, flooring the factors at 1 and ordering them.
    #[must_use]
    pub fn new(p50: f64, p80: f64, shift: f64) -> Self {
        let p50 = if p50.is_finite() {
            p50.max(1.0)
        } else {
            DEFAULT_BAND_P50
        };
        let p80 = if p80.is_finite() {
            p80.max(p50)
        } else {
            DEFAULT_BAND_P80.max(p50)
        };
        Self {
            p50,
            p80,
            shift: if shift.is_finite() {
                shift.max(0.0)
            } else {
                0.0
            },
        }
    }

    /// The band used when nothing was measured.
    #[must_use]
    pub fn fallback() -> Self {
        Self {
            p50: DEFAULT_BAND_P50,
            p80: DEFAULT_BAND_P80,
            shift: 0.0,
        }
    }

    fn range(&self, value: f64, factor: f64) -> (f64, f64) {
        let v = value + self.shift;
        (
            (v / factor - self.shift).max(if value >= 0.0 { 0.0 } else { f64::MIN }),
            v * factor - self.shift,
        )
    }

    /// Range of the P50 band around a predicted value: `value / factor` to `value * factor`.
    #[must_use]
    pub fn interval50(&self, value: f64) -> (f64, f64) {
        self.range(value, self.p50)
    }

    /// Range of the P80 band around a predicted value.
    #[must_use]
    pub fn interval80(&self, value: f64) -> (f64, f64) {
        self.range(value, self.p80)
    }

    /// Absolute log deviation of `actual` from `predicted` on the scale of this band.
    #[must_use]
    pub fn deviation(&self, predicted: f64, actual: f64) -> f64 {
        ln_deviation(predicted, actual, self.shift)
    }
}

/// Bands by stat name.
pub type BandTable = BTreeMap<String, Band>;

/// `|ln((actual + shift) / (predicted + shift))|`, with both terms floored at a tiny positive number.
#[must_use]
pub fn ln_deviation(predicted: f64, actual: f64, shift: f64) -> f64 {
    let a = (actual + shift).max(1e-12);
    let p = (predicted + shift).max(1e-12);
    (a / p).ln().abs()
}

/// Builds a band from residuals (absolute log deviations): the factors are `exp` of their P50 and P80.
/// Fewer than three residuals give [`Band::fallback`].
#[must_use]
pub fn band_from_residuals(residuals: &[f64], shift: f64) -> Band {
    let sorted = crate::classes::numeric::finite_sorted(residuals);
    if sorted.len() < 3 {
        return Band::fallback();
    }
    let p50 = quantile_sorted(&sorted, 0.5).unwrap_or(0.0).exp();
    let p80 = quantile_sorted(&sorted, 0.8).unwrap_or(0.0).exp();
    Band::new(p50, p80, shift)
}

/// A reference item offered as an anchor, with its distance to the new item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    /// Id of the item.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Strength index.
    pub strength: f64,
    /// Natural logarithm of the strength.
    pub ln_strength: f64,
    /// Tier of the item.
    pub tier: u8,
    /// Role of the item.
    pub role: String,
    /// Group of the item.
    pub group: Option<String>,
    /// Normalised distance to the item being designed.
    pub distance: f64,
    /// Stats of the item.
    pub stats: BTreeMap<String, f64>,
}

impl Anchor {
    /// Builds an anchor from a pool item and a distance.
    #[must_use]
    pub fn from_item(item: &PoolItem, distance: f64) -> Self {
        Self {
            id: item.id.clone(),
            label: item.label.clone(),
            strength: item.strength,
            ln_strength: item.ln_strength,
            tier: item.tier,
            role: item.role.clone(),
            group: item.group.clone(),
            distance,
            stats: item.stats.clone(),
        }
    }
}

/// Estimate of one stat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatEstimate {
    /// Estimated value; `None` for a stat flagged "ask" that the user has not typed.
    pub value: Option<f64>,
    /// Where the value came from; `None` when there is no value.
    pub source: Option<Source>,
    /// Error band; `None` for typed values (never re-banded) and for stats without a value.
    pub band: Option<Band>,
    /// P10 of the class distribution of the stat.
    pub p10: f64,
    /// Median of the class distribution.
    pub median: f64,
    /// P90 of the class distribution.
    pub p90: f64,
    /// Number of items behind the class distribution.
    pub n: usize,
    /// Chain level the class distribution was drawn from.
    pub level: ChainLevel,
    /// The predictor the rule selected for this stat.
    pub predictor: Predictor,
}

/// Result of an estimation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Estimate {
    /// Explanation of the class, for example "based on 5 items of role rifle at tier 2".
    pub class_label: String,
    /// Chain level of the class as a whole.
    pub level: ChainLevel,
    /// Number of items in the class.
    pub class_n: usize,
    /// Natural logarithm of the strength the estimate assumed.
    pub ln_strength: f64,
    /// Strength index the estimate assumed.
    pub strength: f64,
    /// Position of that strength inside the class distribution, in `[0, 1]`.
    pub strength_percentile: f64,
    /// Estimates by stat name.
    pub stats: BTreeMap<String, StatEstimate>,
    /// The nearest reference items (at most three).
    pub anchors: Vec<Anchor>,
    /// Remarks for the UI (ignored constraints and similar).
    pub notes: Vec<String>,
}

impl Estimate {
    /// Value of a stat.
    #[must_use]
    pub fn value(&self, stat: &str) -> Option<f64> {
        self.stats.get(stat).and_then(|s| s.value)
    }

    /// Source of a stat.
    #[must_use]
    pub fn source(&self, stat: &str) -> Option<Source> {
        self.stats.get(stat).and_then(|s| s.source)
    }
}

// ---------------------------------------------------------------------------------------------------
// Kernels shared by the public entry points and the model.
// ---------------------------------------------------------------------------------------------------

/// Borrowed view of an anchor used by the predictors.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AnchorRef<'a> {
    pub(crate) id: &'a str,
    pub(crate) ln_strength: f64,
    pub(crate) distance: f64,
    pub(crate) stats: &'a BTreeMap<String, f64>,
}

pub(crate) fn in_interval(v: f64, lo: Option<f64>, hi: Option<f64>) -> bool {
    lo.is_none_or(|l| v >= l) && hi.is_none_or(|h| v < h)
}

fn restricted(values: &[f64], lo: Option<f64>, hi: Option<f64>) -> Vec<f64> {
    values
        .iter()
        .copied()
        .filter(|v| in_interval(*v, lo, hi))
        .collect()
}

/// Representative value of an interval with no items inside: the geometric centre for a positive closed
/// interval, an extrapolation by a factor 1.4 for an open one.
pub(crate) fn interval_midpoint(lo: Option<f64>, hi: Option<f64>, fallback: f64) -> f64 {
    match (lo, hi) {
        (Some(l), Some(h)) if l > 0.0 && h > 0.0 => (l * h).sqrt(),
        (Some(l), Some(h)) => (l + h) / 2.0,
        (None, Some(h)) => h / 1.4,
        (Some(l), None) => l * 1.4,
        (None, None) => fallback,
    }
}

/// Median predictor (M), restricted to an interval when one was answered.
pub(crate) fn median_predict(
    s: &StatSummary,
    interval: Option<(Option<f64>, Option<f64>)>,
    wide: &[f64],
) -> f64 {
    quantile_at(s, 0.5, interval, wide)
}

fn quantile_at(
    s: &StatSummary,
    u: f64,
    interval: Option<(Option<f64>, Option<f64>)>,
    wide: &[f64],
) -> f64 {
    match interval {
        None => quantile_sorted(&s.values, u).unwrap_or(s.median),
        Some((lo, hi)) => {
            let mut inside = restricted(&s.values, lo, hi);
            if inside.len() < 2 {
                let widened = restricted(wide, lo, hi);
                if widened.len() >= 2 || (inside.is_empty() && !widened.is_empty()) {
                    inside = widened;
                }
            }
            if inside.is_empty() {
                interval_midpoint(lo, hi, s.median)
            } else {
                quantile_sorted(&inside, u).unwrap_or(s.median)
            }
        }
    }
}

/// Position of the target for the quantile predictor.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Position {
    /// Strength mid rank override (percentile choices); `None` computes it from `strength`.
    pub(crate) x_strength: Option<f64>,
    /// Strength index of the target.
    pub(crate) strength: f64,
    /// Tier of the target, when known.
    pub(crate) tier: Option<u8>,
    /// Mass of the target, when known.
    pub(crate) mass: Option<f64>,
}

/// Quantile predictor (Q).
pub(crate) fn quantile_predict(
    s: &StatSummary,
    pos: &Position,
    interval: Option<(Option<f64>, Option<f64>)>,
    wide: &[f64],
) -> f64 {
    let xs = pos
        .x_strength
        .unwrap_or_else(|| mid_rank_of(&s.basis.strength, pos.strength));
    let xt = pos
        .tier
        .map_or(0.5, |t| mid_rank_of(&s.basis.tier, f64::from(t)));
    let xm = match pos.mass {
        Some(m) if !s.basis.mass.is_empty() => mid_rank_of(&s.basis.mass, m),
        _ => 0.5,
    };
    let u = if s.flat {
        0.5
    } else {
        let b = s.rank_beta;
        (0.5 + b[0] * (xs - 0.5) + b[1] * (xt - 0.5) + b[2] * (xm - 0.5)).clamp(0.0, 1.0)
    };
    quantile_at(s, u, interval, wide)
}

/// Weighted median (first value whose cumulative weight reaches one half).
fn weighted_median(pairs: &mut [(f64, f64)]) -> Option<f64> {
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = pairs.iter().map(|p| p.1).sum();
    if total <= 0.0 {
        return None;
    }
    let mut acc = 0.0;
    for (v, w) in pairs.iter() {
        acc += w / total;
        if acc >= 0.5 {
            return Some(*v);
        }
    }
    pairs.last().map(|p| p.0)
}

/// Anchor predictor (R). `anchors` must be sorted by ascending distance.
pub(crate) fn anchor_predict(
    s: &StatSummary,
    anchors: &[AnchorRef<'_>],
    ln_p: f64,
    k: usize,
    eps: f64,
) -> Option<f64> {
    let chosen: Vec<(f64, f64, f64)> = anchors
        .iter()
        .filter_map(|a| {
            a.stats
                .get(&s.stat)
                .copied()
                .filter(|v| v.is_finite() && *v >= 0.0)
                .map(|v| (v, a.distance, a.ln_strength))
        })
        .take(k.max(1))
        .collect();
    if chosen.is_empty() {
        return None;
    }
    let weights: Vec<f64> = chosen.iter().map(|c| 1.0 / (c.1 + eps)).collect();
    let wsum: f64 = weights.iter().sum();
    if s.traits.discrete {
        let mut pairs: Vec<(f64, f64)> = chosen
            .iter()
            .zip(&weights)
            .map(|(c, w)| (c.0, *w))
            .collect();
        return weighted_median(&mut pairs);
    }
    if chosen.iter().any(|c| c.0 <= 0.0) {
        return Some(
            chosen
                .iter()
                .zip(&weights)
                .map(|(c, w)| c.0 * w)
                .sum::<f64>()
                / wsum,
        );
    }
    let acc: f64 = chosen
        .iter()
        .zip(&weights)
        .map(|(c, w)| w * (c.0.ln() + s.elasticity * (ln_p - c.2)))
        .sum();
    let value = (acc / wsum).exp();
    value.is_finite().then_some(value)
}

/// Resolves a strength input against the strength distribution of a class: the natural logarithm of the
/// strength index and, for percentile inputs, the position that drives the quantile predictor.
pub(crate) fn resolve_strength(
    summary: &StatSummary,
    strength: &StrengthInput,
) -> (f64, Option<f64>) {
    let ln_values: Vec<f64> = summary.values.iter().map(|v| v.ln()).collect();
    match strength {
        StrengthInput::Unknown => (mean(&ln_values).unwrap_or(0.0), Some(0.5)),
        StrengthInput::Choice { choice } => {
            let p = choice.percentile();
            (quantile_sorted(&ln_values, p).unwrap_or(0.0), Some(p))
        }
        StrengthInput::Percentile { p } => {
            (quantile_sorted(&ln_values, *p).unwrap_or(0.0), Some(*p))
        }
        StrengthInput::Placed { ln_power } => (*ln_power, None),
    }
}

pub(crate) fn check_input(
    strength: &StrengthInput,
    constraints: &BTreeMap<String, StatConstraint>,
) -> DesignResult<()> {
    match strength {
        StrengthInput::Percentile { p } if !p.is_finite() || !(0.0..=1.0).contains(p) => {
            return Err(DesignError::invalid(
                "strength.p",
                "must be between 0 and 1",
            ));
        }
        StrengthInput::Placed { ln_power } if !ln_power.is_finite() => {
            return Err(DesignError::invalid(
                "strength.ln_power",
                "must be a finite number",
            ));
        }
        _ => {}
    }
    for c in constraints.values() {
        match c {
            StatConstraint::Typed { value } if !value.is_finite() => {
                return Err(DesignError::invalid(
                    "constraints.value",
                    "typed value must be finite",
                ));
            }
            StatConstraint::Interval { lo, hi } => {
                if lo.is_some_and(|v| !v.is_finite()) || hi.is_some_and(|v| !v.is_finite()) {
                    return Err(DesignError::invalid(
                        "constraints.interval",
                        "edges must be finite",
                    ));
                }
                if let (Some(l), Some(h)) = (lo, hi)
                    && l >= h
                {
                    return Err(DesignError::invalid(
                        "constraints.interval",
                        "lo must be below hi",
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Arguments of the shared estimation routine.
pub(crate) struct Core<'a> {
    pub(crate) class: &'a ClassStats,
    pub(crate) anchors: &'a [Anchor],
    pub(crate) strength: &'a StrengthInput,
    pub(crate) constraints: &'a BTreeMap<String, StatConstraint>,
    pub(crate) config: &'a BaselineConfig,
    pub(crate) bands: Option<&'a BandTable>,
    pub(crate) wide: Option<&'a BTreeMap<String, Vec<f64>>>,
}

pub(crate) fn estimate_core(args: &Core<'_>) -> DesignResult<Estimate> {
    check_input(args.strength, args.constraints)?;
    let class = args.class;
    let strength_summary = class.strength.as_ref().ok_or(DesignError::EmptyInput {
        what: "reference items",
    })?;
    let (ln_p, x_override) = resolve_strength(strength_summary, args.strength);
    let strength = ln_p.exp();
    let percentile = x_override.unwrap_or_else(|| mid_rank_of(&strength_summary.values, strength));
    let anchor_refs: Vec<AnchorRef<'_>> = args
        .anchors
        .iter()
        .map(|a| AnchorRef {
            id: a.id.as_str(),
            ln_strength: a.ln_strength,
            distance: a.distance,
            stats: &a.stats,
        })
        .collect();
    let mut notes = Vec::new();
    let empty: Vec<f64> = Vec::new();
    let wide_for = |stat: &str| -> &[f64] {
        args.wide
            .and_then(|w| w.get(stat))
            .map_or(empty.as_slice(), Vec::as_slice)
    };
    let interval_of = |stat: &str| match args.constraints.get(stat) {
        Some(StatConstraint::Interval { lo, hi }) => Some((*lo, *hi)),
        _ => None,
    };
    let vs_anchor_value = |stat: &str, summary: &StatSummary| -> Option<f64> {
        match args.constraints.get(stat) {
            Some(StatConstraint::VsAnchor { anchor, bucket }) => {
                let base = anchor_refs
                    .iter()
                    .find(|a| a.id == anchor.as_str())?
                    .stats
                    .get(stat)
                    .copied()?;
                (base.is_finite() && base > 0.0)
                    .then(|| base * summary.bucket_ratios[bucket.index()])
            }
            _ => None,
        }
    };

    // The mass (or other size) stat is resolved first because it positions the quantile predictor.
    let mass_stat = args.config.derive.mass_stat.as_deref();
    let known_mass = mass_stat.and_then(|name| {
        let summary = class.stats.get(name)?;
        match args.constraints.get(name) {
            Some(StatConstraint::Typed { value }) => Some(*value),
            Some(StatConstraint::VsAnchor { .. }) => vs_anchor_value(name, summary),
            Some(StatConstraint::Interval { lo, hi }) => {
                Some(median_predict(summary, Some((*lo, *hi)), wide_for(name)))
            }
            None => None,
        }
    });

    let mut stats: BTreeMap<String, StatEstimate> = BTreeMap::new();
    for (name, summary) in &class.stats {
        let pos = Position {
            x_strength: x_override,
            strength,
            tier: class.key.tier,
            mass: if Some(name.as_str()) == mass_stat {
                None
            } else {
                known_mass
            },
        };
        let traits = args.config.resolve_traits(name, summary.traits);
        let mut value: Option<f64> = None;
        let mut source: Option<Source> = None;
        match args.constraints.get(name) {
            Some(StatConstraint::Typed { value: v }) => {
                value = Some(*v);
                source = Some(Source::Typed);
            }
            Some(StatConstraint::VsAnchor { anchor, .. }) => match vs_anchor_value(name, summary) {
                Some(v) => {
                    value = Some(v);
                    source = Some(Source::Answer);
                }
                None => notes.push(format!(
                    "the answer for {name} names {anchor}, which has no value for it; ignored"
                )),
            },
            _ => {}
        }
        if value.is_none() && !traits.ask {
            let interval = interval_of(name);
            let wide = wide_for(name);
            let (v, s) = if interval.is_some() {
                let v = match summary.predictor {
                    Predictor::Median => median_predict(summary, interval, wide),
                    _ => quantile_predict(summary, &pos, interval, wide),
                };
                (v, Source::Answer)
            } else {
                match summary.predictor {
                    Predictor::Median => (median_predict(summary, None, wide), Source::ClassMedian),
                    Predictor::Quantile => (
                        quantile_predict(summary, &pos, None, wide),
                        Source::Quantile,
                    ),
                    Predictor::Anchor => {
                        match anchor_predict(
                            summary,
                            &anchor_refs,
                            ln_p,
                            args.config.k_anchors,
                            args.config.anchor_eps,
                        ) {
                            Some(v) => (v, Source::Anchor),
                            None => (
                                quantile_predict(summary, &pos, None, wide),
                                Source::Quantile,
                            ),
                        }
                    }
                }
            };
            value = Some(v);
            source = Some(s);
        }
        stats.insert(
            name.clone(),
            StatEstimate {
                value,
                source,
                band: None,
                p10: summary.p10(),
                median: summary.median,
                p90: summary.p90(),
                n: summary.n,
                level: summary.level,
                predictor: summary.predictor,
            },
        );
    }

    // Structural identities.
    for identity in &args.config.identities {
        let Identity::StrengthTimes { stat, other } = identity;
        let other_value = stats.get(other).and_then(|s| s.value);
        if let (Some(entry), Some(o)) = (stats.get_mut(stat), other_value)
            && !matches!(entry.source, Some(Source::Typed | Source::Answer))
        {
            entry.value = Some(strength * o);
            entry.source = Some(Source::Derived);
        }
    }

    // Rounding and bands.
    for (name, entry) in &mut stats {
        let Some(summary) = class.stats.get(name) else {
            continue;
        };
        let traits = args.config.resolve_traits(name, summary.traits);
        if traits.integer
            && entry.source != Some(Source::Typed)
            && let Some(v) = entry.value
        {
            let rounded = v.round();
            entry.value = Some(if summary.min >= 1.0 {
                rounded.max(1.0)
            } else {
                rounded.max(0.0)
            });
        }
        if entry.value.is_some() && entry.source != Some(Source::Typed) {
            entry.band = Some(
                args.bands
                    .and_then(|b| b.get(name))
                    .copied()
                    .unwrap_or_else(Band::fallback),
            );
        }
    }

    Ok(Estimate {
        class_label: class.explanation.clone(),
        level: class.level,
        class_n: class.n,
        ln_strength: ln_p,
        strength,
        strength_percentile: percentile.clamp(0.0, 1.0),
        stats,
        anchors: args.anchors.iter().take(3).cloned().collect(),
        notes,
    })
}

/// Simple mode: the estimate from class statistics and a three position strength choice, with no anchors
/// and no questions. Each stat uses the class median or the quantile predictor (a stat whose selected
/// predictor is the anchor ratio uses the quantile instead, because no anchors are involved). Typed values
/// are passed in `typed` and replace the predictions.
///
/// # Errors
///
/// Returns [`DesignError`] for a non finite typed value or a class without items.
pub fn simple_estimate(
    class: &ClassStats,
    choice: StrengthChoice,
    typed: &BTreeMap<String, f64>,
    config: &BaselineConfig,
    bands: Option<&BandTable>,
) -> DesignResult<Estimate> {
    let constraints: BTreeMap<String, StatConstraint> = typed
        .iter()
        .map(|(k, v)| (k.clone(), StatConstraint::Typed { value: *v }))
        .collect();
    estimate_core(&Core {
        class,
        anchors: &[],
        strength: &StrengthInput::Choice { choice },
        constraints: &constraints,
        config,
        bands,
        wide: None,
    })
}

/// Calibrated mode: the estimate from class statistics, a list of anchors (sorted by ascending distance;
/// they are re-sorted when not) and the answers (a strength and constraints on single stats).
///
/// # Errors
///
/// Returns [`DesignError`] for a non finite strength, interval or typed value or a class without items.
pub fn anchor_baseline(
    anchors: &[Anchor],
    class: &ClassStats,
    input: &AnchorInput,
    config: &BaselineConfig,
    bands: Option<&BandTable>,
) -> DesignResult<Estimate> {
    let mut sorted: Vec<Anchor> = anchors.to_vec();
    sorted.sort_by(|a, b| {
        a.distance
            .total_cmp(&b.distance)
            .then_with(|| a.id.cmp(&b.id))
    });
    estimate_core(&Core {
        class,
        anchors: &sorted,
        strength: &input.strength,
        constraints: &input.constraints,
        config,
        bands,
        wide: None,
    })
}
