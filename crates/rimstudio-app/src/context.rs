//! The application context: one explicit value that every handler receives.
//!
//! [`AppContext`] holds the data roots, the platform ports, the settings handle, the library handle,
//! the workspace hub, the job runner and the event bus. It has no global state: [`crate::boot::boot`]
//! builds it once and every clone shares the same parts (each field is an `Arc` or a cheap handle), so
//! a job can carry its own clone to a worker thread. Tests build it from fakes through the same boot
//! path.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use std::time::Duration;

use rimstudio_core::ids::SourceId;
use rimstudio_core::os::Os;
use rimstudio_core::ports::{
    Clock, CredentialStore, DetectEnvRef, EnvProbe, FsProbe, InstallSourceProbe, Launcher,
    LinkBackend, ProcessProbe, RegistryProbe, SandboxProbe,
};
use rimstudio_core::redact::Redactor;
use rimstudio_core::settings::Settings;
use rimstudio_core::version::GameVersion;
use rimstudio_defs::TypeTable;
use rimstudio_io::real_fs::RealFs;
use rimstudio_io::roots::DataRoots;
use rimstudio_io::statkey::FileIdFn;
use rimstudio_ipc_types::error::ApiError;
use rimstudio_ipc_types::library::SourceReportDto;
use rimstudio_library::index::LibraryIndex;
use rimstudio_manager::scan::LibraryCounts;
use rimstudio_platform::SystemPlatform;

use crate::boot::BootReport;
use crate::error::{api_from_manager, api_from_toolkit, api_from_workspace};
use crate::jobs::JobRunner;
use crate::logging::LogGuard;
use crate::workspace::WorkspaceHub;

/// Every port of the operating system behind one value (`dyn` implementations of the core ports).
#[derive(Clone)]
pub struct Platform {
    /// The operating system to assume.
    pub os: Os,
    /// Time.
    pub clock: Arc<dyn Clock>,
    /// Environment variables, home and executable folders.
    pub env: Arc<dyn EnvProbe>,
    /// The Windows registry (answers nothing elsewhere).
    pub registry: Arc<dyn RegistryProbe>,
    /// The file system, with deadlines.
    pub fs: Arc<dyn FsProbe>,
    /// Running processes.
    pub process: Arc<dyn ProcessProbe>,
    /// Folder links.
    pub links: Arc<dyn LinkBackend>,
    /// Starting programs and URLs.
    pub launcher: Arc<dyn Launcher>,
    /// The credential store.
    pub credentials: Arc<dyn CredentialStore>,
    /// Sandbox facts.
    pub sandbox: Arc<dyn SandboxProbe>,
    /// How the app was installed.
    pub install_source: Arc<dyn InstallSourceProbe>,
    /// The file identity hook of the platform, when it has one (scans use it on FAT volumes).
    pub file_id: Option<FileIdFn>,
}

impl std::fmt::Debug for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Platform")
            .field("os", &self.os)
            .finish_non_exhaustive()
    }
}

impl Platform {
    /// The real platform of the running operating system.
    #[must_use]
    pub fn system() -> Self {
        let sys = SystemPlatform::new();
        Self {
            os: sys.os(),
            clock: Arc::new(sys),
            env: Arc::new(sys),
            registry: Arc::new(sys),
            fs: Arc::new(RealFs::new()),
            process: Arc::new(sys),
            links: Arc::new(sys),
            launcher: Arc::new(sys),
            credentials: Arc::new(sys),
            sandbox: Arc::new(sys),
            install_source: Arc::new(sys),
            file_id: None,
        }
    }

    /// The four ports that detection reads, as one value.
    #[must_use]
    pub fn detect_env(&self) -> DetectEnvRef<'_> {
        DetectEnvRef {
            clock: self.clock.as_ref(),
            env: self.env.as_ref(),
            registry: self.registry.as_ref(),
            fs: self.fs.as_ref(),
        }
    }
}

/// Options that tests and tools set and that production leaves alone.
#[derive(Clone, Default)]
pub struct AppOptions {
    /// A ready def type table. Without it the table is read from the game assemblies at run time.
    pub type_table: Option<Arc<TypeTable>>,
}

impl AppOptions {
    /// Uses the def type table in `json` (the data format of `rimstudio-defs`) instead of reading the
    /// game assemblies. For tools and tests that run without a game install.
    ///
    /// # Errors
    /// The text of the table error when `json` is not a valid table.
    pub fn with_type_table_json(mut self, json: &str) -> Result<Self, String> {
        let table = TypeTable::from_json_str(json).map_err(|e| e.to_string())?;
        self.type_table = Some(Arc::new(table));
        Ok(self)
    }
}

impl std::fmt::Debug for AppOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppOptions")
            .field("type_table", &self.type_table.is_some())
            .finish()
    }
}

/// The settings as the app last read them (the file stays the source of truth).
#[derive(Debug)]
pub struct SettingsHandle {
    current: RwLock<Arc<Settings>>,
}

impl SettingsHandle {
    /// A handle holding `settings`.
    #[must_use]
    pub fn new(settings: Settings) -> Self {
        Self {
            current: RwLock::new(Arc::new(settings)),
        }
    }

    /// The settings as last read.
    #[must_use]
    pub fn get(&self) -> Arc<Settings> {
        match self.current.read() {
            Ok(g) => Arc::clone(&g),
            Err(p) => Arc::clone(&p.into_inner()),
        }
    }

    /// Replaces the cached settings after a write.
    pub fn set(&self, settings: Settings) {
        let value = Arc::new(settings);
        match self.current.write() {
            Ok(mut g) => *g = value,
            Err(p) => *p.into_inner() = value,
        }
    }
}

/// The result of the last library scan, kept for the sessions and the sources list.
#[derive(Debug)]
pub struct LibrarySnapshot {
    /// The revision of the library; every completed scan makes a new one.
    pub rev: u64,
    /// The scanned mods.
    pub index: LibraryIndex,
    /// Counts over the index.
    pub counts: LibraryCounts,
    /// One entry per source.
    pub sources: Vec<SourceReportDto>,
    /// When the scan ended, Unix milliseconds.
    pub scanned_at_ms: u64,
    /// The game version the scan narrowed definitions to.
    pub game_version: Option<GameVersion>,
}

impl LibrarySnapshot {
    /// The number of mods found in a source at the scan.
    #[must_use]
    pub fn mods_in(&self, id: &SourceId) -> Option<u32> {
        self.sources
            .iter()
            .find(|s| s.id == id.as_str())
            .map(|s| s.mods)
    }
}

/// The library handle: the newest snapshot and the revision counter.
#[derive(Debug, Default)]
pub struct LibraryHandle {
    current: RwLock<Option<Arc<LibrarySnapshot>>>,
    rev: AtomicU64,
}

impl LibraryHandle {
    /// The newest snapshot, `None` before the first scan.
    #[must_use]
    pub fn current(&self) -> Option<Arc<LibrarySnapshot>> {
        match self.current.read() {
            Ok(g) => g.clone(),
            Err(p) => p.into_inner().clone(),
        }
    }

    /// The revision of the newest snapshot, zero before the first scan.
    #[must_use]
    pub fn rev(&self) -> u64 {
        self.rev.load(Ordering::Acquire)
    }

    /// The revision the next snapshot will get.
    #[must_use]
    pub fn next_rev(&self) -> u64 {
        self.rev.load(Ordering::Acquire).saturating_add(1)
    }

    /// Installs a snapshot built with [`LibraryHandle::next_rev`].
    pub fn install(&self, snapshot: LibrarySnapshot) -> Arc<LibrarySnapshot> {
        let snapshot = Arc::new(snapshot);
        self.rev.store(snapshot.rev, Ordering::Release);
        match self.current.write() {
            Ok(mut g) => *g = Some(Arc::clone(&snapshot)),
            Err(p) => *p.into_inner() = Some(Arc::clone(&snapshot)),
        }
        snapshot
    }
}

/// A broadcast fact that no single caller owns (`settings:changed` and the like).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
    /// A settings write; the webview refetches the named sections.
    SettingsChanged {
        /// The new revision.
        rev: u64,
        /// The sections that changed.
        sections: Vec<String>,
    },
    /// The set of mod sources changed.
    SourcesChanged {
        /// The ids of the affected sources.
        source_ids: Vec<String>,
    },
}

impl AppEvent {
    /// The wire name, `<area>:<noun>`.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::SettingsChanged { .. } => "settings:changed",
            Self::SourcesChanged { .. } => "sources:changed",
        }
    }
}

/// Fans events out to every subscriber; a dropped receiver is forgotten at the next send.
#[derive(Debug, Clone, Default)]
pub struct EventBus {
    subscribers: Arc<Mutex<Vec<Sender<AppEvent>>>>,
}

impl EventBus {
    fn guard(&self) -> MutexGuard<'_, Vec<Sender<AppEvent>>> {
        self.subscribers.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// A new receiver that gets every event emitted from now on.
    #[must_use]
    pub fn subscribe(&self) -> Receiver<AppEvent> {
        let (tx, rx) = channel();
        self.guard().push(tx);
        rx
    }

    /// Sends an event to every live subscriber.
    pub fn emit(&self, event: &AppEvent) {
        self.guard().retain(|tx| tx.send(event.clone()).is_ok());
    }
}

/// The composition root value every handler receives.
#[derive(Clone)]
pub struct AppContext {
    /// The four data roots and the portable flag; immutable after boot.
    pub roots: Arc<DataRoots>,
    /// The operating system ports; immutable.
    pub platform: Arc<Platform>,
    /// The settings as last read.
    pub settings: Arc<SettingsHandle>,
    /// The newest library scan.
    pub library: Arc<LibraryHandle>,
    /// The workspace sessions (def sessions and projects) of the toolkit.
    pub workspace: Arc<WorkspaceHub>,
    /// The job registry and runner.
    pub jobs: Arc<JobRunner>,
    /// Broadcast events.
    pub events: EventBus,
    /// Redacts private text out of logs and error messages.
    pub redactor: Arc<Redactor>,
    /// Options of tests and tools.
    pub options: Arc<AppOptions>,
    /// What boot found out.
    pub boot: Arc<BootReport>,
    /// Keeps the log writer alive for as long as any clone exists.
    pub log: Arc<LogGuard>,
}

impl std::fmt::Debug for AppContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppContext")
            .field("roots", &self.roots)
            .field("session", &self.boot.session_id)
            .finish_non_exhaustive()
    }
}

impl AppContext {
    /// The clock.
    #[must_use]
    pub fn clock(&self) -> &Arc<dyn Clock> {
        &self.platform.clock
    }

    /// Milliseconds since the Unix epoch.
    #[must_use]
    pub fn now_ms(&self) -> u64 {
        self.platform.clock.now_unix_ms()
    }

    /// Runs `f` with a manager context built from this context's ports.
    pub fn with_manager<R>(&self, f: impl FnOnce(&rimstudio_manager::Ctx<'_>) -> R) -> R {
        with_manager_parts(&self.roots, &self.platform, f)
    }

    /// The workspace document (path overrides and custom folders); a damaged file reads as defaults.
    #[must_use]
    pub fn workspace_settings(&self) -> rimstudio_core::settings::WorkspaceSettings {
        crate::workspace::load_workspace(self)
    }

    /// Maps a manager error to the envelope (the edge of dispatch logs it).
    #[must_use]
    pub fn manager_error(&self, e: &rimstudio_manager::ManagerError) -> ApiError {
        api_from_manager(e, &self.redactor)
    }

    /// Maps a toolkit error.
    #[must_use]
    pub fn toolkit_error(&self, e: &rimstudio_toolkit::error::ToolkitError) -> ApiError {
        api_from_toolkit(e, &self.redactor)
    }

    /// Maps a workspace error.
    #[must_use]
    pub fn workspace_error(&self, e: &rimstudio_workspace::WorkspaceError) -> ApiError {
        api_from_workspace(e, &self.redactor)
    }

    /// Cancels every job, waits up to `wait` for them to end and marks the session as ended cleanly.
    ///
    /// Returns true when every job ended within the time.
    pub fn shutdown(&self, wait: Duration) -> bool {
        self.jobs.cancel_all();
        let idle = self.jobs.wait_idle(wait);
        crate::boot::mark_clean_shutdown(self);
        idle
    }
}

/// Reads the game version note of a `ModsConfig.xml` text. The XML parser lives in
/// `rimstudio-xml`, so the manager takes this function as a hook.
fn mods_config_version(text: &str) -> Option<String> {
    rimstudio_xml::mods_config::read(text.as_bytes())
        .ok()
        .and_then(|read| read.data.version)
}

/// Runs `f` with a manager context built from the roots and the platform ports. Boot uses it before an
/// [`AppContext`] exists.
pub fn with_manager_parts<R>(
    roots: &DataRoots,
    platform: &Platform,
    f: impl FnOnce(&rimstudio_manager::Ctx<'_>) -> R,
) -> R {
    let detect_env = platform.detect_env();
    let mut ctx = rimstudio_manager::Ctx::new(
        roots,
        &detect_env,
        platform.fs.as_ref(),
        platform.clock.as_ref(),
    )
    .with_os(platform.os)
    .with_mods_config_version(mods_config_version)
    .with_scan_fs(Arc::clone(&platform.fs));
    if let Some(hook) = platform.file_id {
        ctx = ctx.with_file_id(hook);
    }
    f(&ctx)
}

#[cfg(test)]
mod options_tests {
    use super::AppOptions;

    #[test]
    fn a_type_table_can_come_from_json_text() {
        let json = r#"{"format":1,"root":"Verse.Def","assemblies":[],"types":{"Verse.Def":{"base":null},"Verse.RsThing":{"base":"Verse.Def"}},"short_name_collisions":{}}"#;
        let options = AppOptions::default().with_type_table_json(json);
        assert!(options.is_ok_and(|o| o.type_table.is_some()));
    }

    #[test]
    fn a_damaged_table_is_reported_not_ignored() {
        assert!(AppOptions::default().with_type_table_json("{").is_err());
    }
}
