//! The job runner: caller minted ids, a registry, rate limited progress and exactly one terminal event.
//!
//! Anything that takes longer than a millisecond is a job (invariant I-13). [`JobRunner::spawn`]
//! registers a job under a caller minted id, sends `started` at once and runs the work on the blocking
//! pool of a small tokio runtime (work that uses `rayon` is bridged the same way: the blocking thread
//! is the caller of the rayon pool). Events go to a [`JobSink`]; a sink that reports a closed channel
//! cancels the job (cancel on channel drop). The runner itself keeps, for 60 seconds after the end, the
//! state, the latest progress and the terminal event of every job, which is what `job_status` and
//! `cancel_job` answer from.
//!
//! Rules that hold for every job:
//!
//! - exactly one terminal event (`finished`, `failed` or `cancelled`) is sent, and nothing after it;
//! - a panic in the work becomes `failed` with code `job.panicked`;
//! - progress is limited to [`PROGRESS_INTERVAL_MS`] between messages (20 per second): the first record
//!   goes out at once, records inside the window replace each other, and the newest pending record is
//!   flushed before the terminal event. The limiter reads the injected [`Clock`], so tests drive it
//!   with a fake clock;
//! - cancellation is cooperative through the [`CancelToken`] of the job.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel as mpsc_channel};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use rimstudio_core::ids::JobId;
use rimstudio_core::jobs::{CancelToken, Progress, ProgressSink};
use rimstudio_core::ports::Clock;
use rimstudio_ipc_types::diagnostic::DiagnosticDto;
use rimstudio_ipc_types::error::{ApiError, codes};
use rimstudio_ipc_types::jobs::{CancelStateDto, JobEvent, JobHandleDto, ProgressDto};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::runtime::{Builder, Handle, Runtime};

use crate::error::{AppError, panic_message};

/// The shortest time between two progress messages of one job: 20 messages per second.
pub const PROGRESS_INTERVAL_MS: u64 = 50;

/// How long a finished job stays in the registry.
pub const RETENTION_MS: u64 = 60_000;

/// Where the events of a job go.
pub trait JobSink: Send + Sync {
    /// Delivers one event. Returns `false` when the receiver is gone; the runner then cancels the job.
    fn send(&self, event: JobEvent<Value>) -> bool;
}

/// A sink that drops every event; the registry still records the state and the terminal event.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullSink;

impl JobSink for NullSink {
    fn send(&self, _event: JobEvent<Value>) -> bool {
        true
    }
}

/// A sink that forwards to a standard channel.
#[derive(Debug)]
pub struct ChannelSink {
    tx: Sender<JobEvent<Value>>,
}

impl JobSink for ChannelSink {
    fn send(&self, event: JobEvent<Value>) -> bool {
        self.tx.send(event).is_ok()
    }
}

/// A channel sink and its receiver. Dropping the receiver cancels the job.
#[must_use]
pub fn channel() -> (Arc<ChannelSink>, Receiver<JobEvent<Value>>) {
    let (tx, rx) = mpsc_channel();
    (Arc::new(ChannelSink { tx }), rx)
}

/// What a job is called and which phases it goes through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobSpec {
    /// The caller minted id.
    pub id: String,
    /// Catalog key of the label.
    pub label_key: String,
    /// The phases of the job, in order; empty when unknown.
    pub phases: Vec<String>,
}

impl JobSpec {
    /// A spec with no phases.
    #[must_use]
    pub fn new(id: impl Into<String>, label_key: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label_key: label_key.into(),
            phases: Vec::new(),
        }
    }
}

/// What a job hands back, in typed form.
#[derive(Debug, Clone)]
pub struct JobOutcome<R> {
    /// The result.
    pub result: R,
    /// Content problems found on the way.
    pub diagnostics: Vec<DiagnosticDto>,
    /// True when the work stopped on a cancel request and `result` is partial.
    pub cancelled: bool,
}

impl<R> JobOutcome<R> {
    /// A complete result without diagnostics.
    #[must_use]
    pub fn done(result: R) -> Self {
        Self {
            result,
            diagnostics: Vec::new(),
            cancelled: false,
        }
    }

    /// A partial result of a cancelled job.
    #[must_use]
    pub fn cancelled(result: R) -> Self {
        Self {
            result,
            diagnostics: Vec::new(),
            cancelled: true,
        }
    }

    /// Adds diagnostics.
    #[must_use]
    pub fn with_diagnostics(mut self, diagnostics: Vec<DiagnosticDto>) -> Self {
        self.diagnostics = diagnostics;
        self
    }
}

impl<R: Serialize> JobOutcome<R> {
    /// Encodes the result as JSON.
    ///
    /// # Errors
    /// `app.internal-error` when the result cannot be encoded.
    pub fn into_output(self) -> Result<JobOutput, ApiError> {
        let value = serde_json::to_value(&self.result).map_err(|e| {
            AppError::internal(format!("a job result cannot be encoded: {e}")).to_api()
        })?;
        Ok(JobOutput {
            value,
            diagnostics: self.diagnostics,
            cancelled: self.cancelled,
        })
    }
}

/// The type erased outcome of a job.
#[derive(Debug, Clone, PartialEq)]
pub struct JobOutput {
    /// The result as JSON.
    pub value: Value,
    /// Content problems found on the way.
    pub diagnostics: Vec<DiagnosticDto>,
    /// True when the result is partial because the job was cancelled.
    pub cancelled: bool,
}

impl JobOutput {
    /// A complete result.
    #[must_use]
    pub fn done(value: Value) -> Self {
        Self {
            value,
            diagnostics: Vec::new(),
            cancelled: false,
        }
    }
}

/// The work of a job. It runs once on the blocking pool.
pub type JobWork = Box<dyn FnOnce(&JobCtx) -> Result<JobOutput, ApiError> + Send + 'static>;

/// Where a job is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JobState {
    /// The work is running.
    Running,
    /// A cancel was requested; the work has not returned yet.
    Cancelling,
    /// The job ended with a result.
    Finished,
    /// The job ended with an error.
    Failed,
    /// The job ended on a cancel request.
    Cancelled,
}

impl JobState {
    /// True once the job has ended.
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Finished | Self::Failed | Self::Cancelled)
    }
}

/// What the registry knows about one job.
#[derive(Debug, Clone, PartialEq)]
pub struct JobStatus {
    /// The job id.
    pub id: String,
    /// The state.
    pub state: JobState,
    /// Catalog key of the label.
    pub label_key: String,
    /// Registration time.
    pub started_at_ms: u64,
    /// End time, once ended.
    pub finished_at_ms: Option<u64>,
    /// The latest progress record that was sent.
    pub progress: Option<ProgressDto>,
    /// The terminal event, once ended.
    pub terminal: Option<JobEvent<Value>>,
}

/// Limits progress to one message per interval and remembers the newest record inside the window.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    interval_ms: u64,
    last_ms: Option<u64>,
    pending: Option<Progress>,
}

impl RateLimiter {
    /// A limiter with the given minimum gap between messages.
    #[must_use]
    pub fn new(interval_ms: u64) -> Self {
        Self {
            interval_ms,
            last_ms: None,
            pending: None,
        }
    }

    /// Offers a record at time `now_ms`. Returns it when it may be sent now; otherwise it becomes the
    /// pending record (replacing an older one) and `None` is returned.
    pub fn offer(&mut self, now_ms: u64, progress: Progress) -> Option<Progress> {
        match self.last_ms {
            Some(last) if now_ms.saturating_sub(last) < self.interval_ms => {
                self.pending = Some(progress);
                None
            }
            _ => {
                self.last_ms = Some(now_ms);
                self.pending = None;
                Some(progress)
            }
        }
    }

    /// Takes the newest record that was held back.
    pub fn take_pending(&mut self) -> Option<Progress> {
        self.pending.take()
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

struct JobEntry {
    state: JobState,
    label_key: String,
    started_at_ms: u64,
    finished_at_ms: Option<u64>,
    cancel: CancelToken,
    progress: Option<ProgressDto>,
    terminal: Option<JobEvent<Value>>,
}

struct Shared {
    clock: Arc<dyn Clock>,
    jobs: Mutex<BTreeMap<String, JobEntry>>,
    changed: Condvar,
    mint: AtomicU64,
}

impl Shared {
    fn now(&self) -> u64 {
        self.clock.now_unix_ms()
    }

    fn prune(&self, jobs: &mut BTreeMap<String, JobEntry>) {
        let now = self.now();
        jobs.retain(|_, e| match e.finished_at_ms {
            Some(done) => now.saturating_sub(done) < RETENTION_MS,
            None => true,
        });
    }
}

struct EmitState {
    limiter: RateLimiter,
    terminal_sent: bool,
}

/// Sends the events of one job: the progress limiter, the terminal guard and the registry updates.
struct Emitter {
    job_id: String,
    sink: Arc<dyn JobSink>,
    cancel: CancelToken,
    shared: Arc<Shared>,
    state: Mutex<EmitState>,
}

impl Emitter {
    fn deliver(&self, event: JobEvent<Value>) {
        if !self.sink.send(event) {
            // the receiver is gone: nobody wants the result any more
            self.cancel.cancel();
            if let Some(entry) = lock(&self.shared.jobs).get_mut(&self.job_id)
                && entry.state == JobState::Running
            {
                entry.state = JobState::Cancelling;
            }
        }
    }

    fn progress_event(&self, progress: &Progress) -> JobEvent<Value> {
        JobEvent::Progress {
            job_id: self.job_id.clone(),
            progress: ProgressDto::from(progress),
        }
    }

    fn record_progress(&self, progress: &Progress) {
        if let Some(entry) = lock(&self.shared.jobs).get_mut(&self.job_id) {
            entry.progress = Some(ProgressDto::from(progress));
        }
    }

    fn report(&self, progress: Progress) {
        let mut state = lock(&self.state);
        if state.terminal_sent {
            return;
        }
        if let Some(send) = state.limiter.offer(self.shared.now(), progress) {
            self.record_progress(&send);
            self.deliver(self.progress_event(&send));
        }
    }

    /// Sends the terminal event unless one was sent already. Returns whether this call sent it.
    fn terminal(&self, build: impl FnOnce(u64) -> (JobState, JobEvent<Value>)) -> bool {
        let mut state = lock(&self.state);
        if state.terminal_sent {
            return false;
        }
        state.terminal_sent = true;
        if let Some(pending) = state.limiter.take_pending() {
            self.record_progress(&pending);
            self.deliver(self.progress_event(&pending));
        }
        let now = self.shared.now();
        let started = lock(&self.shared.jobs)
            .get(&self.job_id)
            .map_or(now, |e| e.started_at_ms);
        let (job_state, event) = build(now.saturating_sub(started));
        if let Some(entry) = lock(&self.shared.jobs).get_mut(&self.job_id) {
            entry.state = job_state;
            entry.finished_at_ms = Some(now);
            entry.terminal = Some(event.clone());
        }
        self.deliver(event);
        self.shared.changed.notify_all();
        true
    }
}

/// What a running job sees: its id, its cancel token and its progress sink.
pub struct JobCtx {
    id: String,
    cancel: CancelToken,
    progress: JobProgress,
}

impl std::fmt::Debug for JobCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobCtx")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl JobCtx {
    /// The job id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The cancel token; check it between units of work.
    #[must_use]
    pub fn cancel(&self) -> &CancelToken {
        &self.cancel
    }

    /// The progress sink of the job (rate limited).
    #[must_use]
    pub fn progress(&self) -> &dyn ProgressSink {
        &self.progress
    }
}

struct JobProgress {
    emitter: Arc<Emitter>,
}

impl ProgressSink for JobProgress {
    fn report(&self, progress: Progress) {
        self.emitter.report(progress);
    }
}

enum Executor {
    Owned(Option<Runtime>),
    Borrowed(Handle),
}

/// Runs jobs and keeps their registry.
pub struct JobRunner {
    shared: Arc<Shared>,
    executor: Executor,
}

impl std::fmt::Debug for JobRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobRunner")
            .field("jobs", &lock(&self.shared.jobs).len())
            .finish_non_exhaustive()
    }
}

impl JobRunner {
    /// A runner with a small runtime of its own.
    ///
    /// # Errors
    /// The operating system's error when the runtime cannot start.
    pub fn new(clock: Arc<dyn Clock>) -> std::io::Result<Self> {
        let runtime = Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(64)
            .thread_name("rimstudio-job")
            .enable_time()
            .build()?;
        Ok(Self::with_executor(clock, Executor::Owned(Some(runtime))))
    }

    /// A runner that uses the blocking pool of an existing runtime (the shell's).
    #[must_use]
    pub fn with_handle(clock: Arc<dyn Clock>, handle: Handle) -> Self {
        Self::with_executor(clock, Executor::Borrowed(handle))
    }

    fn with_executor(clock: Arc<dyn Clock>, executor: Executor) -> Self {
        Self {
            shared: Arc::new(Shared {
                clock,
                jobs: Mutex::new(BTreeMap::new()),
                changed: Condvar::new(),
                mint: AtomicU64::new(0),
            }),
            executor,
        }
    }

    fn handle(&self) -> Option<&Handle> {
        match &self.executor {
            Executor::Owned(rt) => rt.as_ref().map(Runtime::handle),
            Executor::Borrowed(h) => Some(h),
        }
    }

    /// Mints a job id for callers that have none (the CLI): a time stamp and a counter.
    #[must_use]
    pub fn mint_id(&self) -> String {
        let n = self.shared.mint.fetch_add(1, Ordering::Relaxed);
        format!("job-{:x}-{:04x}", self.shared.now(), n & 0xffff)
    }

    /// Registers a job, sends `started` and runs `work` on the blocking pool.
    ///
    /// # Errors
    /// `ipc.invalid-request` for an id that is not a valid job id, `job.duplicate-id` when the id is
    /// registered (running, or ended less than a minute ago).
    pub fn spawn(
        &self,
        spec: JobSpec,
        sink: Arc<dyn JobSink>,
        work: JobWork,
    ) -> Result<JobHandleDto, ApiError> {
        JobId::new(&spec.id)
            .map_err(|e| AppError::invalid("job", "jobId", e.to_string()).to_api())?;
        let cancel = CancelToken::new();
        let started_at_ms = self.shared.now();
        {
            let mut jobs = lock(&self.shared.jobs);
            self.shared.prune(&mut jobs);
            if jobs.contains_key(&spec.id) {
                return Err(AppError::JobDuplicateId { id: spec.id }.to_api());
            }
            jobs.insert(
                spec.id.clone(),
                JobEntry {
                    state: JobState::Running,
                    label_key: spec.label_key.clone(),
                    started_at_ms,
                    finished_at_ms: None,
                    cancel: cancel.clone(),
                    progress: None,
                    terminal: None,
                },
            );
        }
        let emitter = Arc::new(Emitter {
            job_id: spec.id.clone(),
            sink,
            cancel: cancel.clone(),
            shared: Arc::clone(&self.shared),
            state: Mutex::new(EmitState {
                limiter: RateLimiter::new(PROGRESS_INTERVAL_MS),
                terminal_sent: false,
            }),
        });
        emitter.deliver(JobEvent::Started {
            job_id: spec.id.clone(),
            label_key: spec.label_key,
            phases: spec.phases,
        });
        let ctx = JobCtx {
            id: spec.id.clone(),
            cancel,
            progress: JobProgress {
                emitter: Arc::clone(&emitter),
            },
        };
        let Some(handle) = self.handle() else {
            let error = AppError::internal("the job runtime has shut down").to_api();
            let sent = emitter.terminal(|elapsed_ms| {
                (
                    JobState::Failed,
                    JobEvent::Failed {
                        job_id: spec.id.clone(),
                        error,
                        elapsed_ms,
                    },
                )
            });
            debug_assert!(sent);
            return Ok(JobHandleDto {
                job_id: spec.id,
                started_at_ms,
            });
        };
        handle.spawn_blocking(move || run_job(&emitter, &ctx, work));
        Ok(JobHandleDto {
            job_id: spec.id,
            started_at_ms,
        })
    }

    /// Requests cancellation. Idempotent: `cancelling` while the job runs, `finished` once it ended,
    /// `unknown` for an id the registry does not hold.
    pub fn cancel(&self, id: &str) -> CancelStateDto {
        let mut jobs = lock(&self.shared.jobs);
        self.shared.prune(&mut jobs);
        match jobs.get_mut(id) {
            None => CancelStateDto::Unknown,
            Some(entry) if entry.state.is_terminal() => CancelStateDto::Finished,
            Some(entry) => {
                entry.cancel.cancel();
                entry.state = JobState::Cancelling;
                CancelStateDto::Cancelling
            }
        }
    }

    /// Cancels every running job.
    pub fn cancel_all(&self) {
        for entry in lock(&self.shared.jobs).values_mut() {
            if !entry.state.is_terminal() {
                entry.cancel.cancel();
                entry.state = JobState::Cancelling;
            }
        }
    }

    /// What the registry knows about a job.
    #[must_use]
    pub fn status(&self, id: &str) -> Option<JobStatus> {
        let mut jobs = lock(&self.shared.jobs);
        self.shared.prune(&mut jobs);
        jobs.get(id).map(|e| JobStatus {
            id: id.to_owned(),
            state: e.state,
            label_key: e.label_key.clone(),
            started_at_ms: e.started_at_ms,
            finished_at_ms: e.finished_at_ms,
            progress: e.progress.clone(),
            terminal: e.terminal.clone(),
        })
    }

    /// The number of jobs that have not ended.
    #[must_use]
    pub fn running(&self) -> usize {
        lock(&self.shared.jobs)
            .values()
            .filter(|e| !e.state.is_terminal())
            .count()
    }

    /// Waits for the terminal event of a job. `None` when the id is unknown or the timeout passed.
    #[must_use]
    pub fn wait_terminal(&self, id: &str, timeout: Duration) -> Option<JobEvent<Value>> {
        let deadline = Instant::now() + timeout;
        let mut jobs = lock(&self.shared.jobs);
        loop {
            match jobs.get(id) {
                None => return None,
                Some(entry) => {
                    if let Some(event) = &entry.terminal {
                        return Some(event.clone());
                    }
                }
            }
            let left = deadline.checked_duration_since(Instant::now())?;
            let (guard, result) = self
                .shared
                .changed
                .wait_timeout(jobs, left)
                .unwrap_or_else(|p| p.into_inner());
            jobs = guard;
            if result.timed_out() && jobs.get(id).is_none_or(|e| e.terminal.is_none()) {
                return None;
            }
        }
    }

    /// Waits until no job is running. Returns whether that happened within the timeout.
    #[must_use]
    pub fn wait_idle(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut jobs = lock(&self.shared.jobs);
        loop {
            if jobs.values().all(|e| e.state.is_terminal()) {
                return true;
            }
            let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            let (guard, _) = self
                .shared
                .changed
                .wait_timeout(jobs, left)
                .unwrap_or_else(|p| p.into_inner());
            jobs = guard;
        }
    }
}

impl Drop for JobRunner {
    fn drop(&mut self) {
        self.cancel_all();
        if matches!(self.executor, Executor::Owned(_)) {
            let _idle = self.wait_idle(Duration::from_secs(2));
            if let Executor::Owned(slot) = &mut self.executor
                && let Some(runtime) = slot.take()
            {
                runtime.shutdown_background();
            }
        }
    }
}

fn run_job(emitter: &Arc<Emitter>, ctx: &JobCtx, work: JobWork) {
    let id = ctx.id.clone();
    let result = catch_unwind(AssertUnwindSafe(|| work(ctx)));
    let sent = match result {
        Ok(Ok(output)) if output.cancelled => emitter.terminal(|elapsed_ms| {
            (
                JobState::Cancelled,
                JobEvent::Cancelled {
                    job_id: id.clone(),
                    elapsed_ms,
                    partial: Some(output.value),
                },
            )
        }),
        Ok(Ok(output)) => emitter.terminal(|elapsed_ms| {
            (
                JobState::Finished,
                JobEvent::Finished {
                    job_id: id.clone(),
                    result: output.value,
                    elapsed_ms,
                    diagnostics: output.diagnostics,
                },
            )
        }),
        Ok(Err(error)) if error.is_cancelled() => emitter.terminal(|elapsed_ms| {
            (
                JobState::Cancelled,
                JobEvent::Cancelled {
                    job_id: id.clone(),
                    elapsed_ms,
                    partial: None,
                },
            )
        }),
        Ok(Err(error)) => {
            tracing::error!(job = %id, code = %error.code, error_id = %error.error_id, "job failed");
            emitter.terminal(|elapsed_ms| {
                (
                    JobState::Failed,
                    JobEvent::Failed {
                        job_id: id.clone(),
                        error,
                        elapsed_ms,
                    },
                )
            })
        }
        Err(payload) => {
            let message = panic_message(payload.as_ref());
            let error = ApiError::new(codes::JOB_PANICKED, "a job panicked");
            tracing::error!(job = %id, error_id = %error.error_id, panic = %message, "job panicked");
            emitter.terminal(|elapsed_ms| {
                (
                    JobState::Failed,
                    JobEvent::Failed {
                        job_id: id.clone(),
                        error,
                        elapsed_ms,
                    },
                )
            })
        }
    };
    debug_assert!(sent, "a job must end with exactly one terminal event");
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use rimstudio_testing::fakes::FakeClock;
    use serde_json::json;

    use super::*;

    const WAIT: Duration = Duration::from_secs(10);

    fn runner() -> (JobRunner, FakeClock) {
        let clock = FakeClock::new(FakeClock::DEFAULT_START_MS);
        let runner = JobRunner::new(Arc::new(clock.clone())).expect("runtime starts");
        (runner, clock)
    }

    fn drain(rx: &Receiver<JobEvent<Value>>) -> Vec<JobEvent<Value>> {
        let mut out = Vec::new();
        while let Ok(event) = rx.recv_timeout(WAIT) {
            let terminal = event.is_terminal();
            out.push(event);
            if terminal {
                break;
            }
        }
        out
    }

    fn terminal_count(events: &[JobEvent<Value>]) -> usize {
        events.iter().filter(|e| e.is_terminal()).count()
    }

    #[test]
    fn a_finished_job_sends_started_then_one_terminal_event() {
        let (runner, _clock) = runner();
        let (sink, rx) = channel();
        let handle = runner
            .spawn(
                JobSpec::new("j-ok", "job.test"),
                sink,
                Box::new(|_| Ok(JobOutput::done(json!({"n": 1})))),
            )
            .expect("spawn");
        assert_eq!(handle.job_id, "j-ok");
        let events = drain(&rx);
        assert!(matches!(events.first(), Some(JobEvent::Started { .. })));
        assert_eq!(terminal_count(&events), 1);
        assert!(
            matches!(events.last(), Some(JobEvent::Finished { result, .. }) if result == &json!({"n": 1}))
        );
        assert!(
            rx.recv_timeout(Duration::from_millis(100)).is_err(),
            "nothing follows the terminal event"
        );
    }

    #[test]
    fn a_failing_job_sends_failed_with_the_error() {
        let (runner, _clock) = runner();
        let (sink, rx) = channel();
        runner
            .spawn(
                JobSpec::new("j-fail", "job.test"),
                sink,
                Box::new(|_| Err(ApiError::new(codes::LIBRARY_SCAN_FAILED, "no"))),
            )
            .expect("spawn");
        let events = drain(&rx);
        assert_eq!(terminal_count(&events), 1);
        assert!(
            matches!(events.last(), Some(JobEvent::Failed { error, .. }) if error.code == "library.scan-failed")
        );
        assert_eq!(
            runner.status("j-fail").map(|s| s.state),
            Some(JobState::Failed)
        );
    }

    #[test]
    fn a_panic_becomes_job_panicked_and_other_jobs_continue() {
        let (runner, _clock) = runner();
        let (sink, rx) = channel();
        runner
            .spawn(
                JobSpec::new("j-panic", "job.test"),
                sink,
                Box::new(|_| panic!("boom in a test")),
            )
            .expect("spawn");
        let events = drain(&rx);
        assert!(
            matches!(events.last(), Some(JobEvent::Failed { error, .. }) if error.code == "job.panicked")
        );
        let (sink2, rx2) = channel();
        runner
            .spawn(
                JobSpec::new("j-after", "job.test"),
                sink2,
                Box::new(|_| Ok(JobOutput::done(json!(1)))),
            )
            .expect("spawn");
        assert!(matches!(
            drain(&rx2).last(),
            Some(JobEvent::Finished { .. })
        ));
    }

    #[test]
    fn cancel_stops_a_job_that_checks_its_token() {
        let (runner, _clock) = runner();
        let (sink, rx) = channel();
        runner
            .spawn(
                JobSpec::new("j-cancel", "job.test"),
                sink,
                Box::new(|job| {
                    while !job.cancel().is_cancelled() {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(ApiError::new(codes::JOB_CANCELLED, "cancelled"))
                }),
            )
            .expect("spawn");
        assert!(matches!(
            rx.recv_timeout(WAIT),
            Ok(JobEvent::Started { .. })
        ));
        assert_eq!(runner.cancel("j-cancel"), CancelStateDto::Cancelling);
        let events = drain(&rx);
        assert_eq!(terminal_count(&events), 1);
        assert!(matches!(
            events.last(),
            Some(JobEvent::Cancelled { partial: None, .. })
        ));
        assert_eq!(runner.cancel("j-cancel"), CancelStateDto::Finished);
    }

    #[test]
    fn a_cancelled_job_with_a_partial_result_reports_it() {
        let (runner, _clock) = runner();
        let (sink, rx) = channel();
        runner
            .spawn(
                JobSpec::new("j-partial", "job.test"),
                sink,
                Box::new(|_| {
                    Ok(JobOutput {
                        value: json!({"partial": true}),
                        diagnostics: Vec::new(),
                        cancelled: true,
                    })
                }),
            )
            .expect("spawn");
        let events = drain(&rx);
        assert!(
            matches!(events.last(), Some(JobEvent::Cancelled { partial: Some(p), .. }) if p == &json!({"partial": true}))
        );
    }

    #[test]
    fn dropping_the_receiver_cancels_the_job() {
        let (runner, clock) = runner();
        let (sink, rx) = channel();
        let (seen_tx, seen_rx) = mpsc_channel::<bool>();
        let tick = clock.clone();
        runner
            .spawn(
                JobSpec::new("j-drop", "job.test"),
                sink,
                Box::new(move |job| {
                    let mut tries = 0;
                    while !job.cancel().is_cancelled() && tries < 2000 {
                        job.progress().report(Progress::new("work", tries));
                        tick.advance(PROGRESS_INTERVAL_MS);
                        std::thread::sleep(Duration::from_millis(1));
                        tries += 1;
                    }
                    let _ = seen_tx.send(job.cancel().is_cancelled());
                    Ok(JobOutput::done(json!(null)))
                }),
            )
            .expect("spawn");
        drop(rx);
        // the job reports until a send fails; the first progress after the drop cancels it
        let cancelled = seen_rx.recv_timeout(WAIT).expect("job returns");
        assert!(cancelled, "a closed channel must set the cancel token");
        assert!(runner.wait_idle(WAIT));
    }

    #[test]
    fn a_duplicate_id_is_rejected_and_an_invalid_id_too() {
        let (runner, _clock) = runner();
        let (sink, rx) = channel();
        runner
            .spawn(
                JobSpec::new("j-dup", "job.test"),
                sink,
                Box::new(|_| Ok(JobOutput::done(json!(1)))),
            )
            .expect("spawn");
        let again = runner.spawn(
            JobSpec::new("j-dup", "job.test"),
            Arc::new(NullSink),
            Box::new(|_| Ok(JobOutput::done(json!(1)))),
        );
        assert_eq!(
            again.err().map(|e| e.code),
            Some("job.duplicate-id".to_owned())
        );
        let bad = runner.spawn(
            JobSpec::new("not valid!", "job.test"),
            Arc::new(NullSink),
            Box::new(|_| Ok(JobOutput::done(json!(1)))),
        );
        assert_eq!(
            bad.err().map(|e| e.code),
            Some("ipc.invalid-request".to_owned())
        );
        let _ = drain(&rx);
    }

    #[test]
    fn unknown_and_late_cancels_are_answered_truthfully() {
        let (runner, _clock) = runner();
        assert_eq!(runner.cancel("nobody"), CancelStateDto::Unknown);
        assert!(runner.status("nobody").is_none());
    }

    #[test]
    fn finished_jobs_leave_the_registry_after_the_retention_time() {
        let (runner, clock) = runner();
        let (sink, rx) = channel();
        runner
            .spawn(
                JobSpec::new("j-old", "job.test"),
                sink,
                Box::new(|_| Ok(JobOutput::done(json!(1)))),
            )
            .expect("spawn");
        let _ = drain(&rx);
        assert!(runner.status("j-old").is_some());
        clock.advance(RETENTION_MS + 1);
        assert!(runner.status("j-old").is_none());
        assert_eq!(runner.cancel("j-old"), CancelStateDto::Unknown);
    }

    #[test]
    fn progress_inside_one_window_is_coalesced_and_the_newest_is_flushed_before_the_end() {
        let (runner, _clock) = runner();
        let (sink, rx) = channel();
        runner
            .spawn(
                JobSpec::new("j-prog", "job.test"),
                sink,
                Box::new(|job| {
                    for i in 0..1000_u64 {
                        job.progress()
                            .report(Progress::new("scan", i).with_total(1000));
                    }
                    Ok(JobOutput::done(json!(null)))
                }),
            )
            .expect("spawn");
        let events = drain(&rx);
        let progress: Vec<u64> = events
            .iter()
            .filter_map(|e| match e {
                JobEvent::Progress { progress, .. } => Some(progress.done),
                _ => None,
            })
            .collect();
        // the clock never moves: the first record and the flushed newest record are all that goes out
        assert_eq!(progress, vec![0, 999]);
        assert!(matches!(events.last(), Some(JobEvent::Finished { .. })));
    }

    #[test]
    fn progress_is_limited_to_twenty_messages_per_second_of_clock_time() {
        let (runner, clock) = runner();
        let (sink, rx) = channel();
        let tick = clock.clone();
        runner
            .spawn(
                JobSpec::new("j-rate", "job.test"),
                sink,
                Box::new(move |job| {
                    // one second of clock time, 1000 records, one per millisecond
                    for i in 0..1000_u64 {
                        job.progress().report(Progress::new("scan", i));
                        tick.advance(1);
                    }
                    Ok(JobOutput::done(json!(null)))
                }),
            )
            .expect("spawn");
        let events = drain(&rx);
        let sent = events
            .iter()
            .filter(|e| matches!(e, JobEvent::Progress { .. }))
            .count();
        // 20 windows of 50 ms plus the flush of the pending record at the end
        assert!((20..=21).contains(&sent), "sent {sent}");
        let last = events.iter().rev().find_map(|e| match e {
            JobEvent::Progress { progress, .. } => Some(progress.done),
            _ => None,
        });
        assert_eq!(last, Some(999), "the newest record is never lost");
    }

    #[test]
    fn the_limiter_replaces_held_back_records_and_releases_after_the_interval() {
        let mut limiter = RateLimiter::new(50);
        assert!(limiter.offer(1000, Progress::new("a", 1)).is_some());
        assert!(limiter.offer(1010, Progress::new("a", 2)).is_none());
        assert!(limiter.offer(1020, Progress::new("a", 3)).is_none());
        let sent = limiter.offer(1050, Progress::new("a", 4));
        assert_eq!(sent.map(|p| p.done), Some(4));
        assert!(limiter.take_pending().is_none());
        assert!(limiter.offer(1060, Progress::new("a", 5)).is_none());
        assert_eq!(limiter.take_pending().map(|p| p.done), Some(5));
        // a clock that moves backwards never sends early
        assert!(limiter.offer(900, Progress::new("a", 6)).is_none());
    }

    #[test]
    fn wait_terminal_returns_the_stored_terminal_event() {
        let (runner, _clock) = runner();
        runner
            .spawn(
                JobSpec::new("j-wait", "job.test"),
                Arc::new(NullSink),
                Box::new(|_| Ok(JobOutput::done(json!("x")))),
            )
            .expect("spawn");
        let event = runner.wait_terminal("j-wait", WAIT);
        assert!(matches!(event, Some(JobEvent::Finished { result, .. }) if result == json!("x")));
        assert!(
            runner
                .wait_terminal("missing", Duration::from_millis(10))
                .is_none()
        );
    }

    #[test]
    fn dropping_the_runner_cancels_running_jobs() {
        let (runner, _clock) = runner();
        let ended = Arc::new(AtomicUsize::new(0));
        let flag = Arc::clone(&ended);
        runner
            .spawn(
                JobSpec::new("j-end", "job.test"),
                Arc::new(NullSink),
                Box::new(move |job| {
                    while !job.cancel().is_cancelled() {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    flag.fetch_add(1, Ordering::SeqCst);
                    Err(ApiError::new(codes::JOB_CANCELLED, "cancelled"))
                }),
            )
            .expect("spawn");
        drop(runner);
        assert_eq!(ended.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn minted_ids_are_valid_and_distinct() {
        let (runner, _clock) = runner();
        let a = runner.mint_id();
        let b = runner.mint_id();
        assert_ne!(a, b);
        assert!(JobId::new(&a).is_ok());
    }
}
