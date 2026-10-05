//! `project_scaffold_preview`: what a new mod would contain and what is wrong with its values.
//!
//! The handler takes the request of `project_create`, builds the same scaffold spec, hands the toolkit the
//! last library scan and the installed game version for the package id findings, and answers with the entries
//! and the findings. It writes nothing and never starts a scan.

use camino::Utf8Path;
use rimstudio_ipc_types::error::ApiError;
use rimstudio_ipc_types::project_new::ProjectScaffoldPreviewDto;
use rimstudio_toolkit::project::about_lint::{LintContext, PreviewFacts};
use rimstudio_toolkit::project::scaffold_preview;

use crate::api::project::scaffold_spec;
use crate::context::AppContext;
use crate::dto::ProjectCreateRequest;

/// `project_scaffold_preview`: the entries a create would write, the findings about the values, and
/// whether the target exists. Read only.
///
/// # Errors
/// None in practice; the findings carry the problems of the values.
pub fn project_scaffold_preview(
    ctx: &AppContext,
    req: ProjectCreateRequest,
) -> Result<ProjectScaffoldPreviewDto, ApiError> {
    let spec = scaffold_spec(&req);
    let snapshot = ctx.library.current();
    let root = spec.target.clone();
    let lint = LintContext {
        library: snapshot.as_deref().map(|s| (&s.index, root.as_path())),
        game_version: snapshot.as_deref().and_then(|s| s.game_version.as_ref()),
        preview: &PreviewFacts::default(),
        icon_exists: &|_| false,
    };
    let exists = |path: &Utf8Path| path.exists();
    Ok(scaffold_preview::preview(&spec, &lint, &exists))
}
