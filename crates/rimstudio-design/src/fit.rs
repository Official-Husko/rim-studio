//! The fit meter: how well a proposed item fits its peers.
//!
//! [`score`] compares each stat of a [`Proposal`] with the class statistics and the calibrated error bands
//! (section 9 of the math specification). Per stat it reports the value, the P50 and P80 bands around the
//! predicted value, the rank among the reference items and a [`FitLevel`]: typical (inside P50, green),
//! plausible (inside P80, blue) or unusual (outside, amber). There is deliberately no red level: red is
//! reserved for errors. A separate typicality score from 0 to 100 answers "does this item look like its
//! peers?" with a robust z score per stat; low means unusual, not wrong. The calibration date, the pool size
//! and the notices "bands are rough" (few items in the class) and "bands optimistic: many near twins" come
//! back as data for the UI to word.
//!
//! Typed values are only compared with the bands, never overwritten, and the meter has no state: the UI
//! calls [`score`] after every edit.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::baseline::{Band, BandTable, Estimate};
use crate::classes::numeric::{MAD_SCALE, mad, median};
use crate::classes::{ClassStats, MIN_POOL};
use crate::loo::{CalibrationMetrics, CalibrationMode};

/// Smallest robust scale of the typicality score (in ln units).
pub const MIN_ROBUST_SCALE: f64 = 0.1;
/// Number of robust standard deviations tolerated before the typicality score drops.
pub const TYPICAL_Z: f64 = 1.5;

/// The values of an item being designed, plus the predictions the bands are centred on.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Proposal {
    /// The item's values by stat name.
    pub values: BTreeMap<String, f64>,
    /// Predicted values by stat name (from an [`Estimate`]); a stat without one is centred on the class
    /// median.
    #[serde(default)]
    pub predicted: BTreeMap<String, f64>,
}

impl Proposal {
    /// An empty proposal.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a value.
    #[must_use]
    pub fn with_value(mut self, stat: impl Into<String>, value: f64) -> Self {
        self.values.insert(stat.into(), value);
        self
    }

    /// Adds a prediction.
    #[must_use]
    pub fn with_prediction(mut self, stat: impl Into<String>, value: f64) -> Self {
        self.predicted.insert(stat.into(), value);
        self
    }

    /// Takes the predictions from an estimate (stats without a value are skipped).
    #[must_use]
    pub fn with_estimate(mut self, estimate: &Estimate) -> Self {
        for (name, s) in &estimate.stats {
            if let Some(v) = s.value {
                self.predicted.insert(name.clone(), v);
            }
        }
        self
    }
}

/// How well a stat fits. There is no red level on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FitLevel {
    /// Inside the P50 band (green).
    Typical,
    /// Inside the P80 band (blue).
    Plausible,
    /// Outside the P80 band (amber).
    Unusual,
}

impl FitLevel {
    /// Colour name the UI maps to its tokens.
    #[must_use]
    pub fn colour(self) -> &'static str {
        match self {
            Self::Typical => "green",
            Self::Plausible => "blue",
            Self::Unusual => "amber",
        }
    }
}

/// A closed range of values.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Interval {
    /// Lower end.
    pub low: f64,
    /// Upper end.
    pub high: f64,
}

/// Rank of a value among the reference items of the class ("heavier than 12 of 19").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rank {
    /// Reference items with a smaller value.
    pub below: usize,
    /// Reference items with the same value.
    pub equal: usize,
    /// Reference items with a value.
    pub total: usize,
}

/// Fit of one stat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatFit {
    /// Stat name.
    pub stat: String,
    /// The item's value.
    pub value: f64,
    /// The value the bands are centred on.
    pub predicted: f64,
    /// The P50 band around the prediction.
    pub p50: Interval,
    /// The P80 band around the prediction.
    pub p80: Interval,
    /// Rank among the reference items of the class.
    pub rank: Rank,
    /// Fit level.
    pub level: FitLevel,
    /// The reference value closest to the item's value (offered next to amber stats).
    pub nearest_reference: f64,
    /// The stat had no calibrated band and used the default factors.
    pub default_band: bool,
}

/// Counts of the levels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct FitSummary {
    /// Stats scored.
    pub scored: usize,
    /// Stats at the typical level.
    pub typical: usize,
    /// Stats at the plausible level.
    pub plausible: usize,
    /// Stats at the unusual level.
    pub unusual: usize,
    /// Share of the scored stats inside P80 (typical or plausible); 0 when none were scored.
    pub share_in_p80: f64,
}

/// Notices and provenance shown beside the meter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FitNotices {
    /// Calibration date (milliseconds since the Unix epoch), when known.
    pub calibrated_at_ms: Option<u64>,
    /// Number of reference items behind the bands.
    pub pool_size: usize,
    /// Explanation of the class the item was compared with.
    pub class_label: String,
    /// Number of items in the class.
    pub class_n: usize,
    /// "Bands are rough": the class has fewer than about 15 items.
    pub rough: bool,
    /// "Bands optimistic: many near twins": the twin free run was clearly worse.
    pub optimistic: bool,
}

/// The result of [`score`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FitReport {
    /// One entry per scored stat, by stat name.
    pub per_stat: Vec<StatFit>,
    /// Typicality from 0 to 100, or `None` when no stat could be compared with at least three peers.
    pub typicality: Option<f64>,
    /// Level counts.
    pub summary: FitSummary,
    /// Notices and provenance.
    pub notices: FitNotices,
    /// Stats of the proposal that could not be scored (unknown to the class or not finite).
    pub unscored: Vec<String>,
}

/// Typicality of one value against its peers, in `[0, 1]`.
///
/// With robust `z = (ln x - median(ln peers)) / max(1.4826 * MAD(ln peers), 0.1)` the score is
/// `exp(-0.5 * max(|z| - 1.5, 0)^2)`. Returns `None` for fewer than three positive peers or a non
/// positive value.
#[must_use]
pub fn typicality_of(peers: &[f64], x: f64) -> Option<f64> {
    if !(x.is_finite() && x > 0.0) {
        return None;
    }
    let ln: Vec<f64> = peers
        .iter()
        .copied()
        .filter(|v| v.is_finite() && *v > 0.0)
        .map(f64::ln)
        .collect();
    if ln.len() < MIN_POOL {
        return None;
    }
    let centre = median(&ln)?;
    let scale = (MAD_SCALE * mad(&ln)?).max(MIN_ROBUST_SCALE);
    let z = (x.ln() - centre) / scale;
    let excess = (z.abs() - TYPICAL_Z).max(0.0);
    Some((-0.5 * excess * excess).exp())
}

fn level_of(band: &Band, predicted: f64, value: f64) -> FitLevel {
    let dev = band.deviation(predicted, value);
    let eps = 1e-9;
    if dev <= band.p50.ln() + eps {
        FitLevel::Typical
    } else if dev <= band.p80.ln() + eps {
        FitLevel::Plausible
    } else {
        FitLevel::Unusual
    }
}

fn rank_of(values: &[f64], x: f64) -> Rank {
    let tol = 1e-9 * x.abs().max(1.0);
    Rank {
        below: values.iter().filter(|v| **v < x - tol).count(),
        equal: values.iter().filter(|v| (**v - x).abs() <= tol).count(),
        total: values.len(),
    }
}

fn nearest(values: &[f64], x: f64) -> f64 {
    values
        .iter()
        .copied()
        .min_by(|a, b| (a - x).abs().total_cmp(&(b - x).abs()))
        .unwrap_or(x)
}

/// Scores a proposal against the class statistics with the quiz bands of `metrics`.
#[must_use]
pub fn score(proposal: &Proposal, class: &ClassStats, metrics: &CalibrationMetrics) -> FitReport {
    score_with(proposal, class, metrics, CalibrationMode::Quiz)
}

/// Scores a proposal choosing which calibrated bands to use (simple mode shows wider bands).
#[must_use]
pub fn score_with(
    proposal: &Proposal,
    class: &ClassStats,
    metrics: &CalibrationMetrics,
    mode: CalibrationMode,
) -> FitReport {
    let bands: BandTable = metrics.bands_for(mode);
    let mut per_stat = Vec::new();
    let mut unscored = Vec::new();
    let mut typicality_scores = Vec::new();
    for (stat, value) in &proposal.values {
        let Some(summary) = class.stats.get(stat).filter(|_| value.is_finite()) else {
            unscored.push(stat.clone());
            continue;
        };
        let predicted = proposal
            .predicted
            .get(stat)
            .copied()
            .filter(|p| p.is_finite())
            .unwrap_or(summary.median);
        let (band, default_band) = match bands.get(stat) {
            Some(b) => (*b, false),
            None => (Band::fallback(), true),
        };
        let (l50, h50) = band.interval50(predicted);
        let (l80, h80) = band.interval80(predicted);
        if let Some(t) = typicality_of(&summary.values, *value) {
            typicality_scores.push(t);
        }
        per_stat.push(StatFit {
            stat: stat.clone(),
            value: *value,
            predicted,
            p50: Interval {
                low: l50,
                high: h50,
            },
            p80: Interval {
                low: l80,
                high: h80,
            },
            rank: rank_of(&summary.values, *value),
            level: level_of(&band, predicted, *value),
            nearest_reference: nearest(&summary.values, *value),
            default_band,
        });
    }
    let count = |l: FitLevel| per_stat.iter().filter(|s| s.level == l).count();
    let (typical, plausible, unusual) = (
        count(FitLevel::Typical),
        count(FitLevel::Plausible),
        count(FitLevel::Unusual),
    );
    let scored = per_stat.len();
    let summary = FitSummary {
        scored,
        typical,
        plausible,
        unusual,
        share_in_p80: if scored == 0 {
            0.0
        } else {
            (typical + plausible) as f64 / scored as f64
        },
    };
    let typicality = if typicality_scores.is_empty() {
        None
    } else {
        Some(100.0 * typicality_scores.iter().sum::<f64>() / typicality_scores.len() as f64)
    };
    FitReport {
        per_stat,
        typicality,
        summary,
        notices: FitNotices {
            calibrated_at_ms: metrics.calibrated_at_ms,
            pool_size: metrics.pool_size,
            class_label: class.explanation.clone(),
            class_n: class.n,
            rough: class.is_rough(),
            optimistic: metrics.optimistic_twins(),
        },
        unscored,
    }
}

#[cfg(test)]
mod tests;
