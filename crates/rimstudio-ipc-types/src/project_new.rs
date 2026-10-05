//! DTO of the `project_scaffold_preview` command: what creating a new mod would write.
//!
//! The new mod window asks while the person types. The request is the one of `project_create`; the answer
//! lists every folder and file the scaffold would write, the findings about the values (a package id that
//! breaks the game's rule, one that another mod of the last scan already uses, an empty name) and whether
//! the target folder already exists. Nothing is written.

use serde::{Deserialize, Serialize};

use crate::diagnostic::DiagnosticDto;

/// Whether a planned entry is a folder or a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum ScaffoldEntryKindDto {
    /// A folder.
    Folder,
    /// A file.
    File,
}

/// One folder or file the scaffold would write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ScaffoldEntryDto {
    /// The path relative to the new mod folder, `/` separated.
    pub path: String,
    /// Folder or file.
    pub kind: ScaffoldEntryKindDto,
}

/// The answer of `project_scaffold_preview`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProjectScaffoldPreviewDto {
    /// The folder the mod would be created in (the request path as the scaffold reads it).
    pub root: String,
    /// True when no finding is an error: `project_create` would accept the request.
    pub valid: bool,
    /// Findings about the values, with a `field` pointer (`/name`, `/packageId`, `/supportedVersions`,
    /// `/target`). Empty when the request is clean.
    pub diagnostics: Vec<DiagnosticDto>,
    /// The entries the scaffold would write, sorted by path, a folder before its files. Empty when the
    /// request is not valid.
    pub entries: Vec<ScaffoldEntryDto>,
    /// True when the target folder already exists.
    pub target_exists: bool,
    /// Planned files that already exist with other content; a create would refuse them.
    pub conflicts: Vec<String>,
}
