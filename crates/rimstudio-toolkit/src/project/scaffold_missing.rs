//! `project_scaffold_missing`: creates the standard folders a project lacks, and nothing else.
//!
//! The list comes from the layout check: the issues whose fix is automatic (a missing standard folder). A
//! folder is created through the guarded writer (inside the project root, outside the protected folders);
//! files are never created or replaced and nothing is moved or deleted. A second run finds nothing to do.

use camino::Utf8PathBuf;
use rimstudio_ipc_types::project::{
    LayoutFixKindDto, ProjectScaffoldMissingDto, ProjectScaffoldMissingRequest,
};

use super::check::check;
use super::scan::scan;
use crate::error::ToolkitResult;
use crate::shared::env::ProjectEnv;

/// `project_scaffold_missing`: creates (or, with `dry_run`, lists) the missing standard folders.
///
/// # Errors
///
/// [`crate::error::ToolkitError::ProjectNotOpen`] for an unknown project and
/// [`crate::error::ToolkitError::PathRefused`] when the project root lies in a protected folder. A single
/// folder the writer refuses is reported in `skipped` and does not stop the others.
pub fn scaffold_missing(
    env: &ProjectEnv,
    req: &ProjectScaffoldMissingRequest,
    protected: &[Utf8PathBuf],
) -> ToolkitResult<ProjectScaffoldMissingDto> {
    let (record, view) = env.view(&req.project_id)?;
    let found = scan(&view);
    let issues = check(&view, &found);
    let wanted: Vec<String> = issues
        .iter()
        .filter(|i| i.fix.automatic && i.fix.kind == LayoutFixKindDto::CreateFolder)
        .flat_map(|i| i.fix.targets.iter().cloned())
        .collect();
    let mut folders = Vec::new();
    let mut skipped = Vec::new();
    if req.dry_run {
        folders = wanted;
    } else {
        let writer = env.writer(&view.root, record.id.as_str(), protected)?;
        for folder in wanted {
            match writer.make_dir(&folder) {
                Ok(()) => folders.push(folder),
                Err(e) => skipped.push(format!("{folder}: {e}")),
            }
        }
    }
    Ok(ProjectScaffoldMissingDto {
        project_id: req.project_id.clone(),
        dry_run: req.dry_run,
        folders,
        files: Vec::new(),
        skipped,
    })
}
