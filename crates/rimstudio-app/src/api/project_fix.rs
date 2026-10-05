//! `project_layout_fix_plan`, `project_layout_fix_apply`, `project_layout_fix_undo` and
//! `project_layout_fix_history`: carrying out the suggestions of the layout check, with an undo journal.
//!
//! The handlers call the project tool of the toolkit. Nothing is written outside the project folder and the
//! data folder (the backups and the journals); every move goes through the guarded writer, and the plan the
//! caller reviewed must still be the current plan.

use rimstudio_ipc_types::error::ApiError;
use rimstudio_ipc_types::project_fix::{
    ProjectLayoutFixApplyDto, ProjectLayoutFixApplyRequest, ProjectLayoutFixHistoryDto,
    ProjectLayoutFixHistoryRequest, ProjectLayoutFixPlanDto, ProjectLayoutFixPlanRequest,
    ProjectLayoutFixUndoDto, ProjectLayoutFixUndoRequest,
};
use rimstudio_toolkit::project::{fix, fix_apply, history, undo};

use crate::context::AppContext;
use crate::jobs::{JobCtx, JobOutcome};

/// `project_layout_fix_plan`: the fixable findings of the layout check as a list of changes. Read only.
///
/// # Errors
/// `project.not-open` for an unknown project id, `io.not-found` when the folder is no longer a mod,
/// `project.path-outside-root` when the project lies in a protected folder.
pub fn project_layout_fix_plan(
    ctx: &AppContext,
    req: ProjectLayoutFixPlanRequest,
) -> Result<ProjectLayoutFixPlanDto, ApiError> {
    let (env, protected) = ctx.workspace.write_env(ctx)?;
    fix::plan(&env, &req, &protected).map_err(|e| ctx.toolkit_error(&e))
}

/// `project_layout_fix_apply` (job): carries out the selected items of the plan the caller reviewed, writes
/// an undo journal first and runs the layout check again afterwards.
///
/// # Errors
/// `designer.plan-stale` when the project changed since the plan, `job.cancelled` before the first change,
/// `project.path-outside-root` for a refused path, `io.write-failed` for the journal.
pub fn project_layout_fix_apply(
    ctx: &AppContext,
    req: ProjectLayoutFixApplyRequest,
    job: &JobCtx,
) -> Result<JobOutcome<ProjectLayoutFixApplyDto>, ApiError> {
    let (env, protected) = ctx.workspace.write_env(ctx)?;
    fix_apply::apply(&env, &req, &protected, job.progress(), job.cancel())
        .map(JobOutcome::done)
        .map_err(|e| ctx.toolkit_error(&e))
}

/// `project_layout_fix_undo`: reverses an apply from its journal when every moved path still holds the
/// recorded bytes; otherwise it changes nothing and says why.
///
/// # Errors
/// `project.fix-not-found`, `project.fix-journal-damaged`, `project.fix-undo-refused`,
/// `project.path-outside-root`.
pub fn project_layout_fix_undo(
    ctx: &AppContext,
    req: ProjectLayoutFixUndoRequest,
) -> Result<ProjectLayoutFixUndoDto, ApiError> {
    let (env, protected) = ctx.workspace.write_env(ctx)?;
    undo::undo(&env, &req, &protected).map_err(|e| ctx.toolkit_error(&e))
}

/// `project_layout_fix_history`: the journals of a project, newest first, with whether each can still be
/// undone. Read only.
///
/// # Errors
/// `project.not-open`, `io.not-found`, `project.path-outside-root`.
pub fn project_layout_fix_history(
    ctx: &AppContext,
    req: ProjectLayoutFixHistoryRequest,
) -> Result<ProjectLayoutFixHistoryDto, ApiError> {
    let (env, protected) = ctx.workspace.write_env(ctx)?;
    history::history(&env, &req, &protected).map_err(|e| ctx.toolkit_error(&e))
}
