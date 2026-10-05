//! A description of how the game is started, as plain data. This module never starts anything: the
//! launcher port in `rimstudio-core` and its real implementation in `rimstudio-platform` do that.
//!
//! A Steam install is best started through the Steam URL, because Steam sets up its runtime (and
//! Proton when it is the chosen compatibility tool). The direct executable is offered as a second
//! route, except under Proton, where only the Steam route works.

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

use crate::install::GAME_APP_ID;

/// The name of the command line switch that moves the game's user data folder.
pub const SAVE_DATA_SWITCH: &str = "-savedatafolder";

/// How to start the game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchDescription {
    /// The Steam URL that starts the game, `None` for installs that are not Steam installs.
    pub steam_url: Option<String>,
    /// The executable (or the macOS bundle) for a direct start, `None` when unknown or when the game
    /// must be started through Steam (Proton).
    pub executable: Option<Utf8PathBuf>,
    /// The working folder for a direct start.
    pub working_dir: Utf8PathBuf,
    /// Extra arguments for a direct start, in order.
    pub args: Vec<String>,
    /// True when the compatibility layer makes the Steam route the only valid one.
    pub steam_only: bool,
}

/// The Steam URL that starts an app: `steam://rungameid/<id>`.
pub fn steam_run_url(app_id: u64) -> String {
    format!("steam://rungameid/{app_id}")
}

/// The Steam URL that starts RimWorld.
pub fn game_run_url() -> String {
    steam_run_url(GAME_APP_ID)
}

/// Builds the description for an install.
///
/// `steam` is true for Steam installs, `proton` when the Windows build runs under Proton,
/// `executable` is the detected executable path, if any.
pub fn describe_launch(
    install_dir: &Utf8Path,
    steam: bool,
    proton: bool,
    executable: Option<&Utf8Path>,
) -> LaunchDescription {
    LaunchDescription {
        steam_url: steam.then(game_run_url),
        executable: if proton {
            None
        } else {
            executable.map(Utf8Path::to_path_buf)
        },
        working_dir: install_dir.to_path_buf(),
        args: Vec::new(),
        steam_only: steam && proton,
    }
}

impl LaunchDescription {
    /// Adds the switch that points the game at another user data folder (`-savedatafolder=<path>`).
    #[must_use]
    pub fn with_save_data_folder(mut self, folder: &Utf8Path) -> Self {
        self.args.retain(|a| !a.starts_with(SAVE_DATA_SWITCH));
        self.args.push(format!("{SAVE_DATA_SWITCH}={folder}"));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_steam_url_names_the_game() {
        assert_eq!(game_run_url(), "steam://rungameid/294100");
        assert_eq!(steam_run_url(7), "steam://rungameid/7");
    }

    #[test]
    fn steam_installs_get_both_routes_and_proton_only_the_url() {
        let dir = Utf8Path::new("/rs/game");
        let exe = Utf8PathBuf::from("/rs/game/RS_Exe");
        let native = describe_launch(dir, true, false, Some(&exe));
        assert!(native.steam_url.is_some());
        assert_eq!(native.executable.as_deref(), Some(exe.as_path()));
        assert!(!native.steam_only);
        let proton = describe_launch(dir, true, true, Some(&exe));
        assert!(proton.executable.is_none());
        assert!(proton.steam_only);
    }

    #[test]
    fn non_steam_installs_have_no_url() {
        let d = describe_launch(Utf8Path::new("/rs/g"), false, false, None);
        assert!(d.steam_url.is_none());
        assert!(d.executable.is_none());
    }

    #[test]
    fn the_save_data_switch_is_replaced_not_repeated() {
        let d = describe_launch(Utf8Path::new("/rs/g"), false, false, None)
            .with_save_data_folder(Utf8Path::new("/rs/a"))
            .with_save_data_folder(Utf8Path::new("/rs/b"));
        assert_eq!(d.args, vec!["-savedatafolder=/rs/b".to_owned()]);
    }
}
