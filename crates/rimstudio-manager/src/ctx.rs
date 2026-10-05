//! The narrow context every use case receives.
//!
//! [`Ctx`] is a struct of references to ports and roots. `rimstudio-app` builds one per command from
//! its composition root; tests build one from `rimstudio-testing` fakes. The crate never spawns a
//! process and writes only inside the data roots of the context.

use std::sync::Arc;
use std::time::Duration;

use rimstudio_core::os::Os;
use rimstudio_core::ports::{Clock, DetectEnv, FsProbe};
use rimstudio_core::settings::{Settings, WorkspaceSettings};
use rimstudio_io::roots::{DataRoots, RootKind};
use rimstudio_io::schema::Loaded;
use rimstudio_io::statkey::FileIdFn;
use rimstudio_io::store::Store;

use crate::error::{ManagerError, ManagerResult};

/// The deadline for every probing file system call made by this crate.
pub const PROBE_DEADLINE: Duration = Duration::from_secs(2);

/// The file name of the app settings document in the config root.
pub const SETTINGS_FILE: &str = "settings.jsonc";
/// The file name of the workspace document (path overrides and custom folders) in the config root.
pub const WORKSPACE_FILE: &str = "workspace.jsonc";

/// References to the ports and roots the use cases need.
#[derive(Clone)]
pub struct Ctx<'a> {
    /// The four data roots (config, data, cache, logs).
    pub roots: &'a DataRoots,
    /// Everything detection touches (clock, environment, registry, file system).
    pub detect_env: &'a dyn DetectEnv,
    /// The file system used to probe folders (existence, listing, canonical paths).
    pub fs: &'a dyn FsProbe,
    /// The clock, for backup stamps, ids and report times.
    pub clock: &'a dyn Clock,
    /// The operating system to assume (tests run any OS on any host).
    pub os: Os,
    /// Reads the version note of a `ModsConfig.xml` text; supplied by the application because XML
    /// is parsed only in `rimstudio-xml`. Without it the version mismatch warning never appears.
    pub mods_config_version: Option<fn(&str) -> Option<String>>,
    /// An owned file system handle for the scanner, which classifies volumes through it.
    pub scan_fs: Option<Arc<dyn FsProbe>>,
    /// The file identity hook of `rimstudio-platform`, passed to the scanner.
    pub file_id: Option<FileIdFn>,
}

impl<'a> Ctx<'a> {
    /// A context for the current operating system with no optional hooks.
    pub fn new(
        roots: &'a DataRoots,
        detect_env: &'a dyn DetectEnv,
        fs: &'a dyn FsProbe,
        clock: &'a dyn Clock,
    ) -> Self {
        Ctx {
            roots,
            detect_env,
            fs,
            clock,
            os: Os::current(),
            mods_config_version: None,
            scan_fs: None,
            file_id: None,
        }
    }

    /// Replaces the assumed operating system.
    #[must_use]
    pub fn with_os(mut self, os: Os) -> Self {
        self.os = os;
        self
    }

    /// Sets the `ModsConfig.xml` version reader used by detection.
    #[must_use]
    pub fn with_mods_config_version(mut self, reader: fn(&str) -> Option<String>) -> Self {
        self.mods_config_version = Some(reader);
        self
    }

    /// Sets the owned file system handle the scanner uses to classify volumes.
    #[must_use]
    pub fn with_scan_fs(mut self, fs: Arc<dyn FsProbe>) -> Self {
        self.scan_fs = Some(fs);
        self
    }

    /// Sets the file identity hook.
    #[must_use]
    pub fn with_file_id(mut self, hook: FileIdFn) -> Self {
        self.file_id = Some(hook);
        self
    }

    /// A clock that always answers the current time of the context clock; stores need an owned
    /// clock and a use case lives for one instant.
    pub(crate) fn frozen_clock(&self) -> Arc<dyn Clock> {
        Arc::new(FrozenClock(self.clock.now_unix_ms()))
    }

    pub(crate) fn settings_store(&self) -> ManagerResult<Store<Settings>> {
        Ok(Store::open(
            self.roots,
            RootKind::Config,
            SETTINGS_FILE,
            self.frozen_clock(),
        )?)
    }

    pub(crate) fn workspace_store(&self) -> ManagerResult<Store<WorkspaceSettings>> {
        Ok(Store::open(
            self.roots,
            RootKind::Config,
            WORKSPACE_FILE,
            self.frozen_clock(),
        )?)
    }

    /// Loads the workspace document for a change: the load result and the store, refusing when the
    /// file on disk must not be overwritten.
    pub(crate) fn workspace_for_edit(
        &self,
    ) -> ManagerResult<(Store<WorkspaceSettings>, WorkspaceSettings)> {
        let store = self.workspace_store()?;
        let loaded = store.load();
        refuse_read_only(&store, &loaded)?;
        Ok((store, loaded.value))
    }
}

impl std::fmt::Debug for Ctx<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ctx")
            .field("roots", &self.roots)
            .field("os", &self.os)
            .finish_non_exhaustive()
    }
}

/// Fails with [`ManagerError::ReadOnly`] when the loaded document must not be written back.
pub(crate) fn refuse_read_only<T>(store: &Store<T>, loaded: &Loaded<T>) -> ManagerResult<()>
where
    T: rimstudio_io::schema::Versioned + serde::Serialize + serde::de::DeserializeOwned + Default,
{
    if loaded.read_only {
        let reason = store
            .blocked_reason()
            .or_else(|| loaded.problem.as_ref().map(|p| p.message.clone()))
            .unwrap_or_else(|| "the file cannot be read faithfully".to_owned());
        return Err(ManagerError::ReadOnly {
            path: store.path().to_owned(),
            reason,
        });
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct FrozenClock(u64);

impl Clock for FrozenClock {
    fn now_unix_ms(&self) -> u64 {
        self.0
    }
}
