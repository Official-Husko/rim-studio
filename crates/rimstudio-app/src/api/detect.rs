//! `detect_run`, `detect_get_report` and `detect_set_override`.

use rimstudio_ipc_types::error::ApiError;
use rimstudio_ipc_types::library::{
    DetectGetReportRequest as GetReportRequestDto, DetectGetReportResponse, DetectRunRequest,
    DetectSetOverrideRequest, DetectionReportDto, PathFieldDto,
};
use rimstudio_manager::detect::{self, DetectGetReportRequest, OverrideField};

use crate::context::{AppContext, AppEvent};
use crate::convert;
use crate::jobs::{JobCtx, JobOutcome};

/// `detect_run` (job): the cached report unless `force`, otherwise a fresh detection. The report is
/// cached for the next call.
///
/// # Errors
/// The mapped manager error.
pub fn detect_run(
    ctx: &AppContext,
    req: DetectRunRequest,
    _job: &JobCtx,
) -> Result<JobOutcome<DetectionReportDto>, ApiError> {
    if !req.force {
        let cached = ctx
            .with_manager(|m| detect::get_report(m, DetectGetReportRequest::default()))
            .map_err(|e| ctx.manager_error(&e))?;
        if let (Some(report), false) = (cached.report, cached.ignored_cache) {
            return Ok(JobOutcome::done(convert::detection_report_dto(&report)));
        }
    }
    let run = ctx
        .with_manager(|m| detect::run(m, detect::DetectRunRequest::default()))
        .map_err(|e| ctx.manager_error(&e))?;
    ctx.workspace.bump();
    tracing::info!(
        installs = run.report.installs.len(),
        user_dirs = run.report.user_dirs.len(),
        warnings = run.report.warnings.len(),
        "detection finished"
    );
    Ok(JobOutcome::done(convert::detection_report_dto(&run.report)))
}

/// `detect_get_report`: the last cached report, absent when detection never ran.
///
/// # Errors
/// The mapped manager error.
pub fn detect_get_report(
    ctx: &AppContext,
    _req: GetReportRequestDto,
) -> Result<DetectGetReportResponse, ApiError> {
    let cached = ctx
        .with_manager(|m| detect::get_report(m, DetectGetReportRequest::default()))
        .map_err(|e| ctx.manager_error(&e))?;
    Ok(DetectGetReportResponse {
        report: cached.report.as_ref().map(convert::detection_report_dto),
    })
}

fn field_of(field: PathFieldDto) -> OverrideField {
    match field {
        PathFieldDto::GameInstall => OverrideField::GameInstall,
        PathFieldDto::UserDir => OverrideField::UserDir,
        PathFieldDto::SteamRoot => OverrideField::SteamRoot,
    }
}

/// `detect_set_override`: validates and stores a path override (an absent path clears it), runs
/// detection again and returns the new report.
///
/// # Errors
/// `detect.failed` for a folder that does not look right, `settings.invalid` for a read only
/// workspace document, other mapped manager errors.
pub fn detect_set_override(
    ctx: &AppContext,
    req: DetectSetOverrideRequest,
) -> Result<DetectGetReportResponse, ApiError> {
    let run = ctx
        .with_manager(|m| {
            detect::set_override(
                m,
                detect::DetectSetOverrideRequest {
                    field: field_of(req.field),
                    path: req.path,
                    pinned: true,
                },
            )
        })
        .map_err(|e| ctx.manager_error(&e))?;
    ctx.workspace.bump();
    ctx.events.emit(&AppEvent::SettingsChanged {
        rev: 0,
        sections: vec!["paths".to_owned()],
    });
    Ok(DetectGetReportResponse {
        report: Some(convert::detection_report_dto(&run.report)),
    })
}
