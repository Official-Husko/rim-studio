//! Job handles, progress records and the terminal result envelope.
//!
//! A job command returns [`JobHandleDto`] at once; everything else arrives as [`JobEvent`] values on the
//! channel the caller passed. Exactly one terminal event ([`JobResultEnvelope`]) ends every job.

use rimstudio_core::jobs::{Progress, ProgressUnit};
use serde::{Deserialize, Serialize};

use crate::diagnostic::DiagnosticDto;
use crate::error::ApiError;

/// Returned when a job is registered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct JobHandleDto {
    /// The caller minted job id.
    pub job_id: String,
    /// Unix time in milliseconds when the job was registered.
    pub started_at_ms: u64,
}

/// What the numbers of a progress record count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum ProgressUnitDto {
    /// Generic items.
    #[default]
    Items,
    /// Files.
    Files,
    /// Bytes.
    Bytes,
}

impl From<ProgressUnit> for ProgressUnitDto {
    fn from(value: ProgressUnit) -> Self {
        match value {
            ProgressUnit::Items => Self::Items,
            ProgressUnit::Files => Self::Files,
            ProgressUnit::Bytes => Self::Bytes,
        }
    }
}

/// A progress record. The runner coalesces these to at most 20 per second per job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ProgressDto {
    /// Short phase key, for example `scan-metadata`; the webview localises it.
    pub phase: String,
    /// Units done so far.
    pub done: u64,
    /// Units in total. Absent while the total is unknown (an indeterminate bar).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub total: Option<u64>,
    /// What a unit is.
    pub unit: ProgressUnitDto,
    /// Short detail string, such as the folder being scanned. Absent when there is nothing to add.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub detail: Option<String>,
}

impl From<&Progress> for ProgressDto {
    fn from(value: &Progress) -> Self {
        Self {
            phase: value.phase.clone(),
            done: value.done,
            total: value.total,
            unit: value.unit.into(),
            detail: value.detail.clone(),
        }
    }
}

impl From<Progress> for ProgressDto {
    fn from(value: Progress) -> Self {
        Self::from(&value)
    }
}

/// The terminal event of a job: exactly one is sent per job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum JobResultEnvelope<R> {
    /// The job completed.
    Finished {
        /// The job id.
        job_id: String,
        /// The typed result.
        result: R,
        /// Wall time of the job in milliseconds.
        elapsed_ms: u64,
        /// Content problems found on the way; empty when none.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "ts", ts(as = "Option<Vec<DiagnosticDto>>", optional))]
        diagnostics: Vec<DiagnosticDto>,
    },
    /// The job failed.
    Failed {
        /// The job id.
        job_id: String,
        /// The error envelope.
        error: ApiError,
        /// Wall time of the job in milliseconds.
        elapsed_ms: u64,
    },
    /// The job was cancelled.
    Cancelled {
        /// The job id.
        job_id: String,
        /// Wall time of the job in milliseconds.
        elapsed_ms: u64,
        /// The partial result. Absent when the job keeps nothing useful from a cancelled run.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        partial: Option<R>,
    },
}

impl<R> JobResultEnvelope<R> {
    /// The id of the job the envelope ends.
    #[must_use]
    pub fn job_id(&self) -> &str {
        match self {
            Self::Finished { job_id, .. }
            | Self::Failed { job_id, .. }
            | Self::Cancelled { job_id, .. } => job_id,
        }
    }
}

/// Everything a job channel carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum JobEvent<R> {
    /// First message, never coalesced.
    Started {
        /// The job id.
        job_id: String,
        /// Catalog key of the job label.
        label_key: String,
        /// The phases the job goes through, in order. Empty when unknown.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
        phases: Vec<String>,
    },
    /// A progress record; the latest one wins.
    Progress {
        /// The job id.
        job_id: String,
        /// The record.
        progress: ProgressDto,
    },
    /// Terminal: the job completed.
    Finished {
        /// The job id.
        job_id: String,
        /// The typed result.
        result: R,
        /// Wall time of the job in milliseconds.
        elapsed_ms: u64,
        /// Content problems found on the way; empty when none.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "ts", ts(as = "Option<Vec<DiagnosticDto>>", optional))]
        diagnostics: Vec<DiagnosticDto>,
    },
    /// Terminal: the job failed.
    Failed {
        /// The job id.
        job_id: String,
        /// The error envelope.
        error: ApiError,
        /// Wall time of the job in milliseconds.
        elapsed_ms: u64,
    },
    /// Terminal: the job was cancelled.
    Cancelled {
        /// The job id.
        job_id: String,
        /// Wall time of the job in milliseconds.
        elapsed_ms: u64,
        /// The partial result. Absent when nothing useful is kept.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        partial: Option<R>,
    },
}

impl<R> JobEvent<R> {
    /// True for the three terminal variants.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Finished { .. } | Self::Failed { .. } | Self::Cancelled { .. }
        )
    }
}

impl<R> From<JobResultEnvelope<R>> for JobEvent<R> {
    fn from(value: JobResultEnvelope<R>) -> Self {
        match value {
            JobResultEnvelope::Finished {
                job_id,
                result,
                elapsed_ms,
                diagnostics,
            } => Self::Finished {
                job_id,
                result,
                elapsed_ms,
                diagnostics,
            },
            JobResultEnvelope::Failed {
                job_id,
                error,
                elapsed_ms,
            } => Self::Failed {
                job_id,
                error,
                elapsed_ms,
            },
            JobResultEnvelope::Cancelled {
                job_id,
                elapsed_ms,
                partial,
            } => Self::Cancelled {
                job_id,
                elapsed_ms,
                partial,
            },
        }
    }
}

/// The state `cancel_job` reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum CancelStateDto {
    /// The token was set; the job is winding down.
    Cancelling,
    /// The job had already ended.
    Finished,
    /// The runner does not know the id.
    Unknown,
}

/// Request of `cancel_job`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CancelJobRequest {
    /// The job to cancel.
    pub job_id: String,
}

/// Response of `cancel_job`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CancelJobResponse {
    /// The state after the call.
    pub state: CancelStateDto,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_conversion_keeps_every_field() {
        let mut p = Progress::new("scan-metadata", 3);
        p.total = Some(10);
        p.unit = ProgressUnit::Files;
        p.detail = Some("RS_Folder".into());
        let dto = ProgressDto::from(&p);
        assert_eq!(dto.phase, "scan-metadata");
        assert_eq!((dto.done, dto.total), (3, Some(10)));
        assert_eq!(dto.unit, ProgressUnitDto::Files);
        assert_eq!(dto.detail.as_deref(), Some("RS_Folder"));
    }

    #[test]
    fn terminal_envelope_is_tagged_and_round_trips() {
        let done: JobResultEnvelope<u32> = JobResultEnvelope::Finished {
            job_id: "j1".into(),
            result: 7,
            elapsed_ms: 12,
            diagnostics: Vec::new(),
        };
        let text = serde_json::to_string(&done).unwrap_or_default();
        assert_eq!(
            text,
            r#"{"kind":"finished","jobId":"j1","result":7,"elapsedMs":12}"#
        );
        let back: Result<JobResultEnvelope<u32>, _> = serde_json::from_str(&text);
        assert_eq!(back.ok(), Some(done));
    }

    #[test]
    fn cancelled_envelope_omits_absent_partial() {
        let c: JobResultEnvelope<u32> = JobResultEnvelope::Cancelled {
            job_id: "j2".into(),
            elapsed_ms: 1,
            partial: None,
        };
        let text = serde_json::to_string(&c).unwrap_or_default();
        assert_eq!(text, r#"{"kind":"cancelled","jobId":"j2","elapsedMs":1}"#);
        assert_eq!(c.job_id(), "j2");
    }

    #[test]
    fn events_round_trip() {
        let events: Vec<JobEvent<u32>> = vec![
            JobEvent::Started {
                job_id: "j".into(),
                label_key: "job.scan".into(),
                phases: vec!["a".into(), "b".into()],
            },
            JobEvent::Progress {
                job_id: "j".into(),
                progress: ProgressDto {
                    phase: "a".into(),
                    done: 1,
                    total: None,
                    unit: ProgressUnitDto::Items,
                    detail: None,
                },
            },
            JobEvent::from(JobResultEnvelope::<u32>::Failed {
                job_id: "j".into(),
                error: ApiError::with_id("job.panicked", "x", "e-00000002"),
                elapsed_ms: 5,
            }),
        ];
        for event in events {
            assert_eq!(
                event.is_terminal(),
                matches!(event, JobEvent::Failed { .. })
            );
            let text = serde_json::to_string(&event).unwrap_or_default();
            let back: Result<JobEvent<u32>, _> = serde_json::from_str(&text);
            assert_eq!(back.ok(), Some(event), "{text}");
        }
    }
}
