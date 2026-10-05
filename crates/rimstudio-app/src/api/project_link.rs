//! `project_link_status`, `project_link_create` and `project_link_remove`: make a project visible to the
//! game by linking it into `<game install>/Mods`.
//!
//! The only write under the game install in the whole app besides `ModsConfig.xml` (invariant I-05). It goes
//! through the write fence; refusals come back as values with the reason and the command to run by hand.

use rimstudio_ipc_types::error::ApiError;
use rimstudio_ipc_types::project_link::{
    ProjectLinkCreateRequest, ProjectLinkRemoveRequest, ProjectLinkResultDto, ProjectLinkStatusDto,
    ProjectLinkStatusRequest,
};
use rimstudio_toolkit::project::link::{self, LinkContext};

use crate::context::AppContext;
use crate::workspace::WorkspaceHub;

fn with_context<T>(
    ctx: &AppContext,
    f: impl FnOnce(&rimstudio_toolkit::shared::env::ProjectEnv, LinkContext<'_>) -> Result<T, ApiError>,
) -> Result<T, ApiError> {
    let (env, _protected) = ctx.workspace.write_env(ctx)?;
    let machine = WorkspaceHub::link_machine(ctx)?;
    let platform = &ctx.platform;
    let context = LinkContext {
        game_folder: machine.game_folder.as_deref(),
        mods_config: machine.mods_config.as_deref(),
        links: platform.links.as_ref(),
        probe: platform.process.as_ref(),
        os: platform.os,
        hides_processes: platform.sandbox.info().hides_processes,
    };
    f(&env, context)
}

/// `project_link_status`: whether the game can see the project and what is in the way. Read only.
///
/// # Errors
/// `project.not-open` for an unknown project id, `io.not-found` when the folder is no longer a mod.
pub fn project_link_status(
    ctx: &AppContext,
    req: ProjectLinkStatusRequest,
) -> Result<ProjectLinkStatusDto, ApiError> {
    with_context(ctx, |env, context| {
        link::link_status(env, context, &req).map_err(|e| ctx.toolkit_error(&e))
    })
}

/// `project_link_create`: links the project into the game's `Mods` folder. A refusal is part of the answer.
///
/// # Errors
/// As [`project_link_status`].
pub fn project_link_create(
    ctx: &AppContext,
    req: ProjectLinkCreateRequest,
) -> Result<ProjectLinkResultDto, ApiError> {
    with_context(ctx, |env, context| {
        link::link_create(env, context, &req).map_err(|e| ctx.toolkit_error(&e))
    })
}

/// `project_link_remove`: removes the link or marked copy RimStudio made for the project.
///
/// # Errors
/// As [`project_link_status`].
pub fn project_link_remove(
    ctx: &AppContext,
    req: ProjectLinkRemoveRequest,
) -> Result<ProjectLinkResultDto, ApiError> {
    with_context(ctx, |env, context| {
        link::link_remove(env, context, &req).map_err(|e| ctx.toolkit_error(&e))
    })
}
