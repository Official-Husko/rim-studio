//! `defs_search` and `defs_get_resolved`: the def explorer over a workspace session.
//!
//! The `sessionId` of a request names the session: empty or `reference` for the game and its
//! expansions, `ce` for the reference set with Combat Extended, `project:<projectId>` for the reference
//! set with an open project as its last pack. The sessions are built on first use (see
//! [`crate::workspace`]); there is no separate open step in 0.1.0.

use rimstudio_ipc_types::defs::{
    DefPage, DefSearchRequest, DefsGetResolvedRequest, ResolvedDefDto,
};
use rimstudio_ipc_types::error::{ApiError, codes};
use rimstudio_toolkit::defs;
use rimstudio_toolkit::error::ToolkitError;
use rimstudio_workspace::WorkspaceError;

use crate::context::AppContext;
use crate::workspace::SessionSelector;

/// `defs_search`: a page of definitions.
///
/// # Errors
/// `defs.session-not-found` for an unknown session, `designer.reference-unavailable` without a game
/// install, other mapped errors from building the session.
pub fn defs_search(ctx: &AppContext, req: DefSearchRequest) -> Result<DefPage, ApiError> {
    let selector = SessionSelector::parse(&req.session_id)?;
    let session = ctx.workspace.session(ctx, &selector)?;
    defs::search(&session, &req).map_err(|e| ctx.toolkit_error(&e))
}

/// `defs_get_resolved`: one definition after inheritance and patches, with its patch events.
///
/// # Errors
/// `defs.invalid-query` when the snapshot has no such definition, and the errors of
/// [`defs_search`].
pub fn defs_get_resolved(
    ctx: &AppContext,
    req: DefsGetResolvedRequest,
) -> Result<ResolvedDefDto, ApiError> {
    let selector = SessionSelector::parse(&req.session_id)?;
    let session = ctx.workspace.session(ctx, &selector)?;
    defs::resolve(&session, &req).map_err(|e| match &e {
        ToolkitError::Workspace(WorkspaceError::DefNotFound { def_type, def_name }) => {
            ApiError::new(
                codes::DEFS_INVALID_QUERY,
                ctx.redactor.redact(&e.to_string()),
            )
            .detail("defType", def_type.as_str())
            .detail("defName", def_name.as_str())
        }
        _ => ctx.toolkit_error(&e),
    })
}
