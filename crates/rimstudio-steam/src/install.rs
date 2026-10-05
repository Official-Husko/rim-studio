//! Facts about a RimWorld install folder: how to recognise one, how to read its version file and
//! which executable names to look for. Pure data and text functions; probing lives in
//! [`crate::locator`].

use rimstudio_core::error::CoreError;
use rimstudio_core::os::Os;
use rimstudio_core::version::GameVersion;

/// The Steam app id of RimWorld.
pub const GAME_APP_ID: u64 = 294_100;

/// The file whose presence proves a folder holds the base game data (relative to the folder that
/// contains `Data`).
pub const CONTENT_MARKER: &str = "Data/Core/About/About.xml";

/// The version file at the root of an install.
pub const VERSION_FILE: &str = "Version.txt";

/// The suffix of a macOS application bundle folder.
pub const BUNDLE_SUFFIX: &str = ".app";

/// The folder name RimWorld uses for its mods folder.
pub const MODS_DIR: &str = "Mods";

/// The folder name of the official data folders.
pub const DATA_DIR: &str = "Data";

/// Parses the text of `Version.txt` (for example `1.9.9999 rev1`).
///
/// A leading byte order mark and blank lines are ignored and only the first non blank line is read.
///
/// # Errors
/// [`CoreError::InvalidVersion`] when the text is empty or not a version.
pub fn parse_version_txt(text: &str) -> Result<GameVersion, CoreError> {
    let body = text.trim_start_matches('\u{feff}');
    let line = body
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    GameVersion::parse(line)
}

/// The executable names of the game for an OS, best first. On macOS the entry is the bundle folder.
pub fn executable_names(os: Os) -> &'static [&'static str] {
    match os {
        Os::Windows => &["RimWorldWin64.exe", "RimWorldWin.exe"],
        Os::MacOs => &["RimWorldMac.app"],
        Os::Linux | Os::Other => &[
            "RimWorldLinux",
            "start_RimWorld.sh",
            "start_RimWorld_openglfix.sh",
        ],
    }
}

/// The executable names of the Windows build, used to notice a Windows depot on another OS.
pub fn windows_executable_names() -> &'static [&'static str] {
    executable_names(Os::Windows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("1.9.9999 rev1\n", 1, 9, Some(9999), Some(1))]
    #[case("\u{feff}1.9.9999 rev12\r\n", 1, 9, Some(9999), Some(12))]
    #[case("\n\n  1.8.1234  \n", 1, 8, Some(1234), None)]
    #[case("1.9", 1, 9, None, None)]
    fn version_file_text_is_parsed(
        #[case] text: &str,
        #[case] major: u32,
        #[case] minor: u32,
        #[case] build: Option<u32>,
        #[case] rev: Option<u32>,
    ) {
        let v = parse_version_txt(text).unwrap();
        assert_eq!(
            (v.major, v.minor, v.build, v.rev),
            (major, minor, build, rev)
        );
    }

    #[rstest]
    #[case("")]
    #[case("   \n")]
    #[case("not a version")]
    fn bad_version_text_is_an_error(#[case] text: &str) {
        assert_eq!(
            parse_version_txt(text).unwrap_err().code(),
            "core.invalid-version"
        );
    }

    #[test]
    fn every_os_has_executable_names() {
        for os in [Os::Windows, Os::MacOs, Os::Linux, Os::Other] {
            assert!(!executable_names(os).is_empty());
        }
    }
}
