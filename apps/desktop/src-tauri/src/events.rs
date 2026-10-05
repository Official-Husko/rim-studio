//! The JSON of a job event as the webview receives it.
//!
//! The shape is the one of the development bridge's `/dev/events` stream, so the frontend's job store
//! reads both transports with the same code: `{"type":"job-progress","jobId","command","message",
//! "done","total"}` and `{"type":"job-finished","jobId","command","ok"}`.

use rimstudio_ipc_types::jobs::JobEvent;
use serde_json::{Value, json};

/// The name of the Tauri event that carries every job event.
pub const JOB_EVENT: &str = "rs-job";

/// The event of a job event, `None` for `started` (the call itself is the start).
#[must_use]
pub fn job_event_json(command: &str, event: &JobEvent<Value>) -> Option<Value> {
    match event {
        JobEvent::Started { .. } => None,
        JobEvent::Progress { job_id, progress } => {
            let message = match &progress.detail {
                Some(detail) if !detail.is_empty() => format!("{}: {detail}", progress.phase),
                _ => progress.phase.clone(),
            };
            Some(json!({
                "type": "job-progress",
                "jobId": job_id,
                "command": command,
                "message": message,
                "done": progress.done,
                "total": progress.total,
            }))
        }
        JobEvent::Finished { job_id, .. } => Some(finished(command, job_id, true)),
        JobEvent::Failed { job_id, .. } | JobEvent::Cancelled { job_id, .. } => {
            Some(finished(command, job_id, false))
        }
    }
}

fn finished(command: &str, job_id: &str, ok: bool) -> Value {
    json!({"type": "job-finished", "jobId": job_id, "command": command, "ok": ok})
}

#[cfg(test)]
mod tests {
    use rimstudio_ipc_types::error::ApiError;
    use rimstudio_ipc_types::jobs::{ProgressDto, ProgressUnitDto};

    use super::*;

    fn progress(detail: Option<&str>, total: Option<u64>) -> JobEvent<Value> {
        JobEvent::Progress {
            job_id: "j1".into(),
            progress: ProgressDto {
                phase: "scan".into(),
                done: 3,
                total,
                unit: ProgressUnitDto::Items,
                detail: detail.map(str::to_owned),
            },
        }
    }

    #[test]
    fn progress_has_the_bridge_shape() {
        let v = job_event_json("library_scan", &progress(Some("Mods"), Some(10))).unwrap();
        assert_eq!(
            v,
            json!({"type":"job-progress","jobId":"j1","command":"library_scan","message":"scan: Mods","done":3,"total":10})
        );
        let v = job_event_json("c", &progress(None, None)).unwrap();
        assert_eq!(v["message"], "scan");
        assert_eq!(v["total"], Value::Null);
    }

    #[test]
    fn every_terminal_event_is_a_job_finished() {
        let ok = JobEvent::Finished {
            job_id: "j".into(),
            result: json!(1),
            elapsed_ms: 1,
            diagnostics: Vec::new(),
        };
        assert_eq!(
            job_event_json("c", &ok).unwrap(),
            json!({"type":"job-finished","jobId":"j","command":"c","ok":true})
        );
        let failed = JobEvent::<Value>::Failed {
            job_id: "j".into(),
            error: ApiError::new("app.internal-error", "x"),
            elapsed_ms: 1,
        };
        assert_eq!(job_event_json("c", &failed).unwrap()["ok"], false);
        let cancelled = JobEvent::<Value>::Cancelled {
            job_id: "j".into(),
            elapsed_ms: 1,
            partial: None,
        };
        assert_eq!(job_event_json("c", &cancelled).unwrap()["ok"], false);
    }

    #[test]
    fn started_has_no_event() {
        let started = JobEvent::<Value>::Started {
            job_id: "j".into(),
            label_key: "k".into(),
            phases: Vec::new(),
        };
        assert!(job_event_json("c", &started).is_none());
    }
}
