//! `designer_lint_files`: the Combat Extended lint over the patch files of a project, hand written or
//! generated.
//!
//! The backend reads each file through the XML boundary in game mode, runs the structural rules on the raw
//! nodes, runs the data dependent rules when Combat Extended data is loaded and lists the rules it could not
//! run, with the reason. A file that is not XML, is too large or has a document type declaration gives a
//! finding, never an error.

use serde::{Deserialize, Serialize};

use crate::diagnostic::{DiagnosticDto, SeverityDto};

/// Request of `designer_lint_files`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct DesignerLintFilesRequest {
    /// The open project.
    pub project_id: String,
    /// Files to check, relative to the project root with `/` separators. Omitted or empty checks every
    /// patch file of the project: any XML file below a `Patches` folder and every patch file of the gated
    /// Combat Extended folder.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub paths: Vec<String>,
}

/// What happened to one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum LintFileStatusDto {
    /// The file was read and every rule that can run was run.
    Checked,
    /// The file is not well formed XML (or is not text); the game would skip it.
    ParseFailed,
    /// The file has a document type declaration, which the game's loader refuses.
    Doctype,
    /// The file is larger than the limit and was not read.
    TooLarge,
    /// The file named in the request does not exist.
    Missing,
    /// The file could not be read.
    Unreadable,
    /// A file of the gated Combat Extended folder whose root element is not `Patch`; it is no patch file
    /// and gets no findings.
    NotAPatch,
}

/// One finding of the lint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LintFindingDto {
    /// The stable rule id (`CEP013`). Absent for a finding that belongs to no numbered rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub rule_id: Option<String>,
    /// The stable diagnostic code.
    pub code: String,
    /// How serious it is.
    pub severity: SeverityDto,
    /// English text of the finding.
    pub message: String,
    /// What the rule checks and how to fix it, in English. Absent for a code without an explanation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub explanation: Option<String>,
    /// The pointer of the element, for example `/Patch/Operation[3]/value/ammoSet`. One based positions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub field: Option<String>,
    /// The one based index of the top level operation the finding is in. Absent for a finding about the
    /// file or the project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub operation: Option<u32>,
    /// The xpath of the operation (the innermost one that has an xpath). Absent when there is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub xpath: Option<String>,
    /// The file, relative to the project root. Absent for a finding about the project (`LoadFolders.xml`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub file: Option<String>,
}

/// The result for one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LintedFileDto {
    /// The file, relative to the project root.
    pub path: String,
    /// What happened to it.
    pub status: LintFileStatusDto,
    /// Size in bytes; 0 when the file was not found.
    pub bytes: u64,
    /// Top level operations of the file; 0 when it was not read as a patch file.
    pub operations: u32,
    /// The findings of this file, in file order.
    pub findings: Vec<LintFindingDto>,
}

/// A rule that was not run, with the reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LintNotCheckedDto {
    /// The rule id (`CEP013`).
    pub rule_id: String,
    /// Why the rule did not run, in English.
    pub reason: String,
}

/// Totals over the findings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LintCountsDto {
    /// Files listed.
    pub files: u32,
    /// Files that were read and checked.
    pub checked: u32,
    /// Findings of error severity.
    pub errors: u32,
    /// Findings of warning severity.
    pub warnings: u32,
    /// Findings of hint or info severity.
    pub notes: u32,
}

/// Result of `designer_lint_files`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DesignerLintFilesResult {
    /// One entry per file, sorted by path.
    pub files: Vec<LintedFileDto>,
    /// Findings about the project itself, such as a `LoadFolders.xml` entry that cannot match.
    pub project: Vec<LintFindingDto>,
    /// The rules that did not run, with the reason (Combat Extended data is not loaded, for example).
    pub not_checked: Vec<LintNotCheckedDto>,
    /// True when Combat Extended data was loaded, so the data dependent rules ran.
    pub ce_data: bool,
    /// The game version whose `LoadFolders.xml` block was used.
    pub game_version: String,
    /// Totals.
    pub counts: LintCountsDto,
    /// Problems found while reading the project (About quirks, an unreadable `LoadFolders.xml`).
    pub diagnostics: Vec<DiagnosticDto>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_request_means_every_patch_file() {
        let request: Result<DesignerLintFilesRequest, _> =
            serde_json::from_str(r#"{"projectId":"p1"}"#);
        let request = request.unwrap_or_default();
        assert_eq!(request.project_id, "p1");
        assert!(request.paths.is_empty());
        let text = serde_json::to_string(&request).unwrap_or_default();
        assert_eq!(text, r#"{"projectId":"p1"}"#);
    }

    #[test]
    fn statuses_are_kebab_case() {
        let text = serde_json::to_string(&LintFileStatusDto::ParseFailed).unwrap_or_default();
        assert_eq!(text, "\"parse-failed\"");
        let text = serde_json::to_string(&LintFileStatusDto::NotAPatch).unwrap_or_default();
        assert_eq!(text, "\"not-a-patch\"");
    }
}
