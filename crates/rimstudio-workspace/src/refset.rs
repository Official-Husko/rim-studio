//! Reference sets: the content packs the def engine reads, in load order.
//!
//! A [`ReferenceSet`] is the game's Core, each installed expansion, the chosen mods in the order
//! given, and optionally the project folder as the final pack. Every pack carries the
//! [`LoadPlan`] resolved with the core `resolve_load_folders` (the same function the mod manager
//! uses, so manager and tools cannot disagree about which folders load) and the Defs, Patches and
//! Assemblies files the plan contributes, listed with the core `collect_files`.
//!
//! Resolution is pure given a [`ModIndex`] and a [`Listing`]: [`ReferenceSet::resolve_with`] takes
//! the listing as a parameter, [`ReferenceSet::resolve`] uses the real disk.
//!
//! Which copy of a package id is used when the index holds several (a Workshop copy and a local
//! copy) is decided by [`ResolveOptions::pins`] first, then by the source: a copy under the
//! install `Data` folder for official ids, otherwise custom folders, the game `Mods` folder, the
//! Workshop, in that order, then index order.

use std::collections::{BTreeMap, BTreeSet};

use camino::{Utf8Path, Utf8PathBuf};
use rayon::prelude::*;
use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_core::ids::{ModIdx, SourceId};
use rimstudio_core::load_plan::{
    Listing, LoadPlan, SourceFileRef, Subdir, collect_files, resolve_load_folders,
};
use rimstudio_core::mods::{ActiveList, ActiveSet, ModIndex, ModMeta};
use rimstudio_core::paths;
use rimstudio_core::version::GameVersion;
use serde::{Deserialize, Serialize};

use crate::error::{WorkspaceResult, codes};
use crate::listing::DiskListing;
use crate::project::read_mod_meta;

/// What a content pack is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackKind {
    /// The base game.
    Core,
    /// An official expansion.
    Dlc,
    /// Any other mod.
    Mod,
    /// The project folder the user works on (always last).
    Project,
}

/// One mod's effective files under a load plan, in the form the def engine consumes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentPack {
    /// The package id as declared.
    pub package_id: String,
    /// The display name.
    pub name: String,
    /// What the pack is.
    pub kind: PackKind,
    /// The mod root.
    pub root: Utf8PathBuf,
    /// The handle of the mod in the index it came from (`None` for the project folder).
    pub mod_idx: Option<ModIdx>,
    /// The folders the mod loads for the game version and the active set.
    pub plan: LoadPlan,
    /// The Defs files, in the order the game processes them.
    pub defs: Vec<SourceFileRef>,
    /// The Patches files, in the order the game processes them.
    pub patches: Vec<SourceFileRef>,
    /// The DLLs of the Assemblies folders, in load order.
    pub assemblies: Vec<SourceFileRef>,
    /// The metadata the pack was resolved from.
    #[serde(skip)]
    pub meta: ModMeta,
}

impl ContentPack {
    /// Resolves one pack: load plan and file lists.
    fn resolve(
        meta: ModMeta,
        kind: PackKind,
        mod_idx: Option<ModIdx>,
        game: &GameVersion,
        active: &ActiveSet,
        listing: &dyn Listing,
    ) -> ContentPack {
        let plan = resolve_load_folders(&meta, game, active);
        let defs = collect_files(&plan, Subdir::Defs, listing);
        let patches = collect_files(&plan, Subdir::Patches, listing);
        let assemblies = collect_files(&plan, Subdir::Assemblies, listing);
        ContentPack {
            package_id: meta.package_id.as_str().to_owned(),
            name: meta.name.clone(),
            kind,
            root: meta.path.clone(),
            mod_idx,
            plan,
            defs,
            patches,
            assemblies,
            meta,
        }
    }

    /// The number of Defs and Patches files of the pack.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.defs.len() + self.patches.len()
    }

    /// True when the pack's load folders mention this package id in a condition (so adding or
    /// removing that package can change what the pack loads).
    fn mentions(&self, package_id: &str) -> bool {
        let lower = package_id.to_lowercase();
        let hit = |list: &[String]| list.iter().any(|s| s.trim().to_lowercase() == lower);
        self.meta.load_folders.as_ref().is_some_and(|spec| {
            spec.blocks.iter().any(|b| {
                b.entries
                    .iter()
                    .any(|e| hit(&e.if_active) || hit(&e.if_active_all) || hit(&e.if_not_active))
            })
        })
    }
}

/// How the index copy of a package id is chosen and what is added besides the chosen mods.
#[derive(Debug, Clone, Default)]
pub struct ResolveOptions {
    /// The game install folder. Official packs under `<install>/Data` are preferred. When `None`
    /// any official pack of the game data source is used.
    pub install: Option<Utf8PathBuf>,
    /// Package id (lower case) to the index entry to use, overriding the default choice.
    pub pins: BTreeMap<String, ModIdx>,
    /// Add the declared dependencies of the chosen mods (when they are in the index) before the
    /// mod that needs them.
    pub include_dependencies: bool,
}

/// What the item designer references: the game, its expansions and optional reference mods.
///
/// Combat Extended is a reference source here when the user selects it (or has it active); it is
/// never an output choice of the designer.
#[derive(Debug, Clone)]
pub struct DesignerReferenceOptions {
    /// The game version the load folders are resolved for.
    pub game_version: GameVersion,
    /// The package ids of the reference mods, in the order they should load.
    pub reference_mods: Vec<String>,
    /// Add the declared dependencies of the reference mods.
    pub include_dependencies: bool,
    /// Pins as in [`ResolveOptions::pins`].
    pub pins: BTreeMap<String, ModIdx>,
}

impl DesignerReferenceOptions {
    /// Options for a game version with no reference mods.
    #[must_use]
    pub fn new(game_version: GameVersion) -> Self {
        DesignerReferenceOptions {
            game_version,
            reference_mods: Vec::new(),
            include_dependencies: false,
            pins: BTreeMap::new(),
        }
    }

    /// Adds a reference mod by package id.
    #[must_use]
    pub fn with_mod(mut self, package_id: impl Into<String>) -> Self {
        self.reference_mods.push(package_id.into());
        self
    }
}

/// The packs the def engine reads, in load order.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceSet {
    /// The game version the plans were resolved for.
    pub game_version: GameVersion,
    /// The packs in load order, the project (when present) last.
    pub packs: Vec<ContentPack>,
    /// Problems found while resolving (missing mods, a mod that loads nothing).
    pub diagnostics: Vec<Diagnostic>,
}

/// The order of the expansions: release order, then unknown official ids by name.
fn official_rank(package_id: &str) -> (u8, usize, String) {
    let lower = package_id.to_lowercase();
    if lower == paths::CORE_PACKAGE_ID {
        return (0, 0, lower);
    }
    match paths::EXPANSION_PACKAGE_IDS
        .iter()
        .position(|id| *id == lower)
    {
        Some(i) => (1, i, lower),
        None => (2, 0, lower),
    }
}

/// How early a source is preferred for a non official package id (lower is better).
fn source_rank(source: &SourceId) -> u8 {
    if source.is_custom() {
        0
    } else if source.as_str() == SourceId::game_mods().as_str() {
        1
    } else if source.is_workshop() {
        2
    } else {
        3
    }
}

fn pick(
    index: &ModIndex,
    package_id: &str,
    options: &ResolveOptions,
    data_dir: Option<&Utf8Path>,
) -> Option<ModIdx> {
    if let Some(pinned) = options.pins.get(&package_id.to_lowercase())
        && index.get(*pinned).is_some()
    {
        return Some(*pinned);
    }
    let official = paths::is_official_package_id(package_id);
    index
        .by_package_id(package_id)
        .iter()
        .copied()
        .filter_map(|idx| index.get(idx).map(|m| (idx, m)))
        .min_by_key(|(idx, meta)| {
            let under_data = data_dir.is_some_and(|d| meta.path.starts_with(d));
            let rank = if official {
                u8::from(!(under_data || meta.source == SourceId::game_data()))
            } else {
                source_rank(&meta.source)
            };
            (rank, u8::from(!under_data), idx.0)
        })
        .map(|(idx, _)| idx)
}

/// Collects the chosen mods of a reference set: each id once, dependencies first when asked.
struct Chooser<'a> {
    index: &'a ModIndex,
    options: &'a ResolveOptions,
    data_dir: Option<&'a Utf8Path>,
    game: &'a GameVersion,
    seen: BTreeSet<String>,
    chosen: Vec<ModIdx>,
    missing: Vec<String>,
}

impl Chooser<'_> {
    /// Adds `package_id` and, when asked, its dependencies first.
    fn add(&mut self, package_id: &str) {
        let lower = package_id.trim().to_lowercase();
        if lower.is_empty()
            || paths::is_official_package_id(&lower)
            || !self.seen.insert(lower.clone())
        {
            return;
        }
        let Some(idx) = pick(self.index, &lower, self.options, self.data_dir) else {
            self.missing.push(package_id.trim().to_owned());
            return;
        };
        if self.options.include_dependencies
            && let Some(meta) = self.index.get(idx)
        {
            let deps: Vec<String> = meta
                .effective_dependencies(self.game)
                .iter()
                .map(|d| d.package_id.clone())
                .collect();
            for dep in deps {
                // a dependency that is not installed is not a problem of the reference set
                let before = self.missing.len();
                self.add(&dep);
                self.missing.truncate(before);
            }
        }
        self.chosen.push(idx);
    }
}

impl ReferenceSet {
    /// Resolves the reference set of an active list on the real disk.
    ///
    /// The game part is Core and every installed expansion; the chosen mods follow in the order of
    /// the list (official ids in the list are covered by the game part). See
    /// [`ReferenceSet::resolve_with`].
    #[must_use]
    pub fn resolve(index: &ModIndex, active: &ActiveList, game: &GameVersion) -> ReferenceSet {
        Self::resolve_with(
            index,
            active,
            game,
            &ResolveOptions::default(),
            &DiskListing,
        )
    }

    /// Resolves the reference set of an active list with explicit options and listing.
    ///
    /// Pure given the index and the listing. A package id of the list that the index does not know
    /// is left out with a [`codes::MOD_NOT_FOUND`] warning; a missing Core is an error diagnostic
    /// and the set then holds no game part.
    #[must_use]
    pub fn resolve_with(
        index: &ModIndex,
        active: &ActiveList,
        game: &GameVersion,
        options: &ResolveOptions,
        listing: &(dyn Listing + Sync),
    ) -> ReferenceSet {
        let ids: Vec<String> = active.ids().iter().map(|p| p.as_str().to_owned()).collect();
        Self::build(index, &ids, game, options, listing)
    }

    /// The reference set of the item designer: the game, every installed expansion and the optional
    /// reference mods in load order. `install` is the game install folder.
    #[must_use]
    pub fn reference_for_designer(
        index: &ModIndex,
        install: &Utf8Path,
        options: &DesignerReferenceOptions,
    ) -> ReferenceSet {
        Self::reference_for_designer_with(index, install, options, &DiskListing)
    }

    /// [`ReferenceSet::reference_for_designer`] with an explicit listing.
    #[must_use]
    pub fn reference_for_designer_with(
        index: &ModIndex,
        install: &Utf8Path,
        options: &DesignerReferenceOptions,
        listing: &(dyn Listing + Sync),
    ) -> ReferenceSet {
        let resolve = ResolveOptions {
            install: Some(install.to_owned()),
            pins: options.pins.clone(),
            include_dependencies: options.include_dependencies,
        };
        Self::build(
            index,
            &options.reference_mods,
            &options.game_version,
            &resolve,
            listing,
        )
    }

    fn build(
        index: &ModIndex,
        mod_ids: &[String],
        game: &GameVersion,
        options: &ResolveOptions,
        listing: &(dyn Listing + Sync),
    ) -> ReferenceSet {
        let mut diagnostics = Vec::new();
        let data_dir = options.install.as_ref().map(|i| i.join(paths::DATA_DIR));
        let data_dir = data_dir.as_deref();

        // the game part: every official package of the index, once each, in official order
        let mut official: BTreeMap<(u8, usize, String), ModIdx> = BTreeMap::new();
        let mut official_ids: BTreeSet<String> = BTreeSet::new();
        for (_, meta) in index.iter() {
            if meta.package_id.is_official() {
                official_ids.insert(meta.package_id.lower().to_owned());
            }
        }
        for id in &official_ids {
            if let Some(idx) = pick(index, id, options, data_dir) {
                official.insert(official_rank(id), idx);
            }
        }
        if !official_ids.contains(paths::CORE_PACKAGE_ID) {
            diagnostics.push(Diagnostic::new(
                codes::CORE_MISSING,
                Severity::Error,
                "the base game (Core) is not in the mod index",
            ));
        }

        let mut chooser = Chooser {
            index,
            options,
            data_dir,
            game,
            seen: BTreeSet::new(),
            chosen: Vec::new(),
            missing: Vec::new(),
        };
        for id in mod_ids {
            chooser.add(id);
        }
        let Chooser {
            chosen, missing, ..
        } = chooser;
        for id in missing {
            diagnostics.push(
                Diagnostic::new(
                    codes::MOD_NOT_FOUND,
                    Severity::Warning,
                    format!("{id} is not in the mod index and is left out of the reference set"),
                )
                .with_arg("packageId", id),
            );
        }

        let mut entries: Vec<(ModMeta, PackKind, Option<ModIdx>)> = Vec::new();
        for idx in official.into_values() {
            if let Some(meta) = index.get(idx) {
                let kind = if meta.package_id.lower() == paths::CORE_PACKAGE_ID {
                    PackKind::Core
                } else {
                    PackKind::Dlc
                };
                entries.push((meta.clone(), kind, Some(idx)));
            }
        }
        for idx in chosen {
            if let Some(meta) = index.get(idx) {
                entries.push((meta.clone(), PackKind::Mod, Some(idx)));
            }
        }
        Self::from_entries(entries, game, listing, diagnostics)
    }

    fn from_entries(
        entries: Vec<(ModMeta, PackKind, Option<ModIdx>)>,
        game: &GameVersion,
        listing: &(dyn Listing + Sync),
        mut diagnostics: Vec<Diagnostic>,
    ) -> ReferenceSet {
        let active = ActiveSet::from_ids(entries.iter().map(|(m, _, _)| m.package_id.as_str()));
        let packs: Vec<ContentPack> = entries
            .into_par_iter()
            .map(|(meta, kind, idx)| ContentPack::resolve(meta, kind, idx, game, &active, listing))
            .collect();
        for pack in &packs {
            if pack.plan.loads_nothing() {
                diagnostics.push(
                    Diagnostic::new(
                        codes::PACK_LOADS_NOTHING,
                        Severity::Info,
                        format!(
                            "{} loads no folder for game version {}",
                            pack.package_id,
                            game.short()
                        ),
                    )
                    .with_arg("packageId", pack.package_id.clone()),
                );
            }
        }
        ReferenceSet {
            game_version: game.clone(),
            packs,
            diagnostics,
        }
    }

    /// The package ids of all packs as an active set (what `IfModActive` conditions see).
    #[must_use]
    pub fn active_set(&self) -> ActiveSet {
        ActiveSet::from_ids(self.packs.iter().map(|p| p.package_id.as_str()))
    }

    /// The pack with this package id (ignoring case).
    #[must_use]
    pub fn find(&self, package_id: &str) -> Option<&ContentPack> {
        self.packs
            .iter()
            .find(|p| p.package_id.eq_ignore_ascii_case(package_id.trim()))
    }

    /// The index of the pack with this package id.
    #[must_use]
    pub fn position(&self, package_id: &str) -> Option<usize> {
        self.packs
            .iter()
            .position(|p| p.package_id.eq_ignore_ascii_case(package_id.trim()))
    }

    /// The project pack, when the set has one.
    #[must_use]
    pub fn project(&self) -> Option<&ContentPack> {
        self.packs.last().filter(|p| p.kind == PackKind::Project)
    }

    /// Total number of Defs and Patches files over all packs.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.packs.iter().map(ContentPack::file_count).sum()
    }

    /// A copy of the set with the project folder as the final pack.
    ///
    /// A pack with the project's package id (a project that is also in the list) is removed and an
    /// info diagnostic says so: the folder on disk is the one that counts. Packs whose load folders
    /// depend on the project's package id are resolved again. A previous project pack is replaced.
    ///
    /// # Errors
    /// [`crate::WorkspaceError::NotAProject`] when the folder has no About.xml, or an I/O error.
    pub fn with_project(
        &self,
        root: &Utf8Path,
        listing: &(dyn Listing + Sync),
    ) -> WorkspaceResult<ReferenceSet> {
        let read = read_mod_meta(root)?;
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(read.diagnostics);
        let project_id = read.meta.package_id.as_str().to_owned();
        let mut packs: Vec<ContentPack> = Vec::with_capacity(self.packs.len() + 1);
        for pack in &self.packs {
            if pack.kind == PackKind::Project {
                continue;
            }
            if pack.package_id.eq_ignore_ascii_case(&project_id) {
                diagnostics.push(
                    Diagnostic::new(
                        codes::PROJECT_IN_LIST,
                        Severity::Info,
                        format!(
                            "{project_id} is also in the reference list; the project folder is used, last"
                        ),
                    )
                    .with_arg("packageId", project_id.clone()),
                );
                continue;
            }
            packs.push(pack.clone());
        }
        let project = ContentPack::resolve(
            read.meta,
            PackKind::Project,
            None,
            &self.game_version,
            &ActiveSet::from_ids(
                packs
                    .iter()
                    .map(|p| p.package_id.as_str())
                    .chain(std::iter::once(project_id.as_str())),
            ),
            listing,
        );
        packs.push(project);
        let active = ActiveSet::from_ids(packs.iter().map(|p| p.package_id.as_str()));
        let game = self.game_version.clone();
        packs.par_iter_mut().for_each(|pack| {
            if pack.kind != PackKind::Project && pack.mentions(&project_id) {
                let meta = pack.meta.clone();
                *pack =
                    ContentPack::resolve(meta, pack.kind, pack.mod_idx, &game, &active, listing);
            }
        });
        Ok(ReferenceSet {
            game_version: self.game_version.clone(),
            packs,
            diagnostics,
        })
    }

    /// Resolves one pack again from its metadata (used after files of the pack changed). The
    /// project pack is re-read from disk (About and LoadFolders may have changed).
    ///
    /// # Errors
    /// An error from reading the project's About file.
    pub(crate) fn refresh_pack(
        &mut self,
        position: usize,
        listing: &(dyn Listing + Sync),
    ) -> WorkspaceResult<()> {
        let active = self.active_set();
        let game = self.game_version.clone();
        let Some(pack) = self.packs.get(position) else {
            return Ok(());
        };
        let (kind, mod_idx) = (pack.kind, pack.mod_idx);
        let meta = if kind == PackKind::Project {
            let mut meta = read_mod_meta(&pack.root)?.meta;
            meta.package_id = pack.meta.package_id.clone();
            meta
        } else {
            pack.meta.clone()
        };
        let fresh = ContentPack::resolve(meta, kind, mod_idx, &game, &active, listing);
        if let Some(slot) = self.packs.get_mut(position) {
            *slot = fresh;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_core::ids::PackageId;
    use rimstudio_core::load_plan::{LoadEntry, LoadFoldersSpec, MemListing};
    use rimstudio_core::mods::ModMeta;

    fn meta(id: &str, source: SourceId, path: &str, dirs: &[&str]) -> ModMeta {
        let mut m = ModMeta::new(
            PackageId::parse(id).unwrap(),
            id,
            source,
            Utf8PathBuf::from(path),
        );
        m.root_dirs = dirs.iter().map(|d| (*d).to_owned()).collect();
        m
    }

    fn fixture() -> (ModIndex, MemListing) {
        let mut b = ModIndex::builder();
        b.push(meta(
            "ludeon.rimworld",
            SourceId::game_data(),
            "/g/Data/Core",
            &["Defs", "Patches"],
        ));
        b.push(meta(
            "ludeon.rimworld.royalty",
            SourceId::game_data(),
            "/g/Data/Royalty",
            &["Defs"],
        ));
        b.push(meta(
            "ludeon.rimworld.ideology",
            SourceId::game_data(),
            "/g/Data/Ideology",
            &["Defs"],
        ));
        b.push(meta(
            "rs.alpha",
            SourceId::game_mods(),
            "/g/Mods/Alpha",
            &["1.6", "Common"],
        ));
        let mut gated = meta(
            "rs.gated",
            SourceId::game_mods(),
            "/g/Mods/Gated",
            &["Defs", "CE"],
        );
        gated.load_folders = Some(LoadFoldersSpec::from_blocks([(
            "1.6",
            vec![
                LoadEntry::root(),
                LoadEntry::dir("CE").if_active(["rs.alpha"]),
            ],
        )]));
        b.push(gated);
        let mut dependent = meta(
            "rs.dependent",
            SourceId::game_mods(),
            "/g/Mods/Dep",
            &["Defs"],
        );
        dependent
            .mod_dependencies
            .push(rimstudio_core::mods::ModDependency {
                package_id: "rs.alpha".into(),
                ..Default::default()
            });
        b.push(dependent);
        let listing = MemListing::new()
            .with_file("/g/Data/Core/Defs/core.xml")
            .with_file("/g/Data/Core/Patches/p.xml")
            .with_file("/g/Data/Royalty/Defs/r.xml")
            .with_file("/g/Data/Ideology/Defs/i.xml")
            .with_file("/g/Mods/Alpha/1.6/Defs/a.xml")
            .with_file("/g/Mods/Alpha/Common/Defs/c.xml")
            .with_file("/g/Mods/Gated/Defs/g.xml")
            .with_file("/g/Mods/Gated/CE/Defs/ce.xml")
            .with_file("/g/Mods/Dep/Defs/d.xml");
        (b.build(), listing)
    }

    fn game() -> GameVersion {
        GameVersion::parse("1.6.1000").unwrap()
    }

    fn ids(set: &ReferenceSet) -> Vec<&str> {
        set.packs.iter().map(|p| p.package_id.as_str()).collect()
    }

    fn list(ids: &[&str]) -> ActiveList {
        ActiveList::from_ids(ids.iter().map(|i| PackageId::parse(i).unwrap()))
    }

    #[test]
    fn game_dlc_then_mods_in_the_given_order() {
        let (index, listing) = fixture();
        let active = list(&["rs.gated", "ludeon.rimworld", "rs.alpha"]);
        let set = ReferenceSet::resolve_with(
            &index,
            &active,
            &game(),
            &ResolveOptions::default(),
            &listing,
        );
        assert_eq!(
            ids(&set),
            vec![
                "ludeon.rimworld",
                "ludeon.rimworld.royalty",
                "ludeon.rimworld.ideology",
                "rs.gated",
                "rs.alpha"
            ]
        );
        let kinds: Vec<PackKind> = set.packs.iter().map(|p| p.kind).collect();
        assert_eq!(
            kinds,
            vec![
                PackKind::Core,
                PackKind::Dlc,
                PackKind::Dlc,
                PackKind::Mod,
                PackKind::Mod
            ]
        );
        assert!(set.diagnostics.is_empty(), "{:?}", set.diagnostics);
    }

    #[test]
    fn load_folders_conditions_see_the_whole_set() {
        let (index, listing) = fixture();
        let both = ReferenceSet::resolve_with(
            &index,
            &list(&["rs.alpha", "rs.gated"]),
            &game(),
            &ResolveOptions::default(),
            &listing,
        );
        let gated = both.find("rs.gated").unwrap();
        assert_eq!(gated.plan.relative_folders(), vec!["CE", ""]);
        let rel: Vec<&str> = gated.defs.iter().map(|f| f.relative.as_str()).collect();
        assert!(rel.contains(&"Defs/ce.xml") && rel.contains(&"Defs/g.xml"));

        let only = ReferenceSet::resolve_with(
            &index,
            &list(&["rs.gated"]),
            &game(),
            &ResolveOptions::default(),
            &listing,
        );
        assert_eq!(
            only.find("rs.gated").unwrap().plan.relative_folders(),
            vec![""]
        );
        assert_eq!(only.find("rs.gated").unwrap().defs.len(), 1);
    }

    #[test]
    fn version_folders_and_common_are_listed_for_a_mod() {
        let (index, listing) = fixture();
        let set = ReferenceSet::resolve_with(
            &index,
            &list(&["rs.alpha"]),
            &game(),
            &ResolveOptions::default(),
            &listing,
        );
        let alpha = set.find("RS.ALPHA").unwrap();
        assert_eq!(alpha.plan.relative_folders(), vec!["1.6", "Common", ""]);
        assert_eq!(alpha.defs.len(), 2);
        assert_eq!(set.position("rs.alpha"), Some(3));
        assert_eq!(set.file_count(), 1 + 1 + 1 + 1 + 2);
    }

    #[test]
    fn unknown_ids_are_reported_and_left_out() {
        let (index, listing) = fixture();
        let set = ReferenceSet::resolve_with(
            &index,
            &list(&["rs.alpha", "rs.nothere"]),
            &game(),
            &ResolveOptions::default(),
            &listing,
        );
        assert_eq!(set.find("rs.nothere").map(|p| p.package_id.clone()), None);
        let d: Vec<_> = set.diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(d, vec!["workspace.mod-not-found"]);
    }

    #[test]
    fn a_missing_core_is_an_error_diagnostic() {
        let mut b = ModIndex::builder();
        b.push(meta(
            "rs.alpha",
            SourceId::game_mods(),
            "/g/Mods/Alpha",
            &["Defs"],
        ));
        let index = b.build();
        let set = ReferenceSet::resolve_with(
            &index,
            &list(&["rs.alpha"]),
            &game(),
            &ResolveOptions::default(),
            &MemListing::new(),
        );
        assert_eq!(ids(&set), vec!["rs.alpha"]);
        assert!(
            set.diagnostics
                .iter()
                .any(|d| d.code == codes::CORE_MISSING && d.severity == Severity::Error)
        );
    }

    #[test]
    fn designer_reference_includes_dependencies_before_the_mod() {
        let (index, listing) = fixture();
        let mut options = DesignerReferenceOptions::new(game()).with_mod("rs.dependent");
        options.include_dependencies = true;
        let set = ReferenceSet::reference_for_designer_with(
            &index,
            Utf8Path::new("/g"),
            &options,
            &listing,
        );
        assert_eq!(
            ids(&set),
            vec![
                "ludeon.rimworld",
                "ludeon.rimworld.royalty",
                "ludeon.rimworld.ideology",
                "rs.alpha",
                "rs.dependent"
            ]
        );
        options.include_dependencies = false;
        let plain = ReferenceSet::reference_for_designer_with(
            &index,
            Utf8Path::new("/g"),
            &options,
            &listing,
        );
        assert_eq!(plain.packs.len(), 4);
    }

    #[test]
    fn game_only_reference_has_no_mods() {
        let (index, listing) = fixture();
        let set = ReferenceSet::reference_for_designer_with(
            &index,
            Utf8Path::new("/g"),
            &DesignerReferenceOptions::new(game()),
            &listing,
        );
        assert_eq!(set.packs.len(), 3);
        assert!(set.project().is_none());
    }

    #[test]
    fn pins_choose_between_copies_of_one_package() {
        let mut b = ModIndex::builder();
        b.push(meta(
            "ludeon.rimworld",
            SourceId::game_data(),
            "/g/Data/Core",
            &["Defs"],
        ));
        b.push(meta("rs.twin", SourceId::workshop(0), "/w/1", &["Defs"]));
        b.push(meta(
            "rs.twin",
            SourceId::game_mods(),
            "/g/Mods/Twin",
            &["Defs"],
        ));
        let index = b.build();
        let default = ReferenceSet::resolve_with(
            &index,
            &list(&["rs.twin"]),
            &game(),
            &ResolveOptions::default(),
            &MemListing::new(),
        );
        assert_eq!(
            default.find("rs.twin").unwrap().root,
            Utf8PathBuf::from("/g/Mods/Twin")
        );
        let workshop = index
            .by_package_id("rs.twin")
            .iter()
            .copied()
            .find(|i| index.get(*i).unwrap().source == SourceId::workshop(0))
            .unwrap();
        let mut options = ResolveOptions::default();
        options.pins.insert("rs.twin".into(), workshop);
        let pinned = ReferenceSet::resolve_with(
            &index,
            &list(&["rs.twin"]),
            &game(),
            &options,
            &MemListing::new(),
        );
        assert_eq!(
            pinned.find("rs.twin").unwrap().root,
            Utf8PathBuf::from("/w/1")
        );
    }

    #[test]
    fn resolution_is_deterministic() {
        let (index, listing) = fixture();
        let a = list(&["rs.alpha", "rs.gated", "rs.dependent"]);
        let one =
            ReferenceSet::resolve_with(&index, &a, &game(), &ResolveOptions::default(), &listing);
        let two =
            ReferenceSet::resolve_with(&index, &a, &game(), &ResolveOptions::default(), &listing);
        assert_eq!(
            serde_json::to_string(&one).unwrap(),
            serde_json::to_string(&two).unwrap()
        );
    }
}
