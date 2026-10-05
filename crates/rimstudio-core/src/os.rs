//! The operating system as data.
//!
//! Behaviour that depends on the OS is reached through ports (invariant I-11). This module covers the
//! cases where only a data decision is needed, for example which folder names to try or whether file
//! names are case sensitive by default. It uses `std::env::consts::OS`, a runtime constant, so no
//! `cfg(target_os)` appears here.

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

use crate::paths;

/// The operating systems RimStudio distinguishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Os {
    /// Microsoft Windows.
    Windows,
    /// Apple macOS.
    #[serde(rename = "macos")]
    MacOs,
    /// Linux and the Steam Deck.
    Linux,
    /// Anything else (treated like Linux for path rules).
    Other,
}

impl Os {
    /// The OS this binary is running on.
    pub fn current() -> Os {
        Os::from_rust_name(std::env::consts::OS)
    }

    /// Maps a `std::env::consts::OS` style name to an [`Os`].
    pub fn from_rust_name(name: &str) -> Os {
        match name {
            "windows" => Os::Windows,
            "macos" => Os::MacOs,
            "linux" => Os::Linux,
            _ => Os::Other,
        }
    }

    /// The stable lowercase name used in reports: `windows`, `macos`, `linux` or `other`.
    pub fn name(self) -> &'static str {
        match self {
            Os::Windows => "windows",
            Os::MacOs => "macos",
            Os::Linux => "linux",
            Os::Other => "other",
        }
    }

    /// True for every OS with Unix style paths.
    pub fn is_unix(self) -> bool {
        !matches!(self, Os::Windows)
    }

    /// The native directory separator.
    pub fn path_separator(self) -> char {
        if self == Os::Windows { '\\' } else { '/' }
    }

    /// Whether file names are case insensitive by default on this OS. macOS volumes can be formatted
    /// case sensitive, so callers that need certainty probe the volume instead.
    pub fn case_insensitive_default(self) -> bool {
        matches!(self, Os::Windows | Os::MacOs)
    }

    /// The suffix of executable file names (`.exe` on Windows, empty elsewhere).
    pub fn exe_suffix(self) -> &'static str {
        if self == Os::Windows { ".exe" } else { "" }
    }

    /// File names that may be the game executable on this OS, in probing order.
    pub fn game_executable_names(self) -> &'static [&'static str] {
        match self {
            Os::Windows => &["RimWorldWin64.exe", "RimWorldWin.exe"],
            Os::MacOs => &["RimWorldMac.app", "RimWorldMac"],
            Os::Linux | Os::Other => &[
                "RimWorldLinux",
                "start_RimWorld.sh",
                "start_RimWorld_openglfix.sh",
            ],
        }
    }

    /// Candidate user data folders of the game for this OS, best guess first, derived without any IO.
    ///
    /// `home` is the user's home directory. `config_home` is the value of `XDG_CONFIG_HOME` when set
    /// (only used on Linux). On Windows the folder lives below `AppData/LocalLow`, which is derived
    /// from `home` here; callers with a known folder API should prefer its answer.
    pub fn default_user_data_dirs(
        self,
        home: &Utf8Path,
        config_home: Option<&str>,
    ) -> Vec<Utf8PathBuf> {
        match self {
            Os::Windows => {
                vec![
                    home.join("AppData")
                        .join("LocalLow")
                        .join(paths::UNITY_COMPANY_DIR)
                        .join(paths::UNITY_PRODUCT_DIR),
                ]
            }
            Os::MacOs => vec![
                home.join("Library")
                    .join("Application Support")
                    .join("RimWorld"),
            ],
            Os::Linux | Os::Other => {
                let mut out = Vec::new();
                let config_base = match config_home {
                    Some(c) if !c.trim().is_empty() => Utf8PathBuf::from(c.trim()),
                    _ => home.join(".config"),
                };
                out.push(
                    config_base
                        .join("unity3d")
                        .join(paths::UNITY_COMPANY_DIR)
                        .join(paths::UNITY_PRODUCT_DIR),
                );
                let fallback = home
                    .join(".config")
                    .join("unity3d")
                    .join(paths::UNITY_COMPANY_DIR)
                    .join(paths::UNITY_PRODUCT_DIR);
                if !out.contains(&fallback) {
                    out.push(fallback);
                }
                out
            }
        }
    }
}

impl std::fmt::Display for Os {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("windows", Os::Windows)]
    #[case("macos", Os::MacOs)]
    #[case("linux", Os::Linux)]
    #[case("freebsd", Os::Other)]
    fn rust_names_map_to_variants(#[case] name: &str, #[case] expected: Os) {
        assert_eq!(Os::from_rust_name(name), expected);
    }

    #[test]
    fn current_matches_the_rust_constant() {
        assert_eq!(Os::current(), Os::from_rust_name(std::env::consts::OS));
    }

    #[test]
    fn serde_uses_kebab_names() {
        assert_eq!(serde_json::to_string(&Os::MacOs).unwrap(), "\"macos\"");
        assert_eq!(
            serde_json::from_str::<Os>("\"windows\"").unwrap(),
            Os::Windows
        );
    }

    #[test]
    fn case_rules_and_separators() {
        assert!(Os::Windows.case_insensitive_default());
        assert!(!Os::Linux.case_insensitive_default());
        assert_eq!(Os::Windows.path_separator(), '\\');
        assert_eq!(Os::Linux.path_separator(), '/');
        assert_eq!(Os::Windows.exe_suffix(), ".exe");
        assert!(Os::Linux.is_unix());
    }

    #[test]
    fn linux_user_dirs_honour_xdg_config_home_first() {
        let home = Utf8Path::new("/home/rs_user");
        let dirs = Os::Linux.default_user_data_dirs(home, Some("/cfg"));
        assert_eq!(dirs.len(), 2);
        assert!(dirs[0].starts_with("/cfg/unity3d"));
        assert!(dirs[1].starts_with("/home/rs_user/.config/unity3d"));
        let plain = Os::Linux.default_user_data_dirs(home, None);
        assert_eq!(plain.len(), 1);
    }

    #[test]
    fn windows_and_mac_user_dirs_are_below_home() {
        let w = Os::Windows.default_user_data_dirs(Utf8Path::new("C:/Users/rs_user"), None);
        assert!(w[0].as_str().contains("LocalLow"));
        let m = Os::MacOs.default_user_data_dirs(Utf8Path::new("/Users/rs_user"), None);
        assert!(m[0].as_str().ends_with("Application Support/RimWorld"));
    }
}
