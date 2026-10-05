//! Sandbox detection from the environment and the Flatpak info file.
//!
//! AppImage is not a sandbox and is reported by the install source probe instead.

use camino::Utf8PathBuf;
use rimstudio_core::ports::{EnvProbe, SandboxInfo};

/// The file Flatpak creates at the root of every sandbox.
pub const FLATPAK_INFO_PATH: &str = "/.flatpak-info";

fn non_empty(env: &dyn EnvProbe, key: &str) -> Option<String> {
    env.var(key)
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

/// Detects the sandbox. `flatpak_info` is the text of [`FLATPAK_INFO_PATH`] when it exists.
///
/// Flatpak is detected by that file or by `FLATPAK_ID`; the process table of the host is hidden and
/// the granted paths come from the `filesystems` list of its `[Context]` section. Snap is detected
/// by `SNAP` together with `SNAP_NAME`; its writable data folders are the granted paths.
pub fn detect_sandbox(env: &dyn EnvProbe, flatpak_info: Option<&str>) -> SandboxInfo {
    if flatpak_info.is_some() || non_empty(env, "FLATPAK_ID").is_some() {
        let home = env.home_dir();
        let granted = flatpak_info
            .map(|t| flatpak_granted_paths(t, home.as_deref().map(|h| h.as_str())))
            .unwrap_or_default();
        return SandboxInfo {
            kind: Some("flatpak".to_owned()),
            hides_processes: true,
            granted_paths: granted,
        };
    }
    if non_empty(env, "SNAP").is_some() && non_empty(env, "SNAP_NAME").is_some() {
        let mut granted = Vec::new();
        for key in ["SNAP_USER_DATA", "SNAP_USER_COMMON"] {
            if let Some(v) = non_empty(env, key)
                && v.starts_with('/')
            {
                granted.push(Utf8PathBuf::from(v));
            }
        }
        return SandboxInfo {
            kind: Some("snap".to_owned()),
            hides_processes: false,
            granted_paths: granted,
        };
    }
    SandboxInfo::none()
}

/// The absolute folders granted by the `filesystems` entry of the `[Context]` section.
///
/// `home` and `~/x` expand against `home`, `host` and `host-os` grant `/`, entries starting with
/// `!` (denials) and the `xdg-*` and `host-etc` shorthands are skipped, and an access suffix such as
/// `:ro` is removed. The result is sorted and without duplicates.
pub fn flatpak_granted_paths(info: &str, home: Option<&str>) -> Vec<Utf8PathBuf> {
    let mut in_context = false;
    let mut out: Vec<Utf8PathBuf> = Vec::new();
    for line in info.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_context = line == "[Context]";
            continue;
        }
        if !in_context {
            continue;
        }
        let Some(list) = line.strip_prefix("filesystems=") else {
            continue;
        };
        for raw in list.split(';') {
            let entry = raw.trim();
            if entry.is_empty() || entry.starts_with('!') {
                continue;
            }
            let entry = entry
                .rsplit_once(':')
                .filter(|(_, mode)| matches!(*mode, "ro" | "rw" | "create"))
                .map_or(entry, |(path, _)| path);
            let path = if let Some(rest) = entry.strip_prefix("~/") {
                home.map(|h| format!("{}/{}", h.trim_end_matches('/'), rest))
            } else {
                match entry {
                    "host" | "host-os" => Some("/".to_owned()),
                    "home" => home.map(str::to_owned),
                    e if e.starts_with('/') => Some(e.to_owned()),
                    _ => None,
                }
            };
            if let Some(p) = path {
                out.push(Utf8PathBuf::from(p));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    struct E(Vec<(&'static str, &'static str)>);
    impl EnvProbe for E {
        fn var(&self, key: &str) -> Option<String> {
            self.0
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_owned())
        }
        fn home_dir(&self) -> Option<Utf8PathBuf> {
            Some("/home/u".into())
        }
        fn exe_dir(&self) -> Option<Utf8PathBuf> {
            None
        }
    }

    #[test]
    fn plain_environment_is_not_sandboxed() {
        let info = detect_sandbox(&E(vec![]), None);
        assert!(!info.is_sandboxed());
        assert!(!info.hides_processes);
    }

    #[test]
    fn flatpak_is_detected_from_the_variable_or_the_file() {
        let a = detect_sandbox(&E(vec![("FLATPAK_ID", "dev.x.App")]), None);
        assert_eq!(a.kind.as_deref(), Some("flatpak"));
        assert!(a.hides_processes);
        let b = detect_sandbox(&E(vec![]), Some("[Application]\nname=dev.x.App\n"));
        assert_eq!(b.kind.as_deref(), Some("flatpak"));
    }

    #[test]
    fn flatpak_grants_are_parsed_from_the_context_section() {
        let text = "[Application]\nfilesystems=/ignored\n\n[Context]\nshared=network;\nfilesystems=/mnt;/run/media:ro;~/Games;!/secret;xdg-download;home;\n";
        let info = detect_sandbox(&E(vec![]), Some(text));
        let got: Vec<&str> = info.granted_paths.iter().map(|p| p.as_str()).collect();
        assert_eq!(got, vec!["/home/u", "/home/u/Games", "/mnt", "/run/media"]);
    }

    #[test]
    fn host_access_grants_the_root() {
        let got = flatpak_granted_paths("[Context]\nfilesystems=host;\n", None);
        assert_eq!(got, vec![Utf8PathBuf::from("/")]);
    }

    #[test]
    fn snap_needs_both_variables() {
        assert!(!detect_sandbox(&E(vec![("SNAP", "/snap/x/1")]), None).is_sandboxed());
        let info = detect_sandbox(
            &E(vec![
                ("SNAP", "/snap/x/1"),
                ("SNAP_NAME", "x"),
                ("SNAP_USER_DATA", "/home/u/snap/x/1"),
            ]),
            None,
        );
        assert_eq!(info.kind.as_deref(), Some("snap"));
        assert_eq!(
            info.granted_paths,
            vec![Utf8PathBuf::from("/home/u/snap/x/1")]
        );
    }
}
