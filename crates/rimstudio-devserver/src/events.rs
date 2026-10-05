//! The event hub behind `GET /dev/events` and the mapping of job events to the bridge's JSON events.
//!
//! Every connected event stream owns a bounded queue. A broadcast offers the event to each queue and
//! drops a stream whose queue is full or gone, so a stalled browser tab can never block a job.

use std::sync::Mutex;
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};

use rimstudio_ipc_types::jobs::JobEvent;
use serde_json::{Value, json};

/// How many events one stream may have queued.
pub const QUEUE_SIZE: usize = 1024;

/// The subscribers of the event stream.
#[derive(Debug, Default)]
pub struct EventHub {
    subscribers: Mutex<Vec<SyncSender<String>>>,
}

impl EventHub {
    /// An empty hub.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a stream and returns its queue.
    #[must_use]
    pub fn subscribe(&self) -> Receiver<String> {
        let (tx, rx) = sync_channel(QUEUE_SIZE);
        if let Ok(mut subs) = self.subscribers.lock() {
            subs.push(tx);
        }
        rx
    }

    /// Offers one event (a line of JSON text) to every stream.
    pub fn broadcast(&self, line: &str) {
        if let Ok(mut subs) = self.subscribers.lock() {
            subs.retain(|tx| match tx.try_send(line.to_owned()) {
                Ok(()) => true,
                Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => false,
            });
        }
    }

    /// The number of connected streams (dropped ones are counted until the next broadcast).
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.lock().map_or(0, |s| s.len())
    }
}

/// The bridge event of a job event, `None` for the `started` event (the call itself is the start).
#[must_use]
pub fn bridge_event(command: &str, event: &JobEvent<Value>) -> Option<Value> {
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
    use rimstudio_ipc_types::jobs::{JobEvent, ProgressDto, ProgressUnitDto};

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
    fn progress_maps_to_the_documented_shape() {
        let v = bridge_event("library_scan", &progress(Some("Mods"), Some(10))).unwrap();
        assert_eq!(
            v,
            json!({"type":"job-progress","jobId":"j1","command":"library_scan","message":"scan: Mods","done":3,"total":10})
        );
        let v = bridge_event("c", &progress(None, None)).unwrap();
        assert_eq!(v["message"], "scan");
        assert_eq!(v["total"], Value::Null);
    }

    #[test]
    fn terminal_events_map_to_job_finished() {
        let ok = JobEvent::Finished {
            job_id: "j".into(),
            result: json!(1),
            elapsed_ms: 1,
            diagnostics: Vec::new(),
        };
        assert_eq!(bridge_event("c", &ok).unwrap()["ok"], true);
        let failed = JobEvent::<Value>::Failed {
            job_id: "j".into(),
            error: ApiError::new("app.internal-error", "x"),
            elapsed_ms: 1,
        };
        let v = bridge_event("c", &failed).unwrap();
        assert_eq!(v["type"], "job-finished");
        assert_eq!(v["ok"], false);
        let cancelled = JobEvent::<Value>::Cancelled {
            job_id: "j".into(),
            elapsed_ms: 1,
            partial: None,
        };
        assert_eq!(bridge_event("c", &cancelled).unwrap()["ok"], false);
    }

    #[test]
    fn started_has_no_bridge_event() {
        let started = JobEvent::<Value>::Started {
            job_id: "j".into(),
            label_key: "k".into(),
            phases: Vec::new(),
        };
        assert!(bridge_event("c", &started).is_none());
    }

    #[test]
    fn broadcast_reaches_every_stream_and_drops_dead_ones() {
        let hub = EventHub::new();
        let a = hub.subscribe();
        let b = hub.subscribe();
        hub.broadcast("one");
        assert_eq!(a.try_recv().unwrap(), "one");
        assert_eq!(b.try_recv().unwrap(), "one");
        drop(b);
        hub.broadcast("two");
        assert_eq!(hub.subscriber_count(), 1);
        assert_eq!(a.try_recv().unwrap(), "two");
    }

    #[test]
    fn a_full_queue_drops_the_stream() {
        let hub = EventHub::new();
        let slow = hub.subscribe();
        for i in 0..=QUEUE_SIZE {
            hub.broadcast(&i.to_string());
        }
        assert_eq!(hub.subscriber_count(), 0);
        drop(slow);
    }
}
