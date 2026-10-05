//! The check at the end: what the fit meter says about the proposed numbers.
//!
//! The meter compares the numbers with the estimate for the class and the balance target, with the bands of
//! the calibration when there is one and the documented defaults otherwise. A proposal is called typical
//! when at most a tenth of its scored stats are unusual and at least half are typical, plausible when at
//! most a quarter are unusual.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::baseline::{BaselineInput, Model, StrengthInput};
use crate::classes::ClassKey;
use crate::error::DesignResult;
use crate::fit::{FitReport, Proposal as FitProposal, score_with};
use crate::loo::{CalibrationMetrics, CalibrationMode, HARNESS_VERSION};

/// Stats that say nothing about strength and are not scored.
const UNSCORED: [&str; 1] = ["tools"];

/// Stats the designer asks for instead of predicting: they are shown on the meter but do not decide the
/// verdict (the work to make is an input, and the market value follows from it).
pub const ASKED: [&str; 2] = ["work", "market_value"];

/// The overall verdict on a proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// At most a tenth of the scored stats are unusual and at least half are typical.
    Typical,
    /// At most a quarter of the scored stats are unusual.
    Plausible,
    /// More than a quarter are unusual.
    Unusual,
}

/// The overall verdict of a fit report.
#[must_use]
pub fn verdict(report: &FitReport) -> Verdict {
    let counted: Vec<&crate::fit::StatFit> = report
        .per_stat
        .iter()
        .filter(|f| !ASKED.contains(&f.stat.as_str()))
        .collect();
    if counted.is_empty() {
        return Verdict::Plausible;
    }
    let n = counted.len() as f64;
    let count =
        |level: crate::fit::FitLevel| counted.iter().filter(|f| f.level == level).count() as f64;
    let unusual = count(crate::fit::FitLevel::Unusual) / n;
    let typical = count(crate::fit::FitLevel::Typical) / n;
    if unusual <= 0.10 && typical >= 0.5 {
        Verdict::Typical
    } else if unusual <= 0.25 {
        Verdict::Plausible
    } else {
        Verdict::Unusual
    }
}

/// Scores the stats of a proposal against the class for `balance`.
///
/// # Errors
///
/// [`crate::error::DesignError`] from the estimate (an empty pool or an invalid percentile).
pub fn fit_of(
    model: &Model,
    key: &ClassKey,
    strength: StrengthInput,
    stats: &BTreeMap<String, f64>,
    metrics: Option<&CalibrationMetrics>,
) -> DesignResult<FitReport> {
    let estimate = model.estimate(&BaselineInput {
        key: key.clone(),
        strength: Some(strength),
        constraints: BTreeMap::new(),
    })?;
    let class = model.class_stats(key);
    let known = model.pool.stat_names();
    let values: BTreeMap<String, f64> = stats
        .iter()
        .filter(|(k, _)| known.iter().any(|n| n == *k) && !UNSCORED.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    let proposal = FitProposal {
        values,
        predicted: BTreeMap::new(),
    }
    .with_estimate(&estimate);
    let default_metrics;
    let metrics = match metrics {
        Some(m) => m,
        None => {
            default_metrics = CalibrationMetrics {
                harness_version: HARNESS_VERSION,
                kind: model.pool.kind,
                pool_size: model.pool.len(),
                calibrated_at_ms: None,
                variants: BTreeMap::new(),
                twin: None,
            };
            &default_metrics
        }
    };
    Ok(score_with(
        &proposal,
        &class,
        metrics,
        CalibrationMode::Simple,
    ))
}
