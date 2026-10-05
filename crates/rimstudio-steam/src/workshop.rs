//! Workshop item status from `appworkshop_294100.acf` and the item folders on disk.
//!
//! Status is computed from the ACF and the folder list only (no network). The rules, in order of
//! precedence for an item the ACF knows:
//!
//! 1. `downloading`: the item details show `BytesDownloaded < BytesToDownload`, or the top level
//!    `NeedsUpdate` or `NeedsDownload` flag is set and this item is on disk but stale (a pending
//!    update); an item whose folder is missing stays `not-downloaded` whatever the flags say, so the
//!    subscribed but absent items remain visible;
//! 2. `not-downloaded`: the folder is missing;
//! 3. `update-available`: `latest_timeupdated` is newer than `timeupdated`, or `latest_manifest`
//!    differs from `manifest`;
//! 4. `current`.
//!
//! A numeric folder the ACF does not know is an `orphan`. The recorded `size` is never compared with
//! a folder size: it is only passed on as a change hint.

use std::collections::BTreeSet;
use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::load_plan::Listing;
use rimstudio_core::ports::{DetectEnv, EntryKind};
use serde::{Deserialize, Serialize};

use crate::acf::{AppWorkshop, InstalledItem, ItemDetails};
use crate::error::SteamResult;
use crate::vdf;

/// The status of one workshop item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ItemState {
    /// Nothing is pending.
    Current,
    /// The server has a newer version than the installed one.
    UpdateAvailable,
    /// A download is running or queued.
    Downloading,
    /// Subscribed, but the folder is missing.
    NotDownloaded,
    /// A folder Steam's list does not know.
    Orphan,
}

impl ItemState {
    /// The stable kebab-case name.
    pub fn name(self) -> &'static str {
        match self {
            ItemState::Current => "current",
            ItemState::UpdateAvailable => "update-available",
            ItemState::Downloading => "downloading",
            ItemState::NotDownloaded => "not-downloaded",
            ItemState::Orphan => "orphan",
        }
    }
}

/// The status of one item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemStatus {
    /// The published file id (the folder name).
    pub id: u64,
    /// The computed state.
    pub state: ItemState,
    /// True when the folder exists.
    pub on_disk: bool,
    /// The installed version time (seconds since the epoch), when known.
    pub installed_time_updated: Option<u64>,
    /// The newest version time the client knows, when known.
    pub latest_time_updated: Option<u64>,
    /// The size Steam recorded: a change hint, never a folder size.
    pub size_hint: Option<u64>,
}

/// What `item_status` needs to know about the folders on disk.
pub trait WorkshopFolders {
    /// True when the folder of the item exists.
    fn has_folder(&self, id: u64) -> bool;

    /// The ids of the numeric folders that exist.
    fn folder_ids(&self) -> Vec<u64>;
}

/// A [`WorkshopFolders`] over a [`Listing`] of the content folder.
///
/// The ids come from the first path component of every listed file, so a folder with no files at
/// all is not seen as an orphan (it is still found by `has_folder`).
pub struct ListingFolders<'a> {
    listing: &'a dyn Listing,
    dir: &'a Utf8Path,
}

impl<'a> ListingFolders<'a> {
    /// Wraps a listing and the content folder.
    pub fn new(listing: &'a dyn Listing, dir: &'a Utf8Path) -> Self {
        ListingFolders { listing, dir }
    }
}

impl WorkshopFolders for ListingFolders<'_> {
    fn has_folder(&self, id: u64) -> bool {
        self.listing.dir_exists(&self.dir.join(id.to_string()))
    }

    fn folder_ids(&self) -> Vec<u64> {
        let ids: BTreeSet<u64> = self
            .listing
            .list_files(self.dir)
            .iter()
            .filter_map(|p| p.split('/').next()?.parse().ok())
            .collect();
        ids.into_iter().collect()
    }
}

/// A [`WorkshopFolders`] that reads the content folder through a [`DetectEnv`].
pub struct EnvFolders {
    ids: BTreeSet<u64>,
}

impl EnvFolders {
    /// Lists the content folder once. An unreadable folder counts as empty.
    pub fn read(env: &dyn DetectEnv, dir: &Utf8Path, deadline: Duration) -> EnvFolders {
        let ids = env
            .fs()
            .read_dir(dir, deadline)
            .map(|entries| {
                entries
                    .iter()
                    .filter(|e| matches!(e.kind, EntryKind::Dir | EntryKind::Symlink))
                    .filter_map(|e| e.name.parse::<u64>().ok())
                    .collect()
            })
            .unwrap_or_default();
        EnvFolders { ids }
    }
}

impl WorkshopFolders for EnvFolders {
    fn has_folder(&self, id: u64) -> bool {
        self.ids.contains(&id)
    }

    fn folder_ids(&self) -> Vec<u64> {
        self.ids.iter().copied().collect()
    }
}

fn is_stale(installed: Option<&InstalledItem>, details: Option<&ItemDetails>) -> bool {
    let Some(d) = details else { return false };
    let installed_time = d
        .time_updated
        .or_else(|| installed.and_then(|i| i.time_updated));
    let installed_manifest = d
        .manifest
        .as_deref()
        .or_else(|| installed.and_then(|i| i.manifest.as_deref()));
    let newer_time = match (d.latest_time_updated, installed_time) {
        (Some(latest), Some(now)) => latest > now,
        _ => false,
    };
    let other_manifest = match (d.latest_manifest.as_deref(), installed_manifest) {
        (Some(latest), Some(now)) => latest != now,
        _ => false,
    };
    newer_time || other_manifest
}

/// Computes the status of every item the ACF knows plus every orphan folder, sorted by id.
pub fn item_status_with(workshop: &AppWorkshop, folders: &dyn WorkshopFolders) -> Vec<ItemStatus> {
    let mut known: BTreeSet<u64> = workshop.installed.iter().map(|i| i.id).collect();
    known.extend(workshop.details.iter().map(|d| d.id));
    let global_pending = workshop.needs_update || workshop.needs_download;

    let mut out: Vec<ItemStatus> = Vec::with_capacity(known.len());
    for id in &known {
        let installed = workshop.installed.iter().find(|i| i.id == *id);
        let details = workshop.details_of(*id);
        let on_disk = folders.has_folder(*id);
        let stale = is_stale(installed, details);
        let bytes_pending = details.is_some_and(|d| {
            matches!((d.bytes_downloaded, d.bytes_to_download), (Some(done), Some(total)) if done < total)
        });
        let state = if bytes_pending || (global_pending && on_disk && stale) {
            ItemState::Downloading
        } else if !on_disk {
            ItemState::NotDownloaded
        } else if stale {
            ItemState::UpdateAvailable
        } else {
            ItemState::Current
        };
        out.push(ItemStatus {
            id: *id,
            state,
            on_disk,
            installed_time_updated: details
                .and_then(|d| d.time_updated)
                .or_else(|| installed.and_then(|i| i.time_updated)),
            latest_time_updated: details.and_then(|d| d.latest_time_updated),
            size_hint: installed.and_then(|i| i.size),
        });
    }
    for id in folders.folder_ids() {
        if !known.contains(&id) {
            out.push(ItemStatus {
                id,
                state: ItemState::Orphan,
                on_disk: true,
                installed_time_updated: None,
                latest_time_updated: None,
                size_hint: None,
            });
        }
    }
    out.sort_by_key(|s| s.id);
    out
}

/// [`item_status_with`] over a [`Listing`] of the content folder `content_dir`.
pub fn item_status(
    workshop: &AppWorkshop,
    content_dir: &Utf8Path,
    listing: &dyn Listing,
) -> Vec<ItemStatus> {
    item_status_with(workshop, &ListingFolders::new(listing, content_dir))
}

/// Reads the ACF and the content folder through a [`DetectEnv`] and computes the statuses.
///
/// # Errors
/// A port error when the ACF cannot be read, a syntax or shape error when it cannot be understood.
pub fn status_from_env(
    env: &dyn DetectEnv,
    content_dir: &Utf8Path,
    acf_path: &Utf8Path,
    deadline: Duration,
) -> SteamResult<Vec<ItemStatus>> {
    let text = env.fs().read_to_string(acf_path, deadline)?;
    let workshop = AppWorkshop::from_doc(&vdf::parse(&text)?)?;
    let folders = EnvFolders::read(env, content_dir, deadline);
    Ok(item_status_with(&workshop, &folders))
}

/// The path of the workshop ACF of an app inside a library: `<lib>/steamapps/workshop/appworkshop_<id>.acf`.
pub fn acf_path(library: &Utf8Path, app_id: u64) -> Utf8PathBuf {
    library
        .join("steamapps")
        .join("workshop")
        .join(format!("appworkshop_{app_id}.acf"))
}

/// The content folder of an app inside a library: `<lib>/steamapps/workshop/content/<id>`.
pub fn content_dir(library: &Utf8Path, app_id: u64) -> Utf8PathBuf {
    library
        .join("steamapps")
        .join("workshop")
        .join("content")
        .join(app_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_core::load_plan::MemListing;

    const CONTENT: &str = "/rs/lib/steamapps/workshop/content/294100";

    fn acf(extra_top: &str, installed: &str, details: &str) -> AppWorkshop {
        AppWorkshop::parse(&format!(
            "\"AppWorkshop\" {{ \"appid\" \"294100\" {extra_top} \"WorkshopItemsInstalled\" {{ {installed} }} \"WorkshopItemDetails\" {{ {details} }} }}"
        ))
        .unwrap()
    }

    fn listing(ids: &[u64]) -> MemListing {
        let mut l = MemListing::new();
        for id in ids {
            l.add_file(&format!("{CONTENT}/{id}/About/About.xml"));
        }
        l
    }

    fn states(ws: &AppWorkshop, ids: &[u64]) -> Vec<(u64, ItemState)> {
        item_status(ws, Utf8Path::new(CONTENT), &listing(ids))
            .into_iter()
            .map(|s| (s.id, s.state))
            .collect()
    }

    #[test]
    fn current_stale_and_orphan_items() {
        let ws = acf(
            "",
            "\"111\" { \"size\" \"10\" \"timeupdated\" \"100\" \"manifest\" \"1\" } \"222\" { \"size\" \"10\" \"timeupdated\" \"100\" \"manifest\" \"2\" }",
            "\"111\" { \"manifest\" \"1\" \"timeupdated\" \"100\" \"latest_timeupdated\" \"100\" \"latest_manifest\" \"1\" } \"222\" { \"manifest\" \"2\" \"timeupdated\" \"100\" \"latest_timeupdated\" \"250\" \"latest_manifest\" \"3\" }",
        );
        assert_eq!(
            states(&ws, &[111, 222, 333]),
            vec![
                (111, ItemState::Current),
                (222, ItemState::UpdateAvailable),
                (333, ItemState::Orphan)
            ]
        );
    }

    #[test]
    fn an_installed_entry_without_a_folder_is_not_downloaded() {
        let ws = acf("", "\"5\" { \"size\" \"1\" }", "");
        assert_eq!(states(&ws, &[]), vec![(5, ItemState::NotDownloaded)]);
    }

    #[test]
    fn partial_bytes_mean_downloading() {
        let ws = acf(
            "",
            "\"5\" { }",
            "\"5\" { \"BytesToDownload\" \"9\" \"BytesDownloaded\" \"4\" }",
        );
        assert_eq!(states(&ws, &[5]), vec![(5, ItemState::Downloading)]);
        let done = acf(
            "",
            "\"5\" { }",
            "\"5\" { \"BytesToDownload\" \"9\" \"BytesDownloaded\" \"9\" }",
        );
        assert_eq!(states(&done, &[5]), vec![(5, ItemState::Current)]);
    }

    #[test]
    fn global_flags_only_touch_stale_items_that_are_on_disk() {
        let ws = acf(
            "\"NeedsUpdate\" \"1\"",
            "\"1\" { \"timeupdated\" \"5\" } \"2\" { \"timeupdated\" \"5\" } \"3\" { }",
            "\"2\" { \"timeupdated\" \"5\" \"latest_timeupdated\" \"9\" }",
        );
        assert_eq!(
            states(&ws, &[1, 2]),
            vec![
                (1, ItemState::Current),
                (2, ItemState::Downloading),
                (3, ItemState::NotDownloaded)
            ]
        );
    }

    #[test]
    fn precedence_downloading_then_missing_then_stale() {
        let ws = acf(
            "",
            "\"1\" { \"timeupdated\" \"5\" } \"2\" { \"timeupdated\" \"5\" }",
            "\"1\" { \"timeupdated\" \"5\" \"latest_timeupdated\" \"9\" \"BytesToDownload\" \"2\" \"BytesDownloaded\" \"1\" } \"2\" { \"timeupdated\" \"5\" \"latest_timeupdated\" \"9\" }",
        );
        // Item 1 is stale and downloading: downloading wins. Item 2 is stale and missing: missing wins.
        assert_eq!(
            states(&ws, &[1]),
            vec![(1, ItemState::Downloading), (2, ItemState::NotDownloaded)]
        );
    }

    #[test]
    fn the_recorded_size_is_only_a_hint() {
        let ws = acf("", "\"1\" { \"size\" \"3\" }", "");
        let s = item_status(&ws, Utf8Path::new(CONTENT), &listing(&[1]));
        assert_eq!(s[0].size_hint, Some(3));
        assert_eq!(s[0].state, ItemState::Current);
    }

    #[test]
    fn non_numeric_folders_are_ignored_and_output_is_sorted() {
        let ws = acf("", "\"30\" { } \"4\" { }", "");
        let mut l = listing(&[30, 4]);
        l.add_file(&format!("{CONTENT}/notes/readme.txt"));
        l.add_file(&format!("{CONTENT}/12/a.txt"));
        let ids: Vec<u64> = item_status(&ws, Utf8Path::new(CONTENT), &l)
            .iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(ids, vec![4, 12, 30]);
    }
}
