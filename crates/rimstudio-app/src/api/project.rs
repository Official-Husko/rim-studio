//! `project_open`, `project_create` and `project_close`.

use camino::Utf8PathBuf;
use rimstudio_ipc_types::error::ApiError;
use rimstudio_ipc_types::project::{
    ProjectCloseRequest, ProjectCloseResponse, ProjectOpenRequest, ProjectSummaryDto,
};
use rimstudio_toolkit::project;
use rimstudio_workspace::scaffold::{ScaffoldLayout, ScaffoldSpec};

use crate::context::AppContext;
use crate::dto::ProjectCreateRequest;

/// The scaffold spec of a create request.
#[must_use]
pub fn scaffold_spec(req: &ProjectCreateRequest) -> ScaffoldSpec {
    let mut spec = ScaffoldSpec::new(
        Utf8PathBuf::from(req.path.trim()),
        req.name.clone(),
        req.package_id.clone(),
    );
    spec.author = req.author.clone();
    spec.description = req.description.clone();
    if !req.supported_versions.is_empty() {
        spec.supported_versions = req.supported_versions.clone();
    }
    spec.layout = if req.versioned_folders {
        ScaffoldLayout::Versioned
    } else {
        ScaffoldLayout::Flat
    };
    spec.patches_folder = req.patches_folder;
    spec.languages_folder = req.languages_folder;
    spec.assemblies_folder = req.assemblies_folder;
    spec.ce_patch_folder = req.ce_patch_folder;
    spec.placeholder_files = req.placeholder_files;
    spec
}

/// `project_open`: registers a mod folder as a project and returns its summary.
///
/// # Errors
/// `io.not-found` when the folder is not a mod, other mapped toolkit errors.
pub fn project_open(
    ctx: &AppContext,
    req: ProjectOpenRequest,
) -> Result<ProjectSummaryDto, ApiError> {
    let summary = project::open(ctx.workspace.env(), req).map_err(|e| ctx.toolkit_error(&e))?;
    ctx.workspace.note_opened(&summary.project_id);
    Ok(summary)
}

/// `project_create`: writes the scaffold of a new mod and opens it. Never overwrites a file and never
/// writes inside the game install, the Workshop folders or the user data folder.
///
/// # Errors
/// `designer.apply-failed` for an invalid spec or an existing file, `project.path-outside-root` for a
/// protected target, other mapped toolkit errors.
pub fn project_create(
    ctx: &AppContext,
    req: ProjectCreateRequest,
) -> Result<ProjectSummaryDto, ApiError> {
    let spec = scaffold_spec(&req);
    let (env, protected) = ctx.workspace.write_env(ctx)?;
    let report = project::create(&env, &spec, &protected).map_err(|e| ctx.toolkit_error(&e))?;
    let summary = report.project.summary;
    ctx.workspace.note_opened(&summary.project_id);
    tracing::info!(files = report.written.len(), "project created");
    Ok(summary)
}

/// `project_close`: releases a project. The record stays in the project store (it is the recent list).
///
/// # Errors
/// Mapped toolkit errors for a store problem.
pub fn project_close(
    ctx: &AppContext,
    req: ProjectCloseRequest,
) -> Result<ProjectCloseResponse, ApiError> {
    let id = req.project_id.clone();
    let response = project::close(ctx.workspace.env(), req).map_err(|e| ctx.toolkit_error(&e))?;
    ctx.workspace.note_closed(&id);
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_create_request_maps_to_the_scaffold_spec() {
        let req = ProjectCreateRequest {
            path: " /fiction/mods/RS_Mod ".into(),
            name: "RS_Mod".into(),
            package_id: "rs.mod".into(),
            versioned_folders: true,
            ce_patch_folder: true,
            supported_versions: vec!["1.5".into(), "1.6".into()],
            ..ProjectCreateRequest::default()
        };
        let spec = scaffold_spec(&req);
        assert_eq!(spec.target.as_str(), "/fiction/mods/RS_Mod");
        assert_eq!(spec.layout, ScaffoldLayout::Versioned);
        assert!(spec.ce_patch_folder);
        assert_eq!(spec.supported_versions, vec!["1.5", "1.6"]);
    }

    #[test]
    fn the_combat_extended_folder_is_off_unless_asked() {
        let spec = scaffold_spec(&ProjectCreateRequest::default());
        assert!(!spec.ce_patch_folder);
        assert_eq!(spec.supported_versions, vec!["1.6"]);
    }
}
