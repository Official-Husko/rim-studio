//! Which folders of a mod the game loads, and which files they contribute.
//!
//! [`LoadFoldersSpec`] is the parsed content of a mod's `LoadFolders.xml` (the XML itself is read by
//! `rimstudio-xml`). [`resolve_load_folders`] reproduces the game's `InitLoadFolders` exactly and
//! returns a [`LoadPlan`]: the folders to load in descending priority. [`collect_files`] then lists
//! the `Defs`, `Patches` and other files those folders contribute, with the game's de-duplication
//! rule: the first file with a given path relative to its load folder wins.
//!
//! Rules reproduced from the game (see def-engine-semantics section 2 and the mod format note):
//!
//! 1. Block keys are lower cased with one leading `v` removed; repeated blocks merge.
//! 2. Entry conditions: `IfModActive` (any of), `IfModActiveAll` (all of), `IfModNotActive` (none of),
//!    combined with AND, package ids compared ignoring case and the `_steam` postfix. The game reads
//!    no other attribute: an entry with `IfModActiveAny` (a name that does not exist) loads
//!    unconditionally. Such attributes are kept in [`LoadEntry::ignored_attributes`] so lints can
//!    point at them.
//! 3. Block choice: the block named exactly like the running version with build (`1.6.4871`) when it
//!    has entries; else, among keys that contain a dot, are not `default` and parse as a version not
//!    newer than the running one, the greatest by plain string order; else the `default` block. A
//!    chosen block whose entries all fail their conditions loads nothing (no fallback).
//! 4. Without a usable block: the folder named `major.minor` if it exists, else the nearest version
//!    folder not above the game (or the lowest one above it when all are newer), then `Common`, then
//!    the mod root.
//! 5. A plan lists folders in descending priority: entries of a block are visited from last to first.

use std::collections::{BTreeMap, BTreeSet};

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

use crate::mods::{ActiveSet, ModMeta};
use crate::os::Os;
use crate::paths;
use crate::version::{GameVersion, key_order_tuple, normalize_version_key};

/// The identity of an entry for the repeated folder lint: path and the three condition lists.
type EntrySignature<'a> = (&'a str, &'a [String], &'a [String], &'a [String]);

/// The name of the legacy fallback block.
pub const DEFAULT_BLOCK_KEY: &str = "default";

/// One folder entry of a `LoadFolders.xml` block.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadEntry {
    /// The folder relative to the mod root; the empty string is the mod root itself (written `/` or
    /// `\` in the file). The text is not trimmed, as in the game.
    pub path: String,
    /// `IfModActive`: loads when any listed package is active. Empty means no condition.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub if_active: Vec<String>,
    /// `IfModActiveAll`: loads when all listed packages are active. Empty means no condition.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub if_active_all: Vec<String>,
    /// `IfModNotActive`: does not load when any listed package is active. Empty means no condition.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub if_not_active: Vec<String>,
    /// Names of attributes the game does not read (for example `IfModActiveAny`), kept for lints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignored_attributes: Vec<String>,
}

impl LoadEntry {
    /// The mod root entry (`/`).
    pub fn root() -> LoadEntry {
        LoadEntry::default()
    }

    /// An unconditional entry for a sub folder.
    pub fn dir(path: impl Into<String>) -> LoadEntry {
        LoadEntry {
            path: path.into(),
            ..LoadEntry::default()
        }
    }

    /// Adds an `IfModActive` condition (any of).
    #[must_use]
    pub fn if_active<S: Into<String>>(mut self, ids: impl IntoIterator<Item = S>) -> LoadEntry {
        self.if_active.extend(ids.into_iter().map(Into::into));
        self
    }

    /// Adds an `IfModActiveAll` condition (all of).
    #[must_use]
    pub fn if_active_all<S: Into<String>>(mut self, ids: impl IntoIterator<Item = S>) -> LoadEntry {
        self.if_active_all.extend(ids.into_iter().map(Into::into));
        self
    }

    /// Adds an `IfModNotActive` condition (none of).
    #[must_use]
    pub fn if_not_active<S: Into<String>>(mut self, ids: impl IntoIterator<Item = S>) -> LoadEntry {
        self.if_not_active.extend(ids.into_iter().map(Into::into));
        self
    }

    /// True when the entry carries any condition the game evaluates.
    pub fn is_conditional(&self) -> bool {
        !(self.if_active.is_empty()
            && self.if_active_all.is_empty()
            && self.if_not_active.is_empty())
    }

    /// The game's `ShouldLoad`: (any-of empty or one active) and (all-of empty or all active) and
    /// (none-of empty or none active).
    pub fn should_load(&self, active: &ActiveSet) -> bool {
        (self.if_active.is_empty() || active.any_active(&self.if_active))
            && (self.if_active_all.is_empty() || active.all_active(&self.if_active_all))
            && (self.if_not_active.is_empty() || !active.any_active(&self.if_not_active))
    }

    /// The folder text as the game turns it into a path on `os`: on Windows both separators are
    /// accepted and become `/` here; elsewhere a backslash stays an ordinary character.
    pub fn folder_for(&self, os: Os) -> String {
        if os == Os::Windows {
            self.path.replace('\\', "/")
        } else {
            self.path.clone()
        }
    }
}

/// One version block of a `LoadFolders.xml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadBlock {
    /// The normalised key (lower case, no leading `v`), for example `1.6` or `default`.
    pub key: String,
    /// The entries in file order.
    pub entries: Vec<LoadEntry>,
}

/// A problem found by [`LoadFoldersSpec::lint`], for authoring checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadFoldersIssue {
    /// A block has no entries.
    EmptyBlock {
        /// The block key.
        key: String,
    },
    /// The same folder is listed twice with identical conditions in one block.
    RepeatedFolder {
        /// The block key.
        key: String,
        /// The repeated folder text.
        folder: String,
    },
    /// The key is neither `major.minor` nor `default`.
    MalformedVersionKey {
        /// The block key.
        key: String,
    },
    /// The `default` block is deprecated by the game.
    DefaultDeprecated,
    /// An entry carries an attribute that the game ignores.
    IgnoredAttribute {
        /// The block key.
        key: String,
        /// The folder text of the entry.
        folder: String,
        /// The attribute name.
        attribute: String,
    },
}

/// The parsed content of a `LoadFolders.xml`, blocks in first seen order.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadFoldersSpec {
    /// The blocks. Keys are normalised and unique when built through [`LoadFoldersSpec::add_block`].
    pub blocks: Vec<LoadBlock>,
}

impl LoadFoldersSpec {
    /// An empty specification (a file with no blocks).
    pub fn new() -> LoadFoldersSpec {
        LoadFoldersSpec::default()
    }

    /// Adds a block, normalising its key with [`normalize_version_key`]. A repeated key appends its
    /// entries to the existing block, as the game does.
    pub fn add_block(&mut self, raw_key: &str, entries: Vec<LoadEntry>) {
        let key = normalize_version_key(raw_key);
        match self.blocks.iter_mut().find(|b| b.key == key) {
            Some(existing) => existing.entries.extend(entries),
            None => self.blocks.push(LoadBlock { key, entries }),
        }
    }

    /// Builds a specification from raw `(key, entries)` pairs.
    pub fn from_blocks<K: AsRef<str>>(
        blocks: impl IntoIterator<Item = (K, Vec<LoadEntry>)>,
    ) -> LoadFoldersSpec {
        let mut spec = LoadFoldersSpec::new();
        for (key, entries) in blocks {
            spec.add_block(key.as_ref(), entries);
        }
        spec
    }

    /// The block with this normalised key.
    pub fn block(&self, key: &str) -> Option<&LoadBlock> {
        self.blocks.iter().find(|b| b.key == key)
    }

    /// True when the file defines no block at all (the game then uses the implicit rules).
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Structural checks on the specification.
    ///
    /// # Errors
    /// [`crate::error::CoreError::InvalidLoadFolders`] for an empty or duplicated block key or an
    /// entry path containing a NUL character.
    pub fn validate(&self) -> Result<(), crate::error::CoreError> {
        let bad = |reason: &str| crate::error::CoreError::InvalidLoadFolders {
            reason: reason.to_owned(),
        };
        let mut seen = BTreeSet::new();
        for block in &self.blocks {
            if block.key.is_empty() {
                return Err(bad("a block has an empty key"));
            }
            if !seen.insert(block.key.as_str()) {
                return Err(bad("two blocks share a key"));
            }
            if block.entries.iter().any(|e| e.path.contains('\0')) {
                return Err(bad("an entry path contains a NUL character"));
            }
        }
        Ok(())
    }

    /// Authoring checks modelled on the game's own issue list plus the ignored attributes.
    pub fn lint(&self) -> Vec<LoadFoldersIssue> {
        let mut out = Vec::new();
        for block in &self.blocks {
            if block.entries.is_empty() {
                out.push(LoadFoldersIssue::EmptyBlock {
                    key: block.key.clone(),
                });
            }
            let mut seen: BTreeSet<EntrySignature<'_>> = BTreeSet::new();
            for e in &block.entries {
                let sig = (
                    e.path.as_str(),
                    e.if_active.as_slice(),
                    e.if_active_all.as_slice(),
                    e.if_not_active.as_slice(),
                );
                if !seen.insert(sig) {
                    out.push(LoadFoldersIssue::RepeatedFolder {
                        key: block.key.clone(),
                        folder: e.path.clone(),
                    });
                }
                for a in &e.ignored_attributes {
                    out.push(LoadFoldersIssue::IgnoredAttribute {
                        key: block.key.clone(),
                        folder: e.path.clone(),
                        attribute: a.clone(),
                    });
                }
            }
            if block.key == DEFAULT_BLOCK_KEY {
                out.push(LoadFoldersIssue::DefaultDeprecated);
            } else if !is_well_formed_key(&block.key) {
                out.push(LoadFoldersIssue::MalformedVersionKey {
                    key: block.key.clone(),
                });
            }
        }
        out
    }
}

fn is_well_formed_key(key: &str) -> bool {
    let mut parts = key.split('.');
    let (Some(a), Some(b), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    [a, b]
        .iter()
        .all(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit()))
}

/// How the plan's folders were chosen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum LoadSource {
    /// The block named exactly like the running version with build.
    BlockExact {
        /// The block key.
        key: String,
    },
    /// The greatest older block (the usual `1.6` case, or a lower version on a newer game).
    Block {
        /// The block key.
        key: String,
    },
    /// The legacy `default` block.
    BlockDefault,
    /// No usable block; the folder named `major.minor` exists.
    VersionDir,
    /// No usable block; the nearest version folder not above the game version.
    NearestLowerDir,
    /// No usable block; every version folder is above the game, the lowest one is used.
    NearestNewerDir,
    /// No usable block and no version folder: `Common` if present, then the mod root.
    RootOnly,
}

/// One folder a mod loads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadFolder {
    /// The folder relative to the mod root, `/` separated, empty for the root itself.
    pub relative: String,
    /// The absolute folder path (the mod path joined with `relative`).
    pub path: Utf8PathBuf,
}

/// The folders a mod loads for a game version and active set, in descending priority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadPlan {
    /// The mod root the plan was resolved for.
    pub root: Utf8PathBuf,
    /// How the folders were chosen.
    pub source: LoadSource,
    /// The folders, highest priority first. The first file with a given relative path wins.
    pub folders: Vec<LoadFolder>,
    /// Entries of the chosen block whose conditions failed, in visit order.
    pub skipped: Vec<String>,
}

impl LoadPlan {
    /// True when the mod loads no folder at all (a chosen block whose entries all failed).
    pub fn loads_nothing(&self) -> bool {
        self.folders.is_empty()
    }

    /// The relative folder names in priority order.
    pub fn relative_folders(&self) -> Vec<&str> {
        self.folders.iter().map(|f| f.relative.as_str()).collect()
    }
}

fn join_relative(root: &Utf8Path, relative: &str) -> Utf8PathBuf {
    if relative.is_empty() {
        root.to_owned()
    } else {
        root.join(relative)
    }
}

fn push_block(plan: &mut LoadPlan, block: &LoadBlock, active: &ActiveSet, os: Os) {
    for entry in block.entries.iter().rev() {
        if entry.should_load(active) {
            let relative = entry.folder_for(os);
            plan.folders.push(LoadFolder {
                path: join_relative(&plan.root, &relative),
                relative,
            });
        } else {
            plan.skipped.push(entry.folder_for(os));
        }
    }
}

fn dir_present(dirs: &[String], name: &str, os: Os) -> bool {
    if os.case_insensitive_default() {
        dirs.iter().any(|d| d.eq_ignore_ascii_case(name))
    } else {
        dirs.iter().any(|d| d == name)
    }
}

/// Parses a directory name the way `TryParseVersionString` does: at least two dot separated parts,
/// the first two non negative integers (any further parts are ignored).
fn version_dir_name(name: &str) -> Option<(u32, u32)> {
    crate::version::parse_major_minor(name, false)
}

/// Resolves the folders a mod loads on the current OS.
///
/// The implicit rules need to know which folders exist in the mod root: they come from
/// [`ModMeta::root_dirs`], filled by the scanner.
pub fn resolve_load_folders(meta: &ModMeta, game: &GameVersion, active: &ActiveSet) -> LoadPlan {
    resolve_load_folders_os(meta, game, active, Os::current())
}

/// Like [`resolve_load_folders`] with an explicit OS, so tests can pin separator and case rules.
pub fn resolve_load_folders_os(
    meta: &ModMeta,
    game: &GameVersion,
    active: &ActiveSet,
    os: Os,
) -> LoadPlan {
    let mut plan = LoadPlan {
        root: meta.path.clone(),
        source: LoadSource::RootOnly,
        folders: Vec::new(),
        skipped: Vec::new(),
    };

    if let Some(spec) = meta.load_folders.as_ref().filter(|s| !s.is_empty()) {
        // 1. The block named exactly like the running version with build, when it has entries.
        let exact_key = game.with_build();
        if let Some(block) = spec.block(&exact_key).filter(|b| !b.entries.is_empty()) {
            plan.source = LoadSource::BlockExact {
                key: block.key.clone(),
            };
            push_block(&mut plan, block, active, os);
            return plan;
        }
        // 2. The greatest older key, by plain string order (a game quirk kept on purpose).
        let current = game.game_order_key();
        let older = spec
            .blocks
            .iter()
            .filter(|b| b.key != DEFAULT_BLOCK_KEY && b.key.contains('.'))
            .filter(|b| key_order_tuple(&b.key).is_some_and(|t| t <= current))
            .max_by(|a, b| a.key.cmp(&b.key));
        if let Some(block) = older {
            plan.source = LoadSource::Block {
                key: block.key.clone(),
            };
            push_block(&mut plan, block, active, os);
            return plan;
        }
        // 3. The legacy default block.
        if let Some(block) = spec.block(DEFAULT_BLOCK_KEY) {
            plan.source = LoadSource::BlockDefault;
            push_block(&mut plan, block, active, os);
            return plan;
        }
    }

    // Implicit rules: version folder, then Common, then the root.
    let short = game.short();
    if dir_present(&meta.root_dirs, &short, os) {
        plan.source = LoadSource::VersionDir;
        plan.folders.push(LoadFolder {
            relative: short.clone(),
            path: join_relative(&plan.root, &short),
        });
    } else if let Some((chosen, newer)) = nearest_version_dir(&meta.root_dirs, game.major_minor()) {
        let name = format!("{}.{}", chosen.0, chosen.1);
        plan.source = if newer {
            LoadSource::NearestNewerDir
        } else {
            LoadSource::NearestLowerDir
        };
        plan.folders.push(LoadFolder {
            path: join_relative(&plan.root, &name),
            relative: name,
        });
    }
    if dir_present(&meta.root_dirs, paths::COMMON_DIR, os) {
        plan.folders.push(LoadFolder {
            relative: paths::COMMON_DIR.to_owned(),
            path: plan.root.join(paths::COMMON_DIR),
        });
    }
    plan.folders.push(LoadFolder {
        relative: String::new(),
        path: plan.root.clone(),
    });
    plan
}

/// The game's nearest version folder search. Returns the chosen `(major, minor)` and whether it is
/// above the game version. A literal port of the loop in `InitLoadFolders`, including its handling
/// of major version zero.
fn nearest_version_dir(dirs: &[String], current: (u32, u32)) -> Option<((u32, u32), bool)> {
    let mut found: Vec<(u32, u32)> = dirs.iter().filter_map(|d| version_dir_name(d)).collect();
    found.sort_unstable();
    let mut chosen = (0u32, 0u32);
    for item in found {
        if (item > chosen || chosen > current) && (item <= current || chosen.0 == 0) {
            chosen = item;
        }
    }
    if chosen.0 > 0 {
        Some((chosen, chosen > current))
    } else {
        None
    }
}

/// The kinds of content folder `collect_files` can list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Subdir {
    /// `Defs`, XML files, recursive.
    Defs,
    /// `Patches`, XML files, recursive.
    Patches,
    /// `Assemblies`, `.dll` files directly inside the folder, listed in load order (lowest
    /// priority folder first, then by file name).
    Assemblies,
    /// `Textures`, every file, recursive.
    Textures,
    /// `Sounds`, every file, recursive.
    Sounds,
    /// `Languages`, every file, recursive.
    Languages,
}

impl Subdir {
    /// The folder name inside a load folder.
    pub fn dir_name(self) -> &'static str {
        match self {
            Subdir::Defs => paths::DEFS_DIR,
            Subdir::Patches => paths::PATCHES_DIR,
            Subdir::Assemblies => paths::ASSEMBLIES_DIR,
            Subdir::Textures => paths::TEXTURES_DIR,
            Subdir::Sounds => paths::SOUNDS_DIR,
            Subdir::Languages => paths::LANGUAGES_DIR,
        }
    }

    fn accepts(self, relative_below_subdir: &str) -> bool {
        let name = relative_below_subdir
            .rsplit('/')
            .next()
            .unwrap_or(relative_below_subdir);
        if name.starts_with('.') {
            return false;
        }
        match self {
            Subdir::Defs | Subdir::Patches => has_extension(name, paths::XML_EXT),
            Subdir::Assemblies => {
                !relative_below_subdir.contains('/') && has_extension(name, paths::DLL_EXT)
            }
            Subdir::Textures | Subdir::Sounds | Subdir::Languages => true,
        }
    }
}

fn has_extension(name: &str, ext: &str) -> bool {
    match name.rsplit_once('.') {
        Some((stem, e)) => !stem.is_empty() && e.eq_ignore_ascii_case(ext),
        None => false,
    }
}

/// Read access to a folder tree, the only IO `collect_files` needs.
///
/// Implementations: the real filesystem in `rimstudio-io`, [`MemListing`] for tests.
pub trait Listing {
    /// Every file below `dir`, recursively, as paths relative to `dir` with `/` separators, in any
    /// order. An empty vector when `dir` does not exist or cannot be read.
    fn list_files(&self, dir: &Utf8Path) -> Vec<String>;

    /// True when `dir` exists and is a directory.
    fn dir_exists(&self, dir: &Utf8Path) -> bool;
}

/// A file contributed by a load folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFileRef {
    /// Index into [`LoadPlan::folders`] of the folder that provides the file.
    pub folder: usize,
    /// The path relative to the load folder, including the `Defs/` style prefix, `/` separated.
    /// This is the de-duplication key.
    pub relative: String,
    /// The absolute file path.
    pub path: Utf8PathBuf,
}

/// Lists the files a plan contributes for `subdir`, in the order the game processes them.
///
/// Folders are visited from the highest priority; inside a folder files are in ordinal order of
/// their relative path (the game's order is filesystem order, which is not stable; RimStudio fixes
/// one deterministic rule). The first file with a given relative path wins and shadows the same path
/// in lower priority folders without any duplicate error. Names that start with a dot are skipped.
/// `Defs` and `Patches` take only `.xml` files (extension compared ignoring case). `Assemblies`
/// are the exception: they are returned in load order, lowest priority folder first, sorted by name,
/// without de-duplication.
pub fn collect_files(plan: &LoadPlan, subdir: Subdir, listing: &dyn Listing) -> Vec<SourceFileRef> {
    let order: Vec<usize> = if subdir == Subdir::Assemblies {
        (0..plan.folders.len()).rev().collect()
    } else {
        (0..plan.folders.len()).collect()
    };
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out = Vec::new();
    for index in order {
        let folder = &plan.folders[index];
        let dir = folder.path.join(subdir.dir_name());
        if !listing.dir_exists(&dir) {
            continue;
        }
        let mut rels: Vec<String> = listing
            .list_files(&dir)
            .into_iter()
            .filter(|r| subdir.accepts(r))
            .collect();
        rels.sort();
        rels.dedup();
        for rel in rels {
            let relative = format!("{}/{}", subdir.dir_name(), rel);
            if subdir != Subdir::Assemblies && !seen.insert(relative.clone()) {
                continue;
            }
            out.push(SourceFileRef {
                folder: index,
                path: dir.join(&rel),
                relative,
            });
        }
    }
    out
}

/// An in memory [`Listing`] for tests: add files by full path and every parent folder exists.
#[derive(Debug, Clone, Default)]
pub struct MemListing {
    files: BTreeSet<String>,
    dirs: BTreeSet<String>,
}

impl MemListing {
    /// Creates an empty tree.
    pub fn new() -> MemListing {
        MemListing::default()
    }

    fn normalise(path: &str) -> String {
        path.replace('\\', "/").trim_end_matches('/').to_owned()
    }

    fn add_parents(&mut self, path: &str) {
        let mut end = path.len();
        while let Some(i) = path[..end].rfind('/') {
            if i == 0 {
                break;
            }
            self.dirs.insert(path[..i].to_owned());
            end = i;
        }
    }

    /// Adds a file by full path; its parent folders come into existence.
    pub fn add_file(&mut self, path: &str) {
        let p = MemListing::normalise(path);
        self.add_parents(&p);
        self.files.insert(p);
    }

    /// Adds an (empty) folder by full path; its parents come into existence.
    pub fn add_dir(&mut self, path: &str) {
        let p = MemListing::normalise(path);
        self.add_parents(&p);
        self.dirs.insert(p);
    }

    /// Builder form of [`MemListing::add_file`].
    #[must_use]
    pub fn with_file(mut self, path: &str) -> MemListing {
        self.add_file(path);
        self
    }

    /// Builder form of [`MemListing::add_dir`].
    #[must_use]
    pub fn with_dir(mut self, path: &str) -> MemListing {
        self.add_dir(path);
        self
    }

    /// The names of the immediate sub folders of `dir`, sorted. Handy for building
    /// [`ModMeta::root_dirs`] in tests.
    pub fn child_dirs(&self, dir: &str) -> Vec<String> {
        let prefix = format!("{}/", MemListing::normalise(dir));
        let mut names: BTreeMap<String, ()> = BTreeMap::new();
        for d in &self.dirs {
            if let Some(rest) = d.strip_prefix(&prefix).filter(|r| !r.contains('/')) {
                names.insert(rest.to_owned(), ());
            }
        }
        names.into_keys().collect()
    }
}

impl Listing for MemListing {
    fn list_files(&self, dir: &Utf8Path) -> Vec<String> {
        let prefix = format!("{}/", MemListing::normalise(dir.as_str()));
        self.files
            .range(prefix.clone()..)
            .take_while(|f| f.starts_with(&prefix))
            .map(|f| f[prefix.len()..].to_owned())
            .collect()
    }

    fn dir_exists(&self, dir: &Utf8Path) -> bool {
        let d = MemListing::normalise(dir.as_str());
        self.dirs.contains(&d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{PackageId, SourceId};
    use rstest::rstest;

    const ROOT: &str = "/mods/RS_Alpha";

    fn meta(spec: Option<LoadFoldersSpec>, dirs: &[&str]) -> ModMeta {
        let mut m = ModMeta::new(
            PackageId::parse("rs.alpha").unwrap(),
            "RS Alpha",
            SourceId::game_mods(),
            Utf8PathBuf::from(ROOT),
        );
        m.load_folders = spec;
        m.root_dirs = dirs.iter().map(|s| (*s).to_owned()).collect();
        m
    }

    fn game(text: &str) -> GameVersion {
        GameVersion::parse(text).unwrap()
    }

    fn active(ids: &[&str]) -> ActiveSet {
        ActiveSet::from_ids(ids.iter().copied())
    }

    fn resolve(spec: Option<LoadFoldersSpec>, dirs: &[&str], g: &str, act: &[&str]) -> LoadPlan {
        resolve_load_folders_os(&meta(spec, dirs), &game(g), &active(act), Os::Linux)
    }

    fn rels(plan: &LoadPlan) -> Vec<&str> {
        plan.relative_folders()
    }

    fn spec(blocks: &[(&str, Vec<LoadEntry>)]) -> LoadFoldersSpec {
        LoadFoldersSpec::from_blocks(blocks.iter().map(|(k, v)| (*k, v.clone())))
    }

    #[test]
    fn block_keys_are_normalised_and_repeated_blocks_merge() {
        let s = spec(&[
            ("V1.6", vec![LoadEntry::root()]),
            ("v1.6", vec![LoadEntry::dir("A")]),
            ("Default", vec![]),
        ]);
        assert_eq!(s.blocks.len(), 2);
        assert_eq!(s.block("1.6").unwrap().entries.len(), 2);
        assert!(s.block("default").is_some());
        assert!(s.validate().is_ok());
    }

    #[test]
    fn validate_rejects_empty_keys_and_nul_paths() {
        let mut s = LoadFoldersSpec::new();
        s.blocks.push(LoadBlock {
            key: String::new(),
            entries: vec![],
        });
        assert!(s.validate().is_err());
        let s = spec(&[("1.6", vec![LoadEntry::dir("a\0b")])]);
        assert!(s.validate().is_err());
    }

    #[rstest]
    #[case::exact_key_wins_over_everything(&[("1.6.4871", vec![LoadEntry::dir("Exact")]), ("1.6", vec![LoadEntry::dir("Short")])], "1.6.4871 rev598", &["Exact"])]
    #[case::short_key_is_found_as_an_older_block(&[("1.6", vec![LoadEntry::root(), LoadEntry::dir("Sub")])], "1.6.4871 rev598", &["Sub", ""])]
    #[case::lower_block_serves_a_newer_game(&[("1.5", vec![LoadEntry::dir("Five")])], "1.6.4871 rev598", &["Five"])]
    #[case::newer_block_is_ignored_and_default_used(&[("1.7", vec![LoadEntry::dir("Seven")]), ("default", vec![LoadEntry::dir("Dflt")])], "1.6.4871", &["Dflt"])]
    #[case::greatest_older_key_by_string_order(&[("1.4", vec![LoadEntry::dir("Four")]), ("1.5", vec![LoadEntry::dir("Five")]), ("1.3", vec![LoadEntry::dir("Three")])], "1.6.4871", &["Five"])]
    #[case::string_order_quirk_prefers_1_9_over_1_10(&[("1.10", vec![LoadEntry::dir("Ten")]), ("1.9", vec![LoadEntry::dir("Nine")])], "1.11.5000", &["Nine"])]
    #[case::entries_are_visited_last_to_first(&[("1.6", vec![LoadEntry::root(), LoadEntry::dir("A"), LoadEntry::dir("B")])], "1.6", &["B", "A", ""])]
    #[case::unparsable_keys_are_skipped(&[("1.x", vec![LoadEntry::dir("X")]), ("1.5", vec![LoadEntry::dir("Five")])], "1.6", &["Five"])]
    #[case::keys_without_dot_are_skipped(&[("16", vec![LoadEntry::dir("X")])], "1.6", &[""])]
    fn block_selection(
        #[case] blocks: &[(&str, Vec<LoadEntry>)],
        #[case] game_text: &str,
        #[case] expected: &[&str],
    ) {
        let plan = resolve(Some(spec(blocks)), &[], game_text, &[]);
        assert_eq!(rels(&plan), expected);
    }

    #[test]
    fn an_empty_chosen_block_loads_nothing_even_when_a_shorter_key_has_entries() {
        // The game's second step picks the greatest older key by string order, which is the empty
        // exact key here, and an empty list is still a hit.
        let s = spec(&[("1.6.4871", vec![]), ("1.6", vec![LoadEntry::dir("Short")])]);
        let plan = resolve(Some(s), &[], "1.6.4871", &[]);
        assert!(plan.loads_nothing());
        assert_eq!(
            plan.source,
            LoadSource::Block {
                key: "1.6.4871".into()
            }
        );
        let s = spec(&[("1.6", vec![]), ("default", vec![LoadEntry::dir("D")])]);
        let plan = resolve(Some(s), &[], "1.6.4871", &[]);
        assert!(plan.loads_nothing());
        assert_eq!(plan.source, LoadSource::Block { key: "1.6".into() });
    }

    #[test]
    fn a_chosen_block_with_all_entries_failing_loads_nothing_and_does_not_fall_back() {
        let s = spec(&[(
            "1.6",
            vec![LoadEntry::dir("Only").if_active(["rs.missing"])],
        )]);
        let plan = resolve(Some(s), &["1.6", "Common"], "1.6", &[]);
        assert!(plan.loads_nothing());
        assert_eq!(plan.skipped, ["Only"]);
    }

    #[test]
    fn a_file_with_no_blocks_uses_the_implicit_rules() {
        let plan = resolve(
            Some(LoadFoldersSpec::new()),
            &["1.6", "Common"],
            "1.6.4871",
            &[],
        );
        assert_eq!(rels(&plan), ["1.6", "Common", ""]);
        assert_eq!(plan.source, LoadSource::VersionDir);
    }

    #[test]
    fn source_kinds_are_reported() {
        let s = spec(&[("1.6.4871", vec![LoadEntry::root()])]);
        assert_eq!(
            resolve(Some(s), &[], "1.6.4871", &[]).source,
            LoadSource::BlockExact {
                key: "1.6.4871".into()
            }
        );
        let s = spec(&[("default", vec![LoadEntry::root()])]);
        assert_eq!(
            resolve(Some(s), &[], "1.6", &[]).source,
            LoadSource::BlockDefault
        );
        assert_eq!(resolve(None, &[], "1.6", &[]).source, LoadSource::RootOnly);
    }

    #[rstest]
    #[case::exact_version_dir(&["1.5", "1.6", "1.7"], "1.6.4871", &["1.6", ""])]
    #[case::nearest_lower(&["1.3", "1.5"], "1.6.4871", &["1.5", ""])]
    #[case::lowest_newer_when_all_are_newer(&["1.7", "1.8"], "1.6.4871", &["1.7", ""])]
    #[case::lower_beats_newer(&["1.5", "1.7"], "1.6", &["1.5", ""])]
    #[case::common_follows_the_version_dir(&["1.6", "Common"], "1.6", &["1.6", "Common", ""])]
    #[case::common_without_version_dir(&["Common", "Defs"], "1.6", &["Common", ""])]
    #[case::non_version_names_are_ignored(&["v1.6", "1.6NotOdyssey", "1.5 Content"], "1.6", &[""])]
    #[case::three_part_name_maps_to_a_two_part_path(&["1.3.3311"], "1.6", &["1.3", ""])]
    #[case::major_zero_is_never_chosen(&["0.19"], "1.6", &[""])]
    #[case::major_zero_then_real_version(&["0.19", "1.3"], "1.6", &["1.3", ""])]
    #[case::plain_root_only(&["Defs", "Textures"], "1.6", &[""])]
    fn implicit_rules(#[case] dirs: &[&str], #[case] game_text: &str, #[case] expected: &[&str]) {
        let plan = resolve(None, dirs, game_text, &[]);
        assert_eq!(rels(&plan), expected);
    }

    #[test]
    fn implicit_source_kinds() {
        assert_eq!(
            resolve(None, &["1.5"], "1.6", &[]).source,
            LoadSource::NearestLowerDir
        );
        assert_eq!(
            resolve(None, &["1.7"], "1.6", &[]).source,
            LoadSource::NearestNewerDir
        );
        assert_eq!(
            resolve(None, &["1.6"], "1.6", &[]).source,
            LoadSource::VersionDir
        );
    }

    #[test]
    fn version_dir_case_rules_follow_the_os() {
        let m = meta(None, &["common"]);
        let lin = resolve_load_folders_os(&m, &game("1.6"), &active(&[]), Os::Linux);
        let win = resolve_load_folders_os(&m, &game("1.6"), &active(&[]), Os::Windows);
        assert_eq!(rels(&lin), [""]);
        assert_eq!(rels(&win), ["Common", ""]);
    }

    #[rstest]
    #[case::any_of_with_one_active(LoadEntry::dir("A").if_active(["rs.one", "rs.two"]), &["rs.two"], true)]
    #[case::any_of_none_active(LoadEntry::dir("A").if_active(["rs.one", "rs.two"]), &["rs.other"], false)]
    #[case::any_of_is_case_insensitive(LoadEntry::dir("A").if_active(["RS.One"]), &["rs.one"], true)]
    #[case::active_ids_may_carry_the_steam_postfix(LoadEntry::dir("A").if_active(["rs.one"]), &["RS.One_steam"], true)]
    #[case::entry_values_are_trimmed(LoadEntry::dir("A").if_active([" rs.one "]), &["rs.one"], true)]
    #[case::all_of_needs_every_id(LoadEntry::dir("A").if_active_all(["rs.one", "rs.two"]), &["rs.one"], false)]
    #[case::all_of_satisfied(LoadEntry::dir("A").if_active_all(["rs.one", "rs.two"]), &["rs.two", "rs.one"], true)]
    #[case::not_active_blocks_when_any_is_active(LoadEntry::dir("A").if_not_active(["rs.one", "rs.two"]), &["rs.two"], false)]
    #[case::not_active_loads_when_none_is_active(LoadEntry::dir("A").if_not_active(["rs.one"]), &["rs.other"], true)]
    #[case::conditions_combine_with_and(LoadEntry::dir("A").if_active(["rs.one"]).if_not_active(["rs.two"]), &["rs.one", "rs.two"], false)]
    #[case::unconditional_always_loads(LoadEntry::dir("A"), &[], true)]
    #[case::empty_attribute_text_never_matches(LoadEntry::dir("A").if_active([""]), &["rs.one"], false)]
    fn entry_conditions(#[case] entry: LoadEntry, #[case] act: &[&str], #[case] expected: bool) {
        assert_eq!(entry.should_load(&active(act)), expected);
    }

    #[test]
    fn unknown_attributes_do_not_gate_an_entry() {
        let mut e = LoadEntry::dir("A");
        e.ignored_attributes.push("IfModActiveAny".into());
        assert!(e.should_load(&active(&[])));
        assert!(!e.is_conditional());
        let s = spec(&[("1.6", vec![e])]);
        assert_eq!(
            s.lint(),
            [LoadFoldersIssue::IgnoredAttribute {
                key: "1.6".into(),
                folder: "A".into(),
                attribute: "IfModActiveAny".into()
            }]
        );
    }

    #[test]
    fn conditional_entries_use_the_active_set_and_report_skips() {
        let s = spec(&[(
            "1.6",
            vec![
                LoadEntry::root(),
                LoadEntry::dir("Royalty").if_active(["ludeon.rimworld.royalty"]),
                LoadEntry::dir("NoFoo").if_not_active(["rs.foo"]),
            ],
        )]);
        let plan = resolve(
            Some(s.clone()),
            &[],
            "1.6.4871",
            &["Ludeon.RimWorld.Royalty"],
        );
        assert_eq!(rels(&plan), ["NoFoo", "Royalty", ""]);
        let plan = resolve(Some(s), &[], "1.6.4871", &["rs.foo"]);
        assert_eq!(rels(&plan), [""]);
        assert_eq!(plan.skipped, ["NoFoo", "Royalty"]);
    }

    #[test]
    fn windows_accepts_both_separators_and_unix_keeps_backslashes() {
        let s = spec(&[("1.6", vec![LoadEntry::dir("a\\b")])]);
        let m = meta(Some(s), &[]);
        let win = resolve_load_folders_os(&m, &game("1.6"), &active(&[]), Os::Windows);
        let lin = resolve_load_folders_os(&m, &game("1.6"), &active(&[]), Os::Linux);
        assert_eq!(rels(&win), ["a/b"]);
        assert_eq!(rels(&lin), ["a\\b"]);
        assert_eq!(win.folders[0].path, Utf8PathBuf::from("/mods/RS_Alpha/a/b"));
    }

    #[test]
    fn plan_paths_join_the_mod_root() {
        let plan = resolve(
            Some(spec(&[(
                "1.6",
                vec![LoadEntry::root(), LoadEntry::dir("Sub")],
            )])),
            &[],
            "1.6",
            &[],
        );
        assert_eq!(
            plan.folders[0].path,
            Utf8PathBuf::from("/mods/RS_Alpha/Sub")
        );
        assert_eq!(plan.folders[1].path, Utf8PathBuf::from(ROOT));
        assert_eq!(plan.root, Utf8PathBuf::from(ROOT));
    }

    #[test]
    fn lint_reports_authoring_problems() {
        let s = spec(&[
            ("1.6", vec![LoadEntry::dir("A"), LoadEntry::dir("A")]),
            ("1.5", vec![]),
            ("v1x6", vec![LoadEntry::root()]),
            ("default", vec![LoadEntry::root()]),
        ]);
        let issues = s.lint();
        assert!(issues.contains(&LoadFoldersIssue::RepeatedFolder {
            key: "1.6".into(),
            folder: "A".into()
        }));
        assert!(issues.contains(&LoadFoldersIssue::EmptyBlock { key: "1.5".into() }));
        assert!(issues.contains(&LoadFoldersIssue::MalformedVersionKey { key: "1x6".into() }));
        assert!(issues.contains(&LoadFoldersIssue::DefaultDeprecated));
    }

    fn sample_listing() -> MemListing {
        MemListing::new()
            .with_file("/mods/RS_Alpha/Defs/Things/a.xml")
            .with_file("/mods/RS_Alpha/Defs/Things/b.XML")
            .with_file("/mods/RS_Alpha/Defs/Things/.hidden.xml")
            .with_file("/mods/RS_Alpha/Defs/readme.txt")
            .with_file("/mods/RS_Alpha/1.6/Defs/Things/a.xml")
            .with_file("/mods/RS_Alpha/1.6/Defs/only_new.xml")
            .with_file("/mods/RS_Alpha/Common/Defs/common.xml")
            .with_file("/mods/RS_Alpha/Patches/p.xml")
            .with_file("/mods/RS_Alpha/1.6/Assemblies/Zed.dll")
            .with_file("/mods/RS_Alpha/1.6/Assemblies/sub/Nested.dll")
            .with_file("/mods/RS_Alpha/Assemblies/Alpha.DLL")
            .with_file("/mods/RS_Alpha/Assemblies/Beta.dll")
    }

    fn implicit_plan() -> LoadPlan {
        resolve(None, &["1.6", "Common"], "1.6.4871", &[])
    }

    #[test]
    fn higher_priority_folders_shadow_the_same_relative_path_and_come_first() {
        let files = collect_files(&implicit_plan(), Subdir::Defs, &sample_listing());
        let got: Vec<(&str, usize)> = files
            .iter()
            .map(|f| (f.relative.as_str(), f.folder))
            .collect();
        assert_eq!(
            got,
            [
                ("Defs/Things/a.xml", 0),
                ("Defs/only_new.xml", 0),
                ("Defs/common.xml", 1),
                ("Defs/Things/b.XML", 2),
            ]
        );
        assert_eq!(
            files[0].path,
            Utf8PathBuf::from("/mods/RS_Alpha/1.6/Defs/Things/a.xml")
        );
    }

    #[test]
    fn dotfiles_and_non_xml_files_are_skipped() {
        let files = collect_files(&implicit_plan(), Subdir::Defs, &sample_listing());
        assert!(
            files
                .iter()
                .all(|f| !f.relative.contains(".hidden") && !f.relative.ends_with(".txt"))
        );
    }

    #[test]
    fn patches_come_from_the_patches_folder_only() {
        let files = collect_files(&implicit_plan(), Subdir::Patches, &sample_listing());
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative, "Patches/p.xml");
        assert_eq!(files[0].folder, 2);
    }

    #[test]
    fn assemblies_load_from_low_to_high_priority_sorted_by_name_top_level_only() {
        let files = collect_files(&implicit_plan(), Subdir::Assemblies, &sample_listing());
        let got: Vec<&str> = files.iter().map(|f| f.relative.as_str()).collect();
        assert_eq!(
            got,
            [
                "Assemblies/Alpha.DLL",
                "Assemblies/Beta.dll",
                "Assemblies/Zed.dll"
            ]
        );
    }

    #[test]
    fn missing_subfolders_and_empty_plans_give_nothing() {
        assert!(collect_files(&implicit_plan(), Subdir::Sounds, &sample_listing()).is_empty());
        let none = LoadPlan {
            root: Utf8PathBuf::from(ROOT),
            source: LoadSource::RootOnly,
            folders: vec![],
            skipped: vec![],
        };
        assert!(collect_files(&none, Subdir::Defs, &sample_listing()).is_empty());
    }

    #[test]
    fn collection_is_deterministic_regardless_of_listing_order() {
        struct Shuffled(MemListing);
        impl Listing for Shuffled {
            fn list_files(&self, dir: &Utf8Path) -> Vec<String> {
                let mut v = self.0.list_files(dir);
                v.reverse();
                v
            }
            fn dir_exists(&self, dir: &Utf8Path) -> bool {
                self.0.dir_exists(dir)
            }
        }
        let a = collect_files(&implicit_plan(), Subdir::Defs, &sample_listing());
        let b = collect_files(&implicit_plan(), Subdir::Defs, &Shuffled(sample_listing()));
        assert_eq!(a, b);
    }

    #[test]
    fn mem_listing_derives_parents_and_children() {
        let l = sample_listing();
        assert!(l.dir_exists(Utf8Path::new("/mods/RS_Alpha/1.6/Defs")));
        assert!(!l.dir_exists(Utf8Path::new("/mods/RS_Alpha/Nope")));
        assert_eq!(
            l.child_dirs("/mods/RS_Alpha"),
            ["1.6", "Assemblies", "Common", "Defs", "Patches"]
        );
        assert!(
            l.list_files(Utf8Path::new("/mods/RS_Alpha/Nope"))
                .is_empty()
        );
    }

    #[test]
    fn load_plan_serialises_to_json() {
        let plan = resolve(
            Some(spec(&[("1.6", vec![LoadEntry::root()])])),
            &[],
            "1.6",
            &[],
        );
        let v = serde_json::to_value(&plan).unwrap();
        assert_eq!(v["source"]["kind"], "block");
        assert_eq!(v["folders"][0]["relative"], "");
    }
}
