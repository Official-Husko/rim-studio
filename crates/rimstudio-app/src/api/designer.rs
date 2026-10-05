//! The `designer_*` commands the toolkit implements.
//!
//! Every handler gets the toolkit context from the workspace hub (built on first use) and calls the
//! matching function of `rimstudio-toolkit`. The designer always writes vanilla definitions; the
//! Combat Extended patch is part of a plan only when the draft carries the opt in block.

use rimstudio_ipc_types::designer::{
    ApplyReportDto, CalibrateResultDto, CeSuggestionDto, ConvertScanDto, DesignerApplyPlanRequest,
    DesignerAssetInfoRequest, DesignerAssetInfoResponse, DesignerCalibrateRequest,
    DesignerCeSuggestRequest, DesignerCloneDiffRequest, DesignerCloneDiffResponse,
    DesignerCloneRequest, DesignerCloneResponse, DesignerConvertScanRequest,
    DesignerDraftDeleteRequest, DesignerDraftDeleteResponse, DesignerDraftListRequest,
    DesignerDraftListResponse, DesignerDraftSaveRequest, DesignerDraftSaveResponse,
    DesignerExportPlanRequest, DesignerFitRequest, DesignerPreviewRequest,
    DesignerProjectileOwnRequest, DesignerProjectileOwnResponse, DesignerQuizAnswerRequest,
    DesignerQuizAnswerResponse, DesignerQuizBackRequest, DesignerQuizNextRequest,
    DesignerReferenceListRequest, DesignerStructureDefaultsRequest,
    DesignerStructureDefaultsResponse, FitReportDto, PreviewDto, QuizStepDto, ReferenceListDto,
    WritePlanDto,
};
use rimstudio_ipc_types::error::ApiError;
use rimstudio_toolkit::designer as toolkit;

use crate::context::AppContext;
use crate::jobs::{JobCtx, JobOutcome};

/// `designer_reference_list`: the reference weapons of a kind, with per stat summaries.
///
/// # Errors
/// `designer.reference-unavailable` without a game install.
pub fn designer_reference_list(
    ctx: &AppContext,
    req: DesignerReferenceListRequest,
) -> Result<ReferenceListDto, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::reference_list(&d, req).map_err(|e| ctx.toolkit_error(&e))
}

/// `designer_preview`: exact readouts, suggestions and bands for a draft.
///
/// # Errors
/// Mapped toolkit errors (`designer.invalid-draft` for a draft that cannot be read).
pub fn designer_preview(
    ctx: &AppContext,
    req: DesignerPreviewRequest,
) -> Result<PreviewDto, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::preview(&d, req).map_err(|e| ctx.toolkit_error(&e))
}

/// `designer_fit`: the fit meter data of a draft.
///
/// # Errors
/// Mapped toolkit errors.
pub fn designer_fit(ctx: &AppContext, req: DesignerFitRequest) -> Result<FitReportDto, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::fit(&d, req).map_err(|e| ctx.toolkit_error(&e))
}

/// `designer_ce_suggest`: the suggestions for the optional Combat Extended block of a draft. Read only; a
/// draft whose Combat Extended toggle is off is allowed and is never turned on.
///
/// # Errors
/// Mapped toolkit errors (`designer.invalid-draft` for a draft that cannot be read). Without Combat Extended
/// data the answer is a suggestion that is not available and carries the plain reason.
pub fn designer_ce_suggest(
    ctx: &AppContext,
    req: DesignerCeSuggestRequest,
) -> Result<CeSuggestionDto, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::ce_suggest(&d, req).map_err(|e| ctx.toolkit_error(&e))
}

/// `designer_quiz_next`: the next question of the estimate quiz.
///
/// # Errors
/// Mapped toolkit errors (`designer.quiz-finished` when no question is open).
pub fn designer_quiz_next(
    ctx: &AppContext,
    req: DesignerQuizNextRequest,
) -> Result<QuizStepDto, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::quiz_next(&d, req).map_err(|e| ctx.toolkit_error(&e))
}

/// `designer_quiz_answer`: applies an answer and returns the updated estimate.
///
/// # Errors
/// Mapped toolkit errors (`designer.quiz-wrong-answer`).
pub fn designer_quiz_answer(
    ctx: &AppContext,
    req: DesignerQuizAnswerRequest,
) -> Result<DesignerQuizAnswerResponse, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::quiz_answer(&d, req).map_err(|e| ctx.toolkit_error(&e))
}

/// `designer_quiz_back`: takes the last answer of the draft's quiz back and returns the new step.
///
/// # Errors
/// Mapped toolkit errors (`designer.quiz-wrong-answer` when there is no answer to take back).
pub fn designer_quiz_back(
    ctx: &AppContext,
    req: DesignerQuizBackRequest,
) -> Result<DesignerQuizAnswerResponse, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::quiz_back(&d, req).map_err(|e| ctx.toolkit_error(&e))
}

/// `designer_calibrate` (job): builds the pools and runs the leave one out harness; cached.
///
/// # Errors
/// `designer.calibration-unavailable` for a pool that is too small, `job.cancelled` when cancelled.
pub fn designer_calibrate(
    ctx: &AppContext,
    req: DesignerCalibrateRequest,
    job: &JobCtx,
) -> Result<JobOutcome<CalibrateResultDto>, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::calibrate(&d, req, job.progress(), job.cancel())
        .map(JobOutcome::done)
        .map_err(|e| ctx.toolkit_error(&e))
}

/// `designer_convert_scan` (job): lists the convertible weapons of a project with their status.
///
/// # Errors
/// `designer.reference-unavailable` without Combat Extended data, `project.not-open`.
pub fn designer_convert_scan(
    ctx: &AppContext,
    req: DesignerConvertScanRequest,
    job: &JobCtx,
) -> Result<JobOutcome<ConvertScanDto>, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::convert_scan(&d, req, job.progress(), job.cancel())
        .map(JobOutcome::done)
        .map_err(|e| ctx.toolkit_error(&e))
}

/// `designer_export_plan`: the write plan of a draft, pure (nothing is written).
///
/// # Errors
/// `project.not-open` and the other mapped toolkit errors.
pub fn designer_export_plan(
    ctx: &AppContext,
    req: DesignerExportPlanRequest,
) -> Result<WritePlanDto, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::export_plan(&d, req).map_err(|e| ctx.toolkit_error(&e))
}

/// `designer_apply_plan` (job): rebuilds the plan, refuses a changed one, writes with backups and
/// reads back.
///
/// # Errors
/// `designer.plan-stale`, `designer.apply-failed`, `job.cancelled` before the first file.
pub fn designer_apply_plan(
    ctx: &AppContext,
    req: DesignerApplyPlanRequest,
    job: &JobCtx,
) -> Result<JobOutcome<ApplyReportDto>, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::apply_plan(&d, req, job.progress(), job.cancel())
        .map(JobOutcome::done)
        .map_err(|e| ctx.toolkit_error(&e))
}

/// `designer_draft_save`: stores a draft in the `designer-drafts` collection.
///
/// # Errors
/// `designer.invalid-draft`, `designer.draft-newer-schema`, `io.write-failed`.
pub fn designer_draft_save(
    ctx: &AppContext,
    req: DesignerDraftSaveRequest,
) -> Result<DesignerDraftSaveResponse, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::draft_save(&d, req).map_err(|e| ctx.toolkit_error(&e))
}

/// `designer_draft_list`: the drafts of a project, newest first.
///
/// # Errors
/// `designer.invalid-draft` for a bad project id, `io.write-failed` for a store problem.
pub fn designer_draft_list(
    ctx: &AppContext,
    req: DesignerDraftListRequest,
) -> Result<DesignerDraftListResponse, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::draft_list(&d, req).map_err(|e| ctx.toolkit_error(&e))
}

/// `designer_draft_delete`: deletes a draft; an unknown id answers `deleted: false`.
///
/// # Errors
/// `designer.invalid-draft` for a bad id.
pub fn designer_draft_delete(
    ctx: &AppContext,
    req: DesignerDraftDeleteRequest,
) -> Result<DesignerDraftDeleteResponse, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::draft_delete(&d, req).map_err(|e| ctx.toolkit_error(&e))
}

/// `designer_clone`: starts a draft from an existing weapon of the loaded defs and stores it. The clone is
/// vanilla (the Combat Extended toggle is off), records the source as its first anchor and never edits it.
///
/// # Errors
/// `designer.invalid-draft` for a bad project id, an unusable new name or a source with a Combat Extended
/// conversion, `designer.reference-unavailable` without an install or for an unknown source,
/// `design.invalid-input` for a def that is not a weapon, `io.write-failed` for a store problem.
pub fn designer_clone(
    ctx: &AppContext,
    req: DesignerCloneRequest,
) -> Result<DesignerCloneResponse, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::clone_draft(&d, req).map_err(|e| ctx.toolkit_error(&e))
}

/// `designer_clone_diff`: the changed fields of a clone against its source with the effect on the exact
/// readouts. Read only.
///
/// # Errors
/// `designer.invalid-draft` for a draft that is not a clone, `designer.reference-unavailable` without an
/// install or when the source is no longer loaded.
pub fn designer_clone_diff(
    ctx: &AppContext,
    req: DesignerCloneDiffRequest,
) -> Result<DesignerCloneDiffResponse, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::clone_diff(&d, req).map_err(|e| ctx.toolkit_error(&e))
}

/// `designer_structure_defaults`: fills the empty parent, projectile, cost list and stuff of a new weapon
/// draft from the nearest reference weapon, as suggestions. Read only.
///
/// # Errors
/// `designer.invalid-draft` for a draft that cannot be read, `designer.reference-unavailable` without an
/// install.
pub fn designer_structure_defaults(
    ctx: &AppContext,
    req: DesignerStructureDefaultsRequest,
) -> Result<DesignerStructureDefaultsResponse, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::structure_defaults(&d, req).map_err(|e| ctx.toolkit_error(&e))
}

/// `designer_projectile_own`: switches the own projectile of a draft on or off. On copies the projectile
/// the weapon points at into a projectile of its own (new name derived from the weapon); off points the
/// weapon back at the projectile it was copied from. Read only: the answer is not stored.
///
/// # Errors
/// `designer.invalid-draft` for a draft that cannot be read or a switch that cannot be made (the message
/// says why), `designer.reference-unavailable` without an install.
pub fn designer_projectile_own(
    ctx: &AppContext,
    req: DesignerProjectileOwnRequest,
) -> Result<DesignerProjectileOwnResponse, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::projectile_own(&d, req).map_err(|e| ctx.toolkit_error(&e))
}

/// `designer_asset_info`: the facts of one texture or sound clip file the page offers to import: format by
/// signature, size, SHA-256, dimensions or channels, a thumbnail data URL for a small PNG and the
/// diagnostics of importing it. The file is read under the designer's size limits and never decoded; a
/// link or a folder is refused. Read only.
///
/// # Errors
/// `project.not-open` when `projectId` names no open project. A missing, refused or oversized file is a
/// status of the answer.
pub fn designer_asset_info(
    ctx: &AppContext,
    req: DesignerAssetInfoRequest,
) -> Result<DesignerAssetInfoResponse, ApiError> {
    let d = ctx.workspace.designer_ctx(ctx)?;
    toolkit::asset_info(&d, req).map_err(|e| ctx.toolkit_error(&e))
}
