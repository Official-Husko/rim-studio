//! The write plan and the apply report.
//!
//! The plan lists every file a design would write, with the rendered text and a diff against what the
//! project holds. Vanilla definition files are always present; Combat Extended patch files appear only
//! when the draft opted in, always under the folder that `LoadFolders.xml` gates.

use serde::{Deserialize, Serialize};

use super::ce_suggest::AcceptSuggestionsDto;
use super::convert::ConvertRequestDto;
use super::draft::DraftDto;
use crate::diagnostic::DiagnosticDto;

/// What a plan does to a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum FileActionDto {
    /// The file does not exist yet.
    Create,
    /// Only the region owned by the designer changes.
    UpdateRegion,
    /// The rendering equals the file on disk.
    Unchanged,
}

/// What a planned file holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum FileKindDto {
    /// Vanilla definitions.
    VanillaDefs,
    /// A Combat Extended patch.
    CePatch,
    /// `LoadFolders.xml`.
    LoadFolders,
    /// `About.xml`.
    About,
}

/// Request of `designer_export_plan`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerExportPlanRequest {
    /// The open project the files go into.
    pub project_id: String,
    /// The design. A Combat Extended patch is part of the plan only when `draft.spec.ce` is present.
    pub draft: DraftDto,
    /// Convert an existing definition instead of designing a new one. Absent means design.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub convert: Option<ConvertRequestDto>,
    /// Fill the empty fields of the Combat Extended block from the suggestions (never typed values). Only
    /// used when `draft.spec.ce` is present; it never turns the patch on. Absent means no suggestion is
    /// written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub accept_suggestions: Option<AcceptSuggestionsDto>,
}

/// One file of a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PlannedFileDto {
    /// Path relative to the project root, with `/` separators.
    pub path: String,
    /// What the file holds.
    pub kind: FileKindDto,
    /// What the plan does.
    pub action: FileActionDto,
    /// The full rendered text of the file (or of the owned region for `update-region`).
    pub rendered: String,
    /// Unified diff against the file on disk. Absent for `create` and `unchanged`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub diff: Option<String>,
    /// Size of `rendered` in bytes.
    pub bytes: u32,
}

/// Response of `designer_export_plan`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct WritePlanDto {
    /// Content hash of the plan; apply refuses a plan whose rebuilt hash differs.
    pub plan_id: String,
    /// Files in write order.
    pub files: Vec<PlannedFileDto>,
    /// Validation and lint results with field pointers.
    pub diagnostics: Vec<DiagnosticDto>,
    /// True when any diagnostic is an error; such a plan cannot be applied.
    pub has_errors: bool,
}

/// Request of the `designer_apply_plan` job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerApplyPlanRequest {
    /// The plan hash the user reviewed.
    pub plan_id: String,
    /// The same inputs the plan was exported from; the plan is rebuilt and compared with `plan_id`.
    pub request: DesignerExportPlanRequest,
    /// Back up files that are replaced.
    #[serde(default = "default_true")]
    pub backup: bool,
    /// Run the generated patch through the def engine on a scratch copy after writing.
    #[serde(default = "default_true")]
    pub dry_apply: bool,
}

fn default_true() -> bool {
    true
}

/// A file the apply step wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AppliedFileDto {
    /// Path relative to the project root.
    pub path: String,
    /// What was done.
    pub action: FileActionDto,
    /// Bytes written.
    pub bytes: u32,
    /// Backup path relative to the project root. Absent when no backup was made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub backup_path: Option<String>,
    /// True when the file was read back and equals the rendering.
    pub verified: bool,
}

/// Result of the `designer_apply_plan` job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ApplyReportDto {
    /// The applied plan hash.
    pub plan_id: String,
    /// Files written, in write order.
    pub written: Vec<AppliedFileDto>,
    /// Paths of the plan that needed no write.
    pub unchanged: Vec<String>,
    /// Result of the dry apply. Absent when it did not run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub dry_apply_ok: Option<bool>,
    /// Problems found while applying or dry applying.
    pub diagnostics: Vec<DiagnosticDto>,
}
