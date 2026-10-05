//! Suggestions for the optional Combat Extended block of a draft (`designer_ce_suggest`).
//!
//! The suggestion reports, per field of the block, the number the user's own conversions suggest, where it
//! comes from, how far it can be trusted and whether the user has to answer. It is a report only: it never
//! turns the Combat Extended toggle on, and a plan writes a suggested number only when the request opts in
//! (`acceptSuggestions` of the export request) and the draft already carries the Combat Extended block.
//!
//! The JSON shape is identical to the design engine's own types, so the toolkit converts by serialising one
//! and deserialising the other.

use serde::{Deserialize, Serialize};

use super::convert::AskKindDto;
use super::draft::DraftDto;
use super::spec::ItemKindDto;

/// Request of `designer_ce_suggest`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerCeSuggestRequest {
    /// The draft state. `draft.spec.ce` may be absent: the suggestion then describes what turning the
    /// patch on would offer and changes nothing.
    pub draft: DraftDto,
}

/// Which suggestions a plan accepts. Sent with the export or apply request; ignored when the draft has no
/// Combat Extended block.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct AcceptSuggestionsDto {
    /// The fields to accept: a pointer such as `/ce/bulk` or a short name such as `bulk`. Empty means every
    /// derived field (the numbers rated reliable or rough, and the default projectile of a chosen ammo
    /// set). Typed values are never overwritten either way.
    pub fields: Vec<String>,
}

/// A predictor form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum CePredictorDto {
    /// The converted value equals the vanilla value.
    Identity,
    /// The class median.
    Median,
    /// The vanilla value times the class ratio.
    Ratio,
    /// A fitted power law.
    Elastic,
}

/// How far a stat can be trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum CeRatingDto {
    /// Typical error below 15 percent.
    Reliable,
    /// Typical error below 35 percent; usable, labelled as rough.
    Rough,
    /// Too inexact: asked, not written.
    Unreliable,
    /// Too few conversions to measure: asked, not written.
    Unmeasured,
}

/// Where the value shown for a field comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum CeSourceDto {
    /// The user typed the number.
    Typed,
    /// Copied from an anchor item.
    Anchor,
    /// Follows from a quiz answer.
    Answered,
    /// The converted number equals the vanilla number.
    Identity {
        /// Converted weapons behind the finding.
        n: u32,
    },
    /// A predictor over the user's conversions.
    Predicted {
        /// The predictor form.
        predictor: CePredictorDto,
        /// Converted weapons behind the estimate.
        n: u32,
    },
    /// The vanilla number, carried over because the estimate was too poor.
    Vanilla,
    /// The first member of the chosen ammo set.
    FirstOfSet,
}

/// What the suggestion engine did for a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum CeFieldStatusDto {
    /// The draft holds a value the user decided; it is kept.
    Held,
    /// A usable suggestion exists.
    Derived,
    /// No usable suggestion: the user must answer.
    Ask,
}

/// A closed interval.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CeIntervalDto {
    /// Lower end.
    pub low: f64,
    /// Upper end.
    pub high: f64,
}

/// The error band around a suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CeBandDto {
    /// Holds half of the held out conversions.
    pub p50: CeIntervalDto,
    /// Holds four fifths of them.
    pub p80: CeIntervalDto,
}

/// One number of the block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CeSuggestedFieldDto {
    /// JSON pointer into the spec, for example `/ce/bulk`, or a plain stat name for the patch numbers.
    pub field: String,
    /// Short English label.
    pub label: String,
    /// True when a plan that accepts suggestions needs the field.
    pub required: bool,
    /// What the engine did.
    pub status: CeFieldStatusDto,
    /// The value the draft holds, when the user decided it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub held: Option<f64>,
    /// Where the held value came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub held_source: Option<CeSourceDto>,
    /// The suggested number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub value: Option<f64>,
    /// Where the suggested number comes from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub source: Option<CeSourceDto>,
    /// The error band of the suggested number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub band: Option<CeBandDto>,
    /// The rating of the stat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub rating: Option<CeRatingDto>,
    /// Typical relative error of the estimate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub error: Option<f64>,
    /// A rejected estimate, for reference only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub reference: Option<f64>,
    /// Why there is no usable suggestion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub reason: Option<String>,
}

/// One candidate of a choice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CeCandidateDto {
    /// Def name or tag.
    pub name: String,
    /// Converted guns that use it.
    pub used_by: u32,
    /// Sum of the tag similarity of the guns that use it.
    pub score: f64,
    /// Damage of the first projectile of an ammo set, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub first_damage: Option<f64>,
}

/// One choice of the block: a def, a tag or a flag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CeChoiceDto {
    /// JSON pointer into the spec.
    pub field: String,
    /// Short English label.
    pub label: String,
    /// The kind of answer.
    pub kind: AskKindDto,
    /// True when a plan that accepts suggestions needs an answer.
    pub required: bool,
    /// What the engine did.
    pub status: CeFieldStatusDto,
    /// What the draft holds, as text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub held: Option<String>,
    /// A derived value (the default projectile follows the ammo set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub value: Option<String>,
    /// Where the derived value comes from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub source: Option<CeSourceDto>,
    /// The candidates, best first.
    pub candidates: Vec<CeCandidateDto>,
    /// Why the choice is open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub reason: Option<String>,
}

/// One open question of a suggestion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CeAskDto {
    /// JSON pointer into the spec the answer belongs to.
    pub field: String,
    /// Short English question.
    pub label: String,
    /// Kind of answer.
    pub kind: AskKindDto,
    /// The choices, in display order.
    pub options: Vec<String>,
    /// Why the number is asked although an estimate exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub reason: Option<String>,
    /// The rejected estimate, for reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub suggestion: Option<f64>,
}

/// The asks of a suggestion.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CeAskListDto {
    /// The questions.
    pub items: Vec<CeAskDto>,
}

/// Response of `designer_ce_suggest`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CeSuggestionDto {
    /// The kind of item.
    pub kind: ItemKindDto,
    /// The def name of the design.
    pub def_name: String,
    /// False when there is nothing to suggest from; `reason` says why.
    pub available: bool,
    /// The plain reason when `available` is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub reason: Option<String>,
    /// True when the draft's Combat Extended toggle is on. The suggestion never changes it.
    pub toggle_on: bool,
    /// How the class behind the estimates was formed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub class_label: Option<String>,
    /// Converted weapons of the kind in the library.
    pub pool: u32,
    /// The numbers of the block.
    pub fields: Vec<CeSuggestedFieldDto>,
    /// The choices of the block.
    pub choices: Vec<CeChoiceDto>,
    /// Numbers the patch derives although the block has no field for them.
    pub patch_numbers: Vec<CeSuggestedFieldDto>,
    /// The open questions.
    pub asks: CeAskListDto,
    /// Required fields the draft does not hold today.
    pub missing: Vec<String>,
    /// Required fields that stay open even after every derived suggestion is accepted.
    pub still_missing_after_accept: Vec<String>,
    /// Plain words about the estimates.
    pub notes: Vec<String>,
}
