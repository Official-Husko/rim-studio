//! Linux specifics: scanning a `/proc` style folder for processes and the URL opener.

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::ports::RunningProcess;

use crate::process::{file_name_of, name_matches};

/// The program that opens a URL through the desktop.
pub const OPENER: &str = "xdg-open";

/// The kernel truncates `comm` to this many bytes.
const COMM_LIMIT: usize = 15;

/// Scans `root` (normally `/proc`) for processes whose name matches one of `names`.
///
/// The name candidates are the `comm` file, the file name of the `exe` link and, when `comm` looks
/// truncated, the first word of `cmdline`. Entries that vanish or cannot be read are skipped. The
/// result is sorted by pid.
pub fn scan_proc(root: &Utf8Path, names: &[&str]) -> Vec<RunningProcess> {
    let Ok(rd) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<u32>().ok())
        else {
            continue;
        };
        let dir = root.join(pid.to_string());
        let comm = std::fs::read_to_string(dir.join("comm"))
            .map(|c| c.trim_end_matches(['\n', '\r']).to_owned())
            .unwrap_or_default();
        let exe = std::fs::read_link(dir.join("exe"))
            .ok()
            .and_then(|p| Utf8PathBuf::from_path_buf(p).ok())
            .map(|p| {
                let s = p.as_str();
                Utf8PathBuf::from(s.strip_suffix(" (deleted)").unwrap_or(s))
            });
        let exe_name = exe.as_ref().map(|p| file_name_of(p.as_str()).to_owned());
        let argv0 = if comm.len() >= COMM_LIMIT || comm.is_empty() {
            std::fs::read(dir.join("cmdline")).ok().and_then(|b| {
                let first = b.split(|c| *c == 0).next().unwrap_or_default();
                std::str::from_utf8(first)
                    .ok()
                    .map(|s| file_name_of(s).to_owned())
            })
        } else {
            None
        };
        let matched = [Some(&comm), exe_name.as_ref(), argv0.as_ref()]
            .into_iter()
            .flatten()
            .find(|c| !c.is_empty() && name_matches(c, names))
            .cloned();
        if let Some(name) = matched {
            out.push(RunningProcess { pid, name, exe });
        }
    }
    out.sort_by_key(|p| p.pid);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc_entry(root: &std::path::Path, pid: u32, comm: &str, cmdline: &[u8]) {
        let d = root.join(pid.to_string());
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("comm"), format!("{comm}\n")).unwrap();
        std::fs::write(d.join("cmdline"), cmdline).unwrap();
    }

    fn utf8(p: &std::path::Path) -> &Utf8Path {
        Utf8Path::from_path(p).unwrap()
    }

    #[test]
    fn matches_comm_ignoring_case_and_sorts_by_pid() {
        let t = tempfile::tempdir().unwrap();
        proc_entry(t.path(), 30, "RS_Game", b"/x/RS_Game\0");
        proc_entry(t.path(), 4, "rs_game", b"");
        proc_entry(t.path(), 5, "other", b"");
        std::fs::create_dir_all(t.path().join("self")).unwrap();
        let got = scan_proc(utf8(t.path()), &["rs_game"]);
        let pids: Vec<u32> = got.iter().map(|p| p.pid).collect();
        assert_eq!(pids, vec![4, 30]);
    }

    #[test]
    fn truncated_comm_falls_back_to_the_command_line() {
        let t = tempfile::tempdir().unwrap();
        proc_entry(
            t.path(),
            9,
            "RS_VeryLongGameN",
            b"/opt/g/RS_VeryLongGameName\0--flag\0",
        );
        let got = scan_proc(utf8(t.path()), &["RS_VeryLongGameName"]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "RS_VeryLongGameName");
    }

    #[cfg(unix)]
    #[test]
    fn exe_link_name_matches_and_deleted_suffix_is_removed() {
        let t = tempfile::tempdir().unwrap();
        proc_entry(t.path(), 11, "wrapper", b"");
        std::os::unix::fs::symlink(
            "/opt/g/RS_RealName (deleted)",
            t.path().join("11").join("exe"),
        )
        .unwrap();
        let got = scan_proc(utf8(t.path()), &["rs_realname"]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "RS_RealName");
        assert_eq!(
            got[0].exe.as_deref(),
            Some(Utf8Path::new("/opt/g/RS_RealName"))
        );
    }

    #[test]
    fn missing_root_gives_an_empty_list() {
        assert!(scan_proc(Utf8Path::new("/definitely/not/here"), &["x"]).is_empty());
    }
}
