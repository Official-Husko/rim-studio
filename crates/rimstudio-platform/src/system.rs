//! [`SystemPlatform`]: the real implementation of the operating system ports.

use std::time::{SystemTime, UNIX_EPOCH};

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::os::Os;
use rimstudio_core::ports::{
    ChildHandle, Clock, CredentialStore, EnvProbe, Hive, InstallSource, InstallSourceProbe,
    LaunchSpec, Launcher, LinkBackend, LinkKind, LinkSupport, PlatformDirs, PortError,
    PortErrorKind, PortResult, ProcessProbe, RegistryProbe, RunningProcess, SandboxInfo,
    SandboxProbe,
};

use crate::dirs::{PORTABLE_MARKER, platform_dirs, portable_dirs};
use crate::error::io_error;
use crate::generic::{NoCredentialStore, no_link_support};
use crate::install::{decide_install_source, inputs_from_env};
use crate::sandbox::{FLATPAK_INFO_PATH, detect_sandbox};
use crate::{launch, process, unix, windows};

/// Implements `Clock`, `EnvProbe`, `RegistryProbe`, `ProcessProbe`, `LinkBackend`, `Launcher`,
/// `CredentialStore`, `SandboxProbe` and `InstallSourceProbe` for the running operating system.
#[derive(Debug, Clone, Copy)]
pub struct SystemPlatform {
    os: Os,
}

impl Default for SystemPlatform {
    fn default() -> Self {
        SystemPlatform::new()
    }
}

impl SystemPlatform {
    /// A platform for the operating system this binary runs on.
    pub fn new() -> SystemPlatform {
        SystemPlatform { os: Os::current() }
    }

    /// The operating system this platform represents.
    pub fn os(&self) -> Os {
        self.os
    }

    /// The standard app folders. In portable mode (the marker file beside the executable exists)
    /// everything is under `<exe folder>/data`. `None` when no base folder can be determined.
    pub fn platform_dirs(&self) -> Option<PlatformDirs> {
        if let Some(exe) = self.exe_dir()
            && exe.join(PORTABLE_MARKER).is_file()
        {
            return Some(portable_dirs(&exe));
        }
        platform_dirs(self.os, self)
    }

    fn home_from_env(&self) -> Option<String> {
        let key = if self.os == Os::Windows {
            "USERPROFILE"
        } else {
            "HOME"
        };
        self.var(key).filter(|v| !v.trim().is_empty())
    }

    fn symlink_support(&self) -> LinkSupport {
        match self.os {
            Os::Windows => windows::link_support(),
            Os::Linux | Os::MacOs => unix::link_support(),
            Os::Other => {
                if cfg!(unix) {
                    unix::link_support()
                } else {
                    no_link_support()
                }
            }
        }
    }
}

impl Clock for SystemPlatform {
    fn now_unix_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
            .unwrap_or(0)
    }
}

impl EnvProbe for SystemPlatform {
    fn var(&self, key: &str) -> Option<String> {
        if key.is_empty() || key.contains(['=', '\0']) {
            return None;
        }
        std::env::var(key).ok()
    }

    fn home_dir(&self) -> Option<Utf8PathBuf> {
        self.home_from_env().map(Utf8PathBuf::from)
    }

    fn exe_dir(&self) -> Option<Utf8PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let dir = exe.parent()?.to_path_buf();
        Utf8PathBuf::from_path_buf(dir).ok()
    }

    fn locale(&self) -> String {
        sys_locale::get_locale().unwrap_or_else(|| "en-US".to_owned())
    }
}

impl RegistryProbe for SystemPlatform {
    fn read_string(&self, hive: Hive, key: &str, value: &str) -> Option<String> {
        windows::read_registry_string(hive, key, value)
    }
}

impl ProcessProbe for SystemPlatform {
    fn running(&self, names: &[&str]) -> Vec<RunningProcess> {
        process::running_processes(self.os, names)
    }
}

impl LinkBackend for SystemPlatform {
    fn support(&self) -> LinkSupport {
        self.symlink_support()
    }

    fn create_dir_link(&self, target: &Utf8Path, link: &Utf8Path) -> PortResult<LinkKind> {
        if !self.symlink_support().symlink {
            return Err(PortError::new(
                PortErrorKind::Unsupported,
                "no link support",
            ));
        }
        if std::fs::symlink_metadata(link).is_ok() {
            return Err(PortError::new(
                PortErrorKind::Io,
                format!("link path already exists: {link}"),
            ));
        }
        let result = if self.os == Os::Windows {
            windows::create_dir_symlink(target, link)
        } else {
            unix::create_dir_symlink(target, link)
        };
        result.map_err(|e| io_error("create link", &e))?;
        Ok(LinkKind::Symlink)
    }

    fn remove_link(&self, link: &Utf8Path) -> PortResult<()> {
        if !self.is_link(link) {
            return Err(PortError::new(
                PortErrorKind::Io,
                format!("not a link: {link}"),
            ));
        }
        // Removes the link entry only. A symlink to a directory is removed as a file on Unix and
        // as an empty directory entry on Windows; the target's content is never touched.
        use std::fs::{remove_dir, remove_file};
        match remove_file(link) {
            Ok(()) => Ok(()),
            Err(first) => remove_dir(link).map_err(|_| io_error("remove link", &first)),
        }
    }

    fn is_link(&self, path: &Utf8Path) -> bool {
        std::fs::symlink_metadata(path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    }

    fn read_link(&self, path: &Utf8Path) -> Option<Utf8PathBuf> {
        if !self.is_link(path) {
            return None;
        }
        let target = std::fs::read_link(path).ok()?;
        Utf8PathBuf::from_path_buf(target).ok()
    }
}

impl Launcher for SystemPlatform {
    fn spawn(&self, spec: &LaunchSpec) -> PortResult<ChildHandle> {
        launch::spawn_spec(self.os, spec)
    }
}

impl CredentialStore for SystemPlatform {
    fn get(&self, service: &str, account: &str) -> PortResult<Option<String>> {
        NoCredentialStore.get(service, account)
    }

    fn set(&self, service: &str, account: &str, secret: &str) -> PortResult<()> {
        NoCredentialStore.set(service, account, secret)
    }

    fn delete(&self, service: &str, account: &str) -> PortResult<()> {
        NoCredentialStore.delete(service, account)
    }
}

impl SandboxProbe for SystemPlatform {
    fn info(&self) -> SandboxInfo {
        let info_text = if self.os == Os::Linux {
            std::fs::read_to_string(FLATPAK_INFO_PATH).ok()
        } else {
            None
        };
        detect_sandbox(self, info_text.as_deref())
    }
}

impl InstallSourceProbe for SystemPlatform {
    fn detect(&self) -> InstallSource {
        let marker = self
            .exe_dir()
            .is_some_and(|d| d.join(PORTABLE_MARKER).is_file());
        decide_install_source(&inputs_from_env(self.os, self, marker))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn tmp() -> (tempfile::TempDir, Utf8PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let p = Utf8PathBuf::from_path_buf(t.path().to_path_buf()).unwrap();
        (t, p)
    }

    #[test]
    fn clock_is_after_2020_and_monotonic_enough() {
        let p = SystemPlatform::new();
        let a = p.now_unix_ms();
        assert!(a > 1_577_836_800_000);
        assert!(p.now_unix_ms() >= a);
    }

    #[test]
    fn env_reads_process_variables_and_rejects_bad_names() {
        let p = SystemPlatform::new();
        assert!(p.var("").is_none());
        assert!(p.var("A=B").is_none());
        assert!(p.var("RIMSTUDIO_SURELY_UNSET_VARIABLE").is_none());
        assert!(p.exe_dir().is_some());
        assert!(!p.locale().is_empty());
    }

    #[test]
    fn credential_store_is_unsupported() {
        let p = SystemPlatform::new();
        assert_eq!(
            p.get("s", "a").unwrap_err().kind,
            PortErrorKind::Unsupported
        );
        assert_eq!(
            p.set("s", "a", "x").unwrap_err().kind,
            PortErrorKind::Unsupported
        );
        assert_eq!(
            p.delete("s", "a").unwrap_err().kind,
            PortErrorKind::Unsupported
        );
    }

    #[test]
    fn install_source_and_sandbox_do_not_panic() {
        let p = SystemPlatform::new();
        let _ = p.detect();
        let _ = p.info();
        let _ = p.platform_dirs();
    }

    #[cfg(not(windows))]
    #[test]
    fn registry_returns_none_off_windows() {
        assert!(
            SystemPlatform::new()
                .read_string(Hive::LocalMachine, "SOFTWARE", "x")
                .is_none()
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_create_read_and_remove_keep_the_target() {
        let (_t, dir) = tmp();
        let target = dir.join("target");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("f.txt"), "x").unwrap();
        let link = dir.join("link");
        let p = SystemPlatform::new();
        assert!(p.support().symlink);
        assert_eq!(
            p.create_dir_link(&target, &link).unwrap(),
            LinkKind::Symlink
        );
        assert!(p.is_link(&link));
        assert!(!p.is_link(&target));
        assert_eq!(p.read_link(&link), Some(target.clone()));
        assert_eq!(std::fs::read_to_string(link.join("f.txt")).unwrap(), "x");
        p.remove_link(&link).unwrap();
        assert!(!p.is_link(&link));
        assert!(std::fs::symlink_metadata(&link).is_err());
        assert_eq!(std::fs::read_to_string(target.join("f.txt")).unwrap(), "x");
    }

    #[cfg(unix)]
    #[test]
    fn creating_over_an_existing_path_fails() {
        let (_t, dir) = tmp();
        let target = dir.join("t");
        std::fs::create_dir(&target).unwrap();
        let p = SystemPlatform::new();
        let e = p.create_dir_link(&target, &target).unwrap_err();
        assert_eq!(e.kind, PortErrorKind::Io);
    }

    #[cfg(unix)]
    #[test]
    fn dangling_links_are_links_and_can_be_removed() {
        let (_t, dir) = tmp();
        let link = dir.join("dangling");
        let p = SystemPlatform::new();
        p.create_dir_link(&dir.join("missing"), &link).unwrap();
        assert!(p.is_link(&link));
        assert_eq!(p.read_link(&link), Some(dir.join("missing")));
        p.remove_link(&link).unwrap();
        assert!(!p.is_link(&link));
    }

    #[cfg(unix)]
    #[test]
    fn self_loop_link_is_handled_without_following() {
        let (_t, dir) = tmp();
        let link = dir.join("loop");
        let p = SystemPlatform::new();
        p.create_dir_link(&link, &link).unwrap();
        assert!(p.is_link(&link));
        assert_eq!(p.read_link(&link), Some(link.clone()));
        p.remove_link(&link).unwrap();
        assert!(!p.is_link(&link));
    }

    #[cfg(unix)]
    #[test]
    fn removing_a_real_directory_or_missing_path_is_refused() {
        let (_t, dir) = tmp();
        let real = dir.join("real");
        std::fs::create_dir(&real).unwrap();
        let p = SystemPlatform::new();
        assert!(p.remove_link(&real).is_err());
        assert!(real.is_dir());
        assert!(p.remove_link(&dir.join("nothing")).is_err());
        assert_eq!(p.read_link(&real), None);
    }

    #[cfg(unix)]
    #[test]
    fn process_probe_finds_a_spawned_child() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let pid = child.id();
        let p = SystemPlatform::new();
        let mut found = false;
        for _ in 0..100 {
            if p.running(&["sleep"]).iter().any(|r| r.pid == pid) {
                found = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let _ = child.kill();
        let _ = child.wait();
        assert!(found, "sleep child {pid} was not listed");
        assert!(
            !p.running(&["rs_no_such_process_name"])
                .iter()
                .any(|r| r.pid == pid)
        );
        assert!(p.running(&[]).is_empty());
    }
}
