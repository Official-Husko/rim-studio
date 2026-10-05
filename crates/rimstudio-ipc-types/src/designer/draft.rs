//! Drafts: a design spec plus the dialogue state around it, and the draft store requests.
//!
//! Drafts live in the `designer-drafts` collection of the JSON document store. The DTO carries its own
//! `schemaVersion`, so a reader can tell a newer draft from a supported one.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::spec::{DesignSpecDto, ItemKindDto};

/// The schema version of drafts this build writes.
pub const DRAFT_SCHEMA_VERSION: u32 = 1;

fn schema_version() -> u32 {
    DRAFT_SCHEMA_VERSION
}

/// How the numbers of a draft are calibrated.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum CalibrationModeDto {
    /// Class medians, no questions.
    #[default]
    Simple,
    /// Guided by the quiz.
    Quiz,
    /// Anchored on chosen reference weapons.
    Anchored,
}

/// An anchor weapon the user picked.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct AnchorDto {
    /// Definition name of the anchor.
    pub def_name: String,
    /// Display label. Absent means look it up.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub label: Option<String>,
}

/// A saved or in progress design.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DraftDto {
    /// Schema version of this draft.
    #[serde(default = "schema_version")]
    pub schema_version: u32,
    /// Item kind; equals the kind of `spec`.
    pub kind: ItemKindDto,
    /// Calibration mode.
    #[serde(default)]
    pub calibration: CalibrationModeDto,
    /// The design.
    pub spec: DesignSpecDto,
    /// Quiz answers by question id, ordered by id. Omitted when empty.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<BTreeMap<String, Value>>", optional))]
    pub answers: BTreeMap<String, Value>,
    /// Anchor weapons. Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<AnchorDto>>", optional))]
    pub anchors: Vec<AnchorDto>,
    /// The def name of the item this draft was cloned from (flow C). Omitted for a draft that started empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub cloned_from: Option<String>,
}

impl DraftDto {
    /// A draft in simple mode around `spec`.
    #[must_use]
    pub fn new(spec: DesignSpecDto) -> Self {
        Self {
            schema_version: DRAFT_SCHEMA_VERSION,
            kind: spec.kind,
            calibration: CalibrationModeDto::Simple,
            spec,
            answers: BTreeMap::new(),
            anchors: Vec::new(),
            cloned_from: None,
        }
    }
}

/// A stored draft with its listing fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DraftEntryDto {
    /// Draft id (a document id of the store).
    pub id: String,
    /// Definition name of the design; may be empty for a new draft.
    pub def_name: String,
    /// Display label; may be empty.
    pub label: String,
    /// Item kind.
    pub kind: ItemKindDto,
    /// Unix time in milliseconds of the last save.
    pub updated_at_ms: u64,
    /// The draft.
    pub draft: DraftDto,
}

/// Request of `designer_draft_save`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerDraftSaveRequest {
    /// Project the draft belongs to.
    pub project_id: String,
    /// Draft id to overwrite. Absent creates a new draft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub id: Option<String>,
    /// The draft to store.
    pub draft: DraftDto,
}

/// Response of `designer_draft_save`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerDraftSaveResponse {
    /// Draft id.
    pub id: String,
    /// Unix time in milliseconds of the save.
    pub saved_at_ms: u64,
}

/// Request of `designer_draft_list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerDraftListRequest {
    /// Project whose drafts to list.
    pub project_id: String,
}

/// Response of `designer_draft_list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerDraftListResponse {
    /// Drafts, newest first, then by id.
    pub drafts: Vec<DraftEntryDto>,
}

/// Request of `designer_draft_delete`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerDraftDeleteRequest {
    /// Project the draft belongs to.
    pub project_id: String,
    /// Draft id.
    pub id: String,
}

/// Response of `designer_draft_delete`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerDraftDeleteResponse {
    /// True when a draft was removed; false when the id was unknown.
    pub deleted: bool,
}
