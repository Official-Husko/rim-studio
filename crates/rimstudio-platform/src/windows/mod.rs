//! Windows specifics: the registry, `tasklist` parsing, directory symlinks and process flags.
//!
//! Junctions are not created: that needs a dedicated crate, so [`link_support`] reports them as
//! unavailable and the library crate falls back to copying. Symbolic links are tried and fail with a
//! permission error when the process lacks the privilege.

use std::process::Command;

use camino::Utf8Path;
use rimstudio_core::ports::{Hive, LinkSupport, RunningProcess};

use crate::process::name_matches;

/// The program that opens a URL through the shell's protocol handler.
pub const OPENER: &str = "rundll32";

/// Arguments that make [`OPENER`] open `url`.
pub fn opener_args(url: &str) -> Vec<String> {
    vec!["url.dll,FileProtocolHandler".to_owned(), url.to_owned()]
}

/// What links can be created: symbolic links only, and they may need a privilege.
pub fn link_support() -> LinkSupport {
    LinkSupport {
        symlink: true,
        junction: false,
        needs_privilege: true,
    }
}

/// Reads a string value from the registry. Off Windows this always returns `None`.
#[cfg(windows)]
pub fn read_registry_string(hive: Hive, key: &str, value: &str) -> Option<String> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
    let root = match hive {
        Hive::LocalMachine => RegKey::predef(HKEY_LOCAL_MACHINE),
        Hive::CurrentUser => RegKey::predef(HKEY_CURRENT_USER),
    };
    let sub = root.open_subkey_with_flags(key, KEY_READ).ok()?;
    sub.get_value::<String, _>(value).ok()
}

/// Reads a string value from the registry. Off Windows this always returns `None`.
#[cfg(not(windows))]
pub fn read_registry_string(hive: Hive, key: &str, value: &str) -> Option<String> {
    let _ = (hive, key, value);
    None
}

/// Creates a directory symbolic link. Off Windows this fails with `Unsupported`.
#[cfg(windows)]
pub fn create_dir_symlink(target: &Utf8Path, link: &Utf8Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target.as_std_path(), link.as_std_path())
}

/// Creates a directory symbolic link. Off Windows this fails with `Unsupported`.
#[cfg(not(windows))]
pub fn create_dir_symlink(target: &Utf8Path, link: &Utf8Path) -> std::io::Result<()> {
    let _ = (target, link);
    Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
}

/// Marks a command so the child has no console and outlives the app. No effect off Windows.
#[cfg(windows)]
pub fn detach(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

/// Marks a command so the child has no console and outlives the app. No effect off Windows.
#[cfg(not(windows))]
pub fn detach(cmd: &mut Command) {
    let _ = cmd;
}

/// Runs `tasklist /FO CSV /NH` and returns the matching processes; empty when it fails.
pub fn list_with_tasklist(names: &[&str]) -> Vec<RunningProcess> {
    let output = Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output();
    match output {
        Ok(o) if o.status.success() => parse_tasklist(&String::from_utf8_lossy(&o.stdout), names),
        _ => Vec::new(),
    }
}

/// Splits one CSV line of quoted fields (`"a","b,c","d"`); a doubled quote is a literal quote.
pub fn split_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.trim_end_matches(['\r', '\n']).chars().peekable();
    while let Some(c) = chars.next() {
        match (c, in_quotes) {
            ('"', true) => {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            }
            ('"', false) => in_quotes = true,
            (',', false) => fields.push(std::mem::take(&mut cur)),
            (other, _) => cur.push(other),
        }
    }
    fields.push(cur);
    fields
}

/// Parses `tasklist /FO CSV /NH` output: `"Image Name","PID",...`. The executable path is not
/// reported by `tasklist`, so `exe` is always `None`.
pub fn parse_tasklist(text: &str, names: &[&str]) -> Vec<RunningProcess> {
    let mut out = Vec::new();
    for line in text.lines() {
        let fields = split_csv_line(line);
        let (Some(name), Some(pid_text)) = (fields.first(), fields.get(1)) else {
            continue;
        };
        let Ok(pid) = pid_text.trim().parse::<u32>() else {
            continue;
        };
        if name_matches(name, names) {
            out.push(RunningProcess {
                pid,
                name: name.clone(),
                exe: None,
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
    fn csv_lines_split_on_quoted_fields() {
        assert_eq!(
            split_csv_line("\"a\",\"b,c\",\"d\"\r\n"),
            vec!["a", "b,c", "d"]
        );
        assert_eq!(split_csv_line("\"x\"\"y\",\"z\""), vec!["x\"y", "z"]);
        assert_eq!(split_csv_line(""), vec![""]);
    }

    #[test]
    fn tasklist_output_is_parsed_and_filtered() {
        let text = "\"System\",\"4\",\"Services\",\"0\",\"20 K\"\r\n\"RS_Game.exe\",\"1234\",\"Console\",\"1\",\"1,200,000 K\"\r\n\"rs_game.EXE\",\"99\",\"Console\",\"1\",\"5 K\"\r\nINFO: junk\r\n";
        let got = parse_tasklist(text, &["RS_Game.exe"]);
        let pids: Vec<u32> = got.iter().map(|p| p.pid).collect();
        assert_eq!(pids, vec![99, 1234]);
        assert_eq!(got[0].name, "rs_game.EXE");
    }

    #[test]
    fn opener_arguments_carry_the_url_as_one_argument() {
        let args = opener_args("steam://rungameid/1");
        assert_eq!(args.len(), 2);
        assert_eq!(args[1], "steam://rungameid/1");
    }

    #[test]
    fn link_support_reports_no_junctions() {
        let s = link_support();
        assert!(s.symlink && !s.junction && s.needs_privilege);
    }

    #[cfg(not(windows))]
    #[test]
    fn registry_is_absent_off_windows() {
        assert_eq!(read_registry_string(Hive::CurrentUser, "k", "v"), None);
    }
}
