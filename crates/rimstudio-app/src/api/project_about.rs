//! The mod basics commands: `project_about_*`, `project_load_folders_*`, `project_version_add` and
//! `library_mod_search`.
//!
//! Handlers only wire the toolkit to the app: they take the write environment (the fenced project
//! environment and the protected folders), hand the toolkit the last library scan and the installed game
//! version as inputs for the findings, and map toolkit errors to `ApiError`. They never start a scan.

use rimstudio_ipc_types::error::ApiError;
use rimstudio_ipc_types::library::SourceKindDto;
use rimstudio_ipc_types::project_about::{
    LibraryModHitDto, LibraryModSearchDto, LibraryModSearchRequest, ProjectAboutDto,
    ProjectAboutGetRequest, ProjectAboutPreviewDto, ProjectAboutPreviewRequest,
    ProjectAboutRemovePreviewDto, ProjectAboutRemovePreviewRequest, ProjectAboutSetPreviewDto,
    ProjectAboutSetPreviewRequest, ProjectAboutUpdateDto, ProjectAboutUpdateRequest,
    ProjectLoadFoldersDto, ProjectLoadFoldersGetRequest, ProjectLoadFoldersUpdateDto,
    ProjectLoadFoldersUpdateRequest, ProjectVersionAddDto, ProjectVersionAddRequest,
};
use rimstudio_library::query::{search_mods, workshop_url};
use rimstudio_toolkit::project::about::{self, AboutContext};
use rimstudio_toolkit::project::{about_preview, load_folders, versions};

use crate::context::AppContext;

/// The default and the largest number of rows of `library_mod_search`.
const DEFAULT_ROWS: usize = 20;

fn with_context<R>(
    ctx: &AppContext,
    f: impl FnOnce(&AboutContext<'_>) -> Result<R, ApiError>,
) -> Result<R, ApiError> {
    let snapshot = ctx.library.current();
    let about_ctx = AboutContext {
        library: snapshot.as_deref().map(|s| &s.index),
        game_version: snapshot.as_deref().and_then(|s| s.game_version.as_ref()),
    };
    f(&about_ctx)
}

/// `project_about_get`: every basic of a mod with the findings. Read only.
///
/// # Errors
/// `project.not-open`, `io.not-found` when the folder has no About file, `project.path-outside-root`.
pub fn project_about_get(
    ctx: &AppContext,
    req: ProjectAboutGetRequest,
) -> Result<ProjectAboutDto, ApiError> {
    let (env, protected) = ctx.workspace.write_env(ctx)?;
    with_context(ctx, |c| {
        about::get(&env, &req, c, &protected).map_err(|e| ctx.toolkit_error(&e))
    })
}

/// `project_about_preview`: the diff and the result of a list of changes; nothing is written.
///
/// # Errors
/// `project.file-stale`, `project.file-not-editable`, `project.edit-invalid`, and those of
/// [`project_about_get`].
pub fn project_about_preview(
    ctx: &AppContext,
    req: ProjectAboutPreviewRequest,
) -> Result<ProjectAboutPreviewDto, ApiError> {
    let (env, protected) = ctx.workspace.write_env(ctx)?;
    with_context(ctx, |c| {
        about::preview(&env, &req, c, &protected).map_err(|e| ctx.toolkit_error(&e))
    })
}

/// `project_about_update`: applies the changes as byte span edits, with a backup and a read back.
///
/// # Errors
/// As [`project_about_preview`], and `io.write-failed` for a failed write.
pub fn project_about_update(
    ctx: &AppContext,
    req: ProjectAboutUpdateRequest,
) -> Result<ProjectAboutUpdateDto, ApiError> {
    let (env, protected) = ctx.workspace.write_env(ctx)?;
    let out = with_context(ctx, |c| {
        about::update(&env, &req, c, &protected).map_err(|e| ctx.toolkit_error(&e))
    })?;
    if out.written {
        tracing::info!(project = %req.project_id, "About.xml edited");
    }
    Ok(out)
}

/// `project_about_set_preview`: copies a PNG to `About/Preview.png`.
///
/// # Errors
/// `project.edit-invalid` for a source that is not a complete PNG, and those of [`project_about_get`].
pub fn project_about_set_preview(
    ctx: &AppContext,
    req: ProjectAboutSetPreviewRequest,
) -> Result<ProjectAboutSetPreviewDto, ApiError> {
    let (env, protected) = ctx.workspace.write_env(ctx)?;
    about_preview::set_preview(&env, &req, &protected).map_err(|e| ctx.toolkit_error(&e))
}

/// `project_about_remove_preview`: removes `About/Preview.png` after a backup.
///
/// # Errors
/// As [`project_about_get`].
pub fn project_about_remove_preview(
    ctx: &AppContext,
    req: ProjectAboutRemovePreviewRequest,
) -> Result<ProjectAboutRemovePreviewDto, ApiError> {
    let (env, protected) = ctx.workspace.write_env(ctx)?;
    about_preview::remove_preview(&env, &req, &protected).map_err(|e| ctx.toolkit_error(&e))
}

/// `project_load_folders_get`: the blocks and entries of `LoadFolders.xml`. Read only.
///
/// # Errors
/// `project.not-open`, `project.path-outside-root`.
pub fn project_load_folders_get(
    ctx: &AppContext,
    req: ProjectLoadFoldersGetRequest,
) -> Result<ProjectLoadFoldersDto, ApiError> {
    let (env, protected) = ctx.workspace.write_env(ctx)?;
    load_folders::get(&env, &req, &protected).map_err(|e| ctx.toolkit_error(&e))
}

/// `project_load_folders_update`: applies the changes to `LoadFolders.xml` (creating it when asked).
///
/// # Errors
/// `project.file-stale`, `project.file-not-editable`, `project.edit-invalid`, and those of
/// [`project_load_folders_get`].
pub fn project_load_folders_update(
    ctx: &AppContext,
    req: ProjectLoadFoldersUpdateRequest,
) -> Result<ProjectLoadFoldersUpdateDto, ApiError> {
    let (env, protected) = ctx.workspace.write_env(ctx)?;
    load_folders::update(&env, &req, &protected).map_err(|e| ctx.toolkit_error(&e))
}

/// `project_version_add`: creates the folder of one more game version and its block.
///
/// # Errors
/// `project.edit-invalid` for a version that is not `major.minor`, and those of
/// [`project_load_folders_update`].
pub fn project_version_add(
    ctx: &AppContext,
    req: ProjectVersionAddRequest,
) -> Result<ProjectVersionAddDto, ApiError> {
    let (env, protected) = ctx.workspace.write_env(ctx)?;
    versions::add(&env, &req, &protected).map_err(|e| ctx.toolkit_error(&e))
}

/// `library_mod_search`: mods of the last scan by name, package id or author. Never scans.
///
/// # Errors
/// None: with no scan the answer is empty and carries a hint.
pub fn library_mod_search(
    ctx: &AppContext,
    req: LibraryModSearchRequest,
) -> Result<LibraryModSearchDto, ApiError> {
    let Some(snapshot) = ctx.library.current() else {
        return Ok(LibraryModSearchDto {
            scanned: false,
            hits: Vec::new(),
            hint: Some(
                "The library has not been scanned yet. Scan it to search your mods.".to_owned(),
            ),
        });
    };
    if req.query.trim().is_empty() {
        return Ok(LibraryModSearchDto {
            scanned: true,
            hits: Vec::new(),
            hint: Some("Type a name, a package id or an author.".to_owned()),
        });
    }
    let limit = req
        .limit
        .map_or(DEFAULT_ROWS, |n| usize::try_from(n).unwrap_or(DEFAULT_ROWS));
    let kind_of = |source: &str| {
        snapshot
            .sources
            .iter()
            .find(|s| s.id == source)
            .map_or(SourceKindDto::Custom, |s| s.kind)
    };
    let hits: Vec<LibraryModHitDto> = search_mods(&snapshot.index, &req.query, limit)
        .into_iter()
        .map(|row| LibraryModHitDto {
            package_id: row.package_id,
            name: row.name,
            authors: row.authors,
            source: kind_of(row.source.as_str()),
            workshop_id: row.workshop_id.map(|w| w.to_string()),
            workshop_url: row.workshop_id.map(workshop_url),
            path: row.path.to_string(),
            loadable: row.loadable,
            url: row.url,
        })
        .collect();
    let hint = hits
        .is_empty()
        .then(|| "No mod of the last scan matches.".to_owned());
    Ok(LibraryModSearchDto {
        scanned: true,
        hits,
        hint,
    })
}
