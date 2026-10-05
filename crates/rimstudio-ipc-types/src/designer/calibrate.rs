//! Calibration: building the pools and running the leave one out harness.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::spec::ItemKindDto;

/// Request of the `designer_calibrate` job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DesignerCalibrateRequest {
    /// Ranged or melee.
    pub kind: ItemKindDto,
    /// True to ignore the cache and recalibrate.
    pub force: bool,
    /// Worker threads. Absent means automatic. The result is identical for any thread count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threads: Option<u8>,
}

impl Default for DesignerCalibrateRequest {
    fn default() -> Self {
        Self {
            kind: ItemKindDto::Ranged,
            force: false,
            threads: None,
        }
    }
}

/// Error and coverage numbers of one stat under one variant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatMetricsDto {
    /// Weapons scored.
    pub n: u32,
    /// Median relative error. Absent when `n` is zero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub median_error: Option<f64>,
    /// 80th percentile relative error. Absent when `n` is zero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p80_error: Option<f64>,
    /// Mean relative error. Absent when `n` is zero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mean_error: Option<f64>,
    /// Half width factor of the 50 percent band.
    pub factor_p50: f64,
    /// Half width factor of the 80 percent band.
    pub factor_p80: f64,
    /// Systematic shift.
    pub shift: f64,
    /// Share of weapons inside the 50 percent band.
    pub coverage_p50: f64,
    /// Share of weapons inside the 80 percent band.
    pub coverage_p80: f64,
    /// Rank correlation of predicted and true values. Absent when undefined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank_correlation: Option<f64>,
    /// Median error of the class median baseline. Absent when `n` is zero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_median_error: Option<f64>,
}

/// Metrics of one simulated answer profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantMetricsDto {
    /// Profile name.
    pub name: String,
    /// Replicates per weapon.
    pub replicates: u32,
    /// Per stat metrics, ordered by stat name.
    pub per_stat: BTreeMap<String, StatMetricsDto>,
    /// Macro median error. Absent when nothing was scored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macro_median_error: Option<f64>,
    /// Macro mean error. Absent when nothing was scored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macro_mean_error: Option<f64>,
    /// Mean questions asked.
    pub mean_questions: f64,
    /// Most questions asked.
    pub max_questions: u32,
}

/// Result of the twin leakage check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwinCheckDto {
    /// Error with twins in the pool.
    pub plain_error: f64,
    /// Error with twins removed.
    pub twin_free_error: f64,
    /// Difference of the two.
    pub gap: f64,
    /// True when the gap is large enough to call the bands optimistic.
    pub optimistic: bool,
}

/// Result of the `designer_calibrate` job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrateResultDto {
    /// Ranged or melee.
    pub kind: ItemKindDto,
    /// Version of the harness that produced the numbers.
    pub harness_version: u32,
    /// Size of the reference pool.
    pub pool_size: u32,
    /// Unix time in milliseconds of the run. Absent when the clock was unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibrated_at_ms: Option<u64>,
    /// Metrics by variant name, ordered by name.
    pub variants: BTreeMap<String, VariantMetricsDto>,
    /// Twin check. Absent when it did not run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub twin: Option<TwinCheckDto>,
    /// True when the result came from the cache.
    pub from_cache: bool,
}
