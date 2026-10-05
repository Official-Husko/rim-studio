//! Starting programs: the only place in the workspace where an application process is spawned.
//!
//! A [`LaunchSpec`] either runs an executable directly or opens a `steam://` URL through the OS
//! opener, so the game starts through the user's own Steam. Arguments are never interpreted by a
//! shell. The child is detached from the app and reaped by a short lived background thread.

use std::process::{Command, Stdio};

use camino::Utf8PathBuf;
use rimstudio_core::os::Os;
use rimstudio_core::ports::{ChildHandle, LaunchSpec, PortError, PortErrorKind, PortResult};

use crate::error::{PlatformError, PlatformResult, io_error};
use crate::{linux, macos, unix, windows};

/// The URL that starts RimWorld (app id 294100) through Steam.
pub const STEAM_RUN_URL: &str = "steam://rungameid/294100";

/// The longest URL accepted.
pub const MAX_URL_LEN: usize = 512;

/// A spec that opens [`STEAM_RUN_URL`].
pub fn steam_run_spec() -> LaunchSpec {
    LaunchSpec {
        program: Utf8PathBuf::new(),
        args: Vec::new(),
        cwd: None,
        env: Vec::new(),
        open_url: Some(STEAM_RUN_URL.to_owned()),
    }
}

/// A spec that runs an executable with arguments.
pub fn direct_spec(program: impl Into<Utf8PathBuf>, args: Vec<String>) -> LaunchSpec {
    LaunchSpec {
        program: program.into(),
        args,
        cwd: None,
        env: Vec::new(),
        open_url: None,
    }
}

/// Checks that `url` is a plain `steam://` URL: at most [`MAX_URL_LEN`] bytes of ASCII letters,
/// digits and `/ : . _ - ? = % +` after the scheme. Anything else (spaces, quotes, `&`, `|`, control
/// characters) is rejected so no opener can be made to do more than open the URL.
pub fn validate_steam_url(url: &str) -> PlatformResult<()> {
    let Some(rest) = url.strip_prefix("steam://") else {
        return Err(PlatformError::invalid(
            "openUrl",
            "only steam:// URLs are allowed",
        ));
    };
    if rest.is_empty() || url.len() > MAX_URL_LEN {
        return Err(PlatformError::invalid("openUrl", "empty or too long"));
    }
    let ok = rest
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/:._-?=%+".contains(c));
    if ok {
        Ok(())
    } else {
        Err(PlatformError::invalid(
            "openUrl",
            "contains a character that is not allowed",
        ))
    }
}

/// The opener program and its arguments for `url` on `os`.
pub fn opener_command(os: Os, url: &str) -> PlatformResult<(String, Vec<String>)> {
    validate_steam_url(url)?;
    Ok(match os {
        Os::Windows => (windows::OPENER.to_owned(), windows::opener_args(url)),
        Os::MacOs => (macos::OPENER.to_owned(), vec![url.to_owned()]),
        Os::Linux | Os::Other => (linux::OPENER.to_owned(), vec![url.to_owned()]),
    })
}

/// Checks the parts of a spec that run a program directly.
pub fn validate_direct(spec: &LaunchSpec) -> PlatformResult<()> {
    if spec.program.as_str().trim().is_empty() {
        return Err(PlatformError::invalid("program", "empty"));
    }
    if spec.program.as_str().contains('\0') || spec.args.iter().any(|a| a.contains('\0')) {
        return Err(PlatformError::invalid("args", "contains a NUL byte"));
    }
    for (name, value) in &spec.env {
        if name.is_empty() || name.contains(['=', '\0']) || value.contains('\0') {
            return Err(PlatformError::invalid(
                "env",
                format!("bad variable {name:?}"),
            ));
        }
    }
    Ok(())
}

fn finish(mut cmd: Command, what: &str) -> PortResult<u32> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    unix::detach(&mut cmd);
    windows::detach(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| io_error(what, &e))?;
    let pid = child.id();
    // Reap the child when it exits so no zombie is left; failure to start the thread only means
    // the child is reaped when the app exits.
    let _ = std::thread::Builder::new()
        .name("rimstudio-reap".to_owned())
        .spawn(move || {
            let _ = child.wait();
        });
    Ok(pid)
}

/// Starts `spec` for `os`. With `open_url` the OS opener is run and the handle's pid is 0.
pub fn spawn_spec(os: Os, spec: &LaunchSpec) -> PortResult<ChildHandle> {
    if let Some(url) = &spec.open_url {
        let (program, args) = opener_command(os, url).map_err(PortError::from)?;
        let mut cmd = Command::new(program);
        cmd.args(args);
        finish(cmd, "open url")?;
        return Ok(ChildHandle { pid: 0 });
    }
    validate_direct(spec).map_err(PortError::from)?;
    let mut cmd = Command::new(spec.program.as_std_path());
    cmd.args(&spec.args);
    if let Some(cwd) = &spec.cwd {
        cmd.current_dir(cwd.as_std_path());
    }
    for (k, v) in &spec.env {
        cmd.env(k, v);
    }
    let pid = finish(cmd, "spawn")?;
    if pid == 0 {
        return Err(PortError::new(PortErrorKind::Io, "child reported pid 0"));
    }
    Ok(ChildHandle { pid })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("steam://rungameid/294100", true)]
    #[case("steam://run/1?x=2", true)]
    #[case("http://example.com", false)]
    #[case("steam://", false)]
    #[case("steam://a b", false)]
    #[case("steam://a&calc", false)]
    #[case("steam://a\"b", false)]
    #[case("steam://a\nb", false)]
    #[case("STEAM://x", false)]
    fn url_validation_table(#[case] url: &str, #[case] ok: bool) {
        assert_eq!(validate_steam_url(url).is_ok(), ok, "{url}");
    }

    #[test]
    fn overlong_urls_are_rejected() {
        let url = format!("steam://{}", "a".repeat(MAX_URL_LEN));
        assert!(validate_steam_url(&url).is_err());
    }

    #[rstest]
    #[case(Os::Linux, "xdg-open")]
    #[case(Os::MacOs, "open")]
    #[case(Os::Windows, "rundll32")]
    fn opener_per_os(#[case] os: Os, #[case] program: &str) {
        let (p, args) = opener_command(os, STEAM_RUN_URL).unwrap();
        assert_eq!(p, program);
        assert_eq!(args.last().map(String::as_str), Some(STEAM_RUN_URL));
    }

    #[test]
    fn opener_rejects_other_urls() {
        assert!(opener_command(Os::Linux, "file:///etc/passwd").is_err());
    }

    #[test]
    fn direct_spec_validation() {
        assert!(validate_direct(&direct_spec("", vec![])).is_err());
        assert!(validate_direct(&direct_spec("/bin/x", vec!["a\0b".into()])).is_err());
        let mut s = direct_spec("/bin/x", vec![]);
        s.env.push(("A=B".into(), "v".into()));
        assert!(validate_direct(&s).is_err());
        s.env.clear();
        s.env.push(("A".into(), "v".into()));
        assert!(validate_direct(&s).is_ok());
    }

    #[test]
    fn steam_spec_carries_the_run_url() {
        assert_eq!(steam_run_spec().open_url.as_deref(), Some(STEAM_RUN_URL));
    }

    #[test]
    fn missing_program_reports_not_found() {
        let e = spawn_spec(
            Os::current(),
            &direct_spec("/definitely/not/here/prog", vec![]),
        )
        .unwrap_err();
        assert_eq!(e.kind, PortErrorKind::NotFound);
    }

    #[test]
    fn bad_url_spec_is_refused_before_spawning() {
        let mut spec = steam_run_spec();
        spec.open_url = Some("https://example.com".into());
        assert!(spawn_spec(Os::current(), &spec).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn direct_spawn_returns_a_pid_and_honours_cwd_and_env() {
        let t = tempfile::tempdir().unwrap();
        let dir = Utf8PathBuf::from_path_buf(t.path().to_path_buf()).unwrap();
        let mut spec = direct_spec(
            "/bin/sh",
            vec!["-c".into(), "printf %s \"$RS_VAR\" > out.txt".into()],
        );
        spec.cwd = Some(dir.clone());
        spec.env.push(("RS_VAR".into(), "hello".into()));
        let handle = spawn_spec(Os::current(), &spec).unwrap();
        assert!(handle.pid > 0);
        let out = dir.join("out.txt");
        for _ in 0..200 {
            if out.exists()
                && std::fs::metadata(&out)
                    .map(|m| m.len() > 0)
                    .unwrap_or(false)
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "hello");
    }
}
