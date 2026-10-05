//! `library_scan`: the scan job over the detected and custom sources.

use rimstudio_core::jobs::{CancelToken, ProgressSink};
use rimstudio_ipc_types::error::ApiError;
use rimstudio_ipc_types::library::{
    LibraryScanRequest as LibraryScanRequestDto, LibraryScanResult as LibraryScanResultDto,
};
use rimstudio_manager::scan::{self, LibraryScanRequest, LibraryScanResult};

use crate::context::{AppContext, LibrarySnapshot};
use crate::convert;
use crate::jobs::{JobCtx, JobOutcome};

/// Runs a scan, keeps the index in the library handle when it completed and returns the DTO.
///
/// A cancelled scan keeps the previous snapshot and reports `cancelled: true` with the revision it
/// left in place.
///
/// # Errors
/// The mapped manager error (`library.scan-failed` when there is nothing to scan).
pub fn run_scan(
    ctx: &AppContext,
    req: LibraryScanRequest,
    progress: &dyn ProgressSink,
    cancel: &CancelToken,
) -> Result<LibraryScanResultDto, ApiError> {
    let result = ctx
        .with_manager(|m| scan::library_scan(m, req, progress, cancel))
        .map_err(|e| ctx.manager_error(&e))?;
    let rev = if result.cancelled {
        ctx.library.rev()
    } else {
        ctx.library.next_rev()
    };
    let dto = convert::scan_result_dto(&result, rev);
    if !result.cancelled {
        let sources = convert::source_reports_dto(&result);
        let LibraryScanResult {
            index,
            counts,
            game_version,
            ..
        } = result;
        let snapshot = ctx.library.install(LibrarySnapshot {
            rev,
            index,
            counts,
            sources,
            scanned_at_ms: ctx.now_ms(),
            game_version,
        });
        ctx.workspace.bump();
        tracing::info!(
            mods = snapshot.counts.mods,
            loadable = snapshot.counts.loadable,
            defs = snapshot.counts.defs,
            rev = snapshot.rev,
            "library scan finished"
        );
    }
    Ok(dto)
}

/// The `library_scan` job body.
///
/// # Errors
/// See [`run_scan`].
pub fn library_scan(
    ctx: &AppContext,
    req: LibraryScanRequestDto,
    job: &JobCtx,
) -> Result<JobOutcome<LibraryScanResultDto>, ApiError> {
    let manager_req = LibraryScanRequest {
        full: req.full,
        ..LibraryScanRequest::default()
    };
    let dto = run_scan(ctx, manager_req, job.progress(), job.cancel())?;
    Ok(if dto.cancelled {
        JobOutcome::cancelled(dto)
    } else {
        JobOutcome::done(dto)
    })
}
