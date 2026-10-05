//! The preview: exact readouts, suggestions with source labels and bands, and the estimate behind them.
//!
//! Readouts are exact numbers computed from the formulas; suggestions are estimates and always carry where
//! they came from and how wide the plausible band is.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::draft::DraftDto;
use crate::diagnostic::DiagnosticDto;

/// Request of `designer_preview`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerPreviewRequest {
    /// The draft state to preview.
    pub draft: DraftDto,
}

/// Group a readout belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum ReadoutGroupDto {
    /// Ranged weapon numbers.
    Ranged,
    /// Melee weapon numbers.
    Melee,
    /// Price and work.
    Economy,
    /// Combat Extended numbers (only when the user opted in).
    CombatExtended,
}

/// Unit of a readout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum ReadoutUnitDto {
    /// A plain number.
    Number,
    /// Damage per second.
    DamagePerSecond,
    /// Seconds.
    Seconds,
    /// Tiles.
    Tiles,
    /// Silver.
    Silver,
    /// Kilograms.
    Kilograms,
    /// A fraction between zero and one shown as a percentage.
    Fraction,
}

/// One step of a calculation shown on hover.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ReadoutStepDto {
    /// Short English label of the step.
    pub label: String,
    /// Value after the step.
    pub value: f64,
}

/// One exact readout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ReadoutDto {
    /// Stable key, for example `dps`.
    pub key: String,
    /// Group.
    pub group: ReadoutGroupDto,
    /// Unit.
    pub unit: ReadoutUnitDto,
    /// The exact value. Absent when an input of the formula is missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub value: Option<f64>,
    /// The calculation steps. Omitted when the readout has none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<ReadoutStepDto>>", optional))]
    pub steps: Vec<ReadoutStepDto>,
}

/// Where a suggestion came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum SuggestionSourceDto {
    /// The user typed it.
    Typed,
    /// Implied by a quiz answer.
    Answer,
    /// Scaled from the nearest anchors.
    Anchor,
    /// The class median.
    ClassMedian,
    /// A pool quantile.
    Quantile,
    /// Derived from other values by an identity.
    Derived,
}

/// Level of the class fallback chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum ChainLevelDto {
    /// Same role and tier.
    RoleTier,
    /// Same role.
    Role,
    /// Same group and tier.
    GroupTier,
    /// Same tier.
    Tier,
    /// Same group.
    Group,
    /// Every item of the kind.
    All,
}

/// Predictor behind a suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum PredictorDto {
    /// Class median.
    Median,
    /// Pool quantile.
    Quantile,
    /// Anchor ratio.
    Anchor,
}

/// The plausible band around a suggestion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SuggestionBandDto {
    /// Tenth percentile of the pool.
    pub p10: f64,
    /// Median of the pool.
    pub median: f64,
    /// Ninetieth percentile of the pool.
    pub p90: f64,
    /// Half width factor of the 50 percent band around the suggestion.
    pub p50: f64,
    /// Half width factor of the 80 percent band around the suggestion.
    pub p80: f64,
    /// Systematic shift measured by calibration; zero when uncalibrated.
    pub shift: f64,
}

/// A suggested value for one field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SuggestionDto {
    /// JSON pointer of the spec field, for example `/ranged/damage`.
    pub field: String,
    /// Stat name.
    pub stat: String,
    /// The suggested value. Absent when no estimate is possible for the field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub value: Option<f64>,
    /// Where it came from. Absent when `value` is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub source: Option<SuggestionSourceDto>,
    /// The band. Absent when the pool is too small for one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub band: Option<SuggestionBandDto>,
    /// Fallback level of the class used.
    pub level: ChainLevelDto,
    /// Predictor used.
    pub predictor: PredictorDto,
    /// Pool size behind the estimate.
    pub n: u32,
    /// True when the current value is typed and the suggestion will not replace it.
    pub locked: bool,
}

/// An anchor weapon shown as a card.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AnchorCardDto {
    /// Definition name.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Strength score.
    pub strength: f64,
    /// Tier index, zero for neolithic.
    pub tier: u8,
    /// Role.
    pub role: String,
    /// Exact stat values by stat name, ordered by name.
    pub stats: BTreeMap<String, f64>,
}

/// Summary of the estimate behind the suggestions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct EstimateSummaryDto {
    /// Human readable name of the class used.
    pub class_label: String,
    /// Fallback level.
    pub level: ChainLevelDto,
    /// Items in the class.
    pub class_n: u32,
    /// Strength score estimated for the design.
    pub strength: f64,
    /// Percentile of the strength within the class, zero to one.
    pub strength_percentile: f64,
    /// The nearest anchors.
    pub anchors: Vec<AnchorCardDto>,
    /// English notes, for example that the pool is small.
    pub notes: Vec<String>,
}

/// Response of `designer_preview`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PreviewDto {
    /// Exact readouts, in display order.
    pub readouts: Vec<ReadoutDto>,
    /// Suggestions by field, in display order.
    pub suggestions: Vec<SuggestionDto>,
    /// The estimate behind the suggestions. Absent when no reference set is loaded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub estimate: Option<EstimateSummaryDto>,
    /// Static validation results with field pointers.
    pub diagnostics: Vec<DiagnosticDto>,
}

/// Request of `designer_material_matrix`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerMaterialMatrixRequest {
    /// The draft state.
    pub draft: DraftDto,
    /// Readout key to tabulate, for example `dps`.
    pub readout: String,
}

/// One cell of the stuff by quality grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct MatrixCellDto {
    /// Index into `stuffs`.
    pub stuff: u32,
    /// Index into `qualities`.
    pub quality: u32,
    /// The value. Absent when not computable for the combination.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub value: Option<f64>,
}

/// Response of `designer_material_matrix`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct MaterialMatrixDto {
    /// The tabulated readout key.
    pub readout: String,
    /// Stuff definition names, in display order.
    pub stuffs: Vec<String>,
    /// Qualities, weakest first.
    pub qualities: Vec<super::spec::QualityDto>,
    /// Cells, row major by stuff then quality.
    pub cells: Vec<MatrixCellDto>,
}
