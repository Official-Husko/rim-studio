//! Looking up mods of the last scan for a form: by name, package id or author.
//!
//! [`search_mods`] wraps [`LibraryIndex::search`] and returns what a dependency picker needs in one row: the
//! package id and name to write into `About.xml`, the Workshop item id (the page address derives from it),
//! where the mod lives and whether the game can load it as it stands. It only reads the index it is given;
//! it never scans, so with no scan there is nothing to search and the caller says so.

use camino::Utf8PathBuf;
use rimstudio_core::ids::{ModIdx, SourceId, WorkshopId};

use crate::index::{LibraryIndex, Loadability};

/// The most rows [`search_mods`] returns whatever the caller asks for.
pub const MAX_SEARCH_ROWS: usize = 100;

/// One mod found by [`search_mods`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModSearchRow {
    /// The mod in the index.
    pub idx: ModIdx,
    /// The package id as the mod declares it.
    pub package_id: String,
    /// The display name.
    pub name: String,
    /// The authors.
    pub authors: Vec<String>,
    /// The source (folder list) the mod was found in.
    pub source: SourceId,
    /// The Workshop item id, when the folder name or `PublishedFileId.txt` gives one (an item the person
    /// published from a local folder has one too).
    pub workshop_id: Option<WorkshopId>,
    /// The mod folder.
    pub path: Utf8PathBuf,
    /// The game can load the folder as it is.
    pub loadable: bool,
    /// The `url` field of the mod's own About file.
    pub url: Option<String>,
    /// Higher is better.
    pub score: u32,
}

/// The Steam Workshop page of an item.
#[must_use]
pub fn workshop_url(id: WorkshopId) -> String {
    format!("https://steamcommunity.com/sharedfiles/filedetails/?id={id}")
}

/// Searches the package ids, names, authors and folder names of the index, best match first. Every word of
/// `query` must match. An empty query, or a limit of zero, gives no rows; `limit` is capped at
/// [`MAX_SEARCH_ROWS`].
#[must_use]
pub fn search_mods(index: &LibraryIndex, query: &str, limit: usize) -> Vec<ModSearchRow> {
    let limit = limit.min(MAX_SEARCH_ROWS);
    index
        .search(query, limit)
        .into_iter()
        .filter_map(|hit| {
            let meta = index.get(hit.idx)?;
            let info = index.info(hit.idx)?;
            Some(ModSearchRow {
                idx: hit.idx,
                package_id: meta.package_id.as_str().to_owned(),
                name: meta.name.clone(),
                authors: meta.authors.clone(),
                source: meta.source.clone(),
                workshop_id: meta.workshop_id.or(info.published_file_id),
                path: meta.path.clone(),
                loadable: info.loadable == Loadability::Loadable,
                url: meta.url.clone(),
                score: hit.score,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{AboutStatus, LibraryMod, ModContent, ModInfo};
    use rimstudio_core::ids::PackageId;
    use rimstudio_core::mods::ModMeta;

    fn item(
        pkg: &str,
        name: &str,
        path: &str,
        workshop: Option<u64>,
        loadable: bool,
    ) -> LibraryMod {
        let mut meta = ModMeta::new(
            PackageId::parse(pkg).unwrap_or_else(|_| unreachable!()),
            name,
            SourceId::game_mods(),
            Utf8PathBuf::from(path),
        );
        meta.authors = vec!["Tester".to_owned()];
        meta.workshop_id = workshop.and_then(WorkshopId::new);
        LibraryMod {
            meta,
            info: ModInfo {
                folder_name: path.rsplit('/').next().unwrap_or("").to_owned(),
                is_link: false,
                available: true,
                loadable: if loadable {
                    Loadability::Loadable
                } else {
                    Loadability::NeedsLink
                },
                about: AboutStatus::Parsed,
                synthetic_package_id: false,
                published_file_id: None,
                about_mtime_ns: None,
                also_in: Vec::new(),
            },
            content: ModContent::default(),
        }
    }

    fn sample() -> LibraryIndex {
        let mut b = LibraryIndex::builder();
        b.push(item("rs.alpha", "Alpha Guns", "/w/111", Some(111), true));
        b.push(item("rs.beta", "Beta", "/c/Beta", None, false));
        b.build()
    }

    #[test]
    fn rows_carry_what_a_dependency_needs() {
        let rows = search_mods(&sample(), "alpha", 10);
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.package_id, "rs.alpha");
        assert_eq!(row.name, "Alpha Guns");
        assert_eq!(row.workshop_id.map(WorkshopId::get), Some(111));
        assert!(row.loadable);
        assert_eq!(
            workshop_url(row.workshop_id.unwrap_or_else(|| unreachable!())),
            "https://steamcommunity.com/sharedfiles/filedetails/?id=111"
        );
        let rows = search_mods(&sample(), "beta", 10);
        assert!(!rows[0].loadable);
        assert_eq!(rows[0].workshop_id, None);
    }

    #[test]
    fn nothing_matches_an_empty_query_a_zero_limit_or_an_empty_index() {
        assert!(search_mods(&sample(), "  ", 10).is_empty());
        assert!(search_mods(&sample(), "alpha", 0).is_empty());
        assert!(search_mods(&LibraryIndex::builder().build(), "alpha", 10).is_empty());
        assert!(search_mods(&sample(), "zzz", 10).is_empty());
    }

    #[test]
    fn the_limit_is_capped() {
        let mut b = LibraryIndex::builder();
        for n in 0..150 {
            b.push(item(
                &format!("rs.m{n}"),
                "Many",
                &format!("/m/{n}"),
                None,
                true,
            ));
        }
        assert_eq!(search_mods(&b.build(), "many", 1000).len(), MAX_SEARCH_ROWS);
    }
}
