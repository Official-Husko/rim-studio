//! Project DTOs: opening and closing a mod project in the workspace.

use serde::{Deserialize, Serialize};

use crate::diagnostic::DiagnosticDto;

/// Request of `project_open`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectOpenRequest {
    /// The mod folder the user picked.
    pub path: String,
}

/// Summary of an open project; response of `project_open`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummaryDto {
    /// Project id for later calls.
    pub project_id: String,
    /// Display name.
    pub name: String,
    /// Absolute path of the project root.
    pub path: String,
    /// Package id from `About.xml`. Absent when the file lacks one or does not exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_id: Option<String>,
    /// Game versions the mod declares, in file order.
    pub supported_versions: Vec<String>,
    /// `About/About.xml` exists.
    pub has_about: bool,
    /// `LoadFolders.xml` exists.
    pub has_load_folders: bool,
    /// `LoadFolders.xml` already gates a folder on Combat Extended.
    pub has_ce_gate: bool,
    /// Definition files found.
    pub def_files: u32,
    /// Problems found while reading the project.
    pub diagnostics: Vec<DiagnosticDto>,
}

/// Request of `project_close`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCloseRequest {
    /// The project to release.
    pub project_id: String,
}

/// Response of `project_close`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCloseResponse {
    /// True when a project was released; false when the id was unknown.
    pub closed: bool,
}
