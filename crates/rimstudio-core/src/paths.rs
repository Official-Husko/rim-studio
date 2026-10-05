//! RimWorld path and file name constants.
//!
//! These are names of files and folders defined by the game itself, not values read from it. They are
//! the only place where `.xml` file names of RimWorld appear as string literals (invariant I-02);
//! reading and writing those files is done by `rimstudio-xml`. Join helpers use forward slashes, which
//! `camino` and every supported OS accept.

// Folder names inside a mod.

/// The folder that holds a mod's metadata, matched case sensitively by the game.
pub const ABOUT_DIR: &str = "About";
/// The folder with definition files.
pub const DEFS_DIR: &str = "Defs";
/// The folder with patch files.
pub const PATCHES_DIR: &str = "Patches";
/// The folder with managed assemblies.
pub const ASSEMBLIES_DIR: &str = "Assemblies";
/// The folder with textures.
pub const TEXTURES_DIR: &str = "Textures";
/// The folder with sounds.
pub const SOUNDS_DIR: &str = "Sounds";
/// The folder with translations.
pub const LANGUAGES_DIR: &str = "Languages";
/// The shared content folder consulted by the implicit load rules.
pub const COMMON_DIR: &str = "Common";

// Files inside a mod.

/// The metadata file name, matched case insensitively inside `About`.
pub const ABOUT_XML_NAME: &str = "About.xml";
/// The metadata file relative to the mod root.
pub const ABOUT_XML: &str = "About/About.xml";
/// The load folders file name, matched case insensitively in the mod root.
pub const LOAD_FOLDERS_XML: &str = "LoadFolders.xml";
/// The Workshop id file relative to the mod root.
pub const PUBLISHED_FILE_ID_TXT: &str = "About/PublishedFileId.txt";
/// The Workshop preview image relative to the mod root.
pub const PREVIEW_PNG: &str = "About/Preview.png";
/// The default mod list icon relative to the mod root.
pub const MOD_ICON_PNG: &str = "About/ModIcon.png";
/// A tool side manifest the game does not read.
pub const MANIFEST_XML: &str = "About/Manifest.xml";
/// The extension (without the dot) of definition and patch files.
pub const XML_EXT: &str = "xml";
/// The extension (without the dot) of managed assemblies.
pub const DLL_EXT: &str = "dll";

// The install.

/// The game's Steam application id.
pub const STEAM_APP_ID: u32 = 294_100;
/// The install folder holding Core and the expansions.
pub const DATA_DIR: &str = "Data";
/// The install folder holding user installed mods.
pub const MODS_DIR: &str = "Mods";
/// The file with the installed game version, next to the executable.
pub const VERSION_TXT: &str = "Version.txt";
/// The folder of the base game inside `Data`.
pub const CORE_DIR: &str = "Core";

// The user data folder.

/// The configuration folder inside the user data folder.
pub const CONFIG_DIR: &str = "Config";
/// The active mod list file name inside [`CONFIG_DIR`].
pub const MODS_CONFIG_XML_NAME: &str = "ModsConfig.xml";
/// The active mod list file relative to the user data folder.
pub const MODS_CONFIG: &str = "Config/ModsConfig.xml";
/// The game log file name inside the user data folder.
pub const PLAYER_LOG: &str = "Player.log";
/// The saves folder inside the user data folder.
pub const SAVES_DIR: &str = "Saves";
/// The Unity company folder name below the engine's data path.
pub const UNITY_COMPANY_DIR: &str = "Ludeon Studios";
/// The Unity product folder name below the company folder.
pub const UNITY_PRODUCT_DIR: &str = "RimWorld by Ludeon Studios";

// Steam.

/// Where Workshop content of the game lives, relative to a Steam library root.
pub const WORKSHOP_CONTENT_SUBPATH: &str = "steamapps/workshop/content/294100";
/// The Workshop state file, relative to a Steam library root.
pub const WORKSHOP_ACF: &str = "steamapps/workshop/appworkshop_294100.acf";
/// The application manifest file name inside `steamapps`.
pub const APP_MANIFEST_ACF: &str = "appmanifest_294100.acf";
/// The library list file name inside `steamapps`.
pub const LIBRARY_FOLDERS_VDF: &str = "libraryfolders.vdf";

// Official packages (package ids are the game's own identifiers, lower case).

/// The package id of the base game.
pub const CORE_PACKAGE_ID: &str = "ludeon.rimworld";
/// The prefix shared by the package ids of the official expansions.
pub const EXPANSION_PACKAGE_PREFIX: &str = "ludeon.rimworld.";
/// Package ids of the official expansions in release order.
pub const EXPANSION_PACKAGE_IDS: [&str; 5] = [
    "ludeon.rimworld.royalty",
    "ludeon.rimworld.ideology",
    "ludeon.rimworld.biotech",
    "ludeon.rimworld.anomaly",
    "ludeon.rimworld.odyssey",
];

/// True when `package_id` (compared ignoring case) names the base game or an official expansion.
pub fn is_official_package_id(package_id: &str) -> bool {
    let lower = package_id.trim().to_lowercase();
    lower == CORE_PACKAGE_ID || EXPANSION_PACKAGE_IDS.contains(&lower.as_str())
}

/// The `Data` sub folder name for an official package id, for example `Royalty` for
/// `ludeon.rimworld.royalty`, and `Core` for the base game. `None` for any other id.
pub fn official_data_folder_name(package_id: &str) -> Option<&'static str> {
    let lower = package_id.trim().to_lowercase();
    if lower == CORE_PACKAGE_ID {
        return Some(CORE_DIR);
    }
    match lower.as_str() {
        "ludeon.rimworld.royalty" => Some("Royalty"),
        "ludeon.rimworld.ideology" => Some("Ideology"),
        "ludeon.rimworld.biotech" => Some("Biotech"),
        "ludeon.rimworld.anomaly" => Some("Anomaly"),
        "ludeon.rimworld.odyssey" => Some("Odyssey"),
        _ => None,
    }
}

// Names used by RimStudio itself next to the game's files.

/// The folder name RimStudio uses for generated Combat Extended patch content inside a mod. The
/// folder is only made visible to the game through a `LoadFolders` entry that is conditional on the
/// Combat Extended package being active.
pub const CE_PATCH_FOLDER_NAME: &str = "CombatExtended";
/// The Combat Extended package id used in `IfModActive` conditions.
pub const CE_PACKAGE_ID: &str = "ceteam.combatextended";

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("ludeon.rimworld", true)]
    #[case("Ludeon.RimWorld.Royalty", true)]
    #[case("  ludeon.rimworld.odyssey ", true)]
    #[case("rs.fictional.mod", false)]
    #[case("ludeon.rimworld.fake", false)]
    fn official_ids_are_recognised_ignoring_case(#[case] id: &str, #[case] expected: bool) {
        assert_eq!(is_official_package_id(id), expected);
    }

    #[test]
    fn official_data_folders_map_by_id() {
        assert_eq!(official_data_folder_name("ludeon.rimworld"), Some("Core"));
        assert_eq!(
            official_data_folder_name("LUDEON.RIMWORLD.BIOTECH"),
            Some("Biotech")
        );
        assert_eq!(official_data_folder_name("rs.mod"), None);
    }

    #[test]
    fn relative_constants_use_forward_slashes_only() {
        for c in [
            ABOUT_XML,
            PUBLISHED_FILE_ID_TXT,
            PREVIEW_PNG,
            MOD_ICON_PNG,
            MODS_CONFIG,
            WORKSHOP_CONTENT_SUBPATH,
            WORKSHOP_ACF,
        ] {
            assert!(!c.contains('\\'), "{c}");
        }
    }

    #[test]
    fn expansion_ids_share_the_prefix() {
        assert!(
            EXPANSION_PACKAGE_IDS
                .iter()
                .all(|id| id.starts_with(EXPANSION_PACKAGE_PREFIX))
        );
    }
}
