//! Process listing: dispatch by operating system and the pure output parsers.

use rimstudio_core::os::Os;
use rimstudio_core::ports::RunningProcess;

use crate::{linux, macos, windows};

/// The running processes whose name equals one of `names` (ASCII case ignored), sorted by pid.
///
/// Linux scans `/proc`, macOS runs `ps` and Windows runs `tasklist`. Any failure yields an empty
/// list, which callers must read together with the sandbox information.
pub fn running_processes(os: Os, names: &[&str]) -> Vec<RunningProcess> {
    if names.is_empty() {
        return Vec::new();
    }
    let mut found = match os {
        Os::Linux => linux::scan_proc(camino::Utf8Path::new("/proc"), names),
        Os::MacOs => macos::list_with_ps(names),
        Os::Windows => windows::list_with_tasklist(names),
        Os::Other => Vec::new(),
    };
    found.sort_by_key(|p| p.pid);
    found
}

/// True when `candidate` equals one of `names` ignoring ASCII case.
pub fn name_matches(candidate: &str, names: &[&str]) -> bool {
    names.iter().any(|n| n.eq_ignore_ascii_case(candidate))
}

/// The last component of a path written with either separator.
pub fn file_name_of(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_compare_ignoring_ascii_case() {
        assert!(name_matches("RS_Game.EXE", &["rs_game.exe"]));
        assert!(!name_matches("other", &["rs_game"]));
    }

    #[test]
    fn file_name_handles_both_separators() {
        assert_eq!(file_name_of("/a/b/c"), "c");
        assert_eq!(file_name_of("C:\\a\\b.exe"), "b.exe");
        assert_eq!(file_name_of("plain"), "plain");
        assert_eq!(file_name_of(""), "");
    }

    #[test]
    fn empty_name_list_finds_nothing() {
        assert!(running_processes(Os::current(), &[]).is_empty());
    }

    #[test]
    fn unknown_os_lists_nothing() {
        assert!(running_processes(Os::Other, &["x"]).is_empty());
    }
}
