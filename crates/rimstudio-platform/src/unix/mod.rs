//! Unix specifics: directory symlinks and process group detaching.

use std::process::Command;

use camino::Utf8Path;
use rimstudio_core::ports::LinkSupport;

/// What links can be created: symbolic links, never needing a privilege.
pub fn link_support() -> LinkSupport {
    LinkSupport {
        symlink: true,
        junction: false,
        needs_privilege: false,
    }
}

/// Creates a symbolic link at `link` pointing at `target`. Off Unix this fails with `Unsupported`.
#[cfg(unix)]
pub fn create_dir_symlink(target: &Utf8Path, link: &Utf8Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target.as_std_path(), link.as_std_path())
}

/// Creates a symbolic link at `link` pointing at `target`. Off Unix this fails with `Unsupported`.
#[cfg(not(unix))]
pub fn create_dir_symlink(target: &Utf8Path, link: &Utf8Path) -> std::io::Result<()> {
    let _ = (target, link);
    Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
}

/// Puts the child in its own process group so closing the app does not signal it. No effect off
/// Unix.
#[cfg(unix)]
pub fn detach(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    cmd.process_group(0);
}

/// Puts the child in its own process group so closing the app does not signal it. No effect off
/// Unix.
#[cfg(not(unix))]
pub fn detach(cmd: &mut Command) {
    let _ = cmd;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symlink_support_needs_no_privilege() {
        let s = link_support();
        assert!(s.symlink && !s.junction && !s.needs_privilege);
    }
}
