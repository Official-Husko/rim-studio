//! `project_open`, `project_create`, `project_close` and the layout commands `project_tree`,
//! `project_layout_check`, `project_scaffold_missing` and `project_read_file`.

use camino::Utf8PathBuf;
use rimstudio_ipc_types::error::ApiError;
use rimstudio_ipc_types::project::{
    ProjectCloseRequest, ProjectCloseResponse, ProjectFileDto, ProjectLayoutCheckDto,
    ProjectLayoutCheckRequest, ProjectOpenRequest, ProjectReadFileRequest,
    ProjectScaffoldMissingDto, ProjectScaffoldMissingRequest, ProjectSummaryDto, ProjectTreeDto,
    ProjectTreeRequest,
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
    spec.textures_folder = req.textures_folder;
    spec.sounds_folder = req.sounds_folder;
    spec.source_folder = req.source_folder;
    spec.gitignore = req.gitignore;
    spec.ignore_source_art = req.ignore_source_art;
    spec.readme = req.readme;
    spec.credits = req.credits;
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

/// `project_tree`: the annotated folder tree of a project with its roles, sizes, counts and layout issues.
///
/// # Errors
/// `project.not-open` for an unknown project id, `io.not-found` when the folder is no longer a mod.
pub fn project_tree(ctx: &AppContext, req: ProjectTreeRequest) -> Result<ProjectTreeDto, ApiError> {
    project::tree(ctx.workspace.env(), &req).map_err(|e| ctx.toolkit_error(&e))
}

/// `project_layout_check`: the layout issues of a project with a suggested fix each. Read only.
///
/// # Errors
/// As [`project_tree`].
pub fn project_layout_check(
    ctx: &AppContext,
    req: ProjectLayoutCheckRequest,
) -> Result<ProjectLayoutCheckDto, ApiError> {
    project::layout_check(ctx.workspace.env(), &req).map_err(|e| ctx.toolkit_error(&e))
}

/// `project_scaffold_missing`: creates the missing standard folders of a project (never a file, never
/// anything outside the project root or inside the game install).
///
/// # Errors
/// `project.not-open`, `project.path-outside-root` when the project lies in a protected folder.
pub fn project_scaffold_missing(
    ctx: &AppContext,
    req: ProjectScaffoldMissingRequest,
) -> Result<ProjectScaffoldMissingDto, ApiError> {
    let (env, protected) = ctx.workspace.write_env(ctx)?;
    project::scaffold_missing::scaffold_missing(&env, &req, &protected)
        .map_err(|e| ctx.toolkit_error(&e))
}

/// `project_read_file`: the text of one file of a project, size limited, for the file viewer.
///
/// # Errors
/// `project.not-open`, `project.path-outside-root` for an unsafe path, `io.not-found` for a missing file.
pub fn project_read_file(
    ctx: &AppContext,
    req: ProjectReadFileRequest,
) -> Result<ProjectFileDto, ApiError> {
    let (env, protected) = ctx.workspace.write_env(ctx)?;
    project::read::read_file(&env, &req, &protected).map_err(|e| ctx.toolkit_error(&e))
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
