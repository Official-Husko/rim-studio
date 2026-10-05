//! The library index: the core [`ModIndex`] plus what the scanner learned beyond `About.xml`.
//!
//! [`LibraryIndex`] dereferences to [`ModIndex`], so every lookup of the core index works on it
//! (`get`, `by_package_id`, `by_mod_id`, `iter`). On top it keeps one [`ModInfo`] and one
//! [`ModContent`] per mod, addressed by the same [`ModIdx`], a path lookup, and a small search
//! helper by name and package id. Everything is deterministic: the order is the core index order
//! (lowercase package id, then source, then path).

use std::ops::Deref;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::ids::{ModIdx, SourceId, WorkshopId};
use rimstudio_core::mods::{ModIndex, ModIndexBuilder, ModMeta};
use rimstudio_xml::defs_scan::DefIndexRecord;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

/// Whether the game can load a mod as it stands.
///
/// The game reads only its own `Mods` folder, the install `Data` folder and the Steam Workshop
/// folder. A mod that lives only in a custom folder is not loadable until it is linked or copied
/// into `Mods` (owner decision; the library itself never writes into game folders).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Loadability {
    /// The game sees the folder.
    Loadable,
    /// The folder is in a custom source and is not visible to the game until it is linked.
    NeedsLink,
}

/// How reading `About.xml` went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AboutStatus {
    /// The file parsed (possibly with warnings).
    Parsed,
    /// The file exists but could not be parsed or read; the mod is listed with defaults.
    Unparsed,
}

/// What the scanner knows about a mod besides its [`ModMeta`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModInfo {
    /// The name of the mod folder.
    pub folder_name: String,
    /// True when the mod root is a link to a folder.
    pub is_link: bool,
    /// False for a row restored from the cache because its source is offline.
    pub available: bool,
    /// Whether the game can load the folder as it is.
    pub loadable: Loadability,
    /// How reading `About.xml` went.
    pub about: AboutStatus,
    /// True when the package id was made up because `About.xml` has none.
    pub synthetic_package_id: bool,
    /// The id written in `About/PublishedFileId.txt`, when the file holds one.
    pub published_file_id: Option<WorkshopId>,
    /// Modification time of `About.xml` in nanoseconds since the epoch.
    pub about_mtime_ns: Option<i128>,
    /// Other sources that reach the same folder (it is listed once, under the first source).
    pub also_in: Vec<SourceId>,
}

/// The definitions of one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefFile {
    /// The path relative to the mod root, `/` separated.
    pub rel_path: String,
    /// The file size in bytes.
    pub size: u64,
    /// One record per definition. Shared with the manifest, so reusing a cached file copies nothing.
    #[serde(with = "arc_records")]
    pub records: Arc<Vec<DefIndexRecord>>,
}

/// Serde support for the shared record list (a plain JSON array on the wire).
mod arc_records {
    use std::sync::Arc;

    use rimstudio_xml::defs_scan::DefIndexRecord;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(super) fn serialize<S: Serializer>(
        records: &Arc<Vec<DefIndexRecord>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        records.as_slice().serialize(serializer)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Arc<Vec<DefIndexRecord>>, D::Error> {
        Vec::<DefIndexRecord>::deserialize(deserializer).map(Arc::new)
    }
}

/// The level 1 view of a mod: content folders, markers and the definition index.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModContent {
    /// The folders (relative to the mod root, empty for the root) searched for content.
    pub folders: Vec<String>,
    /// Definition files with their indexed definitions, sorted by path.
    pub def_files: Vec<DefFile>,
    /// Patch files (relative paths, not parsed at this level), sorted.
    pub patch_files: Vec<String>,
    /// Language folders found below `Languages`, as `folder/Languages/Name`, sorted.
    pub languages: Vec<String>,
    /// Assembly files found directly inside `Assemblies` folders, as relative paths, sorted.
    pub assemblies: Vec<String>,
    /// False when level 1 did not run or was cancelled before it finished this mod.
    pub complete: bool,
}

impl ModContent {
    /// The number of definitions across all files.
    pub fn def_count(&self) -> usize {
        self.def_files.iter().map(|f| f.records.len()).sum()
    }

    /// True when the mod ships at least one assembly.
    pub fn has_assemblies(&self) -> bool {
        !self.assemblies.is_empty()
    }
}

/// One mod as the builder takes it.
#[derive(Debug, Clone)]
pub struct LibraryMod {
    /// The `About.xml` data and location.
    pub meta: ModMeta,
    /// The scan facts.
    pub info: ModInfo,
    /// The level 1 data.
    pub content: ModContent,
}

/// Collects mods and builds a [`LibraryIndex`].
#[derive(Debug, Default)]
pub struct LibraryIndexBuilder {
    mods: Vec<LibraryMod>,
}

impl LibraryIndexBuilder {
    /// An empty builder.
    pub fn new() -> Self {
        LibraryIndexBuilder::default()
    }

    /// Adds a mod.
    pub fn push(&mut self, item: LibraryMod) -> &mut Self {
        self.mods.push(item);
        self
    }

    /// Builds the index. A second mod with the same path replaces the first.
    pub fn build(self) -> LibraryIndex {
        let mut by_path: FxHashMap<Utf8PathBuf, LibraryMod> = FxHashMap::default();
        for item in self.mods {
            by_path.insert(item.meta.path.clone(), item);
        }
        let mut core = ModIndexBuilder::new();
        for item in by_path.values() {
            core.push(item.meta.clone());
        }
        let core = core.build();
        let mut infos = Vec::with_capacity(core.len());
        let mut contents = Vec::with_capacity(core.len());
        let mut paths: FxHashMap<Utf8PathBuf, ModIdx> = FxHashMap::default();
        for (idx, meta) in core.iter() {
            if let Some(item) = by_path.remove(&meta.path) {
                infos.push(item.info);
                contents.push(item.content);
                paths.insert(meta.path.clone(), idx);
            }
        }
        LibraryIndex {
            core,
            infos,
            contents,
            by_path: paths,
        }
    }
}

/// The result of a scan: the core index with the scanner's extra data.
#[derive(Debug, Clone, Default)]
pub struct LibraryIndex {
    core: ModIndex,
    infos: Vec<ModInfo>,
    contents: Vec<ModContent>,
    by_path: FxHashMap<Utf8PathBuf, ModIdx>,
}

impl Deref for LibraryIndex {
    type Target = ModIndex;

    fn deref(&self) -> &ModIndex {
        &self.core
    }
}

/// Which field a search hit matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchField {
    /// The package id.
    PackageId,
    /// The display name.
    Name,
    /// An author.
    Author,
    /// The folder name.
    Folder,
}

/// One search result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    /// The matching mod.
    pub idx: ModIdx,
    /// Higher is better.
    pub score: u32,
    /// The field that produced the score.
    pub field: SearchField,
}

impl LibraryIndex {
    /// Starts a builder.
    pub fn builder() -> LibraryIndexBuilder {
        LibraryIndexBuilder::new()
    }

    /// The core index.
    pub fn core(&self) -> &ModIndex {
        &self.core
    }

    /// The scan facts of a mod.
    pub fn info(&self, idx: ModIdx) -> Option<&ModInfo> {
        self.infos.get(idx.index())
    }

    /// The level 1 data of a mod.
    pub fn content(&self, idx: ModIdx) -> Option<&ModContent> {
        self.contents.get(idx.index())
    }

    /// The mod whose root is exactly `path`.
    pub fn by_path(&self, path: &Utf8Path) -> Option<ModIdx> {
        self.by_path.get(path).copied()
    }

    /// Every mod with its metadata, scan facts and content, in index order.
    pub fn iter_full(&self) -> impl Iterator<Item = (ModIdx, &ModMeta, &ModInfo, &ModContent)> {
        self.core.iter().filter_map(|(idx, meta)| {
            let info = self.infos.get(idx.index())?;
            let content = self.contents.get(idx.index())?;
            Some((idx, meta, info, content))
        })
    }

    /// The number of mods the game could load as they stand.
    pub fn loadable_count(&self) -> usize {
        self.infos
            .iter()
            .filter(|i| i.loadable == Loadability::Loadable)
            .count()
    }

    /// The total number of indexed definitions.
    pub fn def_count(&self) -> usize {
        self.contents.iter().map(ModContent::def_count).sum()
    }

    /// Searches package ids, names, authors and folder names, case insensitively.
    ///
    /// The query is split on white space and every word must match somewhere. The score favours an
    /// exact package id, then a package id or name prefix, then a word prefix, then a substring.
    /// Results are sorted by score (best first) and then by index, so equal input gives equal output.
    /// An empty query returns nothing.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        if words.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut hits: Vec<SearchHit> = Vec::new();
        for (idx, meta, info, _) in self.iter_full() {
            let mut total = 0u32;
            let mut best_field = SearchField::Folder;
            let mut best_word = 0u32;
            let mut all = true;
            for word in &words {
                match score_word(word, meta, info) {
                    Some((score, field)) => {
                        total = total.saturating_add(score);
                        if score > best_word {
                            best_word = score;
                            best_field = field;
                        }
                    }
                    None => {
                        all = false;
                        break;
                    }
                }
            }
            if all {
                hits.push(SearchHit {
                    idx,
                    score: total,
                    field: best_field,
                });
            }
        }
        hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.idx.cmp(&b.idx)));
        hits.truncate(limit);
        hits
    }
}

fn text_score(word: &str, text: &str, exact: u32, prefix: u32, word_prefix: u32, sub: u32) -> u32 {
    let lower = text.to_lowercase();
    if lower == word {
        exact
    } else if lower.starts_with(word) {
        prefix
    } else if lower
        .split(|c: char| !c.is_alphanumeric())
        .any(|part| !part.is_empty() && part.starts_with(word))
    {
        word_prefix
    } else if lower.contains(word) {
        sub
    } else {
        0
    }
}

fn score_word(word: &str, meta: &ModMeta, info: &ModInfo) -> Option<(u32, SearchField)> {
    let mut best: Option<(u32, SearchField)> = None;
    let mut consider = |score: u32, field: SearchField| {
        if score > 0 && best.is_none_or(|(s, _)| score > s) {
            best = Some((score, field));
        }
    };
    consider(
        text_score(word, meta.package_id.lower(), 1000, 600, 300, 150),
        SearchField::PackageId,
    );
    consider(
        text_score(word, &meta.name, 900, 500, 250, 120),
        SearchField::Name,
    );
    for author in &meta.authors {
        consider(
            text_score(word, author, 200, 100, 80, 60),
            SearchField::Author,
        );
    }
    consider(
        text_score(word, &info.folder_name, 150, 90, 70, 50),
        SearchField::Folder,
    );
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_core::ids::PackageId;

    fn item(pkg: &str, name: &str, path: &str, author: &str) -> LibraryMod {
        let mut meta = ModMeta::new(
            PackageId::parse(pkg).unwrap(),
            name,
            SourceId::game_mods(),
            Utf8PathBuf::from(path),
        );
        meta.authors = vec![author.to_owned()];
        LibraryMod {
            meta,
            info: ModInfo {
                folder_name: path.rsplit('/').next().unwrap().to_owned(),
                is_link: false,
                available: true,
                loadable: Loadability::Loadable,
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
        b.push(item("rs.alpha.guns", "RS Alpha Guns", "/m/alpha", "Tester"));
        b.push(item("rs.beta", "Beta Armor Pack", "/m/beta", "Someone"));
        b.push(item("rs.gamma", "Gamma", "/m/Gamma Guns", "Tester Two"));
        b.build()
    }

    #[test]
    fn index_derefs_to_core_and_keeps_extras_aligned() {
        let idx = sample();
        assert_eq!(idx.len(), 3);
        for (i, meta, info, _) in idx.iter_full() {
            assert_eq!(idx.by_path(&meta.path), Some(i));
            assert!(meta.path.as_str().ends_with(&info.folder_name));
        }
    }

    #[test]
    fn search_ranks_exact_id_before_prefix_before_substring() {
        let idx = sample();
        let hits = idx.search("rs.beta", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].field, SearchField::PackageId);
        let hits = idx.search("guns", 10);
        let names: Vec<&str> = hits
            .iter()
            .map(|h| idx.get(h.idx).unwrap().name.as_str())
            .collect();
        assert_eq!(names, vec!["RS Alpha Guns", "Gamma"]);
    }

    #[test]
    fn search_needs_every_word_and_ignores_empty_queries() {
        let idx = sample();
        assert_eq!(idx.search("beta armor", 10).len(), 1);
        assert!(idx.search("beta guns", 10).is_empty());
        assert!(idx.search("   ", 10).is_empty());
        assert!(idx.search("beta", 0).is_empty());
    }

    #[test]
    fn search_is_deterministic_and_respects_limit() {
        let idx = sample();
        let a = idx.search("rs", 2);
        let b = idx.search("rs", 2);
        assert_eq!(a, b);
        assert_eq!(a.len(), 2);
    }

    #[test]
    fn duplicate_paths_replace_each_other() {
        let mut b = LibraryIndex::builder();
        b.push(item("rs.one", "One", "/m/x", "a"));
        b.push(item("rs.two", "Two", "/m/x", "a"));
        let idx = b.build();
        assert_eq!(idx.len(), 1);
        assert_eq!(idx.iter().next().unwrap().1.name, "Two");
    }
}
