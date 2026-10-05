//! Typed, tolerant views over parsed Steam files: `libraryfolders.vdf`, `appmanifest_<id>.acf`,
//! `appworkshop_<id>.acf` and the compatibility tool table of `config.vdf`.
//!
//! Every optional field is optional: Steam changes these files between releases, so a missing key
//! never fails a view. Numbers are read from strings; a value that does not parse is treated as
//! absent. Account identifiers (`LastOwner`, `subscribedby`) are never stored: the manifest keeps
//! only a stable redacted tag.

use rimstudio_core::redact::Redactor;

use crate::error::{SteamError, SteamResult};
use crate::vdf::{self, VdfDoc, VdfObject, VdfValue};

/// Valve's app state bit for "uninstalled".
pub const STATE_UNINSTALLED: u64 = 1;
/// The app needs an update.
pub const STATE_UPDATE_REQUIRED: u64 = 2;
/// The app is fully installed.
pub const STATE_FULLY_INSTALLED: u64 = 4;
/// Files are missing.
pub const STATE_FILES_MISSING: u64 = 32;
/// The app is running.
pub const STATE_APP_RUNNING: u64 = 64;
/// Files are corrupt.
pub const STATE_FILES_CORRUPT: u64 = 128;
/// An update is running.
pub const STATE_UPDATE_RUNNING: u64 = 256;
/// An update is paused.
pub const STATE_UPDATE_PAUSED: u64 = 512;
/// An update has started.
pub const STATE_UPDATE_STARTED: u64 = 1024;

fn number(object: &VdfObject, key: &str) -> Option<u64> {
    object.get_str(key).and_then(|t| t.trim().parse().ok())
}

fn text(object: &VdfObject, key: &str) -> Option<String> {
    object.get_str(key).map(str::to_owned)
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

// ---------------------------------------------------------------------------------------------
// libraryfolders.vdf
// ---------------------------------------------------------------------------------------------

/// One app listed in a library's `apps` table, with the byte count Steam recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LibraryApp {
    /// The Steam app id.
    pub app_id: u64,
    /// The recorded size in bytes (a hint only).
    pub size: u64,
}

/// One library of `libraryfolders.vdf`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryEntry {
    /// The key of the entry (`0`, `1`, ...).
    pub index: String,
    /// The library folder as written, backslash escapes already decoded.
    pub path: String,
    /// The user label, empty when none.
    pub label: String,
    /// The content id when present.
    pub content_id: Option<String>,
    /// The total size recorded by Steam.
    pub total_size: Option<u64>,
    /// The apps table (may be stale; the app manifests are the truth).
    pub apps: Vec<LibraryApp>,
}

impl LibraryEntry {
    /// True when the apps table lists the app.
    pub fn lists_app(&self, app_id: u64) -> bool {
        self.apps.iter().any(|a| a.app_id == app_id)
    }
}

/// The libraries of a `libraryfolders.vdf`, in file order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LibraryFolders {
    /// The libraries.
    pub libraries: Vec<LibraryEntry>,
    /// How many entries were ignored for lacking a path.
    pub skipped: usize,
}

impl LibraryFolders {
    /// Reads the view from a parsed document.
    ///
    /// Both shapes are understood: an object per library with a `path` key, and the older
    /// `"1" "D:\\path"` string form.
    ///
    /// # Errors
    /// [`SteamError::Shape`] when the document has no `libraryfolders` object.
    pub fn from_doc(doc: &VdfDoc) -> SteamResult<LibraryFolders> {
        let root = doc
            .get_obj("libraryfolders")
            .ok_or_else(|| SteamError::shape("libraryfolders", "no libraryfolders object"))?;
        let mut out = LibraryFolders::default();
        for (key, value) in root.iter() {
            match value {
                VdfValue::Obj(obj) => match obj.get_str("path") {
                    Some(path) if !path.trim().is_empty() => {
                        out.libraries.push(LibraryEntry {
                            index: key.to_owned(),
                            path: path.to_owned(),
                            label: obj.get_str("label").unwrap_or("").to_owned(),
                            content_id: text(obj, "contentid"),
                            total_size: number(obj, "totalsize"),
                            apps: obj.get_obj("apps").map(read_apps).unwrap_or_default(),
                        });
                    }
                    _ => out.skipped += 1,
                },
                VdfValue::Str(path) if all_digits(key) => {
                    if path.trim().is_empty() {
                        out.skipped += 1;
                    } else {
                        out.libraries.push(LibraryEntry {
                            index: key.to_owned(),
                            path: path.clone(),
                            label: String::new(),
                            content_id: None,
                            total_size: None,
                            apps: Vec::new(),
                        });
                    }
                }
                VdfValue::Str(_) => {}
            }
        }
        Ok(out)
    }

    /// Parses text and reads the view.
    ///
    /// # Errors
    /// A syntax error or a shape error.
    pub fn parse(input: &str) -> SteamResult<LibraryFolders> {
        LibraryFolders::from_doc(&vdf::parse(input)?)
    }
}

fn read_apps(apps: &VdfObject) -> Vec<LibraryApp> {
    apps.iter()
        .filter_map(|(k, v)| {
            let app_id = k.trim().parse().ok()?;
            let size = v.as_str().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            Some(LibraryApp { app_id, size })
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// appmanifest
// ---------------------------------------------------------------------------------------------

/// The install state a manifest describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ManifestState {
    /// Fully installed, nothing pending.
    Installed,
    /// Steam is updating the app or has an update queued.
    UpdatePending,
    /// Not fully installed or files are missing or corrupt.
    NeedsVerify,
}

/// An installed depot of a manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Depot {
    /// The depot id.
    pub id: String,
    /// The manifest id (changes with every game update).
    pub manifest: Option<String>,
}

/// The view of `appmanifest_<id>.acf`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppManifest {
    /// The app id.
    pub app_id: u64,
    /// The app name.
    pub name: String,
    /// The `StateFlags` bit set, 0 when absent.
    pub state_flags: u64,
    /// The folder name under `steamapps/common`.
    pub install_dir: String,
    /// The installed build id (a string: Steam writes large numbers).
    pub build_id: Option<String>,
    /// The pending target build id.
    pub target_build_id: Option<String>,
    /// Seconds since the epoch of the last update.
    pub last_updated: Option<u64>,
    /// Seconds since the epoch of the last start.
    pub last_played: Option<u64>,
    /// The size Steam recorded for the install.
    pub size_on_disk: Option<u64>,
    /// A stable redacted tag of the owning account (`LastOwner` is never stored).
    pub last_owner_tag: Option<String>,
    /// The installed depots.
    pub depots: Vec<Depot>,
}

impl AppManifest {
    /// Reads the view from a parsed document.
    ///
    /// # Errors
    /// [`SteamError::Shape`] when `AppState` or a numeric `appid` is missing.
    pub fn from_doc(doc: &VdfDoc) -> SteamResult<AppManifest> {
        let state = doc
            .get_obj("AppState")
            .ok_or_else(|| SteamError::shape("appmanifest", "no AppState object"))?;
        let app_id = number(state, "appid")
            .ok_or_else(|| SteamError::shape("appmanifest", "missing or invalid appid"))?;
        let depots = state
            .get_obj("InstalledDepots")
            .map(|d| {
                d.iter()
                    .filter_map(|(id, v)| {
                        let obj = v.as_obj()?;
                        Some(Depot {
                            id: id.to_owned(),
                            manifest: text(obj, "manifest"),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(AppManifest {
            app_id,
            name: text(state, "name").unwrap_or_default(),
            state_flags: number(state, "StateFlags").unwrap_or(0),
            install_dir: text(state, "installdir").unwrap_or_default(),
            build_id: text(state, "buildid"),
            target_build_id: text(state, "TargetBuildID"),
            last_updated: number(state, "LastUpdated"),
            last_played: number(state, "LastPlayed"),
            size_on_disk: number(state, "SizeOnDisk"),
            last_owner_tag: state.get_str("LastOwner").map(|owner| {
                owner
                    .trim()
                    .parse::<u64>()
                    .map_or_else(|_| "steam-unknown".to_owned(), Redactor::steam_id_tag)
            }),
            depots,
        })
    }

    /// Parses text and reads the view.
    ///
    /// # Errors
    /// A syntax error or a shape error.
    pub fn parse(input: &str) -> SteamResult<AppManifest> {
        AppManifest::from_doc(&vdf::parse(input)?)
    }

    /// True when the fully installed bit is set.
    pub fn fully_installed(&self) -> bool {
        self.state_flags & STATE_FULLY_INSTALLED != 0
    }

    /// True when a target build is pending (nonzero and different from the installed build).
    pub fn has_pending_build(&self) -> bool {
        match (&self.target_build_id, &self.build_id) {
            (Some(target), Some(build)) => {
                let t = target.trim();
                !t.is_empty() && t != "0" && t != build.trim()
            }
            (Some(target), None) => {
                let t = target.trim();
                !t.is_empty() && t != "0"
            }
            _ => false,
        }
    }

    /// The install state derived from the flags and the build ids.
    pub fn state(&self) -> ManifestState {
        const UPDATING: u64 = STATE_UPDATE_REQUIRED
            | STATE_UPDATE_RUNNING
            | STATE_UPDATE_STARTED
            | STATE_UPDATE_PAUSED;
        if self.state_flags & UPDATING != 0 || self.has_pending_build() {
            ManifestState::UpdatePending
        } else if !self.fully_installed()
            || self.state_flags & (STATE_FILES_MISSING | STATE_FILES_CORRUPT) != 0
        {
            ManifestState::NeedsVerify
        } else {
            ManifestState::Installed
        }
    }
}

// ---------------------------------------------------------------------------------------------
// appworkshop
// ---------------------------------------------------------------------------------------------

/// One entry of `WorkshopItemsInstalled`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledItem {
    /// The published file id.
    pub id: u64,
    /// The recorded size (a change hint, never a folder size).
    pub size: Option<u64>,
    /// Seconds since the epoch of the installed version.
    pub time_updated: Option<u64>,
    /// The installed manifest id.
    pub manifest: Option<String>,
}

/// One entry of `WorkshopItemDetails`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDetails {
    /// The published file id.
    pub id: u64,
    /// The manifest id of the installed content.
    pub manifest: Option<String>,
    /// Seconds since the epoch of the installed version.
    pub time_updated: Option<u64>,
    /// The newest update time the client learned from the server.
    pub latest_time_updated: Option<u64>,
    /// The newest manifest id the client learned from the server.
    pub latest_manifest: Option<String>,
    /// Bytes still to download, present during a download.
    pub bytes_to_download: Option<u64>,
    /// Bytes downloaded so far.
    pub bytes_downloaded: Option<u64>,
}

/// The view of `appworkshop_<id>.acf`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AppWorkshop {
    /// The app id when present.
    pub app_id: Option<u64>,
    /// The top level `NeedsUpdate` flag.
    pub needs_update: bool,
    /// The top level `NeedsDownload` flag.
    pub needs_download: bool,
    /// Seconds since the epoch of the last update check.
    pub time_last_updated: Option<u64>,
    /// The installed items in file order.
    pub installed: Vec<InstalledItem>,
    /// The item details in file order.
    pub details: Vec<ItemDetails>,
}

impl AppWorkshop {
    /// Reads the view from a parsed document.
    ///
    /// # Errors
    /// [`SteamError::Shape`] when the document has no `AppWorkshop` object.
    pub fn from_doc(doc: &VdfDoc) -> SteamResult<AppWorkshop> {
        let root = doc
            .get_obj("AppWorkshop")
            .ok_or_else(|| SteamError::shape("appworkshop", "no AppWorkshop object"))?;
        let installed = root
            .get_obj("WorkshopItemsInstalled")
            .map(|items| {
                items
                    .iter()
                    .filter_map(|(k, v)| {
                        let id = k.trim().parse().ok()?;
                        let o = v.as_obj()?;
                        Some(InstalledItem {
                            id,
                            size: number(o, "size"),
                            time_updated: number(o, "timeupdated"),
                            manifest: text(o, "manifest"),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let details = root
            .get_obj("WorkshopItemDetails")
            .map(|items| {
                items
                    .iter()
                    .filter_map(|(k, v)| {
                        let id = k.trim().parse().ok()?;
                        let o = v.as_obj()?;
                        Some(ItemDetails {
                            id,
                            manifest: text(o, "manifest"),
                            time_updated: number(o, "timeupdated"),
                            latest_time_updated: number(o, "latest_timeupdated"),
                            latest_manifest: text(o, "latest_manifest"),
                            bytes_to_download: number(o, "BytesToDownload"),
                            bytes_downloaded: number(o, "BytesDownloaded"),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(AppWorkshop {
            app_id: number(root, "appid"),
            needs_update: number(root, "NeedsUpdate").unwrap_or(0) != 0,
            needs_download: number(root, "NeedsDownload").unwrap_or(0) != 0,
            time_last_updated: number(root, "TimeLastUpdated"),
            installed,
            details,
        })
    }

    /// Parses text and reads the view.
    ///
    /// # Errors
    /// A syntax error or a shape error.
    pub fn parse(input: &str) -> SteamResult<AppWorkshop> {
        AppWorkshop::from_doc(&vdf::parse(input)?)
    }

    /// The details of an item, the first entry with that id.
    pub fn details_of(&self, id: u64) -> Option<&ItemDetails> {
        self.details.iter().find(|d| d.id == id)
    }
}

// ---------------------------------------------------------------------------------------------
// config.vdf
// ---------------------------------------------------------------------------------------------

/// The compatibility tool Steam maps to an app, read from `config/config.vdf`.
///
/// Returns `Some(name)` (possibly empty) when the table has an entry for the app and `None`
/// otherwise. Only this one table is read; nothing else of the file is interpreted.
pub fn compat_tool(doc: &VdfDoc, app_id: u64) -> Option<String> {
    let mapping = doc
        .root
        .lookup(&[
            "InstallConfigStore",
            "Software",
            "Valve",
            "Steam",
            "CompatToolMapping",
        ])?
        .as_obj()?;
    let entry = mapping.get(&app_id.to_string())?;
    Some(
        entry
            .as_obj()
            .and_then(|o| o.get_str("name"))
            .unwrap_or("")
            .to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIBS: &str = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"/rs/steam"
		"label"		"RS main"
		"contentid"		"123"
		"totalsize"		"1000"
		"apps"
		{
			"294100"		"500"
			"10"		"1"
		}
	}
	"1"		"D:\\RS Library"
	"2"
	{
		"label"		"no path"
	}
	"contentstatsid"		"55"
}
"#;

    #[test]
    fn library_folders_reads_both_shapes_and_skips_pathless_entries() {
        let l = LibraryFolders::parse(LIBS).unwrap();
        assert_eq!(l.libraries.len(), 2);
        assert_eq!(l.skipped, 1);
        let first = &l.libraries[0];
        assert_eq!(first.path, "/rs/steam");
        assert_eq!(first.label, "RS main");
        assert_eq!(first.total_size, Some(1000));
        assert!(first.lists_app(294_100));
        assert!(!first.lists_app(7));
        assert_eq!(l.libraries[1].path, "D:\\RS Library");
        assert!(l.libraries[1].apps.is_empty());
    }

    #[test]
    fn library_folders_without_root_is_a_shape_error() {
        let err = LibraryFolders::parse("\"other\" {}").unwrap_err();
        assert_eq!(err.code(), "steam.shape");
        assert_eq!(
            LibraryFolders::parse("\"a\"").unwrap_err().code(),
            "steam.vdf-missing-value"
        );
    }

    const MANIFEST: &str = r#"
"AppState"
{
	"appid"		"294100"
	"name"		"RS_TestGame"
	"StateFlags"		"4"
	"installdir"		"RS_Game"
	"LastUpdated"		"1700000000"
	"SizeOnDisk"		"42"
	"buildid"		"1000"
	"LastOwner"		"76561190000000001"
	"TargetBuildID"		"0"
	"InstalledDepots"
	{
		"294103"
		{
			"manifest"		"999"
			"size"		"42"
		}
	}
}
"#;

    #[test]
    fn manifest_view_reads_fields_and_never_keeps_the_account_id() {
        let m = AppManifest::parse(MANIFEST).unwrap();
        assert_eq!(m.app_id, 294_100);
        assert_eq!(m.install_dir, "RS_Game");
        assert_eq!(m.build_id.as_deref(), Some("1000"));
        assert_eq!(m.depots[0].id, "294103");
        assert_eq!(m.depots[0].manifest.as_deref(), Some("999"));
        assert_eq!(m.state(), ManifestState::Installed);
        let tag = m.last_owner_tag.clone().unwrap();
        assert!(tag.starts_with("steam-"));
        assert!(!format!("{m:?}").contains("76561190000000001"));
    }

    #[test]
    fn manifest_without_appid_is_a_shape_error() {
        let err = AppManifest::parse("\"AppState\" { \"name\" \"x\" }").unwrap_err();
        assert_eq!(err.code(), "steam.shape");
    }

    fn with(flags: u64, build: &str, target: &str) -> AppManifest {
        let mut m = AppManifest::parse(MANIFEST).unwrap();
        m.state_flags = flags;
        m.build_id = Some(build.into());
        m.target_build_id = Some(target.into());
        m
    }

    #[test]
    fn manifest_state_follows_flags_and_builds() {
        assert_eq!(with(4, "1", "0").state(), ManifestState::Installed);
        assert_eq!(with(4, "1", "1").state(), ManifestState::Installed);
        assert_eq!(with(4, "1", "2").state(), ManifestState::UpdatePending);
        assert_eq!(with(6, "1", "0").state(), ManifestState::UpdatePending);
        assert_eq!(
            with(4 | 256, "1", "0").state(),
            ManifestState::UpdatePending
        );
        assert_eq!(with(2 | 4, "1", "1").state(), ManifestState::UpdatePending);
        assert_eq!(with(4 | 32, "1", "0").state(), ManifestState::NeedsVerify);
        assert_eq!(with(4 | 128, "1", "0").state(), ManifestState::NeedsVerify);
        assert_eq!(with(1, "1", "0").state(), ManifestState::NeedsVerify);
    }

    const WORKSHOP: &str = r#"
"AppWorkshop"
{
	"appid"		"294100"
	"NeedsUpdate"		"1"
	"WorkshopItemsInstalled"
	{
		"111"	{ "size" "10" "timeupdated" "100" "manifest" "1" }
		"bad"	{ "size" "10" }
	}
	"WorkshopItemDetails"
	{
		"111"	{ "manifest" "1" "timeupdated" "100" "latest_timeupdated" "250" "latest_manifest" "3" "BytesToDownload" "9" "BytesDownloaded" "4" "subscribedby" "77" }
	}
}
"#;

    #[test]
    fn workshop_view_reads_items_and_ignores_bad_ids() {
        let w = AppWorkshop::parse(WORKSHOP).unwrap();
        assert!(w.needs_update);
        assert!(!w.needs_download);
        assert_eq!(w.installed.len(), 1);
        assert_eq!(w.installed[0].size, Some(10));
        let d = w.details_of(111).unwrap();
        assert_eq!(d.latest_time_updated, Some(250));
        assert_eq!(d.bytes_to_download, Some(9));
        assert!(w.details_of(5).is_none());
        assert!(AppWorkshop::parse("\"x\" {}").is_err());
    }

    #[test]
    fn compat_tool_reads_only_the_mapping_for_the_app() {
        let doc = vdf::parse(
            "\"InstallConfigStore\" { \"Software\" { \"Valve\" { \"Steam\" { \"CompatToolMapping\" { \"0\" { \"name\" \"default_rs\" } \"294100\" { \"name\" \"rs_tool\" } \"5\" { } } } } } }",
        )
        .unwrap();
        assert_eq!(compat_tool(&doc, 294_100).as_deref(), Some("rs_tool"));
        assert_eq!(compat_tool(&doc, 5).as_deref(), Some(""));
        assert_eq!(compat_tool(&doc, 6), None);
        assert_eq!(compat_tool(&VdfDoc::default(), 1), None);
    }
}
