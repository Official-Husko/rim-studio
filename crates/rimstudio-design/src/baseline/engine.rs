//! The fitted baseline model: per stat predictor selection, bucket ratios, anchors and estimation.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{
    Anchor, AnchorRef, BandTable, BaselineConfig, BaselineInput, Core, Estimate, Position,
    StrengthInput, anchor_predict, band_from_residuals, estimate_core, ln_deviation,
    quantile_predict, resolve_strength,
};
use crate::classes::numeric::{finite_sorted, median, std_dev};
use crate::classes::{
    ClassKey, ClassStats, Pool, PoolItem, Predictor, StatSummary, StatTraits,
    default_bucket_ratios, detect_traits, fit_elasticity,
};
use crate::error::{DesignError, DesignResult};

/// Minimum number of nested leave one out samples before a predictor other than the median is trusted.
const MIN_RULE_SAMPLES: usize = 5;

/// Evidence behind the predictor chosen for one stat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleScore {
    /// Number of held out items with a positive value.
    pub n: usize,
    /// Median relative error of the class median.
    pub median_m: Option<f64>,
    /// Median relative error of the quantile predictor.
    pub median_q: Option<f64>,
    /// Median relative error of the anchor predictor.
    pub median_r: Option<f64>,
    /// The predictor chosen (ties resolve in the order M, Q, R).
    pub chosen: Predictor,
}

/// A baseline model fitted on one pool.
///
/// Fitting is deterministic. The model owns its pool, the learned elasticities, the bucket ratios, the
/// per stat predictor choice and the internal (ideal answer) bands, and estimates from them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Model {
    /// Constants the model was fitted with.
    pub config: BaselineConfig,
    /// The reference pool.
    pub pool: Pool,
    /// Spread of ln strength used to normalise anchor distances (at least `min_sd_p`).
    pub sd_ln_p: f64,
    /// Elasticity of every stat with respect to strength.
    pub elasticity: BTreeMap<String, f64>,
    /// Resolved traits of every stat.
    pub traits: BTreeMap<String, StatTraits>,
    /// Learned multipliers of the three buckets of a "compared with the anchor" answer.
    pub bucket_ratios: BTreeMap<String, [f64; 3]>,
    /// Predictor chosen per stat by the nested leave one out.
    pub rules: BTreeMap<String, Predictor>,
    /// Evidence behind each choice.
    pub rule_scores: BTreeMap<String, RuleScore>,
    /// Bands from the nested leave one out residuals of the chosen predictors (ideal class and strength).
    pub internal_bands: BandTable,
    /// All values of every stat in the pool, ascending (the widening fallback of interval answers).
    pub all_values: BTreeMap<String, Vec<f64>>,
}

struct Sample {
    y: f64,
    m: f64,
    q: Option<f64>,
    r: Option<f64>,
}

impl Model {
    /// Fits the model on `pool`.
    #[must_use]
    pub fn fit(pool: Pool, config: BaselineConfig) -> Self {
        let names = pool.stat_names();
        let ln_ps: Vec<f64> = pool.items.iter().map(|i| i.ln_strength).collect();
        let sd_ln_p = std_dev(&ln_ps).unwrap_or(0.0).max(config.min_sd_p);
        let mut elasticity = BTreeMap::new();
        let mut traits = BTreeMap::new();
        let mut all_values = BTreeMap::new();
        for name in &names {
            elasticity.insert(
                name.clone(),
                fit_elasticity(&pool, name, config.derive.ridge_r, None),
            );
            traits.insert(
                name.clone(),
                config.resolve_traits(name, detect_traits(&pool, name)),
            );
            let values: Vec<f64> = pool.items.iter().filter_map(|i| i.stat(name)).collect();
            all_values.insert(name.clone(), finite_sorted(&values));
        }
        let mut model = Self {
            config,
            pool,
            sd_ln_p,
            elasticity,
            traits,
            bucket_ratios: BTreeMap::new(),
            rules: BTreeMap::new(),
            rule_scores: BTreeMap::new(),
            internal_bands: BTreeMap::new(),
            all_values,
        };
        model.bucket_ratios = model.learn_ratios();
        model.select_rules();
        model
    }

    /// Normalised distance between the described target and a pool item.
    #[must_use]
    pub fn distance(&self, key: &ClassKey, ln_p: f64, item: &PoolItem) -> f64 {
        let c = &self.config;
        let mut d2 = 0.0;
        if let Some(role) = &key.role
            && *role != item.role
        {
            d2 += c.w_role * c.w_role;
        }
        if let Some(tier) = key.tier {
            let dt = c.w_tier * f64::from(tier.abs_diff(item.tier));
            d2 += dt * dt;
        }
        if let Some(group) = &key.group
            && item.group.as_deref() != Some(group.as_str())
        {
            d2 += c.w_group * c.w_group;
        }
        let dp = (ln_p - item.ln_strength) / self.sd_ln_p;
        (d2 + dp * dp).sqrt()
    }

    /// Pool positions with their distance, nearest first (ties by position), optionally leaving one out.
    #[must_use]
    pub fn ranked(&self, key: &ClassKey, ln_p: f64, exclude: Option<usize>) -> Vec<(usize, f64)> {
        let mut out: Vec<(usize, f64)> = self
            .pool
            .items
            .iter()
            .enumerate()
            .filter(|(i, _)| Some(*i) != exclude)
            .map(|(i, item)| (i, self.distance(key, ln_p, item)))
            .collect();
        out.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        out
    }

    /// The `limit` nearest reference items as anchors.
    #[must_use]
    pub fn anchors(&self, key: &ClassKey, ln_p: f64, limit: usize) -> Vec<Anchor> {
        self.ranked(key, ln_p, None)
            .into_iter()
            .take(limit)
            .filter_map(|(i, d)| self.pool.item(i).map(|item| Anchor::from_item(item, d)))
            .collect()
    }

    /// Mean ln strength of the peers of `key` (the first non empty level of the chain), the prior of the
    /// comparison dialogue. Falls back to 0 for an empty pool.
    #[must_use]
    pub fn prior_ln_strength(&self, key: &ClassKey) -> f64 {
        let cohort = self.pool.cohort(key, None, None, 1);
        let values: Vec<f64> = cohort
            .members
            .iter()
            .filter_map(|i| self.pool.item(*i))
            .map(|i| i.ln_strength)
            .collect();
        crate::classes::numeric::mean(&values).unwrap_or(0.0)
    }

    /// Class statistics of `key` with the model's predictors, traits and bucket ratios attached.
    #[must_use]
    pub fn class_stats(&self, key: &ClassKey) -> ClassStats {
        let mut class =
            ClassStats::derive_with(&self.pool, key, &self.config.derive, &self.elasticity);
        for (name, summary) in &mut class.stats {
            summary.predictor = self.rules.get(name).copied().unwrap_or(Predictor::Median);
            if let Some(t) = self.traits.get(name) {
                summary.traits = *t;
            }
            summary.bucket_ratios = self
                .bucket_ratios
                .get(name)
                .copied()
                .unwrap_or_else(default_bucket_ratios);
        }
        class
    }

    /// Estimates every stat with the model's internal bands.
    ///
    /// # Errors
    ///
    /// Returns [`DesignError`] for an empty pool or a non finite strength, interval or typed value.
    pub fn estimate(&self, input: &BaselineInput) -> DesignResult<Estimate> {
        self.estimate_with_bands(input, &self.internal_bands)
    }

    /// Estimates every stat with the given bands (for example the noisy answer bands of a calibration).
    ///
    /// # Errors
    ///
    /// Returns [`DesignError`] for an empty pool or a non finite strength, interval or typed value.
    pub fn estimate_with_bands(
        &self,
        input: &BaselineInput,
        bands: &BandTable,
    ) -> DesignResult<Estimate> {
        if self.pool.is_empty() {
            return Err(DesignError::EmptyInput {
                what: "reference items",
            });
        }
        let strength = input.strength.unwrap_or(StrengthInput::Unknown);
        super::check_input(&strength, &input.constraints)?;
        let class = self.class_stats(&input.key);
        let summary = class.strength.as_ref().ok_or(DesignError::EmptyInput {
            what: "reference items",
        })?;
        let (ln_p, _) = resolve_strength(summary, &strength);
        let anchors: Vec<Anchor> = self
            .ranked(&input.key, ln_p, None)
            .into_iter()
            .filter_map(|(i, d)| self.pool.item(i).map(|item| Anchor::from_item(item, d)))
            .collect();
        estimate_core(&Core {
            class: &class,
            anchors: &anchors,
            strength: &strength,
            constraints: &input.constraints,
            config: &self.config,
            bands: Some(bands),
            wide: Some(&self.all_values),
        })
    }

    /// Learns the multipliers of the three answer buckets: each item against its nearest other item.
    fn learn_ratios(&self) -> BTreeMap<String, [f64; 3]> {
        let mut samples: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
        for (t, item) in self.pool.items.iter().enumerate() {
            let key = ClassKey::of_item(item);
            let Some((a, _)) = self
                .ranked(&key, item.ln_strength, Some(t))
                .into_iter()
                .next()
            else {
                continue;
            };
            let Some(anchor) = self.pool.item(a) else {
                continue;
            };
            for (name, y) in &item.stats {
                let (Some(yt), Some(ya)) = (
                    Some(*y).filter(|v| *v > 0.0),
                    anchor.stat(name).filter(|v| *v > 0.0),
                ) else {
                    continue;
                };
                samples
                    .entry(name.as_str())
                    .or_default()
                    .push((yt / ya).ln());
            }
        }
        let edge = self.config.bucket_edge;
        let mut out = BTreeMap::new();
        for (name, ratios) in samples {
            let mut result = default_bucket_ratios();
            let buckets: [Vec<f64>; 3] = [
                ratios.iter().copied().filter(|r| *r < -edge).collect(),
                ratios
                    .iter()
                    .copied()
                    .filter(|r| *r >= -edge && *r <= edge)
                    .collect(),
                ratios.iter().copied().filter(|r| *r > edge).collect(),
            ];
            for (slot, bucket) in result.iter_mut().zip(&buckets) {
                if bucket.len() >= 2
                    && let Some(m) = median(bucket)
                {
                    *slot = m.exp();
                }
            }
            out.insert(name.to_string(), result);
        }
        out
    }

    /// Nested leave one out inside the pool: for each stat compares M, Q and R with the true axes of each
    /// held out item known, keeps the lowest median relative error (ties M, Q, R) and records the
    /// residuals of the chosen predictor as the internal bands.
    fn select_rules(&mut self) {
        let names = self.pool.stat_names();
        let mut samples: BTreeMap<String, Vec<Sample>> = BTreeMap::new();
        for (t, item) in self.pool.items.iter().enumerate() {
            let key = ClassKey::of_item(item);
            let ranked = self.ranked(&key, item.ln_strength, Some(t));
            let refs: Vec<AnchorRef<'_>> = ranked
                .iter()
                .filter_map(|(i, d)| {
                    self.pool.item(*i).map(|a| AnchorRef {
                        id: a.id.as_str(),
                        ln_strength: a.ln_strength,
                        distance: *d,
                        stats: &a.stats,
                    })
                })
                .collect();
            for name in &names {
                let Some(y) = item.stat(name).filter(|v| *v >= 0.0) else {
                    continue;
                };
                let elasticity = self.elasticity.get(name).copied().unwrap_or(0.0);
                let Some(mut summary) = StatSummary::derive(
                    &self.pool,
                    &key,
                    name,
                    Some(t),
                    &self.config.derive,
                    elasticity,
                ) else {
                    continue;
                };
                summary.traits = self.traits.get(name).copied().unwrap_or_default();
                let pos = Position {
                    x_strength: None,
                    strength: item.strength,
                    tier: Some(item.tier),
                    mass: None,
                };
                let q = Some(quantile_predict(&summary, &pos, None, &[]));
                let r = anchor_predict(
                    &summary,
                    &refs,
                    item.ln_strength,
                    self.config.k_anchors,
                    self.config.anchor_eps,
                );
                samples.entry(name.clone()).or_default().push(Sample {
                    y,
                    m: summary.median,
                    q,
                    r,
                });
            }
        }
        for name in &names {
            let list = samples.remove(name).unwrap_or_default();
            let rel = |pick: &dyn Fn(&Sample) -> Option<f64>| -> Vec<f64> {
                list.iter()
                    .filter(|s| s.y > 0.0)
                    .filter_map(|s| pick(s).map(|p| (p - s.y).abs() / s.y))
                    .collect()
            };
            let em = rel(&|s| Some(s.m));
            let eq = rel(&|s| s.q);
            let er = rel(&|s| s.r);
            let (mm, mq, mr) = (median(&em), median(&eq), median(&er));
            let n = em.len();
            let mut chosen = Predictor::Median;
            if n >= MIN_RULE_SAMPLES {
                let mut best = mm.unwrap_or(f64::INFINITY);
                if let Some(v) = mq.filter(|v| *v < best && eq.len() >= MIN_RULE_SAMPLES) {
                    best = v;
                    chosen = Predictor::Quantile;
                }
                if mr.is_some_and(|v| v < best) && er.len() >= MIN_RULE_SAMPLES {
                    chosen = Predictor::Anchor;
                }
            }
            self.rules.insert(name.clone(), chosen);
            self.rule_scores.insert(
                name.clone(),
                RuleScore {
                    n,
                    median_m: mm,
                    median_q: mq,
                    median_r: mr,
                    chosen,
                },
            );
            // Internal band from the residuals of the chosen predictor.
            let shift = band_shift(self.all_values.get(name).map_or(&[][..], Vec::as_slice));
            let residuals: Vec<f64> = list
                .iter()
                .filter_map(|s| {
                    let p = match chosen {
                        Predictor::Median => Some(s.m),
                        Predictor::Quantile => s.q,
                        Predictor::Anchor => s.r.or(s.q),
                    }?;
                    Some(ln_deviation(p, s.y, shift))
                })
                .collect();
            self.internal_bands
                .insert(name.clone(), band_from_residuals(&residuals, shift));
        }
    }

    /// True when the pool is too small for the quiz (fewer than about 8 reference items).
    #[must_use]
    pub fn quiz_available(&self) -> bool {
        self.pool.len() >= MIN_QUIZ_ITEMS
    }
}

/// Fewer reference items than this and the quiz is hidden (simple mode only).
pub const MIN_QUIZ_ITEMS: usize = 8;

/// Shift of the log scale for a stat that can be zero: 5 percent of the median positive value, else 0.
#[must_use]
pub(crate) fn band_shift(sorted_values: &[f64]) -> f64 {
    if sorted_values.iter().all(|v| *v > 0.0) {
        return 0.0;
    }
    let positive: Vec<f64> = sorted_values.iter().copied().filter(|v| *v > 0.0).collect();
    median(&positive).map_or(1e-6, |m| 0.05 * m)
}
