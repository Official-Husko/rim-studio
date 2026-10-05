//! Mod metadata, sources, the mod index and the active list.
//!
//! These are plain data types. Scanning (reading `About.xml` and listing folders) happens in other
//! crates; they fill a [`ModMeta`] and hand it to a [`ModIndexBuilder`]. Package ids inside the
//! relation lists are kept as text because `About.xml` is user written and may hold values that are
//! not valid [`PackageId`]s; membership tests compare lower case text.

use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

use crate::ids::{ModId, ModIdx, PackageId, SourceId, WorkshopId};
use crate::load_plan::LoadFoldersSpec;
use crate::version::GameVersion;

/// A `modDependencies` entry of `About.xml`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDependency {
    /// The required package id.
    pub package_id: String,
    /// The display name the author gave the dependency.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub display_name: String,
    /// The Steam Workshop URL, when given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steam_workshop_url: Option<String>,
    /// The direct download URL, when given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    /// Alternative package ids that satisfy the dependency.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<String>,
}

/// The relation lists that `About.xml` allows per game version (the `ByVersion` variants).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionRelations {
    /// `loadBeforeByVersion` entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub load_before: Vec<String>,
    /// `loadAfterByVersion` entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub load_after: Vec<String>,
    /// `forceLoadBeforeByVersion` entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub force_load_before: Vec<String>,
    /// `forceLoadAfterByVersion` entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub force_load_after: Vec<String>,
    /// `incompatibleWithByVersion` entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub incompatible_with: Vec<String>,
    /// `modDependenciesByVersion` entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mod_dependencies: Vec<ModDependency>,
}

/// Everything `About.xml` carries about a mod, plus where the mod lives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModMeta {
    /// The declared package id.
    pub package_id: PackageId,
    /// The display name.
    pub name: String,
    /// The source the mod was found in.
    pub source: SourceId,
    /// The mod root folder.
    pub path: Utf8PathBuf,
    /// The Workshop item id, when the folder name or `PublishedFileId.txt` gives one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workshop_id: Option<WorkshopId>,
    /// Authors (`author` and `authors`), in file order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    /// The description text.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The `url` field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The `supportedVersions` entries as written.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_versions: Vec<String>,
    /// `loadAfter` entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub load_after: Vec<String>,
    /// `loadBefore` entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub load_before: Vec<String>,
    /// `forceLoadAfter` entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub force_load_after: Vec<String>,
    /// `forceLoadBefore` entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub force_load_before: Vec<String>,
    /// `incompatibleWith` entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub incompatible_with: Vec<String>,
    /// `modDependencies` entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mod_dependencies: Vec<ModDependency>,
    /// Per game version overrides keyed by the version text as written (`1.6`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub by_version: BTreeMap<String, VersionRelations>,
    /// The parsed `LoadFolders.xml`, when the mod has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_folders: Option<LoadFoldersSpec>,
    /// Names of the folders directly inside the mod root, filled by the scanner.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub root_dirs: Vec<String>,
    /// Path of the mod icon, when the file exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_path: Option<Utf8PathBuf>,
    /// Path of the preview image, when the file exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_path: Option<Utf8PathBuf>,
}

impl ModMeta {
    /// Creates metadata with the identity fields and everything else empty.
    pub fn new(
        package_id: PackageId,
        name: impl Into<String>,
        source: SourceId,
        path: Utf8PathBuf,
    ) -> Self {
        ModMeta {
            package_id,
            name: name.into(),
            source,
            path,
            workshop_id: None,
            authors: Vec::new(),
            description: String::new(),
            url: None,
            supported_versions: Vec::new(),
            load_after: Vec::new(),
            load_before: Vec::new(),
            force_load_after: Vec::new(),
            force_load_before: Vec::new(),
            incompatible_with: Vec::new(),
            mod_dependencies: Vec::new(),
            by_version: BTreeMap::new(),
            load_folders: None,
            root_dirs: Vec::new(),
            icon_path: None,
            preview_path: None,
        }
    }

    /// The persistent id: `w<id>` for Workshop items, else `<source>:<packageId>`.
    pub fn mod_id(&self) -> ModId {
        match self.workshop_id {
            Some(w) => ModId::workshop(w),
            None => ModId::local(&self.source, &self.package_id),
        }
    }

    /// True when `supportedVersions` matches the game version (exact major.minor).
    pub fn supports(&self, game: &GameVersion) -> bool {
        game.supported_matches(&self.supported_versions)
    }

    /// The per version relations that apply to `game`, if the mod declares any.
    pub fn relations_for(&self, game: &GameVersion) -> Option<&VersionRelations> {
        self.by_version
            .iter()
            .find(|(key, _)| game.supports_entry(key))
            .map(|(_, v)| v)
    }

    /// `loadAfter` plus the entries of the matching version block.
    pub fn effective_load_after(&self, game: &GameVersion) -> Vec<&str> {
        self.merged(game, |m| &m.load_after, |r| &r.load_after)
    }

    /// `loadBefore` plus the entries of the matching version block.
    pub fn effective_load_before(&self, game: &GameVersion) -> Vec<&str> {
        self.merged(game, |m| &m.load_before, |r| &r.load_before)
    }

    /// `forceLoadAfter` plus the entries of the matching version block.
    pub fn effective_force_load_after(&self, game: &GameVersion) -> Vec<&str> {
        self.merged(game, |m| &m.force_load_after, |r| &r.force_load_after)
    }

    /// `forceLoadBefore` plus the entries of the matching version block.
    pub fn effective_force_load_before(&self, game: &GameVersion) -> Vec<&str> {
        self.merged(game, |m| &m.force_load_before, |r| &r.force_load_before)
    }

    /// `incompatibleWith` plus the entries of the matching version block.
    pub fn effective_incompatible_with(&self, game: &GameVersion) -> Vec<&str> {
        self.merged(game, |m| &m.incompatible_with, |r| &r.incompatible_with)
    }

    /// `modDependencies` plus the entries of the matching version block.
    pub fn effective_dependencies(&self, game: &GameVersion) -> Vec<&ModDependency> {
        let mut out: Vec<&ModDependency> = self.mod_dependencies.iter().collect();
        if let Some(r) = self.relations_for(game) {
            out.extend(r.mod_dependencies.iter());
        }
        out
    }

    fn merged<'a>(
        &'a self,
        game: &GameVersion,
        base: impl Fn(&'a ModMeta) -> &'a Vec<String>,
        over: impl Fn(&'a VersionRelations) -> &'a Vec<String>,
    ) -> Vec<&'a str> {
        let mut out: Vec<&str> = base(self).iter().map(String::as_str).collect();
        if let Some(r) = self.relations_for(game) {
            out.extend(over(r).iter().map(String::as_str));
        }
        out
    }
}

/// The kind of a mod source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceKind {
    /// The install `Data` folder (official content).
    GameData,
    /// The install `Mods` folder.
    GameMods,
    /// A Steam Workshop content folder.
    Workshop,
    /// A user added folder.
    Custom,
}

/// A folder that holds mods.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModSource {
    /// The stable id.
    pub id: SourceId,
    /// What kind of folder this is.
    pub kind: SourceKind,
    /// The folder path.
    pub path: Utf8PathBuf,
    /// A display label.
    #[serde(default)]
    pub label: String,
    /// Disabled sources are not scanned.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Higher priority sources win duplicate package ids.
    #[serde(default)]
    pub priority: i32,
    /// True when the app must never write into the folder.
    #[serde(default)]
    pub read_only: bool,
}

fn default_true() -> bool {
    true
}

impl ModSource {
    /// Creates an enabled, writable source with the id as label.
    pub fn new(id: SourceId, kind: SourceKind, path: Utf8PathBuf) -> Self {
        let label = id.as_str().to_owned();
        ModSource {
            id,
            kind,
            path,
            label,
            enabled: true,
            priority: 0,
            read_only: false,
        }
    }
}

/// An ordered set of sources with unique ids.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceSet {
    sources: Vec<ModSource>,
}

impl SourceSet {
    /// An empty set.
    pub fn new() -> Self {
        SourceSet::default()
    }

    /// Adds a source; an existing source with the same id is replaced in place.
    pub fn insert(&mut self, source: ModSource) {
        match self.sources.iter_mut().find(|s| s.id == source.id) {
            Some(slot) => *slot = source,
            None => self.sources.push(source),
        }
    }

    /// Removes a source by id and returns it.
    pub fn remove(&mut self, id: &SourceId) -> Option<ModSource> {
        let pos = self.sources.iter().position(|s| &s.id == id)?;
        Some(self.sources.remove(pos))
    }

    /// Looks a source up by id.
    pub fn get(&self, id: &SourceId) -> Option<&ModSource> {
        self.sources.iter().find(|s| &s.id == id)
    }

    /// All sources in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &ModSource> {
        self.sources.iter()
    }

    /// The enabled sources in insertion order.
    pub fn enabled(&self) -> impl Iterator<Item = &ModSource> {
        self.sources.iter().filter(|s| s.enabled)
    }

    /// The number of sources.
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    /// True when there are no sources.
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

/// Collects mods and builds an immutable [`ModIndex`].
#[derive(Debug, Default)]
pub struct ModIndexBuilder {
    mods: Vec<ModMeta>,
}

impl ModIndexBuilder {
    /// An empty builder.
    pub fn new() -> Self {
        ModIndexBuilder::default()
    }

    /// Adds a mod.
    pub fn push(&mut self, meta: ModMeta) -> &mut Self {
        self.mods.push(meta);
        self
    }

    /// Sorts the mods by lowercase package id, then source id, then path (so the result does not
    /// depend on scan order) and builds the lookup tables.
    pub fn build(mut self) -> ModIndex {
        self.mods.sort_by(|a, b| {
            a.package_id
                .lower()
                .cmp(b.package_id.lower())
                .then_with(|| a.source.cmp(&b.source))
                .then_with(|| a.path.cmp(&b.path))
        });
        let mut by_id: FxHashMap<ModId, Vec<ModIdx>> = FxHashMap::default();
        let mut by_package: FxHashMap<String, Vec<ModIdx>> = FxHashMap::default();
        for (i, m) in self.mods.iter().enumerate() {
            let Some(idx) = ModIdx::from_index(i) else {
                break;
            };
            by_id.entry(m.mod_id()).or_default().push(idx);
            by_package
                .entry(m.package_id.lower().to_owned())
                .or_default()
                .push(idx);
        }
        ModIndex {
            mods: self.mods,
            by_id,
            by_package,
        }
    }
}

/// An immutable, deterministically ordered collection of mods with lookup tables.
#[derive(Debug, Clone, Default)]
pub struct ModIndex {
    mods: Vec<ModMeta>,
    by_id: FxHashMap<ModId, Vec<ModIdx>>,
    by_package: FxHashMap<String, Vec<ModIdx>>,
}

impl ModIndex {
    /// Starts a builder.
    pub fn builder() -> ModIndexBuilder {
        ModIndexBuilder::new()
    }

    /// The number of mods.
    pub fn len(&self) -> usize {
        self.mods.len()
    }

    /// True when the index holds no mods.
    pub fn is_empty(&self) -> bool {
        self.mods.is_empty()
    }

    /// The mod behind a handle, `None` for a handle of another index.
    pub fn get(&self, idx: ModIdx) -> Option<&ModMeta> {
        self.mods.get(idx.index())
    }

    /// All mods with their handles, in index order.
    pub fn iter(&self) -> impl Iterator<Item = (ModIdx, &ModMeta)> {
        self.mods
            .iter()
            .enumerate()
            .filter_map(|(i, m)| ModIdx::from_index(i).map(|idx| (idx, m)))
    }

    /// Every mod with this persistent id, in index order (more than one for duplicates).
    pub fn by_mod_id(&self, id: &ModId) -> &[ModIdx] {
        self.by_id.get(id).map_or(&[], Vec::as_slice)
    }

    /// Every mod with this package id, compared ignoring case, in index order.
    pub fn by_package_id(&self, package_id: &str) -> &[ModIdx] {
        self.by_package
            .get(&package_id.to_lowercase())
            .map_or(&[], Vec::as_slice)
    }

    /// The first mod with this package id.
    pub fn first_by_package_id(&self, package_id: &str) -> Option<(ModIdx, &ModMeta)> {
        let idx = *self.by_package_id(package_id).first()?;
        self.get(idx).map(|m| (idx, m))
    }
}

/// An ordered list of active package ids, as in `ModsConfig.xml`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ActiveList {
    ids: Vec<PackageId>,
}

impl ActiveList {
    /// An empty list.
    pub fn new() -> Self {
        ActiveList::default()
    }

    /// Builds a list from ids, dropping later duplicates (compared ignoring case).
    pub fn from_ids(ids: impl IntoIterator<Item = PackageId>) -> Self {
        let mut list = ActiveList::new();
        for id in ids {
            list.push(id);
        }
        list
    }

    /// Appends an id; returns false and does nothing when it is already present.
    pub fn push(&mut self, id: PackageId) -> bool {
        if self.contains(id.lower()) {
            return false;
        }
        self.ids.push(id);
        true
    }

    /// True when the package id (any case) is in the list.
    pub fn contains(&self, package_id: &str) -> bool {
        self.ids
            .iter()
            .any(|p| p.lower().eq_ignore_ascii_case(package_id))
    }

    /// The ids in order.
    pub fn ids(&self) -> &[PackageId] {
        &self.ids
    }

    /// The number of ids.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// True when the list is empty.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// The membership set of this list.
    pub fn to_set(&self) -> ActiveSet {
        ActiveSet::from_ids(self.ids.iter().map(PackageId::lower))
    }
}

fn key(package_id: &str) -> String {
    let lower = package_id.trim().to_lowercase();
    match lower.strip_suffix("_steam") {
        Some(base) => base.to_owned(),
        None => lower,
    }
}

/// The set of active package ids, compared by lower case text with the game's `_steam` postfix
/// removed (the game appends it to the Workshop copy of a duplicated id).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActiveSet {
    ids: FxHashSet<String>,
}

impl ActiveSet {
    /// An empty set.
    pub fn new() -> Self {
        ActiveSet::default()
    }

    /// Builds a set from package id texts.
    pub fn from_ids<I, S>(ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        ActiveSet {
            ids: ids
                .into_iter()
                .map(|s| key(s.as_ref()))
                .filter(|k| !k.is_empty())
                .collect(),
        }
    }

    /// Adds a package id; returns true when it was new.
    pub fn insert(&mut self, package_id: &str) -> bool {
        let k = key(package_id);
        !k.is_empty() && self.ids.insert(k)
    }

    /// True when the package id (any case) is active.
    pub fn is_active(&self, package_id: &str) -> bool {
        let k = key(package_id);
        !k.is_empty() && self.ids.contains(&k)
    }

    /// True when at least one listed package is active.
    pub fn any_active<S: AsRef<str>>(&self, ids: &[S]) -> bool {
        ids.iter().any(|s| self.is_active(s.as_ref()))
    }

    /// True when every listed package is active (true for an empty list).
    pub fn all_active<S: AsRef<str>>(&self, ids: &[S]) -> bool {
        ids.iter().all(|s| self.is_active(s.as_ref()))
    }

    /// The number of active ids.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// True when nothing is active.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pid(s: &str) -> PackageId {
        PackageId::parse(s).unwrap()
    }

    fn meta(package: &str, source: &str, path: &str) -> ModMeta {
        ModMeta::new(
            pid(package),
            package,
            SourceId::new(source).unwrap(),
            Utf8PathBuf::from(path),
        )
    }

    #[test]
    fn active_set_is_case_insensitive() {
        let set = ActiveSet::from_ids(["RS.Alpha", "rs.beta"]);
        assert!(set.is_active("rs.ALPHA"));
        assert!(set.any_active(&["x".to_owned(), "RS.BETA".to_owned()]));
        assert!(!set.all_active(&["rs.alpha", "rs.gamma"]));
        assert!(set.all_active::<&str>(&[]));
    }

    #[test]
    fn active_list_drops_duplicates_ignoring_case() {
        let list = ActiveList::from_ids([pid("RS.A"), pid("rs.a"), pid("rs.b")]);
        assert_eq!(list.len(), 2);
        assert!(list.to_set().is_active("RS.B"));
    }

    #[test]
    fn index_order_does_not_depend_on_insertion_order() {
        let build = |order: &[usize]| {
            let all = [
                meta("rs.b", "game-mods", "/m/b"),
                meta("rs.a", "workshop-0", "/w/a"),
                meta("rs.a", "game-mods", "/m/a"),
            ];
            let mut b = ModIndex::builder();
            for i in order {
                b.push(all[*i].clone());
            }
            b.build()
        };
        let one = build(&[0, 1, 2]);
        let two = build(&[2, 0, 1]);
        let paths = |ix: &ModIndex| {
            ix.iter()
                .map(|(_, m)| m.path.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(paths(&one), paths(&two));
        assert_eq!(paths(&one), ["/m/a", "/w/a", "/m/b"]);
        assert_eq!(one.by_package_id("RS.A").len(), 2);
        assert!(one.get(ModIdx::new(9)).is_none());
        assert!(one.first_by_package_id("rs.b").is_some());
    }

    #[test]
    fn mod_id_prefers_workshop_id() {
        let mut m = meta("rs.a", "workshop-0", "/w/a");
        assert_eq!(m.mod_id().as_str(), "workshop-0:rs.a");
        m.workshop_id = WorkshopId::new(42);
        assert_eq!(m.mod_id().as_str(), "w42");
    }

    #[test]
    fn by_version_relations_apply_only_to_the_matching_version() {
        let mut m = meta("rs.a", "game-mods", "/m/a");
        m.load_after = vec!["rs.base".into()];
        m.by_version.insert(
            "1.6".into(),
            VersionRelations {
                load_after: vec!["rs.six".into()],
                ..VersionRelations::default()
            },
        );
        let g16 = GameVersion::parse("1.6.4871 rev598").unwrap();
        let g15 = GameVersion::parse("1.5.4000").unwrap();
        assert_eq!(m.effective_load_after(&g16), ["rs.base", "rs.six"]);
        assert_eq!(m.effective_load_after(&g15), ["rs.base"]);
        assert!(m.effective_dependencies(&g16).is_empty());
    }

    #[test]
    fn source_set_replaces_by_id() {
        let mut set = SourceSet::new();
        let id = SourceId::game_mods();
        set.insert(ModSource::new(
            id.clone(),
            SourceKind::GameMods,
            "/a".into(),
        ));
        set.insert(ModSource::new(
            id.clone(),
            SourceKind::GameMods,
            "/b".into(),
        ));
        assert_eq!(set.len(), 1);
        assert_eq!(set.get(&id).map(|s| s.path.as_str()), Some("/b"));
        assert!(set.remove(&id).is_some());
        assert!(set.is_empty());
    }

    #[test]
    fn meta_json_round_trips() {
        let mut m = meta("rs.a", "game-mods", "/m/a");
        m.supported_versions = vec!["1.6".into()];
        let json = serde_json::to_string(&m).unwrap();
        let back: ModMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
    }
}
