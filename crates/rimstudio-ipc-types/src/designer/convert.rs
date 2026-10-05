//! Automatic conversion of an existing mod's weapons to Combat Extended (flow D).
//!
//! The scan lists the weapon definitions of the open project with a status; for a candidate that needs
//! input, the ask list names what could not be derived. The answers go back with the export request.

use serde::{Deserialize, Serialize};

use super::spec::{CePatchSpecDto, CeToolPenetrationDto, ItemKindDto};
use crate::diagnostic::DiagnosticDto;

/// Request of the `designer_convert_scan` job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DesignerConvertScanRequest {
    /// The open project to scan.
    pub project_id: String,
    /// True to list definitions that already have a conversion as well.
    pub include_converted: bool,
}

impl Default for DesignerConvertScanRequest {
    fn default() -> Self {
        Self {
            project_id: String::new(),
            include_converted: true,
        }
    }
}

/// The conversion status of a weapon definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConvertStatusDto {
    /// A conversion can be generated.
    NotConverted,
    /// The definition already carries a conversion (update mode applies).
    AlreadyCe,
    /// The weapon kind is not supported (for example an explosive).
    UnsupportedKind,
    /// The patch target definition was not found.
    TargetNotFound,
}

/// The kind of an ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AskKindDto {
    /// Pick one of `options`.
    Choice,
    /// Yes or no.
    Flag,
    /// A number.
    Number,
}

/// One thing the automatic mode could not derive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AskItemDto {
    /// JSON pointer into the spec the answer belongs to, for example `/ce/ammoSet`.
    pub field: String,
    /// Short English question.
    pub label: String,
    /// Kind of answer.
    pub kind: AskKindDto,
    /// The choices, in display order. Empty unless `kind` is `choice`.
    pub options: Vec<String>,
    /// Why the number is asked although an estimate exists: the estimate was rated unreliable or could
    /// not be measured. Absent for questions with no estimate (the ammo set, for example).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The rejected estimate, for reference only: it is never written unless the user answers with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<f64>,
}

/// One weapon definition of the project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertCandidateDto {
    /// Definition name.
    pub def_name: String,
    /// Display label; empty when the definition has none.
    pub label: String,
    /// Weapon kind. Absent when the kind could not be determined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ItemKindDto>,
    /// Status.
    pub status: ConvertStatusDto,
    /// English fallback explanation of the status.
    pub reason: String,
    /// Project file that holds the definition. Absent when it comes from another mod.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// What the user must still decide. Empty means the conversion can run as is.
    pub asks: Vec<AskItemDto>,
    /// The weapon family key: weapons with the same key (kind, first weapon tag and, for guns, the default
    /// projectile) can share one answer group. Empty for a definition that is not a listed weapon.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub family: String,
}

/// Counts of a scan.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertCountsDto {
    /// Candidates that can be converted.
    pub not_converted: u32,
    /// Candidates already converted.
    pub already_ce: u32,
    /// Candidates of an unsupported kind.
    pub unsupported_kind: u32,
    /// Candidates whose target was not found.
    pub target_not_found: u32,
}

/// Result of the `designer_convert_scan` job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertScanDto {
    /// Candidates ordered by definition name.
    pub candidates: Vec<ConvertCandidateDto>,
    /// Counts by status.
    pub counts: ConvertCountsDto,
    /// Content problems found while reading the project.
    pub diagnostics: Vec<DiagnosticDto>,
}

/// The user's answers to an ask list.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConvertAnswersDto {
    /// Ammo set definition name. Absent means not answered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ammo_set: Option<String>,
    /// Default projectile. Absent means not answered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_projectile: Option<String>,
    /// Weapon tag class. Absent means not answered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weapon_tag_class: Option<String>,
    /// One handed. Absent means not answered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub one_handed: Option<bool>,
    /// Belt fed. Absent means not answered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub belt_fed: Option<bool>,
    /// Tool penetration values the user gave. Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tool_penetration: Vec<CeToolPenetrationDto>,
    /// Explicit overrides of any Combat Extended field.
    pub overrides: CePatchSpecDto,
}

/// One answer set that applies to several weapons of a conversion.
///
/// A group applies to a definition when its name is in `defNames` or its family key equals `family`. A group
/// that names neither applies to every weapon (the defaults of the file). The answers are kept as the raw
/// object the user wrote, so that a member left out is not mistaken for a member set to its default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertAnswerGroupDto {
    /// A family key (see `ConvertCandidateDto.family`). Absent when the group names definitions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    /// Definition names the group applies to. Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub def_names: Vec<String>,
    /// The answers, an object with the members of `ConvertAnswersDto`.
    pub answers: serde_json::Value,
}

/// A conversion of an existing definition, as part of an export request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertRequestDto {
    /// Definition name of the weapon to convert.
    pub def_name: String,
    /// The answers of this weapon. They win over every group.
    #[serde(default)]
    pub answers: ConvertAnswersDto,
    /// Answer groups, applied in order (a later group wins over an earlier one) beneath `answers`. Omitted
    /// when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<ConvertAnswerGroupDto>,
}
