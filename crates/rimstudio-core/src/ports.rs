//! Port traits: the only way crates below the shell reach the operating system.
//!
//! Every trait is small, object safe and `Send + Sync`. `rimstudio-platform` and `rimstudio-io`
//! implement them for real; `rimstudio-testing` provides fakes. Methods are synchronous; callers that
//! need a deadline pass it explicitly (a probe that cannot honour it reports
//! [`PortErrorKind::TimedOut`] only when it actually timed out).

use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The class of a [`PortError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PortErrorKind {
    /// The path or item does not exist.
    NotFound,
    /// The OS refused access.
    PermissionDenied,
    /// The operation did not finish within its deadline.
    TimedOut,
    /// The port or the operation is not available on this platform.
    Unsupported,
    /// Any other IO failure.
    Io,
}

impl PortErrorKind {
    /// The stable code of this kind, `port.<kebab-name>`.
    pub fn code(self) -> &'static str {
        match self {
            PortErrorKind::NotFound => "port.not-found",
            PortErrorKind::PermissionDenied => "port.permission-denied",
            PortErrorKind::TimedOut => "port.timed-out",
            PortErrorKind::Unsupported => "port.unsupported",
            PortErrorKind::Io => "port.io",
        }
    }
}

/// An error returned by a port implementation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{}: {message}", kind.code())]
pub struct PortError {
    /// What class of failure this is.
    pub kind: PortErrorKind,
    /// A short developer facing message (never contains secrets).
    pub message: String,
}

impl PortError {
    /// Builds an error.
    pub fn new(kind: PortErrorKind, message: impl Into<String>) -> Self {
        PortError {
            kind,
            message: message.into(),
        }
    }

    /// The stable code of the error kind.
    pub fn code(&self) -> &'static str {
        self.kind.code()
    }
}

/// Result alias for port calls.
pub type PortResult<T> = Result<T, PortError>;

// ---------------------------------------------------------------------------------------------
// Clock
// ---------------------------------------------------------------------------------------------

/// A source of time, so tests control it.
pub trait Clock: Send + Sync {
    /// Milliseconds since the Unix epoch (UTC). Never panics; a clock set before 1970 returns 0.
    fn now_unix_ms(&self) -> u64;
}

// ---------------------------------------------------------------------------------------------
// Environment
// ---------------------------------------------------------------------------------------------

/// The standard folders of the app, resolved once by the shell (never discovered by core crates).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformDirs {
    /// Configuration (settings, workspace overrides).
    pub config: Utf8PathBuf,
    /// Durable app data (documents, drafts).
    pub data: Utf8PathBuf,
    /// Rebuildable caches.
    pub cache: Utf8PathBuf,
    /// Log files.
    pub logs: Utf8PathBuf,
    /// Volatile state (locks, session files).
    pub state: Utf8PathBuf,
}

/// Read only access to the process environment and well known folders.
pub trait EnvProbe: Send + Sync {
    /// The value of an environment variable; `None` when unset or not valid UTF-8.
    fn var(&self, key: &str) -> Option<String>;

    /// The user's home folder, `None` when unknown.
    fn home_dir(&self) -> Option<Utf8PathBuf>;

    /// The folder of the running executable, `None` when unknown.
    fn exe_dir(&self) -> Option<Utf8PathBuf>;

    /// The user interface locale as a BCP 47 tag; `en-US` when unknown.
    fn locale(&self) -> String {
        "en-US".to_owned()
    }
}

/// A Windows registry hive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Hive {
    /// `HKEY_LOCAL_MACHINE`.
    LocalMachine,
    /// `HKEY_CURRENT_USER`.
    CurrentUser,
}

/// Read only registry access. Off Windows every call returns `None`.
pub trait RegistryProbe: Send + Sync {
    /// The string value `value` under `key` in `hive`; `None` when absent, unreadable, not a string
    /// or when the platform has no registry.
    fn read_string(&self, hive: Hive, key: &str, value: &str) -> Option<String>;
}

// ---------------------------------------------------------------------------------------------
// File system probing
// ---------------------------------------------------------------------------------------------

/// The kind of a file system entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EntryKind {
    /// A regular file.
    File,
    /// A directory.
    Dir,
    /// A symbolic link (not followed).
    Symlink,
    /// Anything else.
    Other,
}

/// Metadata of one entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsMeta {
    /// The kind of the entry, symbolic links not followed.
    pub kind: EntryKind,
    /// The size in bytes (0 for directories when unknown).
    pub size: u64,
    /// The modification time in nanoseconds since the Unix epoch, `None` when unavailable.
    pub mtime_ns: Option<i128>,
    /// A volume and file identity (inode on Unix, file index on Windows), `None` when unavailable.
    pub file_id: Option<u128>,
}

/// One entry of a directory listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntryInfo {
    /// The file name without its parent.
    pub name: String,
    /// The kind of the entry, symbolic links not followed.
    pub kind: EntryKind,
}

/// The file system class of a volume, which decides timestamp precision and link support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VolumeClass {
    /// FAT32.
    Fat,
    /// exFAT.
    Exfat,
    /// NTFS.
    Ntfs,
    /// A POSIX file system.
    Posix,
    /// Not known.
    Unknown,
}

/// Read only file system queries with a deadline, used where a dead network share must not hang the
/// caller. Writes never go through this port.
pub trait FsProbe: Send + Sync {
    /// True when the path exists (symbolic links followed). A timeout reports `Err(TimedOut)`.
    fn exists(&self, path: &Utf8Path, deadline: Duration) -> PortResult<bool>;

    /// True when the path is a directory (symbolic links followed).
    fn is_dir(&self, path: &Utf8Path, deadline: Duration) -> PortResult<bool>;

    /// Metadata of the path itself (symbolic links not followed).
    fn metadata(&self, path: &Utf8Path, deadline: Duration) -> PortResult<FsMeta>;

    /// The entries of a directory sorted by name; names that are not UTF-8 are skipped.
    fn read_dir(&self, path: &Utf8Path, deadline: Duration) -> PortResult<Vec<DirEntryInfo>>;

    /// The whole file as UTF-8 text. A leading byte order mark is kept; the caller strips it.
    fn read_to_string(&self, path: &Utf8Path, deadline: Duration) -> PortResult<String>;

    /// The canonical absolute path with links resolved.
    fn canonicalize(&self, path: &Utf8Path, deadline: Duration) -> PortResult<Utf8PathBuf>;

    /// The class of the volume that holds `path`; [`VolumeClass::Unknown`] when not determinable.
    fn volume_class(&self, path: &Utf8Path) -> VolumeClass {
        let _ = path;
        VolumeClass::Unknown
    }
}

/// Everything game and Steam detection touches, composed from four ports.
pub trait DetectEnv: Send + Sync {
    /// The clock.
    fn clock(&self) -> &dyn Clock;
    /// The environment.
    fn env(&self) -> &dyn EnvProbe;
    /// The registry (a stub off Windows).
    fn registry(&self) -> &dyn RegistryProbe;
    /// The file system.
    fn fs(&self) -> &dyn FsProbe;
}

/// A plain [`DetectEnv`] holding four references.
#[derive(Clone, Copy)]
pub struct DetectEnvRef<'a> {
    /// The clock.
    pub clock: &'a dyn Clock,
    /// The environment.
    pub env: &'a dyn EnvProbe,
    /// The registry.
    pub registry: &'a dyn RegistryProbe,
    /// The file system.
    pub fs: &'a dyn FsProbe,
}

impl std::fmt::Debug for DetectEnvRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DetectEnvRef").finish_non_exhaustive()
    }
}

impl DetectEnv for DetectEnvRef<'_> {
    fn clock(&self) -> &dyn Clock {
        self.clock
    }
    fn env(&self) -> &dyn EnvProbe {
        self.env
    }
    fn registry(&self) -> &dyn RegistryProbe {
        self.registry
    }
    fn fs(&self) -> &dyn FsProbe {
        self.fs
    }
}

// ---------------------------------------------------------------------------------------------
// Processes
// ---------------------------------------------------------------------------------------------

/// A running process as seen by [`ProcessProbe`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningProcess {
    /// The OS process id.
    pub pid: u32,
    /// The process name (executable file name).
    pub name: String,
    /// The executable path when the OS reveals it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exe: Option<Utf8PathBuf>,
}

/// Read only process listing.
pub trait ProcessProbe: Send + Sync {
    /// The running processes whose name equals one of `names` (ignoring ASCII case), sorted by pid.
    /// An empty list means none are running or the sandbox hides the process table; use
    /// [`SandboxProbe`] to tell the two apart.
    fn running(&self, names: &[&str]) -> Vec<RunningProcess>;
}

// ---------------------------------------------------------------------------------------------
// Links
// ---------------------------------------------------------------------------------------------

/// The kind of directory link that was created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkKind {
    /// A symbolic link.
    Symlink,
    /// A Windows junction.
    Junction,
}

/// What the platform can create right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkSupport {
    /// Directory symbolic links can be created.
    pub symlink: bool,
    /// Junctions can be created.
    pub junction: bool,
    /// Creating a symbolic link needs a privilege the process lacks.
    pub needs_privilege: bool,
}

/// Directory link primitives. The write fence in `rimstudio-io` is the only caller allowed to use
/// the mutating methods.
pub trait LinkBackend: Send + Sync {
    /// What the platform can create.
    fn support(&self) -> LinkSupport;

    /// Creates a directory link at `link` pointing at the absolute path `target`. Fails with
    /// [`PortErrorKind::Io`] when `link` exists. Returns the kind that was created.
    fn create_dir_link(&self, target: &Utf8Path, link: &Utf8Path) -> PortResult<LinkKind>;

    /// Removes the link itself and never the target's content. Fails when `link` is not a link.
    fn remove_link(&self, link: &Utf8Path) -> PortResult<()>;

    /// True when `path` is a symbolic link or junction (not followed).
    fn is_link(&self, path: &Utf8Path) -> bool;

    /// The target of a link; `None` when `path` is not a link.
    fn read_link(&self, path: &Utf8Path) -> Option<Utf8PathBuf>;
}

// ---------------------------------------------------------------------------------------------
// Launching
// ---------------------------------------------------------------------------------------------

/// A request to start a program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchSpec {
    /// The program to start.
    pub program: Utf8PathBuf,
    /// Arguments in order, passed without shell interpretation.
    #[serde(default)]
    pub args: Vec<String>,
    /// The working folder, `None` for the current one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<Utf8PathBuf>,
    /// Extra environment variables as `(name, value)` pairs.
    #[serde(default)]
    pub env: Vec<(String, String)>,
    /// A Steam URL to open instead of starting `program`, when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_url: Option<String>,
}

/// A started child process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildHandle {
    /// The OS process id, 0 when the launch handed off to another program (an `open_url`).
    pub pid: u32,
}

/// The single place a process is spawned.
pub trait Launcher: Send + Sync {
    /// Starts the program detached from the app and returns at once.
    fn spawn(&self, spec: &LaunchSpec) -> PortResult<ChildHandle>;
}

// ---------------------------------------------------------------------------------------------
// Secrets
// ---------------------------------------------------------------------------------------------

/// Storage for secrets. Values never appear in logs or error messages.
pub trait CredentialStore: Send + Sync {
    /// The secret stored under `service` and `account`; `Ok(None)` when absent.
    fn get(&self, service: &str, account: &str) -> PortResult<Option<String>>;

    /// Stores or replaces a secret.
    fn set(&self, service: &str, account: &str, secret: &str) -> PortResult<()>;

    /// Deletes a secret; deleting an absent one succeeds.
    fn delete(&self, service: &str, account: &str) -> PortResult<()>;
}

// ---------------------------------------------------------------------------------------------
// Sandbox and install source
// ---------------------------------------------------------------------------------------------

/// The sandbox the app runs in, as plain data.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxInfo {
    /// A sandbox kind such as `flatpak` or `snap`; `None` when not sandboxed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// True when the process table of the host is hidden.
    #[serde(default)]
    pub hides_processes: bool,
    /// Folders the sandbox is known to grant access to.
    #[serde(default)]
    pub granted_paths: Vec<Utf8PathBuf>,
}

impl SandboxInfo {
    /// The value for an app that is not sandboxed.
    pub fn none() -> Self {
        SandboxInfo::default()
    }

    /// True when a sandbox was detected.
    pub fn is_sandboxed(&self) -> bool {
        self.kind.is_some()
    }
}

/// Detects the sandbox.
pub trait SandboxProbe: Send + Sync {
    /// The sandbox information; the default is "not sandboxed".
    fn info(&self) -> SandboxInfo {
        SandboxInfo::none()
    }
}

/// How the app was installed, which decides whether it may update itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallSource {
    /// An AppImage.
    AppImage,
    /// A Flatpak.
    Flatpak,
    /// A Snap.
    Snap,
    /// A package owned by the system package manager.
    SystemPackage,
    /// A macOS application bundle.
    MacosApp,
    /// A Windows installer.
    WindowsInstaller,
    /// A portable copy (marker file beside the executable).
    Portable,
    /// Not determinable.
    Unknown,
}

/// Detects how the app was installed.
pub trait InstallSourceProbe: Send + Sync {
    /// The install source.
    fn detect(&self) -> InstallSource;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedClock(u64);
    impl Clock for FixedClock {
        fn now_unix_ms(&self) -> u64 {
            self.0
        }
    }

    struct NoEnv;
    impl EnvProbe for NoEnv {
        fn var(&self, _key: &str) -> Option<String> {
            None
        }
        fn home_dir(&self) -> Option<Utf8PathBuf> {
            None
        }
        fn exe_dir(&self) -> Option<Utf8PathBuf> {
            None
        }
    }

    struct NoReg;
    impl RegistryProbe for NoReg {
        fn read_string(&self, _: Hive, _: &str, _: &str) -> Option<String> {
            None
        }
    }

    struct NoFs;
    impl FsProbe for NoFs {
        fn exists(&self, _: &Utf8Path, _: Duration) -> PortResult<bool> {
            Ok(false)
        }
        fn is_dir(&self, _: &Utf8Path, _: Duration) -> PortResult<bool> {
            Ok(false)
        }
        fn metadata(&self, _: &Utf8Path, _: Duration) -> PortResult<FsMeta> {
            Err(PortError::new(PortErrorKind::NotFound, "none"))
        }
        fn read_dir(&self, _: &Utf8Path, _: Duration) -> PortResult<Vec<DirEntryInfo>> {
            Ok(Vec::new())
        }
        fn read_to_string(&self, _: &Utf8Path, _: Duration) -> PortResult<String> {
            Err(PortError::new(PortErrorKind::NotFound, "none"))
        }
        fn canonicalize(&self, p: &Utf8Path, _: Duration) -> PortResult<Utf8PathBuf> {
            Ok(p.to_owned())
        }
    }

    #[test]
    fn traits_are_object_safe_and_compose_into_detect_env() {
        let (clock, env, reg, fs) = (FixedClock(7), NoEnv, NoReg, NoFs);
        let detect = DetectEnvRef {
            clock: &clock,
            env: &env,
            registry: &reg,
            fs: &fs,
        };
        let dynamic: &dyn DetectEnv = &detect;
        assert_eq!(dynamic.clock().now_unix_ms(), 7);
        assert_eq!(dynamic.env().locale(), "en-US");
        assert!(
            dynamic
                .registry()
                .read_string(Hive::LocalMachine, "k", "v")
                .is_none()
        );
        let d = Duration::from_millis(5);
        assert_eq!(dynamic.fs().exists(Utf8Path::new("/x"), d), Ok(false));
        assert_eq!(
            dynamic.fs().volume_class(Utf8Path::new("/x")),
            VolumeClass::Unknown
        );
    }

    #[test]
    fn port_errors_have_stable_codes() {
        let e = PortError::new(PortErrorKind::TimedOut, "slow share");
        assert_eq!(e.code(), "port.timed-out");
        assert_eq!(e.to_string(), "port.timed-out: slow share");
    }

    #[test]
    fn sandbox_default_is_not_sandboxed() {
        struct P;
        impl SandboxProbe for P {}
        assert!(!P.info().is_sandboxed());
    }

    #[test]
    fn launch_spec_round_trips_through_json() {
        let spec = LaunchSpec {
            program: "/g/RimWorldLinux".into(),
            args: vec!["-savedatafolder=/s".into()],
            cwd: None,
            env: vec![("A".into(), "b".into())],
            open_url: None,
        };
        let json = serde_json::to_string(&spec).unwrap();
        assert_eq!(serde_json::from_str::<LaunchSpec>(&json).unwrap(), spec);
    }
}
