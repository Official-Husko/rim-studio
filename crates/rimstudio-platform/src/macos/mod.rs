//! macOS specifics: process listing through `ps` and the URL opener.

use camino::Utf8PathBuf;
use rimstudio_core::ports::RunningProcess;

use crate::process::{file_name_of, name_matches};

/// The program that opens a URL.
pub const OPENER: &str = "open";

/// Runs `ps -axo pid=,comm=` and returns the matching processes; empty when `ps` fails.
pub fn list_with_ps(names: &[&str]) -> Vec<RunningProcess> {
    let output = std::process::Command::new("ps")
        .args(["-axo", "pid=,comm="])
        .output();
    match output {
        Ok(o) if o.status.success() => parse_ps(&String::from_utf8_lossy(&o.stdout), names),
        _ => Vec::new(),
    }
}

/// Parses `ps -axo pid=,comm=` output. Each line is a pid and then the command, which on macOS is
/// the full executable path (it may contain spaces). The name is the last path component.
pub fn parse_ps(text: &str, names: &[&str]) -> Vec<RunningProcess> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim_start();
        let Some((pid_text, rest)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let Ok(pid) = pid_text.parse::<u32>() else {
            continue;
        };
        let command = rest.trim();
        if command.is_empty() {
            continue;
        }
        let name = file_name_of(command);
        if name_matches(name, names) {
            out.push(RunningProcess {
                pid,
                name: name.to_owned(),
                exe: command.starts_with('/').then(|| Utf8PathBuf::from(command)),
            });
        }
    }
    out.sort_by_key(|p| p.pid);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ps_lines_with_spaces_in_the_path() {
        let text = "  1 /sbin/launchd\n 220 /Applications/RS Test.app/Contents/MacOS/RS_Game\n  77 RS_Game\nbad line\n";
        let got = parse_ps(text, &["rs_game"]);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].pid, 77);
        assert_eq!(got[0].exe, None);
        assert_eq!(got[1].pid, 220);
        assert_eq!(got[1].name, "RS_Game");
        assert_eq!(
            got[1].exe.as_deref().map(|p| p.as_str()),
            Some("/Applications/RS Test.app/Contents/MacOS/RS_Game")
        );
    }

    #[test]
    fn ignores_malformed_lines() {
        assert!(parse_ps("\n   \nabc def\n12\n", &["def"]).is_empty());
    }
}
