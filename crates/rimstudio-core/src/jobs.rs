//! Job vocabulary: progress records, progress sinks and cancellation.
//!
//! Any operation expected to take more than a millisecond is a job (invariant I-13). The job runner
//! lives in `rimstudio-app`; this module holds the small types that engine and service crates use to
//! report progress and to observe a cancel request, without depending on a runtime.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// What the numbers of a [`Progress`] record count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ProgressUnit {
    /// Abstract items such as mods or defs.
    #[default]
    Items,
    /// Files.
    Files,
    /// Bytes.
    Bytes,
}

/// One progress record, the shape shared by the webview and the CLI (ipc-and-state section 7.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// The phase name, for example `scan` or `index`.
    pub phase: String,
    /// Units finished so far.
    pub done: u64,
    /// Units in total when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    /// What a unit is.
    pub unit: ProgressUnit,
    /// A short message key (not formatted prose) so the UI can translate it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Progress {
    /// Creates a record counting items with no total and no detail.
    pub fn new(phase: impl Into<String>, done: u64) -> Progress {
        Progress {
            phase: phase.into(),
            done,
            total: None,
            unit: ProgressUnit::Items,
            detail: None,
        }
    }

    /// Sets the total.
    #[must_use]
    pub fn with_total(mut self, total: u64) -> Progress {
        self.total = Some(total);
        self
    }

    /// Sets the unit.
    #[must_use]
    pub fn with_unit(mut self, unit: ProgressUnit) -> Progress {
        self.unit = unit;
        self
    }

    /// Sets the detail key.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Progress {
        self.detail = Some(detail.into());
        self
    }

    /// The finished fraction between 0 and 1, `None` when the total is unknown. A zero total counts
    /// as complete, and `done` above `total` is clamped.
    pub fn fraction(&self) -> Option<f64> {
        let total = self.total?;
        if total == 0 {
            return Some(1.0);
        }
        Some((self.done.min(total) as f64) / (total as f64))
    }
}

/// Receives progress records. Implementations must be cheap and thread safe: producers call it from
/// worker threads between units of work.
pub trait ProgressSink: Send + Sync {
    /// Reports a record. Later records of the same phase supersede earlier ones.
    fn report(&self, progress: Progress);
}

/// A sink that drops every record.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopProgress;

impl ProgressSink for NoopProgress {
    fn report(&self, _progress: Progress) {}
}

/// A sink that keeps every record in arrival order, for tests and for the CLI summary.
#[derive(Debug, Default)]
pub struct CollectingProgress {
    records: Mutex<Vec<Progress>>,
}

impl CollectingProgress {
    /// Creates an empty collector.
    pub fn new() -> CollectingProgress {
        CollectingProgress::default()
    }

    /// A copy of every record received so far, in arrival order.
    pub fn records(&self) -> Vec<Progress> {
        match self.records.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    /// The most recent record, if any.
    pub fn last(&self) -> Option<Progress> {
        self.records().pop()
    }

    /// The number of records received.
    pub fn len(&self) -> usize {
        match self.records.lock() {
            Ok(guard) => guard.len(),
            Err(poisoned) => poisoned.into_inner().len(),
        }
    }

    /// True when no record has been received.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl ProgressSink for CollectingProgress {
    fn report(&self, progress: Progress) {
        match self.records.lock() {
            Ok(mut guard) => guard.push(progress),
            Err(poisoned) => poisoned.into_inner().push(progress),
        }
    }
}

impl<T: ProgressSink + ?Sized> ProgressSink for Arc<T> {
    fn report(&self, progress: Progress) {
        (**self).report(progress);
    }
}

/// A cooperative cancel flag shared between the job runner and the work (`Arc<AtomicBool>`).
///
/// Clones share the flag. Work checks it between units of work (per mod folder, per file batch), never
/// inside a tight loop, and returns [`CoreError::Cancelled`] promptly. No thread is ever killed.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// Creates a token that is not cancelled.
    pub fn new() -> CancelToken {
        CancelToken::default()
    }

    /// Requests cancellation. Idempotent.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    /// True once cancellation was requested through this token or any clone.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    /// Returns `Err(CoreError::Cancelled)` when cancellation was requested, so loops can write
    /// `token.check()?`.
    ///
    /// # Errors
    /// [`CoreError::Cancelled`] after [`CancelToken::cancel`].
    pub fn check(&self) -> Result<(), CoreError> {
        if self.is_cancelled() {
            Err(CoreError::Cancelled)
        } else {
            Ok(())
        }
    }

    /// The shared flag, for code that must hand it to a lower level API.
    pub fn flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.0)
    }

    /// Wraps an existing flag.
    pub fn from_flag(flag: Arc<AtomicBool>) -> CancelToken {
        CancelToken(flag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn cancel_is_shared_between_clones_and_idempotent() {
        let a = CancelToken::new();
        let b = a.clone();
        assert!(!b.is_cancelled());
        assert!(b.check().is_ok());
        a.cancel();
        a.cancel();
        assert!(b.is_cancelled());
        assert_eq!(b.check(), Err(CoreError::Cancelled));
    }

    #[test]
    fn tokens_can_share_a_raw_flag() {
        let t = CancelToken::new();
        let u = CancelToken::from_flag(t.flag());
        t.cancel();
        assert!(u.is_cancelled());
    }

    #[test]
    fn token_is_visible_across_threads() {
        let t = CancelToken::new();
        let u = t.clone();
        let handle = std::thread::spawn(move || {
            u.cancel();
        });
        handle.join().unwrap();
        assert!(t.is_cancelled());
    }

    #[test]
    fn collecting_sink_keeps_arrival_order() {
        let sink = CollectingProgress::new();
        assert!(sink.is_empty());
        sink.report(Progress::new("scan", 1).with_total(3));
        sink.report(Progress::new("scan", 2).with_total(3));
        sink.report(
            Progress::new("index", 0)
                .with_unit(ProgressUnit::Files)
                .with_detail("job.index.start"),
        );
        assert_eq!(sink.len(), 3);
        let all = sink.records();
        assert_eq!(all[0].done, 1);
        assert_eq!(all[2].phase, "index");
        assert_eq!(
            sink.last().unwrap().detail.as_deref(),
            Some("job.index.start")
        );
    }

    #[test]
    fn sinks_work_as_trait_objects_and_through_arc() {
        let collector = Arc::new(CollectingProgress::new());
        let dyn_sink: Arc<dyn ProgressSink> = collector.clone();
        dyn_sink.report(Progress::new("p", 1));
        NoopProgress.report(Progress::new("p", 2));
        assert_eq!(collector.len(), 1);
    }

    #[test]
    fn collecting_sink_accepts_records_from_many_threads() {
        let sink = Arc::new(CollectingProgress::new());
        let handles: Vec<_> = (0..4)
            .map(|t| {
                let s = Arc::clone(&sink);
                std::thread::spawn(move || {
                    for i in 0..25 {
                        s.report(Progress::new("w", t * 100 + i));
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(sink.len(), 100);
    }

    #[rstest]
    #[case(0, Some(0), Some(1.0))]
    #[case(5, Some(10), Some(0.5))]
    #[case(20, Some(10), Some(1.0))]
    #[case(3, None, None)]
    fn fraction_handles_edges(
        #[case] done: u64,
        #[case] total: Option<u64>,
        #[case] expected: Option<f64>,
    ) {
        let mut p = Progress::new("x", done);
        p.total = total;
        assert_eq!(p.fraction(), expected);
    }

    #[test]
    fn progress_serialises_camel_case_and_omits_empty_fields() {
        let p = Progress::new("scan", 2)
            .with_total(5)
            .with_unit(ProgressUnit::Bytes);
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["unit"], "bytes");
        assert_eq!(v["total"], 5);
        assert!(v.get("detail").is_none());
        let back: Progress = serde_json::from_value(v).unwrap();
        assert_eq!(back, p);
    }
}
