//! `cancel_job` and `job_status`: the questions every job screen asks of the runner.

use rimstudio_ipc_types::error::ApiError;
use rimstudio_ipc_types::jobs::{CancelJobRequest, CancelJobResponse};

use crate::context::AppContext;
use crate::dto::{JobStateDto, JobStatusRequest, JobStatusResponse};

/// `cancel_job`: sets the cancel token of a job. Idempotent; an unknown id answers `unknown`.
///
/// # Errors
/// Never fails.
pub fn cancel_job(ctx: &AppContext, req: CancelJobRequest) -> Result<CancelJobResponse, ApiError> {
    Ok(CancelJobResponse {
        state: ctx.jobs.cancel(&req.job_id),
    })
}

/// `job_status`: the state, the latest progress and, once ended, the terminal event of a job. Jobs
/// stay known for 60 seconds after they end.
///
/// # Errors
/// Never fails; an unknown id answers state `unknown`.
pub fn job_status(ctx: &AppContext, req: JobStatusRequest) -> Result<JobStatusResponse, ApiError> {
    Ok(match ctx.jobs.status(&req.job_id) {
        Some(status) => JobStatusResponse {
            job_id: req.job_id,
            state: status.state.into(),
            label_key: Some(status.label_key),
            started_at_ms: Some(status.started_at_ms),
            progress: status.progress,
            terminal: status.terminal,
        },
        None => JobStatusResponse {
            job_id: req.job_id,
            state: JobStateDto::Unknown,
            label_key: None,
            started_at_ms: None,
            progress: None,
            terminal: None,
        },
    })
}
