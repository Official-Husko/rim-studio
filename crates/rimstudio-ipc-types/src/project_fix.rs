//! DTOs of the layout fix commands: `project_layout_fix_plan`, `project_layout_fix_apply`,
//! `project_layout_fix_undo` and `project_layout_fix_history`.
//!
//! The plan lists what the layout check suggests and the tool can carry out, one item per change. Every
//! item says what it moves or edits, why, how risky it is and, where it applies, the exact edit as a diff,
//! the conflict (the destination exists) and the references found in the project's definition files. The
//! apply call takes the plan id and the ids of the items to carry out; every apply writes an undo journal.

use serde::{Deserialize, Serialize};

use crate::project::ProjectLayoutCheckDto;

/// What a plan item does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum LayoutFixItemKindDto {
    /// Move one file.
    MoveFile,
    /// Move or rename one folder with everything in it.
    MoveFolder,
    /// Create an empty standard folder.
    CreateFolder,
    /// Edit the existing `LoadFolders.xml`.
    EditLoadFolders,
    /// Create `LoadFolders.xml` (the project had none).
    CreateLoadFolders,
}

/// How risky an item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum LayoutFixRiskDto {
    /// Nothing refers to the old place; the item can be applied and undone.
    Safe,
    /// The move may break a reference (a texture path, a sound clip path) or needs a decision. The tool
    /// lists it and never applies it.
    NeedsReview,
}

/// A place in a project file that mentions the old path of a moved file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LayoutFixReferenceDto {
    /// The file that holds the reference, relative to the project root.
    pub path: String,
    /// The one based line.
    pub line: u32,
    /// The line, trimmed and cut to 200 characters.
    pub text: String,
}

/// The destination of a move exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LayoutFixConflictDto {
    /// The destination exists, so the move is refused unless the caller accepts the other name.
    pub destination_exists: bool,
    /// A free name next to the destination (a numbered suffix); absent when none was found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub suggested_to: Option<String>,
}

/// One change of a fix plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LayoutFixItemDto {
    /// The item id, stable for the same project state (`fix-<number>-<short hash>`).
    pub id: String,
    /// What it does.
    pub kind: LayoutFixItemKindDto,
    /// The layout issue code it carries out (`layout.ce-outside-gate`).
    pub issue_code: String,
    /// The source path relative to the project root; empty for a creation.
    pub from: String,
    /// The destination path relative to the project root (the file or folder created or edited).
    pub to: String,
    /// Why, in one plain sentence.
    pub why: String,
    /// How risky it is.
    pub risk: LayoutFixRiskDto,
    /// True when `project_layout_fix_apply` can carry it out (a safe item without an unresolved error).
    pub applicable: bool,
    /// Why the item is not applicable, when it is not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub review_reason: Option<String>,
    /// The exact edit as a unified diff, for an item that edits `LoadFolders.xml`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub diff: Option<String>,
    /// Set when the destination exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub conflict: Option<LayoutFixConflictDto>,
    /// Places in the project's definition and patch files that mention the old path.
    pub references: Vec<LayoutFixReferenceDto>,
    /// Ids of items that must be applied together with this one (a Combat Extended patch needs the item
    /// that gates its folder).
    pub requires: Vec<String>,
}

/// Request of `project_layout_fix_plan`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectLayoutFixPlanRequest {
    /// The project (from `project_open`).
    pub project_id: String,
    /// Only plan the fixes of these layout issue codes; absent plans every fixable finding.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub fixes: Option<Vec<String>>,
}

/// The fix plan of a project; response of `project_layout_fix_plan`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectLayoutFixPlanDto {
    /// The project.
    pub project_id: String,
    /// A hash of the content of the plan: items, paths, diffs and the state of the files involved. The
    /// apply call refuses a plan id that no longer matches.
    pub plan_id: String,
    /// The items in the order they are carried out.
    pub items: Vec<LayoutFixItemDto>,
    /// Items that can be applied.
    pub safe: u32,
    /// Items listed for review only.
    pub needs_review: u32,
    /// Items whose destination exists.
    pub conflicts: u32,
}

/// One item the caller selected.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct LayoutFixSelectionDto {
    /// The item id of the plan.
    pub id: String,
    /// When the destination exists, use the suggested numbered name instead of refusing the item.
    pub rename_on_conflict: bool,
}

/// Request of `project_layout_fix_apply`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectLayoutFixApplyRequest {
    /// The project (from `project_open`).
    pub project_id: String,
    /// The plan id the caller reviewed.
    pub plan_id: String,
    /// The items to carry out; an item that is not in the plan or not applicable is skipped with a reason.
    pub items: Vec<LayoutFixSelectionDto>,
}

/// A move or creation that was carried out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LayoutFixDoneDto {
    /// The item id.
    pub id: String,
    /// What it did.
    pub kind: LayoutFixItemKindDto,
    /// The old path; empty for a creation.
    pub from: String,
    /// The new path.
    pub to: String,
    /// True when the move crossed a volume boundary and was done as copy, verify and remove.
    pub copied: bool,
}

/// A file whose content was edited or created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LayoutFixEditedDto {
    /// The file, relative to the project root.
    pub path: String,
    /// True when the file did not exist before.
    pub created: bool,
    /// The backup of the previous content in the app data folder; absent for a created file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub backup_path: Option<String>,
}

/// An item that was not carried out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LayoutFixSkippedDto {
    /// The item id as given.
    pub id: String,
    /// Why, in one plain sentence.
    pub reason: String,
}

/// What `project_layout_fix_apply` did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectLayoutFixApplyDto {
    /// The project.
    pub project_id: String,
    /// The id of this apply; `project_layout_fix_undo` takes it. No journal exists, and the id is of no use,
    /// when nothing was carried out.
    pub apply_id: String,
    /// The plan id that was applied.
    pub plan_id: String,
    /// The moves and creations, in the order they were done.
    pub done: Vec<LayoutFixDoneDto>,
    /// The files edited or created.
    pub edited: Vec<LayoutFixEditedDto>,
    /// The items that were not carried out.
    pub skipped: Vec<LayoutFixSkippedDto>,
    /// True when the job was cancelled between items; the items done stay and can be undone.
    pub cancelled: bool,
    /// The layout check run again after the apply.
    pub check: ProjectLayoutCheckDto,
}

/// Request of `project_layout_fix_undo`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectLayoutFixUndoRequest {
    /// The project (from `project_open`).
    pub project_id: String,
    /// The apply id to reverse.
    pub apply_id: String,
}

/// What `project_layout_fix_undo` did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectLayoutFixUndoDto {
    /// The project.
    pub project_id: String,
    /// The apply id that was reversed.
    pub apply_id: String,
    /// The paths moved back to where they were.
    pub moved_back: Vec<String>,
    /// The files whose previous content was restored, or that were removed because the apply created them.
    pub restored: Vec<String>,
    /// The folders removed because the apply created them.
    pub removed_folders: Vec<String>,
    /// The layout check run again after the undo.
    pub check: ProjectLayoutCheckDto,
}

/// Request of `project_layout_fix_history`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectLayoutFixHistoryRequest {
    /// The project (from `project_open`).
    pub project_id: String,
}

/// One journal of the history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LayoutFixJournalDto {
    /// The apply id.
    pub apply_id: String,
    /// The plan id that was applied.
    pub plan_id: String,
    /// When the apply started, in Unix milliseconds.
    pub created_ms: u64,
    /// Items in the journal.
    pub items_total: u32,
    /// Items that were carried out.
    pub items_done: u32,
    /// Items that were not.
    pub items_skipped: u32,
    /// True when the apply was undone.
    pub undone: bool,
    /// True when `project_layout_fix_undo` would succeed now.
    pub undo_possible: bool,
    /// Why not, when it would not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub undo_blocker: Option<String>,
}

/// The journals of a project, newest first; response of `project_layout_fix_history`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectLayoutFixHistoryDto {
    /// The project.
    pub project_id: String,
    /// The journals, newest first.
    pub journals: Vec<LayoutFixJournalDto>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_selection_reads_with_defaults() {
        let s: LayoutFixSelectionDto =
            serde_json::from_str(r#"{"id":"fix-1-ab"}"#).unwrap_or_default();
        assert_eq!(s.id, "fix-1-ab");
        assert!(!s.rename_on_conflict);
    }

    #[test]
    fn kinds_are_kebab_case() {
        let text =
            serde_json::to_string(&LayoutFixItemKindDto::EditLoadFolders).unwrap_or_default();
        assert_eq!(text, "\"edit-load-folders\"");
        let text = serde_json::to_string(&LayoutFixRiskDto::NeedsReview).unwrap_or_default();
        assert_eq!(text, "\"needs-review\"");
    }

    #[test]
    fn an_item_round_trips_and_omits_empty_options() {
        let item = LayoutFixItemDto {
            id: "fix-1-ab".into(),
            kind: LayoutFixItemKindDto::MoveFile,
            issue_code: "layout.ce-outside-gate".into(),
            from: "Patches/a.xml".into(),
            to: "Compat/CombatExtended/Patches/a.xml".into(),
            why: "x".into(),
            risk: LayoutFixRiskDto::Safe,
            applicable: true,
            review_reason: None,
            diff: None,
            conflict: None,
            references: Vec::new(),
            requires: vec!["fix-2-cd".into()],
        };
        let text = serde_json::to_string(&item).unwrap_or_default();
        assert!(!text.contains("conflict") && !text.contains("diff"));
        let back: Result<LayoutFixItemDto, _> = serde_json::from_str(&text);
        assert_eq!(back.ok(), Some(item));
    }
}
