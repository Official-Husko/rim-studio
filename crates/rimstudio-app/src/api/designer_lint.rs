//! `designer_lint_files`: the Combat Extended lint over the patch files of a project.

use rimstudio_ipc_types::designer::{DesignerLintFilesRequest, DesignerLintFilesResult};
use rimstudio_ipc_types::error::ApiError;
use rimstudio_toolkit::designer as toolkit;

use crate::context::AppContext;

/// `designer_lint_files` (query): checks the patch files of a project, or the files named in the request.
/// Reads the project and writes nothing. A file that is not XML, is too large or has a document type
/// declaration is a finding, not an error. The rules that need Combat Extended data are listed as not
/// checked when none is loaded.
///
/// # Errors
/// `project.not-open` for an unknown project and the other mapped toolkit errors.
pub fn designer_lint_files(
    ctx: &AppContext,
    req: DesignerLintFilesRequest,
) -> Result<DesignerLintFilesResult, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::lint_files(&d, &req).map_err(|e| ctx.toolkit_error(&e))
}
