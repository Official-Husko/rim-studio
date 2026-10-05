//! Fakes for the ports of `rimstudio_core::ports`, except the file system (see [`crate::fake_fs`]).
//!
//! Every fake is deterministic, thread safe and uses interior mutability, so one value can be
//! shared by reference between the code under test and the assertions. Clones of [`FakeClock`]
//! share the same time.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::ports::{
    ChildHandle, Clock, CredentialStore, EnvProbe, Hive, InstallSource, InstallSourceProbe,
    LaunchSpec, Launcher, LinkBackend, LinkKind, LinkSupport, PortError, PortErrorKind, PortResult,
    ProcessProbe, RegistryProbe, RunningProcess, SandboxInfo, SandboxProbe,
};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---------------------------------------------------------------------------------------------
// Clock
// ---------------------------------------------------------------------------------------------

/// A clock that only moves when told to. Clones share the time.
#[derive(Debug, Clone)]
pub struct FakeClock {
    ms: Arc<AtomicU64>,
}

impl FakeClock {
    /// The default start: 2023-11-14T22:13:20Z.
    pub const DEFAULT_START_MS: u64 = 1_700_000_000_000;

    /// A clock at `ms` milliseconds since the epoch.
    pub fn new(ms: u64) -> Self {
        FakeClock {
            ms: Arc::new(AtomicU64::new(ms)),
        }
    }

    /// Sets the time.
    pub fn set(&self, ms: u64) {
        self.ms.store(ms, Ordering::SeqCst);
    }

    /// Moves the time forward (saturating).
    pub fn advance(&self, ms: u64) {
        let _ = self
            .ms
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| {
                Some(v.saturating_add(ms))
            });
    }
}

impl Default for FakeClock {
    fn default() -> Self {
        FakeClock::new(Self::DEFAULT_START_MS)
    }
}

impl Clock for FakeClock {
    fn now_unix_ms(&self) -> u64 {
        self.ms.load(Ordering::SeqCst)
    }
}

// ---------------------------------------------------------------------------------------------
// Environment
// ---------------------------------------------------------------------------------------------

#[derive(Debug)]
struct EnvState {
    vars: BTreeMap<String, String>,
    home: Option<Utf8PathBuf>,
    exe_dir: Option<Utf8PathBuf>,
    locale: String,
}

/// An environment with explicit variables, home folder, executable folder and locale.
#[derive(Debug)]
pub struct FakeEnv {
    state: Mutex<EnvState>,
}

impl Default for FakeEnv {
    fn default() -> Self {
        FakeEnv {
            state: Mutex::new(EnvState {
                vars: BTreeMap::new(),
                home: None,
                exe_dir: None,
                locale: "en-US".to_owned(),
            }),
        }
    }
}

impl FakeEnv {
    /// An empty environment: no variables, no home, locale `en-US`.
    pub fn new() -> Self {
        FakeEnv::default()
    }

    /// Sets a variable.
    pub fn with_var(self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.set_var(key, value);
        self
    }

    /// Sets the home folder.
    pub fn with_home(self, home: impl Into<Utf8PathBuf>) -> Self {
        self.set_home(Some(home.into()));
        self
    }

    /// Sets the executable folder.
    pub fn with_exe_dir(self, dir: impl Into<Utf8PathBuf>) -> Self {
        lock(&self.state).exe_dir = Some(dir.into());
        self
    }

    /// Sets the locale.
    pub fn with_locale(self, locale: impl Into<String>) -> Self {
        lock(&self.state).locale = locale.into();
        self
    }

    /// Sets or replaces a variable on a shared value.
    pub fn set_var(&self, key: impl Into<String>, value: impl Into<String>) {
        lock(&self.state).vars.insert(key.into(), value.into());
    }

    /// Removes a variable.
    pub fn remove_var(&self, key: &str) {
        lock(&self.state).vars.remove(key);
    }

    /// Sets or clears the home folder.
    pub fn set_home(&self, home: Option<Utf8PathBuf>) {
        lock(&self.state).home = home;
    }
}

impl EnvProbe for FakeEnv {
    fn var(&self, key: &str) -> Option<String> {
        lock(&self.state).vars.get(key).cloned()
    }
    fn home_dir(&self) -> Option<Utf8PathBuf> {
        lock(&self.state).home.clone()
    }
    fn exe_dir(&self) -> Option<Utf8PathBuf> {
        lock(&self.state).exe_dir.clone()
    }
    fn locale(&self) -> String {
        lock(&self.state).locale.clone()
    }
}

// ---------------------------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------------------------

type RegKey = (bool, String, String);

fn reg_key(hive: Hive, key: &str, value: &str) -> RegKey {
    // Registry key and value names are case insensitive; both separators are accepted.
    (
        hive == Hive::LocalMachine,
        key.replace('/', "\\").trim_matches('\\').to_lowercase(),
        value.to_lowercase(),
    )
}

/// A registry with string values. Names compare case insensitively, as on Windows.
#[derive(Debug, Default)]
pub struct FakeRegistry {
    values: Mutex<BTreeMap<RegKey, String>>,
}

impl FakeRegistry {
    /// An empty registry (every read is `None`, as off Windows).
    pub fn new() -> Self {
        FakeRegistry::default()
    }

    /// Sets a string value.
    pub fn set(&self, hive: Hive, key: &str, value: &str, data: &str) {
        lock(&self.values).insert(reg_key(hive, key, value), data.to_owned());
    }

    /// Builder form of [`FakeRegistry::set`].
    pub fn with(self, hive: Hive, key: &str, value: &str, data: &str) -> Self {
        self.set(hive, key, value, data);
        self
    }

    /// Removes a value.
    pub fn remove(&self, hive: Hive, key: &str, value: &str) {
        lock(&self.values).remove(&reg_key(hive, key, value));
    }
}

impl RegistryProbe for FakeRegistry {
    fn read_string(&self, hive: Hive, key: &str, value: &str) -> Option<String> {
        lock(&self.values).get(&reg_key(hive, key, value)).cloned()
    }
}

// ---------------------------------------------------------------------------------------------
// Processes
// ---------------------------------------------------------------------------------------------

/// A process table that tests edit.
#[derive(Debug, Default)]
pub struct FakeProcessProbe {
    table: Mutex<Vec<RunningProcess>>,
}

impl FakeProcessProbe {
    /// An empty table.
    pub fn new() -> Self {
        FakeProcessProbe::default()
    }

    /// Adds a process.
    pub fn add(&self, pid: u32, name: &str, exe: Option<&str>) {
        lock(&self.table).push(RunningProcess {
            pid,
            name: name.to_owned(),
            exe: exe.map(Utf8PathBuf::from),
        });
    }

    /// Builder form of [`FakeProcessProbe::add`].
    pub fn with(self, pid: u32, name: &str) -> Self {
        self.add(pid, name, None);
        self
    }

    /// Removes the process with this pid.
    pub fn remove(&self, pid: u32) {
        lock(&self.table).retain(|p| p.pid != pid);
    }

    /// Empties the table.
    pub fn clear(&self) {
        lock(&self.table).clear();
    }
}

impl ProcessProbe for FakeProcessProbe {
    fn running(&self, names: &[&str]) -> Vec<RunningProcess> {
        let mut v: Vec<RunningProcess> = lock(&self.table)
            .iter()
            .filter(|p| names.iter().any(|n| n.eq_ignore_ascii_case(&p.name)))
            .cloned()
            .collect();
        v.sort_by_key(|p| p.pid);
        v
    }
}

// ---------------------------------------------------------------------------------------------
// Links
// ---------------------------------------------------------------------------------------------

/// One call seen by [`FakeLinkBackend`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkOp {
    /// `create_dir_link(target, link)` (recorded even when it fails).
    Create {
        /// The target.
        target: Utf8PathBuf,
        /// The link path.
        link: Utf8PathBuf,
    },
    /// `remove_link(link)` (recorded even when it fails).
    Remove {
        /// The link path.
        link: Utf8PathBuf,
    },
}

#[derive(Debug)]
struct LinkState {
    support: LinkSupport,
    kind: LinkKind,
    links: BTreeMap<Utf8PathBuf, Utf8PathBuf>,
    ops: Vec<LinkOp>,
    /// Number of mutating calls still allowed; `None` means unlimited.
    budget: Option<usize>,
}

/// An in memory link table. Links exist only here; pair it with a [`crate::fake_fs::FakeFs`] when
/// the test also needs the folders.
#[derive(Debug)]
pub struct FakeLinkBackend {
    state: Mutex<LinkState>,
}

impl Default for FakeLinkBackend {
    fn default() -> Self {
        FakeLinkBackend::new()
    }
}

impl FakeLinkBackend {
    /// A backend that creates symbolic links without a privilege.
    pub fn new() -> Self {
        FakeLinkBackend {
            state: Mutex::new(LinkState {
                support: LinkSupport {
                    symlink: true,
                    junction: false,
                    needs_privilege: false,
                },
                kind: LinkKind::Symlink,
                links: BTreeMap::new(),
                ops: Vec::new(),
                budget: None,
            }),
        }
    }

    /// A backend that cannot create any link.
    pub fn unsupported() -> Self {
        let b = FakeLinkBackend::new();
        lock(&b.state).support = LinkSupport {
            symlink: false,
            junction: false,
            needs_privilege: false,
        };
        b
    }

    /// A Windows like backend: junctions work, symbolic links need a privilege.
    pub fn windows_like() -> Self {
        let b = FakeLinkBackend::new();
        {
            let mut st = lock(&b.state);
            st.support = LinkSupport {
                symlink: false,
                junction: true,
                needs_privilege: true,
            };
            st.kind = LinkKind::Junction;
        }
        b
    }

    /// Lets `n` more mutating calls succeed and fails the rest with an IO error.
    pub fn fail_after(&self, n: usize) {
        lock(&self.state).budget = Some(n);
    }

    /// The current links as `(link, target)` pairs, sorted by link.
    pub fn links(&self) -> Vec<(Utf8PathBuf, Utf8PathBuf)> {
        lock(&self.state)
            .links
            .iter()
            .map(|(l, t)| (l.clone(), t.clone()))
            .collect()
    }

    /// Every mutating call so far.
    pub fn ops(&self) -> Vec<LinkOp> {
        lock(&self.state).ops.clone()
    }
}

fn spend(st: &mut LinkState) -> PortResult<()> {
    if let Some(b) = st.budget.as_mut() {
        if *b == 0 {
            return Err(PortError::new(PortErrorKind::Io, "simulated crash"));
        }
        *b -= 1;
    }
    Ok(())
}

impl LinkBackend for FakeLinkBackend {
    fn support(&self) -> LinkSupport {
        lock(&self.state).support
    }

    fn create_dir_link(&self, target: &Utf8Path, link: &Utf8Path) -> PortResult<LinkKind> {
        let mut st = lock(&self.state);
        st.ops.push(LinkOp::Create {
            target: target.to_owned(),
            link: link.to_owned(),
        });
        if !st.support.symlink && !st.support.junction {
            return Err(PortError::new(
                PortErrorKind::Unsupported,
                "no link support",
            ));
        }
        spend(&mut st)?;
        if st.links.contains_key(link) {
            return Err(PortError::new(PortErrorKind::Io, "link exists"));
        }
        st.links.insert(link.to_owned(), target.to_owned());
        Ok(st.kind)
    }

    fn remove_link(&self, link: &Utf8Path) -> PortResult<()> {
        let mut st = lock(&self.state);
        st.ops.push(LinkOp::Remove {
            link: link.to_owned(),
        });
        spend(&mut st)?;
        match st.links.remove(link) {
            Some(_) => Ok(()),
            None => Err(PortError::new(PortErrorKind::Io, "not a link")),
        }
    }

    fn is_link(&self, path: &Utf8Path) -> bool {
        lock(&self.state).links.contains_key(path)
    }

    fn read_link(&self, path: &Utf8Path) -> Option<Utf8PathBuf> {
        lock(&self.state).links.get(path).cloned()
    }
}

// ---------------------------------------------------------------------------------------------
// Launcher
// ---------------------------------------------------------------------------------------------

#[derive(Debug)]
struct LaunchState {
    launched: Vec<LaunchSpec>,
    next_pid: u32,
    failure: Option<PortError>,
}

/// A launcher that records specs and starts nothing.
#[derive(Debug)]
pub struct FakeLauncher {
    state: Mutex<LaunchState>,
}

impl Default for FakeLauncher {
    fn default() -> Self {
        FakeLauncher::new()
    }
}

impl FakeLauncher {
    /// A launcher that succeeds, handing out pids from 1000 (0 for URL launches).
    pub fn new() -> Self {
        FakeLauncher {
            state: Mutex::new(LaunchState {
                launched: Vec::new(),
                next_pid: 1000,
                failure: None,
            }),
        }
    }

    /// Makes every later launch fail with `error`.
    pub fn fail_with(&self, error: PortError) {
        lock(&self.state).failure = Some(error);
    }

    /// Makes later launches succeed again.
    pub fn succeed(&self) {
        lock(&self.state).failure = None;
    }

    /// The specs launched so far, in order (failed attempts are not included).
    pub fn launched(&self) -> Vec<LaunchSpec> {
        lock(&self.state).launched.clone()
    }
}

impl Launcher for FakeLauncher {
    fn spawn(&self, spec: &LaunchSpec) -> PortResult<ChildHandle> {
        let mut st = lock(&self.state);
        if let Some(e) = &st.failure {
            return Err(e.clone());
        }
        st.launched.push(spec.clone());
        if spec.open_url.is_some() {
            return Ok(ChildHandle { pid: 0 });
        }
        let pid = st.next_pid;
        st.next_pid = st.next_pid.saturating_add(1);
        Ok(ChildHandle { pid })
    }
}

// ---------------------------------------------------------------------------------------------
// Credentials
// ---------------------------------------------------------------------------------------------

/// An in memory credential store, or one that always reports `Unsupported`.
#[derive(Debug, Default)]
pub struct FakeCredentialStore {
    unsupported: bool,
    secrets: Mutex<BTreeMap<(String, String), String>>,
}

impl FakeCredentialStore {
    /// A working empty store.
    pub fn new() -> Self {
        FakeCredentialStore::default()
    }

    /// A store whose every call fails with `Unsupported`, like the 0.1.0 platform store.
    pub fn unsupported() -> Self {
        FakeCredentialStore {
            unsupported: true,
            ..FakeCredentialStore::default()
        }
    }

    fn check(&self) -> PortResult<()> {
        if self.unsupported {
            Err(PortError::new(
                PortErrorKind::Unsupported,
                "no credential store",
            ))
        } else {
            Ok(())
        }
    }

    /// The number of stored secrets.
    pub fn len(&self) -> usize {
        lock(&self.secrets).len()
    }

    /// True when nothing is stored.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl CredentialStore for FakeCredentialStore {
    fn get(&self, service: &str, account: &str) -> PortResult<Option<String>> {
        self.check()?;
        Ok(lock(&self.secrets)
            .get(&(service.to_owned(), account.to_owned()))
            .cloned())
    }

    fn set(&self, service: &str, account: &str, secret: &str) -> PortResult<()> {
        self.check()?;
        lock(&self.secrets).insert((service.to_owned(), account.to_owned()), secret.to_owned());
        Ok(())
    }

    fn delete(&self, service: &str, account: &str) -> PortResult<()> {
        self.check()?;
        lock(&self.secrets).remove(&(service.to_owned(), account.to_owned()));
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// Sandbox and install source
// ---------------------------------------------------------------------------------------------

/// A sandbox probe returning fixed data.
#[derive(Debug, Clone, Default)]
pub struct FakeSandbox {
    info: SandboxInfo,
}

impl FakeSandbox {
    /// Not sandboxed.
    pub fn none() -> Self {
        FakeSandbox::default()
    }

    /// A Flatpak sandbox that hides host processes and grants the given folders.
    pub fn flatpak(granted: &[&str]) -> Self {
        FakeSandbox {
            info: SandboxInfo {
                kind: Some("flatpak".to_owned()),
                hides_processes: true,
                granted_paths: granted.iter().map(Utf8PathBuf::from).collect(),
            },
        }
    }

    /// A sandbox with exactly this information.
    pub fn with_info(info: SandboxInfo) -> Self {
        FakeSandbox { info }
    }
}

impl SandboxProbe for FakeSandbox {
    fn info(&self) -> SandboxInfo {
        self.info.clone()
    }
}

/// An install source probe returning a fixed value.
#[derive(Debug, Clone, Copy)]
pub struct FakeInstallSource(pub InstallSource);

impl Default for FakeInstallSource {
    fn default() -> Self {
        FakeInstallSource(InstallSource::Unknown)
    }
}

impl InstallSourceProbe for FakeInstallSource {
    fn detect(&self) -> InstallSource {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_moves_only_when_told_and_clones_share_time() {
        let c = FakeClock::new(10);
        let d = c.clone();
        assert_eq!(c.now_unix_ms(), 10);
        d.advance(5);
        assert_eq!(c.now_unix_ms(), 15);
        c.set(1);
        assert_eq!(d.now_unix_ms(), 1);
        c.advance(u64::MAX);
        assert_eq!(c.now_unix_ms(), u64::MAX);
        assert_eq!(
            FakeClock::default().now_unix_ms(),
            FakeClock::DEFAULT_START_MS
        );
    }

    #[test]
    fn env_returns_what_was_set() {
        let e = FakeEnv::new()
            .with_var("A", "1")
            .with_home("/home/rs_user")
            .with_exe_dir("/opt/app")
            .with_locale("de-DE");
        assert_eq!(e.var("A").as_deref(), Some("1"));
        assert_eq!(e.var("B"), None);
        assert_eq!(
            e.home_dir().as_deref(),
            Some(Utf8Path::new("/home/rs_user"))
        );
        assert_eq!(e.exe_dir().as_deref(), Some(Utf8Path::new("/opt/app")));
        assert_eq!(e.locale(), "de-DE");
        e.remove_var("A");
        e.set_home(None);
        assert_eq!(e.var("A"), None);
        assert_eq!(e.home_dir(), None);
        assert_eq!(FakeEnv::new().locale(), "en-US");
    }

    #[test]
    fn registry_names_are_case_insensitive() {
        let r = FakeRegistry::new().with(
            Hive::CurrentUser,
            "Software\\Valve\\Steam",
            "SteamPath",
            "c:/steam",
        );
        assert_eq!(
            r.read_string(Hive::CurrentUser, "software/valve/steam", "steampath")
                .as_deref(),
            Some("c:/steam")
        );
        assert_eq!(
            r.read_string(Hive::LocalMachine, "Software\\Valve\\Steam", "SteamPath"),
            None
        );
        r.remove(Hive::CurrentUser, "Software\\Valve\\Steam", "SteamPath");
        assert_eq!(
            r.read_string(Hive::CurrentUser, "Software\\Valve\\Steam", "SteamPath"),
            None
        );
    }

    #[test]
    fn process_probe_filters_by_name_and_sorts_by_pid() {
        let p = FakeProcessProbe::new();
        p.add(30, "RS_Game", Some("/g/RS_Game"));
        p.add(4, "rs_game", None);
        p.add(5, "other", None);
        let got = p.running(&["RS_GAME"]);
        assert_eq!(got.iter().map(|r| r.pid).collect::<Vec<_>>(), vec![4, 30]);
        p.remove(4);
        assert_eq!(p.running(&["rs_game"]).len(), 1);
        p.clear();
        assert!(p.running(&["rs_game"]).is_empty());
        assert_eq!(
            FakeProcessProbe::new().with(1, "x").running(&["x"]).len(),
            1
        );
    }

    #[test]
    fn link_backend_creates_reads_and_removes() {
        let b = FakeLinkBackend::new();
        assert!(b.support().symlink);
        let (t, l) = (Utf8Path::new("/t"), Utf8Path::new("/l"));
        assert_eq!(b.create_dir_link(t, l), Ok(LinkKind::Symlink));
        assert!(b.is_link(l));
        assert_eq!(b.read_link(l).as_deref(), Some(t));
        assert_eq!(b.create_dir_link(t, l).unwrap_err().kind, PortErrorKind::Io);
        assert_eq!(b.remove_link(l), Ok(()));
        assert!(b.remove_link(l).is_err());
        assert_eq!(b.ops().len(), 4);
        assert!(b.links().is_empty());
    }

    #[test]
    fn link_backend_variants_and_faults() {
        assert_eq!(
            FakeLinkBackend::unsupported()
                .create_dir_link(Utf8Path::new("/t"), Utf8Path::new("/l"))
                .unwrap_err()
                .kind,
            PortErrorKind::Unsupported
        );
        let w = FakeLinkBackend::windows_like();
        assert!(w.support().junction && w.support().needs_privilege);
        assert_eq!(
            w.create_dir_link(Utf8Path::new("/t"), Utf8Path::new("/l")),
            Ok(LinkKind::Junction)
        );
        let b = FakeLinkBackend::new();
        b.fail_after(1);
        assert!(
            b.create_dir_link(Utf8Path::new("/t"), Utf8Path::new("/a"))
                .is_ok()
        );
        assert!(
            b.create_dir_link(Utf8Path::new("/t"), Utf8Path::new("/b"))
                .is_err()
        );
        assert_eq!(b.links().len(), 1);
    }

    #[test]
    fn launcher_records_specs_and_hands_out_pids() {
        let l = FakeLauncher::new();
        let direct = LaunchSpec {
            program: "/g/game".into(),
            args: vec![],
            cwd: None,
            env: vec![],
            open_url: None,
        };
        let url = LaunchSpec {
            open_url: Some("steam://rungameid/294100".into()),
            ..direct.clone()
        };
        assert_eq!(l.spawn(&direct).unwrap().pid, 1000);
        assert_eq!(l.spawn(&direct).unwrap().pid, 1001);
        assert_eq!(l.spawn(&url).unwrap().pid, 0);
        l.fail_with(PortError::new(PortErrorKind::NotFound, "gone"));
        assert_eq!(l.spawn(&direct).unwrap_err().kind, PortErrorKind::NotFound);
        assert_eq!(l.launched().len(), 3);
        l.succeed();
        assert!(l.spawn(&direct).is_ok());
    }

    #[test]
    fn credential_store_round_trips_and_can_be_unsupported() {
        let s = FakeCredentialStore::new();
        assert_eq!(s.get("svc", "acc"), Ok(None));
        s.set("svc", "acc", "secret").unwrap();
        assert_eq!(s.get("svc", "acc").unwrap().as_deref(), Some("secret"));
        assert_eq!(s.len(), 1);
        s.delete("svc", "acc").unwrap();
        s.delete("svc", "acc").unwrap();
        assert!(s.is_empty());
        let u = FakeCredentialStore::unsupported();
        assert_eq!(
            u.get("a", "b").unwrap_err().kind,
            PortErrorKind::Unsupported
        );
        assert!(u.set("a", "b", "c").is_err());
        assert!(u.delete("a", "b").is_err());
    }

    #[test]
    fn sandbox_and_install_source_return_fixed_values() {
        assert!(!FakeSandbox::none().info().is_sandboxed());
        let f = FakeSandbox::flatpak(&["/mnt"]).info();
        assert!(f.is_sandboxed() && f.hides_processes);
        assert_eq!(f.granted_paths, vec![Utf8PathBuf::from("/mnt")]);
        assert_eq!(FakeSandbox::with_info(f.clone()).info(), f);
        assert_eq!(
            FakeInstallSource::default().detect(),
            InstallSource::Unknown
        );
        assert_eq!(
            FakeInstallSource(InstallSource::Flatpak).detect(),
            InstallSource::Flatpak
        );
    }
}
