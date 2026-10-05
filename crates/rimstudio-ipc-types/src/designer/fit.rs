//! The fit report: how typical each number of a design is among the reference weapons.
//!
//! There is no red level on purpose: the levels are `typical`, `plausible` and `unusual`.

use serde::{Deserialize, Serialize};

use super::draft::DraftDto;

/// Request of `designer_fit`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerFitRequest {
    /// The draft state to score.
    pub draft: DraftDto,
}

/// How well a stat fits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum FitLevelDto {
    /// Inside the 50 percent band.
    Typical,
    /// Inside the 80 percent band.
    Plausible,
    /// Outside both bands.
    Unusual,
}

/// A closed interval.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct IntervalDto {
    /// Lower bound.
    pub low: f64,
    /// Upper bound.
    pub high: f64,
}

/// Rank of a value among the reference values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RankDto {
    /// References below the value.
    pub below: u32,
    /// References equal to the value.
    pub equal: u32,
    /// References in total.
    pub total: u32,
}

/// The fit of one stat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct StatFitDto {
    /// Stat name.
    pub stat: String,
    /// The design's value.
    pub value: f64,
    /// The predicted value.
    pub predicted: f64,
    /// The 50 percent band.
    pub p50: IntervalDto,
    /// The 80 percent band.
    pub p80: IntervalDto,
    /// Rank among the references.
    pub rank: RankDto,
    /// Level.
    pub level: FitLevelDto,
    /// The reference value nearest to the design's value.
    pub nearest_reference: f64,
    /// True when the band is a default (no calibration behind it).
    pub default_band: bool,
}

/// Counts over all scored stats.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct FitSummaryDto {
    /// Stats scored.
    pub scored: u32,
    /// Stats at `typical`.
    pub typical: u32,
    /// Stats at `plausible`.
    pub plausible: u32,
    /// Stats at `unusual`.
    pub unusual: u32,
    /// Share of stats inside the 80 percent band, zero to one.
    pub share_in_p80: f64,
}

/// Caveats the UI shows beside the meter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct FitNoticesDto {
    /// Unix time in milliseconds of the calibration behind the bands. Absent when never calibrated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub calibrated_at_ms: Option<u64>,
    /// Size of the reference pool.
    pub pool_size: u32,
    /// Human readable class name.
    pub class_label: String,
    /// Items in the class.
    pub class_n: u32,
    /// True when the pool is small and the bands are rough.
    pub rough: bool,
    /// True when the calibration suspects twin leakage and the bands are optimistic.
    pub optimistic: bool,
}

/// Response of `designer_fit`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct FitReportDto {
    /// Per stat results, ordered by stat name.
    pub per_stat: Vec<StatFitDto>,
    /// Overall typicality, zero to one. Absent when too few stats could be scored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub typicality: Option<f64>,
    /// Counts.
    pub summary: FitSummaryDto,
    /// Caveats.
    pub notices: FitNoticesDto,
    /// Stats that could not be scored, ordered by name.
    pub unscored: Vec<String>,
}
