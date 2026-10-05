//! The leave one out calibration harness.
//!
//! [`validate`] holds out each reference item of a pool in turn, fits a [`Model`] on the rest, simulates the
//! answers a modder who knows the item would give (tier and role exact; comparisons by the true strength
//! with a same band and optional noise; interval and bucket answers from the true values, optionally
//! dropped or one bin off), predicts every stat and records the relative error. Several answer profiles run:
//! ideal answers, noisy answers (the source of the stored bands, because ideal answers are over confident),
//! simple mode answers, a stress case with a wide same band, dropped answers, and the class median as the
//! baseline arm. A twin free rerun, with near copies of the held out item removed from training, detects
//! pools whose bands are flattered by sibling items.
//!
//! The output, [`CalibrationMetrics`], feeds the fit meter: per stat residual factors (P50 and P80), error
//! summaries, coverage, rank correlation, question counts, the pool size and the calibration date. The job is
//! deterministic: each item uses the generator seed `1000 + index` plus a per replicate offset, work is split
//! into contiguous chunks per thread and joined in item order, so the result is byte identical for any
//! thread count.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::baseline::Bucket;
use crate::baseline::{
    BandTable, BaselineConfig, BaselineInput, Estimate, Model, StrengthChoice, StrengthInput,
    band_from_residuals, band_shift, ln_deviation,
};
use crate::classes::numeric::{SplitMix64, finite_sorted, median, quantile_sorted, spearman};
use crate::classes::{ClassKey, ItemKind, Pool, PoolItem};
use crate::error::{DesignError, DesignResult};
use crate::quiz::{Answer, Question, Quiz, QuizConfig};

/// Version of the harness; part of the calibration cache key.
pub const HARNESS_VERSION: u32 = 1;
/// Seed of item `i` is `SEED_BASE + i` plus a replicate offset.
pub const SEED_BASE: u64 = 1000;
/// Offset per replicate added to the seed.
pub const REPLICATE_STRIDE: u64 = 7919;
/// A pool whose twin free error exceeds the plain error by more than this is flagged optimistic.
pub const TWIN_GAP: f64 = 0.05;
/// Fewest reference items the harness accepts.
pub const MIN_ITEMS: usize = 4;

/// What the simulated modder does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileMode {
    /// The full quiz.
    Quiz,
    /// Simple mode: tier, role and a three position strength choice, no quiz.
    Simple,
    /// The class median of the true role and tier (the baseline arm).
    ClassMedian,
}

/// A simulated modder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnswerProfile {
    /// Name of the variant in the metrics.
    pub name: String,
    /// What the modder does.
    pub mode: ProfileMode,
    /// Standard deviation (ln units) of the perceived strength in every comparison.
    pub comparison_sigma: f64,
    /// Standard deviation (ln units) of the perceived ratio in every anchor question.
    pub bucket_sigma: f64,
    /// "About the same" band of the comparisons (ln units).
    pub same_band: f64,
    /// Probability that a question other than the setup taps is answered "not sure" (a dropped comparison
    /// ends the dialogue with the bracket found so far).
    pub drop_rate: f64,
    /// Probability that an interval answer is one bin off.
    pub bin_error_rate: f64,
    /// Number of replicates per item (1 for deterministic answers).
    pub replicates: usize,
}

impl AnswerProfile {
    /// Ideal answers: exact, same band 10 percent.
    #[must_use]
    pub fn ideal() -> Self {
        Self {
            name: "ideal".into(),
            mode: ProfileMode::Quiz,
            comparison_sigma: 0.0,
            bucket_sigma: 0.0,
            same_band: 0.10,
            drop_rate: 0.0,
            bin_error_rate: 0.0,
            replicates: 1,
        }
    }

    /// Noisy answers: normal error with sd 0.2 on every comparison and 0.1 on every bucket answer, ten
    /// replicates per item. The stored bands come from this profile.
    #[must_use]
    pub fn noisy() -> Self {
        Self {
            name: "noisy".into(),
            comparison_sigma: 0.2,
            bucket_sigma: 0.1,
            replicates: 10,
            ..Self::ideal()
        }
    }

    /// The stress case: noisy answers with a 25 percent same band.
    #[must_use]
    pub fn noisy_wide() -> Self {
        Self {
            name: "noisy-wide".into(),
            same_band: 0.25,
            ..Self::noisy()
        }
    }

    /// Careless answers: a share `rate` of the comparisons and refine questions dropped.
    #[must_use]
    pub fn dropped(rate: f64) -> Self {
        Self {
            name: "dropped".into(),
            drop_rate: rate,
            ..Self::ideal()
        }
    }

    /// Simple mode answers.
    #[must_use]
    pub fn simple() -> Self {
        Self {
            name: "simple".into(),
            mode: ProfileMode::Simple,
            ..Self::ideal()
        }
    }

    /// The class median baseline arm.
    #[must_use]
    pub fn class_median() -> Self {
        Self {
            name: "class-median".into(),
            mode: ProfileMode::ClassMedian,
            ..Self::ideal()
        }
    }

    /// The default set: ideal, noisy, noisy-wide, simple and the class median baseline.
    #[must_use]
    pub fn standard() -> Vec<Self> {
        vec![
            Self::ideal(),
            Self::noisy(),
            Self::noisy_wide(),
            Self::simple(),
            Self::class_median(),
        ]
    }
}

/// Options of [`validate`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidateOptions {
    /// The profiles to run.
    pub profiles: Vec<AnswerProfile>,
    /// Quiz configuration; `None` uses the default of the pool's kind.
    pub quiz: Option<QuizConfig>,
    /// Number of worker threads (at least 1). The result does not depend on it.
    pub threads: usize,
    /// Calibration date to record (milliseconds since the Unix epoch); the crate has no clock.
    pub now_unix_ms: Option<u64>,
    /// Run the twin free check.
    pub twin_check: bool,
    /// Stat compared to decide whether two items are twins (`mass` by default); strength alone decides when
    /// an item lacks it.
    pub twin_stat: Option<String>,
    /// Relative tolerance of the twin test on strength (ln units) and on the twin stat.
    pub twin_tolerance: f64,
}

impl Default for ValidateOptions {
    fn default() -> Self {
        Self {
            profiles: AnswerProfile::standard(),
            quiz: None,
            threads: 1,
            now_unix_ms: None,
            twin_check: true,
            twin_stat: Some("mass".into()),
            twin_tolerance: 0.05,
        }
    }
}

/// How a calibrated estimate is used, to pick the matching bands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationMode {
    /// Simple mode (no comparisons).
    Simple,
    /// The quiz.
    Quiz,
}

/// Error summary of one stat under one profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatMetrics {
    /// Number of simulated predictions behind the numbers.
    pub n: usize,
    /// Median relative error (items with a positive true value).
    pub median_error: Option<f64>,
    /// 80th percentile of the relative error.
    pub p80_error: Option<f64>,
    /// Mean relative error.
    pub mean_error: Option<f64>,
    /// Multiplicative residual factor holding half of the predictions.
    pub factor_p50: f64,
    /// Multiplicative residual factor holding four fifths of the predictions.
    pub factor_p80: f64,
    /// Shift of the log scale for stats that can be zero (0 otherwise).
    pub shift: f64,
    /// Share of predictions whose residual is within the P50 factor built from the other predictions.
    pub coverage_p50: f64,
    /// Share within the P80 factor built from the other predictions.
    pub coverage_p80: f64,
    /// Spearman correlation between predicted and true values.
    pub rank_correlation: Option<f64>,
    /// Median relative error of the class median baseline for the same stat, when that arm ran.
    pub baseline_median_error: Option<f64>,
}

/// Metrics of one profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariantMetrics {
    /// Name of the profile.
    pub name: String,
    /// Replicates per item.
    pub replicates: usize,
    /// Per stat metrics.
    pub per_stat: BTreeMap<String, StatMetrics>,
    /// Median over stats of the median relative error.
    pub macro_median_error: Option<f64>,
    /// Mean over stats of the median relative error.
    pub macro_mean_error: Option<f64>,
    /// Mean number of questions asked.
    pub mean_questions: f64,
    /// Most questions asked.
    pub max_questions: usize,
}

impl VariantMetrics {
    /// The bands of this profile as a table the estimator accepts.
    #[must_use]
    pub fn band_table(&self) -> BandTable {
        self.per_stat
            .iter()
            .map(|(name, m)| {
                (
                    name.clone(),
                    crate::baseline::Band::new(m.factor_p50, m.factor_p80, m.shift),
                )
            })
            .collect()
    }
}

/// Result of the twin free check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TwinCheck {
    /// Macro median error of the ideal profile with the plain training sets.
    pub plain_error: f64,
    /// Macro median error with near copies of the held out item removed.
    pub twin_free_error: f64,
    /// `twin_free_error - plain_error`.
    pub gap: f64,
    /// True when the gap exceeds [`TWIN_GAP`]: "bands optimistic: many near twins".
    pub optimistic: bool,
}

/// Everything the fit meter needs from a calibration run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationMetrics {
    /// Harness version.
    pub harness_version: u32,
    /// Kind of the pool.
    pub kind: ItemKind,
    /// Number of reference items.
    pub pool_size: usize,
    /// Calibration date, when the caller supplied one.
    pub calibrated_at_ms: Option<u64>,
    /// Metrics by profile name.
    pub variants: BTreeMap<String, VariantMetrics>,
    /// Twin free check, when it ran.
    pub twin: Option<TwinCheck>,
}

impl CalibrationMetrics {
    /// Bands of a named profile.
    #[must_use]
    pub fn bands(&self, variant: &str) -> Option<BandTable> {
        self.variants.get(variant).map(VariantMetrics::band_table)
    }

    /// The bands to show for an estimate: the noisy run for the quiz (ideal answers are over confident,
    /// so the ideal run is only a fallback), the simple run for simple mode. Empty when nothing ran.
    #[must_use]
    pub fn bands_for(&self, mode: CalibrationMode) -> BandTable {
        let order: &[&str] = match mode {
            CalibrationMode::Quiz => &["noisy", "noisy-wide", "ideal", "simple"],
            CalibrationMode::Simple => &["simple", "noisy", "ideal"],
        };
        order.iter().find_map(|n| self.bands(n)).unwrap_or_default()
    }

    /// True when the twin free check found the bands flattered by near copies.
    #[must_use]
    pub fn optimistic_twins(&self) -> bool {
        self.twin.as_ref().is_some_and(|t| t.optimistic)
    }
}

// ---------------------------------------------------------------------------------------------------
// Simulation
// ---------------------------------------------------------------------------------------------------

/// Result of simulating one item under one profile replicate.
#[derive(Debug, Clone, PartialEq)]
pub struct Simulated {
    /// Predicted values by stat (only stats with a value).
    pub values: BTreeMap<String, f64>,
    /// Number of questions asked.
    pub questions: usize,
    /// The full estimate for quiz and simple profiles.
    pub estimate: Option<Estimate>,
}

fn bin_index(bins: &[crate::quiz::Bin], v: f64) -> usize {
    bins.iter()
        .position(|b| b.lo.is_none_or(|l| v >= l) && b.hi.is_none_or(|h| v < h))
        .unwrap_or(0)
}

/// Simulates a modder who knows `item` answering with `profile` against a model fitted on the other items.
///
/// # Errors
///
/// A [`DesignError`] from the estimation (an empty model).
pub fn simulate_item(
    model: &Model,
    item: &PoolItem,
    profile: &AnswerProfile,
    quiz_config: &QuizConfig,
    rng: &mut SplitMix64,
) -> DesignResult<Simulated> {
    let key = ClassKey {
        role: Some(item.role.clone()),
        tier: Some(item.tier),
        group: None,
    };
    match profile.mode {
        ProfileMode::ClassMedian => {
            let class = model.class_stats(&key);
            let values = class
                .stats
                .iter()
                .map(|(n, s)| (n.clone(), s.median))
                .collect();
            Ok(Simulated {
                values,
                questions: 2,
                estimate: None,
            })
        }
        ProfileMode::Simple => {
            let class = model.class_stats(&key);
            let mid = class.strength.as_ref().map_or(0.5, |s| {
                crate::classes::numeric::mid_rank_of(&s.values, item.strength)
            });
            let choice = if mid < 1.0 / 3.0 {
                StrengthChoice::Weaker
            } else if mid < 2.0 / 3.0 {
                StrengthChoice::Typical
            } else {
                StrengthChoice::Stronger
            };
            let input = BaselineInput {
                key,
                strength: Some(StrengthInput::Choice { choice }),
                constraints: BTreeMap::new(),
            };
            let estimate = model.estimate(&input)?;
            let values = estimate
                .stats
                .iter()
                .filter_map(|(n, s)| s.value.map(|v| (n.clone(), v)))
                .collect();
            Ok(Simulated {
                values,
                questions: 3,
                estimate: Some(estimate),
            })
        }
        ProfileMode::Quiz => {
            let edge = model.config.bucket_edge;
            let mut quiz = Quiz::new(model.pool.kind, quiz_config.clone());
            let result = quiz.play(model, |q| {
                let perceived = |rng: &mut SplitMix64| {
                    item.ln_strength
                        + if profile.comparison_sigma > 0.0 {
                            profile.comparison_sigma * rng.next_normal()
                        } else {
                            0.0
                        }
                };
                let dropped = |rng: &mut SplitMix64| {
                    profile.drop_rate > 0.0 && rng.next_f64() < profile.drop_rate
                };
                match q {
                    // A tier or role that only the held out item has is not a choice in the training
                    // pool; a modder facing the question would say "not sure".
                    Question::Tier { options } if options.iter().any(|o| o.tier == item.tier) => {
                        Answer::Tier { tier: item.tier }
                    }
                    Question::Role { options } if options.iter().any(|o| o.name == item.role) => {
                        Answer::Role {
                            role: item.role.clone(),
                        }
                    }
                    Question::Tier { .. } | Question::Role { .. } => Answer::NotSure,
                    Question::Group { .. } => match (&item.group, dropped(rng)) {
                        (Some(g), false) => Answer::Group { group: g.clone() },
                        _ => Answer::NotSure,
                    },
                    Question::Compare { anchor, .. } => {
                        if dropped(rng) {
                            return Answer::NotSure;
                        }
                        let p = perceived(rng);
                        let d = p - anchor.strength.ln();
                        if d.abs() < profile.same_band {
                            Answer::Same
                        } else if d > 0.0 {
                            Answer::Stronger
                        } else {
                            Answer::Weaker
                        }
                    }
                    Question::CloserTo { lower, upper } => {
                        if dropped(rng) {
                            return Answer::NotSure;
                        }
                        let p = perceived(rng);
                        if p - lower.strength.ln() < upper.strength.ln() - p {
                            Answer::CloserToLower
                        } else {
                            Answer::CloserToUpper
                        }
                    }
                    Question::Interval { stat, bins, .. } => {
                        let Some(v) = item.stat(stat) else {
                            return Answer::NotSure;
                        };
                        if dropped(rng) {
                            return Answer::NotSure;
                        }
                        let mut idx = bin_index(bins, v);
                        if profile.bin_error_rate > 0.0 && rng.next_f64() < profile.bin_error_rate {
                            idx = if rng.next_f64() < 0.5 {
                                idx.saturating_sub(1)
                            } else {
                                (idx + 1).min(bins.len().saturating_sub(1))
                            };
                        }
                        Answer::Bin { index: idx }
                    }
                    Question::VsAnchor { stat, value, .. } => {
                        let Some(v) = item.stat(stat).filter(|v| *v > 0.0) else {
                            return Answer::NotSure;
                        };
                        if dropped(rng) {
                            return Answer::NotSure;
                        }
                        let noise = if profile.bucket_sigma > 0.0 {
                            profile.bucket_sigma * rng.next_normal()
                        } else {
                            0.0
                        };
                        let r = (v / value).ln() + noise;
                        Answer::Bucket {
                            bucket: if r < -edge {
                                Bucket::Lower
                            } else if r > edge {
                                Bucket::Higher
                            } else {
                                Bucket::Similar
                            },
                        }
                    }
                }
            });
            // An oracle that produces an invalid answer would be a harness bug; surface it as a design error.
            result.map_err(|_| DesignError::Degenerate {
                what: "the simulated modder gave an invalid answer",
            })?;
            let estimate = quiz
                .estimate(model, None)
                .map_err(|_| DesignError::Degenerate {
                    what: "the simulated quiz could not be estimated",
                })?;
            let values = estimate
                .stats
                .iter()
                .filter_map(|(n, s)| s.value.map(|v| (n.clone(), v)))
                .collect();
            Ok(Simulated {
                values,
                questions: quiz.answers.len(),
                estimate: Some(estimate),
            })
        }
    }
}

struct Row {
    stat: String,
    pred: f64,
    truth: f64,
}

struct ProfileOutcome {
    rows: Vec<Row>,
    questions: Vec<usize>,
}

struct ItemOutcome {
    profiles: Vec<ProfileOutcome>,
    twin_free: Vec<Row>,
}

fn rows_of(item: &PoolItem, sim: &Simulated) -> Vec<Row> {
    let mut rows = Vec::new();
    for (stat, truth) in &item.stats {
        if !truth.is_finite() || *truth < 0.0 {
            continue;
        }
        if let Some(pred) = sim.values.get(stat).copied().filter(|v| v.is_finite()) {
            rows.push(Row {
                stat: stat.clone(),
                pred,
                truth: *truth,
            });
        }
    }
    rows
}

fn is_twin(a: &PoolItem, b: &PoolItem, options: &ValidateOptions) -> bool {
    if a.role != b.role
        || a.tier != b.tier
        || (a.ln_strength - b.ln_strength).abs() >= options.twin_tolerance
    {
        return false;
    }
    match options
        .twin_stat
        .as_deref()
        .and_then(|s| a.stat(s).zip(b.stat(s)))
    {
        Some((x, y)) => (x - y).abs() <= options.twin_tolerance * x.abs(),
        None => true,
    }
}

fn run_item(
    pool: &Pool,
    index: usize,
    config: &BaselineConfig,
    options: &ValidateOptions,
    quiz_config: &QuizConfig,
) -> DesignResult<ItemOutcome> {
    let Some(item) = pool.item(index) else {
        return Ok(ItemOutcome {
            profiles: Vec::new(),
            twin_free: Vec::new(),
        });
    };
    let model = Model::fit(pool.without(&[index]), config.clone());
    let mut profiles = Vec::with_capacity(options.profiles.len());
    for profile in &options.profiles {
        let mut rows = Vec::new();
        let mut questions = Vec::new();
        for rep in 0..profile.replicates.max(1) {
            let mut rng = SplitMix64::new(SEED_BASE + index as u64 + rep as u64 * REPLICATE_STRIDE);
            let sim = simulate_item(&model, item, profile, quiz_config, &mut rng)?;
            questions.push(sim.questions);
            rows.extend(rows_of(item, &sim));
        }
        profiles.push(ProfileOutcome { rows, questions });
    }
    let mut twin_free = Vec::new();
    if options.twin_check {
        let ideal = options
            .profiles
            .iter()
            .find(|p| p.name == "ideal")
            .or_else(|| {
                options
                    .profiles
                    .iter()
                    .find(|p| p.mode == ProfileMode::Quiz)
            });
        if let Some(profile) = ideal {
            let mut drop = vec![index];
            for (j, other) in pool.items.iter().enumerate() {
                if j != index && is_twin(item, other, options) {
                    drop.push(j);
                }
            }
            let model = Model::fit(pool.without(&drop), config.clone());
            let mut rng = SplitMix64::new(SEED_BASE + index as u64);
            let sim = simulate_item(&model, item, profile, quiz_config, &mut rng)?;
            twin_free = rows_of(item, &sim);
        }
    }
    Ok(ItemOutcome {
        profiles,
        twin_free,
    })
}

fn macro_errors(per_stat: &BTreeMap<String, StatMetrics>) -> (Option<f64>, Option<f64>) {
    let errs: Vec<f64> = per_stat.values().filter_map(|m| m.median_error).collect();
    let mean = if errs.is_empty() {
        None
    } else {
        Some(errs.iter().sum::<f64>() / errs.len() as f64)
    };
    (median(&errs), mean)
}

fn stat_metrics(rows: &[&Row], shift: f64) -> StatMetrics {
    let rel: Vec<f64> = rows
        .iter()
        .filter(|r| r.truth > 0.0)
        .map(|r| (r.pred - r.truth).abs() / r.truth)
        .collect();
    let rel_sorted = finite_sorted(&rel);
    let resid: Vec<f64> = rows
        .iter()
        .map(|r| ln_deviation(r.pred, r.truth, shift))
        .collect();
    let band = band_from_residuals(&resid, shift);
    let (mut c50, mut c80) = (0usize, 0usize);
    for (k, r) in resid.iter().enumerate() {
        let others: Vec<f64> = resid
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != k)
            .map(|(_, v)| *v)
            .collect();
        let o = finite_sorted(&others);
        if quantile_sorted(&o, 0.5).is_some_and(|q| *r <= q) {
            c50 += 1;
        }
        if quantile_sorted(&o, 0.8).is_some_and(|q| *r <= q) {
            c80 += 1;
        }
    }
    let n = rows.len();
    let preds: Vec<f64> = rows.iter().map(|r| r.pred).collect();
    let truths: Vec<f64> = rows.iter().map(|r| r.truth).collect();
    StatMetrics {
        n,
        median_error: quantile_sorted(&rel_sorted, 0.5),
        p80_error: quantile_sorted(&rel_sorted, 0.8),
        mean_error: if rel.is_empty() {
            None
        } else {
            Some(rel.iter().sum::<f64>() / rel.len() as f64)
        },
        factor_p50: band.p50,
        factor_p80: band.p80,
        shift,
        coverage_p50: if n == 0 { 0.0 } else { c50 as f64 / n as f64 },
        coverage_p80: if n == 0 { 0.0 } else { c80 as f64 / n as f64 },
        rank_correlation: spearman(&preds, &truths),
        baseline_median_error: None,
    }
}

fn per_stat_metrics(rows: &[Row], shifts: &BTreeMap<String, f64>) -> BTreeMap<String, StatMetrics> {
    let mut by_stat: BTreeMap<&str, Vec<&Row>> = BTreeMap::new();
    for r in rows {
        by_stat.entry(r.stat.as_str()).or_default().push(r);
    }
    by_stat
        .into_iter()
        .map(|(stat, rows)| {
            (
                stat.to_string(),
                stat_metrics(&rows, shifts.get(stat).copied().unwrap_or(0.0)),
            )
        })
        .collect()
}

/// Runs the leave one out harness over a pool.
///
/// # Errors
///
/// [`DesignError::Degenerate`] for fewer than [`MIN_ITEMS`] items, or an error of a simulated quiz.
pub fn validate(
    pool: &Pool,
    config: &BaselineConfig,
    options: &ValidateOptions,
) -> DesignResult<CalibrationMetrics> {
    if pool.len() < MIN_ITEMS {
        return Err(DesignError::Degenerate {
            what: "fewer than four reference items for a leave one out run",
        });
    }
    let quiz_config = options
        .quiz
        .clone()
        .unwrap_or_else(|| QuizConfig::for_kind(pool.kind));
    let n = pool.len();
    let threads = options.threads.clamp(1, n);
    let chunk = n.div_ceil(threads);
    let mut outcomes: Vec<Option<DesignResult<ItemOutcome>>> = (0..n).map(|_| None).collect();
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for start in (0..n).step_by(chunk) {
            let end = (start + chunk).min(n);
            let quiz_config = &quiz_config;
            handles.push(scope.spawn(move || {
                (start..end)
                    .map(|i| (i, run_item(pool, i, config, options, quiz_config)))
                    .collect::<Vec<_>>()
            }));
        }
        for h in handles {
            if let Ok(done) = h.join() {
                for (i, r) in done {
                    if let Some(slot) = outcomes.get_mut(i) {
                        *slot = Some(r);
                    }
                }
            }
        }
    });
    let mut items = Vec::with_capacity(n);
    for o in outcomes {
        items.push(o.ok_or(DesignError::Degenerate {
            what: "a calibration worker failed",
        })??);
    }

    let mut shifts = BTreeMap::new();
    for name in pool.stat_names() {
        let values = finite_sorted(
            &pool
                .items
                .iter()
                .filter_map(|i| i.stat(&name))
                .collect::<Vec<_>>(),
        );
        shifts.insert(name, band_shift(&values));
    }

    let mut variants = BTreeMap::new();
    for (pi, profile) in options.profiles.iter().enumerate() {
        let mut rows: Vec<Row> = Vec::new();
        let mut questions: Vec<usize> = Vec::new();
        for item in &mut items {
            if let Some(po) = item.profiles.get_mut(pi) {
                rows.append(&mut po.rows);
                questions.append(&mut po.questions);
            }
        }
        let per_stat = per_stat_metrics(&rows, &shifts);
        let (macro_median_error, macro_mean_error) = macro_errors(&per_stat);
        let mean_questions = if questions.is_empty() {
            0.0
        } else {
            questions.iter().sum::<usize>() as f64 / questions.len() as f64
        };
        variants.insert(
            profile.name.clone(),
            VariantMetrics {
                name: profile.name.clone(),
                replicates: profile.replicates.max(1),
                per_stat,
                macro_median_error,
                macro_mean_error,
                mean_questions,
                max_questions: questions.iter().copied().max().unwrap_or(0),
            },
        );
    }
    // Fill the baseline arm into every other variant.
    let baseline: Option<BTreeMap<String, Option<f64>>> = options
        .profiles
        .iter()
        .find(|p| p.mode == ProfileMode::ClassMedian)
        .and_then(|p| variants.get(&p.name))
        .map(|v| {
            v.per_stat
                .iter()
                .map(|(k, m)| (k.clone(), m.median_error))
                .collect()
        });
    if let Some(base) = baseline {
        for v in variants.values_mut() {
            for (stat, m) in &mut v.per_stat {
                m.baseline_median_error = base.get(stat).copied().flatten();
            }
        }
    }

    let twin = if options.twin_check {
        let mut rows: Vec<Row> = Vec::new();
        for item in &mut items {
            rows.append(&mut item.twin_free);
        }
        let twin_free_error = macro_errors(&per_stat_metrics(&rows, &shifts)).0;
        let ideal_name = options
            .profiles
            .iter()
            .find(|p| p.name == "ideal")
            .or_else(|| {
                options
                    .profiles
                    .iter()
                    .find(|p| p.mode == ProfileMode::Quiz)
            })
            .map(|p| p.name.clone());
        let plain = ideal_name
            .and_then(|n| variants.get(&n))
            .and_then(|v| v.macro_median_error);
        match (plain, twin_free_error) {
            (Some(plain_error), Some(twin_free_error)) => {
                let gap = twin_free_error - plain_error;
                Some(TwinCheck {
                    plain_error,
                    twin_free_error,
                    gap,
                    optimistic: gap > TWIN_GAP,
                })
            }
            _ => None,
        }
    } else {
        None
    };

    Ok(CalibrationMetrics {
        harness_version: HARNESS_VERSION,
        kind: pool.kind,
        pool_size: pool.len(),
        calibrated_at_ms: options.now_unix_ms,
        variants,
        twin,
    })
}

#[cfg(test)]
mod tests;
