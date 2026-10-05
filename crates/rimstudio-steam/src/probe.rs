//! Directory probes: the candidate folders per operating system and a thin deadline aware wrapper
//! over the file system port.
//!
//! The candidate functions are pure: they read the environment and the registry through
//! [`DetectEnv`] and return paths. Nothing here touches the real machine on its own.
//!
//! Paths are normalised to `/` separators on every OS (see [`norm_path`]) so reports are identical
//! on every host; the Windows APIs accept `/`.

use std::cell::RefCell;
use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::os::Os;
use rimstudio_core::ports::{DetectEnv, DirEntryInfo, EntryKind, Hive, PortError, PortErrorKind};

use crate::report::{How, UserDirKind};

/// The folder holding the game's user data, below the Unity parent folder.
pub const USER_DIR_SUFFIX: &str = "Ludeon Studios/RimWorld by Ludeon Studios";

/// The Flatpak application id of Steam.
pub const FLATPAK_STEAM_ID: &str = "com.valvesoftware.Steam";

/// A Steam root candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootCandidate {
    /// The candidate folder (normalised).
    pub path: Utf8PathBuf,
    /// Why it is a candidate.
    pub how: How,
}

/// A user data folder candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserDirCandidate {
    /// The candidate folder.
    pub path: Utf8PathBuf,
    /// Its kind.
    pub kind: UserDirKind,
}

/// Normalises a path text: on Windows backslashes become `/` and a `\\?\` prefix is dropped; on every
/// OS trailing separators are removed (a root stays a root) and the text is trimmed.
pub fn norm_path(os: Os, text: &str) -> Utf8PathBuf {
    let mut t = text.trim().to_owned();
    if os == Os::Windows {
        t = t.replace('\\', "/");
        if let Some(rest) = t.strip_prefix("//?/") {
            t = match rest.strip_prefix("UNC/") {
                Some(unc) => format!("//{unc}"),
                None => rest.to_owned(),
            };
        }
    }
    while t.len() > 1 && t.ends_with('/') && !t.ends_with(":/") {
        t.pop();
    }
    Utf8PathBuf::from(t)
}

/// Joins a relative text to a base with exactly one `/`.
pub fn join(base: &Utf8Path, rel: &str) -> Utf8PathBuf {
    let b = base.as_str();
    if b.ends_with('/') {
        Utf8PathBuf::from(format!("{b}{rel}"))
    } else {
        Utf8PathBuf::from(format!("{b}/{rel}"))
    }
}

/// The key two paths are compared by: case folded on Windows and macOS.
pub fn path_key(os: Os, path: &Utf8Path) -> String {
    match os {
        Os::Windows | Os::MacOs => path.as_str().to_lowercase(),
        _ => path.as_str().to_owned(),
    }
}

/// True when the OS compares paths case insensitively by default.
pub fn folds_case(os: Os) -> bool {
    matches!(os, Os::Windows | Os::MacOs)
}

fn home(env: &dyn DetectEnv) -> Option<Utf8PathBuf> {
    env.env().home_dir()
}

/// The Steam root candidates of an OS in the documented order, without the user override.
pub fn steam_root_candidates(os: Os, env: &dyn DetectEnv) -> Vec<RootCandidate> {
    let mut out = Vec::new();
    let mut push = |text: &str, how: How| {
        if !text.trim().is_empty() {
            out.push(RootCandidate {
                path: norm_path(os, text),
                how,
            });
        }
    };
    match os {
        Os::Windows => {
            let reg = env.registry();
            if let Some(v) =
                reg.read_string(Hive::CurrentUser, "Software\\Valve\\Steam", "SteamPath")
            {
                push(&v, How::RegistryHkcu);
            }
            if let Some(v) = reg.read_string(
                Hive::LocalMachine,
                "SOFTWARE\\WOW6432Node\\Valve\\Steam",
                "InstallPath",
            ) {
                push(&v, How::RegistryHklm);
            }
            if let Some(v) =
                reg.read_string(Hive::LocalMachine, "SOFTWARE\\Valve\\Steam", "InstallPath")
            {
                push(&v, How::RegistryHklm);
            }
            push("C:/Program Files (x86)/Steam", How::DirectoryProbe);
            push("C:/Program Files/Steam", How::DirectoryProbe);
        }
        Os::MacOs => {
            if let Some(h) = home(env) {
                push(
                    &format!("{h}/Library/Application Support/Steam"),
                    How::DirectoryProbe,
                );
            }
        }
        Os::Linux | Os::Other => {
            let home = home(env);
            if let Some(xdg) = env
                .env()
                .var("XDG_DATA_HOME")
                .filter(|v| v.starts_with('/'))
            {
                push(&format!("{xdg}/Steam"), How::XdgDataHome);
            }
            if let Some(h) = &home {
                push(&format!("{h}/.local/share/Steam"), How::XdgDataHome);
                push(&format!("{h}/.steam/steam"), How::SymlinkSteam);
                push(&format!("{h}/.steam/root"), How::SymlinkSteam);
                push(
                    &format!("{h}/.steam/debian-installation"),
                    How::DirectoryProbe,
                );
                let app = format!("{h}/.var/app/{FLATPAK_STEAM_ID}");
                push(&format!("{app}/.local/share/Steam"), How::Flatpak);
                push(&format!("{app}/.steam/steam"), How::Flatpak);
                push(&format!("{app}/.steam/root"), How::Flatpak);
                push(
                    &format!("{h}/snap/steam/common/.local/share/Steam"),
                    How::Snap,
                );
                push(&format!("{h}/snap/steam/common/.steam/steam"), How::Snap);
            }
            if let Some(snap) = env
                .env()
                .var("SNAP_USER_DATA")
                .filter(|v| v.starts_with('/'))
            {
                push(&format!("{snap}/.local/share/Steam"), How::Snap);
            }
        }
    }
    out
}

/// The user data folder candidates that do not depend on a Steam library, in probe order.
///
/// Proton prefixes depend on the library that holds the game; see [`proton_user_dir`].
pub fn user_dir_candidates(os: Os, env: &dyn DetectEnv) -> Vec<UserDirCandidate> {
    let mut out = Vec::new();
    let Some(h) = home(env) else { return out };
    let mut push = |path: String, kind| {
        out.push(UserDirCandidate {
            path: norm_path(os, &path),
            kind,
        });
    };
    match os {
        Os::Windows => {
            let base = env
                .env()
                .var("USERPROFILE")
                .map_or_else(|| h.to_string(), |v| v.replace('\\', "/"));
            push(
                format!("{base}/AppData/LocalLow/{USER_DIR_SUFFIX}"),
                UserDirKind::Windows,
            );
        }
        Os::MacOs => push(
            format!("{h}/Library/Application Support/RimWorld"),
            UserDirKind::Macos,
        ),
        Os::Linux | Os::Other => {
            if let Some(xdg) = env
                .env()
                .var("XDG_CONFIG_HOME")
                .filter(|v| v.starts_with('/'))
            {
                push(
                    format!("{xdg}/unity3d/{USER_DIR_SUFFIX}"),
                    UserDirKind::Native,
                );
            }
            push(
                format!("{h}/.config/unity3d/{USER_DIR_SUFFIX}"),
                UserDirKind::Native,
            );
            let app = format!("{h}/.var/app/{FLATPAK_STEAM_ID}");
            push(
                format!("{app}/.config/unity3d/{USER_DIR_SUFFIX}"),
                UserDirKind::Flatpak,
            );
            push(
                format!("{app}/config/unity3d/{USER_DIR_SUFFIX}"),
                UserDirKind::Flatpak,
            );
            push(
                format!("{h}/snap/steam/common/.config/unity3d/{USER_DIR_SUFFIX}"),
                UserDirKind::Snap,
            );
        }
    }
    out
}

/// The user data folder inside the Proton prefix of the game in a library.
pub fn proton_user_dir(library: &Utf8Path, app_id: u64) -> Utf8PathBuf {
    join(
        library,
        &format!(
            "steamapps/compatdata/{app_id}/pfx/drive_c/users/steamuser/AppData/LocalLow/{USER_DIR_SUFFIX}"
        ),
    )
}

/// The log files a user folder may hold, in probe order (macOS keeps its log elsewhere).
pub fn player_log_candidates(
    os: Os,
    user_dir: &Utf8Path,
    home: Option<&Utf8Path>,
) -> Vec<Utf8PathBuf> {
    let mut out = vec![join(user_dir, "Player.log")];
    if os == Os::MacOs
        && let Some(h) = home
    {
        out.push(join(
            h,
            &format!("Library/Logs/{USER_DIR_SUFFIX}/Player.log"),
        ));
        out.push(join(h, "Library/Logs/Unity/Player.log"));
    }
    out
}

/// Folders where a non Steam copy of the game may live, probed by content only.
pub fn non_steam_install_candidates(os: Os, env: &dyn DetectEnv) -> Vec<Utf8PathBuf> {
    let mut out: Vec<String> = Vec::new();
    let home = home(env);
    match os {
        Os::Windows => {
            out.push("C:/GOG Games/RimWorld".into());
            out.push("C:/Program Files (x86)/GOG Galaxy/Games/RimWorld".into());
            if let Some(h) = &home {
                out.push(format!("{h}/GOG Games/RimWorld"));
                out.push(format!("{h}/Games/RimWorld"));
            }
        }
        Os::MacOs => {
            out.push("/Applications/RimWorld".into());
            if let Some(h) = &home {
                out.push(format!("{h}/Applications/RimWorld"));
            }
        }
        Os::Linux | Os::Other => {
            if let Some(h) = &home {
                out.push(format!("{h}/GOG Games/RimWorld/game"));
                out.push(format!("{h}/GOG Games/RimWorld"));
                out.push(format!("{h}/Games/RimWorld/game"));
                out.push(format!("{h}/Games/RimWorld"));
                out.push(format!("{h}/Games/Heroic/RimWorld"));
            }
        }
    }
    out.iter().map(|p| norm_path(os, p)).collect()
}

/// The state of a folder after a probe with a deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirState {
    /// The folder exists.
    Present,
    /// The folder does not exist (or is not a folder).
    Missing,
    /// The probe did not finish in time.
    TimedOut,
}

/// A file system wrapper that applies the deadline to every call and remembers timeouts.
pub struct Prober<'a> {
    env: &'a dyn DetectEnv,
    deadline: Duration,
    timeouts: RefCell<Vec<Utf8PathBuf>>,
}

impl<'a> Prober<'a> {
    /// Wraps an environment.
    pub fn new(env: &'a dyn DetectEnv, deadline: Duration) -> Self {
        Prober {
            env,
            deadline,
            timeouts: RefCell::new(Vec::new()),
        }
    }

    /// The wrapped environment.
    pub fn env(&self) -> &'a dyn DetectEnv {
        self.env
    }

    /// The deadline applied to each call.
    pub fn deadline(&self) -> Duration {
        self.deadline
    }

    fn note<T>(&self, path: &Utf8Path, result: Result<T, PortError>) -> Result<T, PortError> {
        if let Err(e) = &result
            && e.kind == PortErrorKind::TimedOut
        {
            let mut t = self.timeouts.borrow_mut();
            if !t.iter().any(|p| p == path) {
                t.push(path.to_path_buf());
            }
        }
        result
    }

    /// The paths that timed out so far, in the order they were noticed.
    pub fn timeouts(&self) -> Vec<Utf8PathBuf> {
        self.timeouts.borrow().clone()
    }

    /// Forgets a recorded timeout (used when the caller reports it in a more specific way).
    pub fn forget_timeout(&self, path: &Utf8Path) {
        self.timeouts.borrow_mut().retain(|p| p != path);
    }

    /// True when the path exists.
    pub fn exists(&self, path: &Utf8Path) -> bool {
        self.note(path, self.env.fs().exists(path, self.deadline))
            .unwrap_or(false)
    }

    /// True when the path is a directory.
    pub fn is_dir(&self, path: &Utf8Path) -> bool {
        self.note(path, self.env.fs().is_dir(path, self.deadline))
            .unwrap_or(false)
    }

    /// Present, missing or timed out.
    pub fn dir_state(&self, path: &Utf8Path) -> DirState {
        match self.env.fs().is_dir(path, self.deadline) {
            Ok(true) => DirState::Present,
            Ok(false) => DirState::Missing,
            Err(e) if e.kind == PortErrorKind::TimedOut => DirState::TimedOut,
            Err(_) => DirState::Missing,
        }
    }

    /// The kind of the entry itself, links not followed; `None` when it does not exist.
    pub fn entry_kind(&self, path: &Utf8Path) -> Option<EntryKind> {
        self.note(path, self.env.fs().metadata(path, self.deadline))
            .ok()
            .map(|m| m.kind)
    }

    /// The modification time in milliseconds since the epoch.
    pub fn mtime_ms(&self, path: &Utf8Path) -> Option<i64> {
        let meta = self
            .note(path, self.env.fs().metadata(path, self.deadline))
            .ok()?;
        let ns = meta.mtime_ns?;
        i64::try_from(ns / 1_000_000).ok()
    }

    /// The text of a file.
    ///
    /// # Errors
    /// The port error, for example `port.not-found`.
    pub fn read_text(&self, path: &Utf8Path) -> Result<String, PortError> {
        self.note(path, self.env.fs().read_to_string(path, self.deadline))
    }

    /// The sorted entries of a directory; `None` when it cannot be listed.
    pub fn read_dir(&self, path: &Utf8Path) -> Option<Vec<DirEntryInfo>> {
        self.note(path, self.env.fs().read_dir(path, self.deadline))
            .ok()
    }

    /// The canonical path with links resolved, normalised for `os`; `None` when it cannot be resolved.
    pub fn canonical(&self, os: Os, path: &Utf8Path) -> Option<Utf8PathBuf> {
        self.note(path, self.env.fs().canonicalize(path, self.deadline))
            .ok()
            .map(|p| norm_path(os, p.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_testing::detect_env::{FakeDetectEnv, SteamPreset};
    use rstest::rstest;

    #[rstest]
    #[case(
        Os::Windows,
        "C:\\Program Files (x86)\\Steam\\",
        "C:/Program Files (x86)/Steam"
    )]
    #[case(
        Os::Windows,
        "c:/program files (x86)/steam",
        "c:/program files (x86)/steam"
    )]
    #[case(Os::Windows, "\\\\?\\D:\\Lib", "D:/Lib")]
    #[case(Os::Windows, "\\\\?\\UNC\\srv\\share", "//srv/share")]
    #[case(Os::Windows, "D:\\", "D:/")]
    #[case(Os::Linux, "/a/b//", "/a/b")]
    #[case(Os::Linux, "/", "/")]
    #[case(Os::Linux, "  /a  ", "/a")]
    fn paths_are_normalised(#[case] os: Os, #[case] input: &str, #[case] want: &str) {
        assert_eq!(norm_path(os, input), want);
    }

    #[test]
    fn join_uses_one_separator() {
        assert_eq!(join(Utf8Path::new("/a"), "b"), "/a/b");
        assert_eq!(join(Utf8Path::new("/"), "b"), "/b");
    }

    #[test]
    fn path_keys_fold_case_only_where_the_os_does() {
        let p = Utf8Path::new("/A/b");
        assert_eq!(path_key(Os::Linux, p), "/A/b");
        assert_eq!(path_key(Os::Windows, p), "/a/b");
        assert_eq!(path_key(Os::MacOs, p), "/a/b");
    }

    #[test]
    fn linux_root_candidates_come_in_the_documented_order() {
        let env = FakeDetectEnv::from_preset(SteamPreset::LinuxNative)
            .with_var("XDG_DATA_HOME", "/rs/xdg");
        let hows: Vec<(String, &str)> = steam_root_candidates(Os::Linux, &env)
            .iter()
            .map(|c| (c.path.to_string(), c.how.as_str()))
            .collect();
        let h = "/home/rs_user";
        assert_eq!(hows[0], ("/rs/xdg/Steam".to_owned(), "xdg-data-home"));
        assert_eq!(
            hows[1],
            (format!("{h}/.local/share/Steam"), "xdg-data-home")
        );
        assert_eq!(hows[2], (format!("{h}/.steam/steam"), "symlink-steam"));
        assert_eq!(hows[3], (format!("{h}/.steam/root"), "symlink-steam"));
        assert_eq!(
            hows[4],
            (format!("{h}/.steam/debian-installation"), "directory-probe")
        );
        assert!(hows[5].0.contains(".var/app/com.valvesoftware.Steam") && hows[5].1 == "flatpak");
        assert!(
            hows.iter()
                .any(|(p, how)| p.contains("snap/steam") && *how == "snap")
        );
    }

    #[test]
    fn a_relative_xdg_value_is_ignored() {
        let env = FakeDetectEnv::from_preset(SteamPreset::LinuxNative)
            .with_var("XDG_DATA_HOME", "relative");
        assert!(
            steam_root_candidates(Os::Linux, &env)
                .iter()
                .all(|c| !c.path.as_str().starts_with("relative"))
        );
    }

    #[test]
    fn windows_candidates_follow_registry_order_then_defaults() {
        let env = FakeDetectEnv::from_preset(SteamPreset::WindowsRegistry);
        let c = steam_root_candidates(Os::Windows, &env);
        let hows: Vec<&str> = c.iter().map(|c| c.how.as_str()).collect();
        assert_eq!(
            hows,
            [
                "registry-hkcu",
                "registry-hklm",
                "directory-probe",
                "directory-probe"
            ]
        );
        assert_eq!(c[0].path, "c:/program files (x86)/steam");
        assert_eq!(c[1].path, "C:/Program Files (x86)/Steam");
        let only_hklm = steam_root_candidates(
            Os::Windows,
            &FakeDetectEnv::from_preset(SteamPreset::WindowsHklmOnly),
        );
        assert_eq!(only_hklm[0].how, How::RegistryHklm);
        let none = steam_root_candidates(
            Os::Windows,
            &FakeDetectEnv::from_preset(SteamPreset::WindowsDefault),
        );
        assert!(none.iter().all(|c| c.how == How::DirectoryProbe));
    }

    #[test]
    fn macos_has_one_root_candidate() {
        let env = FakeDetectEnv::from_preset(SteamPreset::MacOs);
        let c = steam_root_candidates(Os::MacOs, &env);
        assert_eq!(c.len(), 1);
        assert_eq!(
            c[0].path,
            "/Users/rs_user/Library/Application Support/Steam"
        );
    }

    #[test]
    fn user_dir_candidates_per_os() {
        let linux = user_dir_candidates(
            Os::Linux,
            &FakeDetectEnv::from_preset(SteamPreset::LinuxNative),
        );
        let kinds: Vec<UserDirKind> = linux.iter().map(|c| c.kind).collect();
        assert_eq!(
            kinds,
            [
                UserDirKind::Native,
                UserDirKind::Flatpak,
                UserDirKind::Flatpak,
                UserDirKind::Snap
            ]
        );
        let win = user_dir_candidates(
            Os::Windows,
            &FakeDetectEnv::from_preset(SteamPreset::WindowsRegistry),
        );
        assert_eq!(
            win[0].path,
            "C:/Users/rs_user/AppData/LocalLow/Ludeon Studios/RimWorld by Ludeon Studios"
        );
        let mac = user_dir_candidates(Os::MacOs, &FakeDetectEnv::from_preset(SteamPreset::MacOs));
        assert_eq!(
            mac[0].path,
            "/Users/rs_user/Library/Application Support/RimWorld"
        );
    }

    #[test]
    fn proton_user_dir_is_inside_the_prefix() {
        let p = proton_user_dir(Utf8Path::new("/lib"), 294_100);
        assert!(
            p.as_str()
                .starts_with("/lib/steamapps/compatdata/294100/pfx/drive_c/users/steamuser")
        );
        assert!(p.as_str().ends_with("RimWorld by Ludeon Studios"));
    }

    #[test]
    fn the_prober_records_timeouts_once() {
        let env = FakeDetectEnv::from_preset(SteamPreset::LinuxNative);
        env.fs.hang("/home/rs_user/.local/share/Steam/steamapps");
        let p = Prober::new(&env, Duration::from_millis(5));
        let dir = Utf8Path::new("/home/rs_user/.local/share/Steam/steamapps");
        assert_eq!(p.dir_state(dir), DirState::TimedOut);
        assert!(!p.exists(dir));
        assert!(!p.exists(dir));
        assert_eq!(p.timeouts(), vec![dir.to_path_buf()]);
        p.forget_timeout(dir);
        assert!(p.timeouts().is_empty());
        assert_eq!(p.dir_state(Utf8Path::new("/nowhere")), DirState::Missing);
    }
}
