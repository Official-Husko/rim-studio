//! Resolution of the standard app folders from an injected environment.
//!
//! Everything here is a pure function of an [`Os`] and an [`EnvProbe`], so tests cover all three
//! operating systems on any host. Paths are assembled as text with the separator of the target OS,
//! never through the host's `Path` rules.

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::os::Os;
use rimstudio_core::ports::{EnvProbe, PlatformDirs};

/// Name of the marker file beside the executable that selects portable mode.
pub const PORTABLE_MARKER: &str = "rimstudio.portable";

/// The folder name used under the standard roots on Linux.
pub const LINUX_APP_DIR: &str = "rimstudio";

/// The folder name used under the standard roots on Windows and macOS.
pub const APP_DIR: &str = "RimStudio";

fn sep(os: Os) -> char {
    if os == Os::Windows { '\\' } else { '/' }
}

fn join(os: Os, base: &str, segments: &[&str]) -> Utf8PathBuf {
    let s = sep(os);
    let mut out = base.trim_end_matches(['/', '\\']).to_owned();
    if out.is_empty() && !base.is_empty() {
        // The base was only separators, for example the root "/".
        out.push(s);
        out.pop();
    }
    for seg in segments {
        out.push(s);
        out.push_str(seg);
    }
    Utf8PathBuf::from(out)
}

fn is_absolute(os: Os, text: &str) -> bool {
    match os {
        Os::Windows => {
            let b = text.as_bytes();
            let drive = b.len() >= 3
                && b.first().is_some_and(u8::is_ascii_alphabetic)
                && b.get(1) == Some(&b':')
                && matches!(b.get(2), Some(b'\\' | b'/'));
            drive || text.starts_with("\\\\")
        }
        _ => text.starts_with('/'),
    }
}

/// A variable that is set, non empty and an absolute path for `os`.
fn abs_var(os: Os, env: &dyn EnvProbe, key: &str) -> Option<String> {
    env.var(key)
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty() && is_absolute(os, v))
}

/// The standard folders of the app for `os`, or `None` when no usable base folder is known (no
/// home folder and none of the variables set).
///
/// - Linux: `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME`, `XDG_STATE_HOME` (absolute values
///   only, as the XDG specification says) with fallbacks `~/.config`, `~/.local/share`, `~/.cache`,
///   `~/.local/state`. Inside a Flatpak (`FLATPAK_ID` set) an unset variable falls back to the
///   per application folder `~/.var/app/<id>/...`. Logs live under the state folder.
/// - macOS: `~/Library/Application Support`, `~/Library/Caches` and `~/Library/Logs`.
/// - Windows: `%APPDATA%` for configuration, `%LOCALAPPDATA%` for the rest (with `%USERPROFILE%`
///   fallbacks).
pub fn platform_dirs(os: Os, env: &dyn EnvProbe) -> Option<PlatformDirs> {
    match os {
        Os::Windows => windows_dirs(env),
        Os::MacOs => macos_dirs(env),
        Os::Linux | Os::Other => linux_dirs(env),
    }
}

fn home_string(os: Os, env: &dyn EnvProbe) -> Option<String> {
    env.home_dir()
        .map(|h| h.into_string())
        .filter(|h| !h.is_empty() && is_absolute(os, h))
}

fn linux_dirs(env: &dyn EnvProbe) -> Option<PlatformDirs> {
    let os = Os::Linux;
    let home = home_string(os, env);
    let flatpak = env
        .var("FLATPAK_ID")
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty() && !v.contains(['/', '\\']));
    let pick = |var: &str, plain: &[&str], flat: &[&str]| -> Option<Utf8PathBuf> {
        if let Some(v) = abs_var(os, env, var) {
            return Some(join(os, &v, &[LINUX_APP_DIR]));
        }
        let h = home.as_deref()?;
        match &flatpak {
            Some(id) => {
                let mut segs = vec![".var", "app", id.as_str()];
                segs.extend_from_slice(flat);
                segs.push(LINUX_APP_DIR);
                Some(join(os, h, &segs))
            }
            None => {
                let mut segs = plain.to_vec();
                segs.push(LINUX_APP_DIR);
                Some(join(os, h, &segs))
            }
        }
    };
    let config = pick("XDG_CONFIG_HOME", &[".config"], &["config"])?;
    let data = pick("XDG_DATA_HOME", &[".local", "share"], &["data"])?;
    let cache = pick("XDG_CACHE_HOME", &[".cache"], &["cache"])?;
    let state = pick("XDG_STATE_HOME", &[".local", "state"], &[".local", "state"])?;
    let logs = state.join("logs");
    Some(PlatformDirs {
        config,
        data,
        cache,
        logs,
        state,
    })
}

fn macos_dirs(env: &dyn EnvProbe) -> Option<PlatformDirs> {
    let os = Os::MacOs;
    let home = home_string(os, env)?;
    let support = join(os, &home, &["Library", "Application Support", APP_DIR]);
    Some(PlatformDirs {
        config: support.clone(),
        data: support.clone(),
        cache: join(os, &home, &["Library", "Caches", APP_DIR]),
        logs: join(os, &home, &["Library", "Logs", APP_DIR]),
        state: support.join("state"),
    })
}

fn windows_dirs(env: &dyn EnvProbe) -> Option<PlatformDirs> {
    let os = Os::Windows;
    let profile = home_string(os, env).or_else(|| abs_var(os, env, "USERPROFILE"));
    let roaming = abs_var(os, env, "APPDATA").or_else(|| {
        profile
            .as_deref()
            .map(|p| join(os, p, &["AppData", "Roaming"]).into_string())
    })?;
    let local = abs_var(os, env, "LOCALAPPDATA").or_else(|| {
        profile
            .as_deref()
            .map(|p| join(os, p, &["AppData", "Local"]).into_string())
    })?;
    let local_root = join(os, &local, &[APP_DIR]);
    Some(PlatformDirs {
        config: join(os, &roaming, &[APP_DIR, "config"]),
        data: join(os, local_root.as_str(), &["data"]),
        cache: join(os, local_root.as_str(), &["cache"]),
        logs: join(os, local_root.as_str(), &["logs"]),
        state: join(os, local_root.as_str(), &["state"]),
    })
}

/// The folders used in portable mode: everything under `<exe_dir>/data`.
pub fn portable_dirs(exe_dir: &Utf8Path) -> PlatformDirs {
    let root = exe_dir.join("data");
    PlatformDirs {
        config: root.join("config"),
        data: root.join("data"),
        cache: root.join("cache"),
        logs: root.join("logs"),
        state: root.join("state"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::collections::BTreeMap;

    struct MapEnv {
        vars: BTreeMap<&'static str, &'static str>,
        home: Option<&'static str>,
    }

    impl EnvProbe for MapEnv {
        fn var(&self, key: &str) -> Option<String> {
            self.vars.get(key).map(|v| (*v).to_owned())
        }
        fn home_dir(&self) -> Option<Utf8PathBuf> {
            self.home.map(Utf8PathBuf::from)
        }
        fn exe_dir(&self) -> Option<Utf8PathBuf> {
            None
        }
    }

    fn env(home: Option<&'static str>, vars: &[(&'static str, &'static str)]) -> MapEnv {
        MapEnv {
            vars: vars.iter().copied().collect(),
            home,
        }
    }

    #[test]
    fn linux_falls_back_to_home_folders() {
        let d = platform_dirs(Os::Linux, &env(Some("/home/u"), &[])).unwrap();
        assert_eq!(d.config, "/home/u/.config/rimstudio");
        assert_eq!(d.data, "/home/u/.local/share/rimstudio");
        assert_eq!(d.cache, "/home/u/.cache/rimstudio");
        assert_eq!(d.state, "/home/u/.local/state/rimstudio");
        assert_eq!(d.logs, "/home/u/.local/state/rimstudio/logs");
    }

    #[test]
    fn linux_honours_absolute_xdg_variables_only() {
        let e = env(
            Some("/home/u"),
            &[
                ("XDG_CONFIG_HOME", "/cfg"),
                ("XDG_DATA_HOME", "relative/data"),
                ("XDG_CACHE_HOME", ""),
                ("XDG_STATE_HOME", "/st/"),
            ],
        );
        let d = platform_dirs(Os::Linux, &e).unwrap();
        assert_eq!(d.config, "/cfg/rimstudio");
        assert_eq!(d.data, "/home/u/.local/share/rimstudio");
        assert_eq!(d.cache, "/home/u/.cache/rimstudio");
        assert_eq!(d.state, "/st/rimstudio");
    }

    #[test]
    fn linux_flatpak_uses_the_per_app_folders() {
        let e = env(Some("/home/u"), &[("FLATPAK_ID", "dev.example.Studio")]);
        let d = platform_dirs(Os::Linux, &e).unwrap();
        let base = "/home/u/.var/app/dev.example.Studio";
        assert_eq!(d.config, format!("{base}/config/rimstudio"));
        assert_eq!(d.data, format!("{base}/data/rimstudio"));
        assert_eq!(d.cache, format!("{base}/cache/rimstudio"));
        assert_eq!(d.state, format!("{base}/.local/state/rimstudio"));
    }

    #[test]
    fn linux_flatpak_prefers_variables_the_runtime_sets() {
        let e = env(
            Some("/home/u"),
            &[
                ("FLATPAK_ID", "dev.example.Studio"),
                ("XDG_CONFIG_HOME", "/home/u/.var/app/x/config"),
            ],
        );
        let d = platform_dirs(Os::Linux, &e).unwrap();
        assert_eq!(d.config, "/home/u/.var/app/x/config/rimstudio");
    }

    #[test]
    fn linux_without_home_and_variables_is_none() {
        assert!(platform_dirs(Os::Linux, &env(None, &[])).is_none());
        assert!(platform_dirs(Os::Linux, &env(Some("relative"), &[])).is_none());
    }

    #[test]
    fn linux_variables_alone_are_enough() {
        let e = env(
            None,
            &[
                ("XDG_CONFIG_HOME", "/c"),
                ("XDG_DATA_HOME", "/d"),
                ("XDG_CACHE_HOME", "/k"),
                ("XDG_STATE_HOME", "/s"),
            ],
        );
        let d = platform_dirs(Os::Linux, &e).unwrap();
        assert_eq!(d.data, "/d/rimstudio");
    }

    #[test]
    fn macos_uses_library_folders() {
        let d = platform_dirs(Os::MacOs, &env(Some("/Users/u"), &[])).unwrap();
        assert_eq!(d.config, "/Users/u/Library/Application Support/RimStudio");
        assert_eq!(d.data, d.config);
        assert_eq!(d.cache, "/Users/u/Library/Caches/RimStudio");
        assert_eq!(d.logs, "/Users/u/Library/Logs/RimStudio");
        assert_eq!(
            d.state,
            "/Users/u/Library/Application Support/RimStudio/state"
        );
        assert!(platform_dirs(Os::MacOs, &env(None, &[])).is_none());
    }

    #[test]
    fn windows_uses_appdata_and_localappdata() {
        let e = env(
            Some("C:\\Users\\u"),
            &[
                ("APPDATA", "C:\\Users\\u\\AppData\\Roaming"),
                ("LOCALAPPDATA", "D:\\Local"),
            ],
        );
        let d = platform_dirs(Os::Windows, &e).unwrap();
        assert_eq!(
            d.config,
            "C:\\Users\\u\\AppData\\Roaming\\RimStudio\\config"
        );
        assert_eq!(d.data, "D:\\Local\\RimStudio\\data");
        assert_eq!(d.cache, "D:\\Local\\RimStudio\\cache");
        assert_eq!(d.logs, "D:\\Local\\RimStudio\\logs");
        assert_eq!(d.state, "D:\\Local\\RimStudio\\state");
    }

    #[rstest]
    #[case(Some("C:\\Users\\u"), &[], true)]
    #[case(None, &[("USERPROFILE", "C:\\Users\\v")], true)]
    #[case(None, &[("APPDATA", "C:\\A"), ("LOCALAPPDATA", "C:\\L")], true)]
    #[case(None, &[], false)]
    #[case(None, &[("APPDATA", "rel"), ("LOCALAPPDATA", "rel")], false)]
    fn windows_resolution_table(
        #[case] home: Option<&'static str>,
        #[case] vars: &[(&'static str, &'static str)],
        #[case] some: bool,
    ) {
        assert_eq!(platform_dirs(Os::Windows, &env(home, vars)).is_some(), some);
    }

    #[test]
    fn windows_fallbacks_derive_from_the_profile() {
        let d = platform_dirs(Os::Windows, &env(Some("C:\\Users\\u"), &[])).unwrap();
        assert_eq!(
            d.config,
            "C:\\Users\\u\\AppData\\Roaming\\RimStudio\\config"
        );
        assert_eq!(d.data, "C:\\Users\\u\\AppData\\Local\\RimStudio\\data");
    }

    #[test]
    fn portable_dirs_sit_under_the_executable_folder() {
        let d = portable_dirs(Utf8Path::new("/opt/app"));
        assert_eq!(d.config, "/opt/app/data/config");
        assert_eq!(d.logs, "/opt/app/data/logs");
        assert_eq!(d.state, "/opt/app/data/state");
    }

    #[test]
    fn join_handles_roots_and_trailing_separators() {
        assert_eq!(join(Os::Linux, "/", &["a"]), "/a");
        assert_eq!(join(Os::Linux, "/x//", &["a", "b"]), "/x/a/b");
        assert_eq!(join(Os::Windows, "C:\\", &["a"]), "C:\\a");
    }
}
