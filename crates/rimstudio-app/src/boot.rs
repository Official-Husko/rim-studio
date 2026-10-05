//! Boot: turns a [`BootInput`] into an [`AppContext`].
//!
//! Boot resolves the data roots (the portable marker beside the executable wins over the standard
//! folders), makes sure they exist and are writable, opens the settings through the manager (which
//! writes the commented defaults file on first run), installs logging with the level the settings ask
//! for, checks the crash marker of the previous run, starts the job runner and builds the context.
//! Detection is lazy: nothing probes Steam or scans a library here, so boot stays fast and a machine
//! without Steam still starts.

use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::ports::PlatformDirs;
use rimstudio_core::redact::Redactor;
use rimstudio_io::atomic::atomic_write;
use rimstudio_io::roots::DataRoots;
use rimstudio_manager::settings::{self, SettingsGetRequest};
use rimstudio_toolkit::shared::env::ProjectEnv;
use serde_json::{Value, json};

use crate::context::{
    AppContext, AppOptions, EventBus, LibraryHandle, Platform, SettingsHandle, with_manager_parts,
};
use crate::error::BootError;
use crate::jobs::JobRunner;
use crate::logging::{self, FILTER_VAR, LogConfig};
use crate::workspace::WorkspaceHub;

/// The file in the logs root that tells the next start whether this run ended cleanly.
pub const CRASH_MARKER_FILE: &str = "crash-marker.json";

/// Everything boot needs from the outside.
#[derive(Clone)]
pub struct BootInput {
    /// The operating system ports.
    pub platform: Platform,
    /// Explicit data roots (tests, `--data-dir`); `None` resolves them from the platform.
    pub roots: Option<DataRoots>,
    /// Logging; `None` is the default for an application: the rolling file and the panic hook.
    pub log: Option<LogConfig>,
    /// Options of tests and tools.
    pub options: AppOptions,
    /// The runtime whose blocking pool runs jobs; `None` starts a small runtime of its own.
    pub runtime: Option<tokio::runtime::Handle>,
}

impl std::fmt::Debug for BootInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BootInput")
            .field("platform", &self.platform)
            .field("roots", &self.roots)
            .finish_non_exhaustive()
    }
}

impl BootInput {
    /// An input over the given platform with every option at its default.
    #[must_use]
    pub fn new(platform: Platform) -> Self {
        Self {
            platform,
            roots: None,
            log: None,
            options: AppOptions::default(),
            runtime: None,
        }
    }

    /// An input over the real platform.
    #[must_use]
    pub fn system() -> Self {
        Self::new(Platform::system())
    }

    /// Fixes the data roots.
    #[must_use]
    pub fn with_roots(mut self, roots: DataRoots) -> Self {
        self.roots = Some(roots);
        self
    }

    /// Sets the logging configuration.
    #[must_use]
    pub fn with_log(mut self, log: LogConfig) -> Self {
        self.log = Some(log);
        self
    }

    /// Sets the options.
    #[must_use]
    pub fn with_options(mut self, options: AppOptions) -> Self {
        self.options = options;
        self
    }

    /// Runs jobs on the blocking pool of an existing runtime.
    #[must_use]
    pub fn with_runtime(mut self, handle: tokio::runtime::Handle) -> Self {
        self.runtime = Some(handle);
        self
    }
}

/// What boot found out; part of the context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootReport {
    /// The random id of this run.
    pub session_id: String,
    /// When boot started, Unix milliseconds.
    pub started_at_ms: u64,
    /// True when the previous run left a crash marker that was never marked clean.
    pub previous_run_ended_abnormally: bool,
    /// The session id of that previous run, when known.
    pub previous_session: Option<String>,
    /// True when this run uses the folders beside the executable.
    pub portable: bool,
    /// True when boot created the settings file with its default comments.
    pub settings_created: bool,
    /// Why the settings file was not read faithfully, when it was not (the app then runs on
    /// defaults and the file stays untouched).
    pub settings_problem: Option<String>,
}

fn resolve_roots(platform: &Platform) -> Result<DataRoots, BootError> {
    let env = platform.env.as_ref();
    let dirs: Option<PlatformDirs> = rimstudio_platform::dirs::platform_dirs(platform.os, env)
        .or_else(|| {
            DataRoots::portable_base(env).map(|base| rimstudio_platform::dirs::portable_dirs(&base))
        });
    let dirs = dirs.ok_or(BootError::NoDataDir)?;
    Ok(DataRoots::resolve(&dirs, env, platform.fs.as_ref()))
}

fn make_session_id(now_ms: u64, roots: &DataRoots) -> String {
    let seed = format!("{now_ms}:{}:{}", roots.data, std::process::id());
    let hash = blake3::hash(seed.as_bytes()).to_hex();
    format!(
        "s-{:x}-{}",
        now_ms,
        hash.as_str().chars().take(6).collect::<String>()
    )
}

fn marker_path(roots: &DataRoots) -> Utf8PathBuf {
    roots.logs.join(CRASH_MARKER_FILE)
}

fn read_marker(path: &Utf8Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_marker(path: &Utf8Path, session: &str, started_at_ms: u64, state: &str) {
    let marker = json!({
        "schemaVersion": 1,
        "session": session,
        "startedAt": started_at_ms,
        "version": env!("CARGO_PKG_VERSION"),
        "state": state,
    });
    match serde_json::to_vec_pretty(&marker) {
        Ok(bytes) => {
            if let Err(e) = atomic_write(path, &bytes) {
                tracing::warn!(error = %e, "the crash marker could not be written");
            }
        }
        Err(e) => tracing::warn!(error = %e, "the crash marker could not be encoded"),
    }
}

/// Marks this session as ended cleanly, so the next start does not report a crash. The marker is
/// rewritten (state `clean`) because deleting files is the business of `rimstudio-io`.
pub fn mark_clean_shutdown(ctx: &AppContext) {
    write_marker(
        &marker_path(&ctx.roots),
        &ctx.boot.session_id,
        ctx.boot.started_at_ms,
        "clean",
    );
}

/// Builds the application context.
///
/// # Errors
/// [`BootError`] when no data folder can be determined or written, when the settings cannot be opened
/// at all, or when the job runtime cannot start. A damaged settings file is not an error: the app runs
/// on defaults and reports the problem in [`BootReport::settings_problem`].
pub fn boot(input: BootInput) -> Result<AppContext, BootError> {
    let BootInput {
        platform,
        roots,
        log,
        options,
        runtime,
    } = input;
    let roots = match roots {
        Some(roots) => roots,
        None => resolve_roots(&platform)?,
    };
    roots.ensure_all()?;
    if let Some((kind, reason)) = roots.check_writable().into_iter().next() {
        return Err(BootError::NotWritable {
            root: kind.as_str(),
            reason,
        });
    }
    let started_at_ms = platform.clock.now_unix_ms();
    let session_id = make_session_id(started_at_ms, &roots);

    let view = with_manager_parts(&roots, &platform, |m| {
        settings::get(m, SettingsGetRequest::default())
    })?;

    let mut redactor = Redactor::new();
    if let Some(home) = platform.env.home_dir() {
        redactor = redactor.with_home(home.as_str());
    }
    let redactor = Arc::new(redactor);

    let mut log_config = log.unwrap_or_default();
    log_config.level = Some(view.settings.logging.level);
    if log_config.filter.is_none() {
        log_config.filter = platform.env.var(FILTER_VAR);
    }
    log_config.session.clone_from(&session_id);
    let log_guard = logging::init(&roots, &log_config, Arc::clone(&redactor));

    let marker = marker_path(&roots);
    let previous = read_marker(&marker);
    let previous_session = previous
        .as_ref()
        .and_then(|m| m.get("session"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let previous_run_ended_abnormally = previous
        .as_ref()
        .is_some_and(|m| m.get("state").and_then(Value::as_str) != Some("clean"));
    write_marker(&marker, &session_id, started_at_ms, "running");
    if previous_run_ended_abnormally {
        tracing::warn!(
            previous_session = previous_session.as_deref().unwrap_or("unknown"),
            "the previous session ended abnormally"
        );
    }

    let jobs = match runtime {
        Some(handle) => JobRunner::with_handle(Arc::clone(&platform.clock), handle),
        None => JobRunner::new(Arc::clone(&platform.clock))
            .map_err(|e| BootError::Runtime(e.to_string()))?,
    };
    let env = ProjectEnv::new(&roots, Arc::clone(&platform.clock))?;
    let settings_problem = view.file_problem.as_ref().map(|p| p.message.clone());
    let report = BootReport {
        session_id,
        started_at_ms,
        previous_run_ended_abnormally,
        previous_session,
        portable: roots.portable,
        settings_created: view.created,
        settings_problem,
    };
    tracing::info!(
        target: "rimstudio_app::boot",
        version = env!("CARGO_PKG_VERSION"),
        os = ?platform.os,
        portable = roots.portable,
        session = %report.session_id,
        threads = std::thread::available_parallelism().map_or(1, usize::from),
        settings_created = report.settings_created,
        read_only_settings = view.read_only,
        "application started"
    );
    Ok(AppContext {
        roots: Arc::new(roots),
        platform: Arc::new(platform),
        settings: Arc::new(SettingsHandle::new(view.settings)),
        library: Arc::new(LibraryHandle::default()),
        workspace: Arc::new(WorkspaceHub::new(env)),
        jobs: Arc::new(jobs),
        events: EventBus::default(),
        redactor,
        options: Arc::new(options),
        boot: Arc::new(report),
        log: Arc::new(log_guard),
    })
}
