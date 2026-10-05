//! Small fictional builders for core types. None of them needs XML.
//!
//! The builders panic on invalid input (an empty package id, say) with a message that names the
//! fixture: they are test code, and a wrong fixture should stop the test at once.

#![allow(clippy::expect_used)]

use camino::Utf8PathBuf;
use rimstudio_core::ids::{PackageId, SourceId, WorkshopId};
use rimstudio_core::mods::{
    ModDependency, ModIndex, ModMeta, ModSource, SourceKind, SourceSet, VersionRelations,
};
use rimstudio_core::settings::{CustomFolder, WorkspaceSettings};
use rimstudio_core::version::GameVersion;

/// A package id from text.
///
/// # Panics
/// When the text is not a valid package id.
pub fn package_id(text: &str) -> PackageId {
    PackageId::parse(text).expect("fixture package id must be valid")
}

/// The fictional package id `rs.mod<n>`.
pub fn rs_package_id(n: usize) -> PackageId {
    package_id(&format!("rs.mod{n}"))
}

/// A game version from text such as `1.9` or `1.9.9999 rev1`.
///
/// # Panics
/// When the text is not a valid version.
pub fn game_version(text: &str) -> GameVersion {
    GameVersion::parse(text).expect("fixture game version must be valid")
}

/// Builds a [`ModIndex`] from mods (the index sorts them, so the order given does not matter).
pub fn mod_index(mods: impl IntoIterator<Item = ModMeta>) -> ModIndex {
    let mut b = ModIndex::builder();
    for m in mods {
        b.push(m);
    }
    b.build()
}

/// A fluent builder of [`ModMeta`] with fictional defaults.
///
/// The defaults are: name equal to the package id, source `game-mods`, path
/// `/rs/mods/<package id>`, supported versions empty.
#[derive(Debug, Clone)]
pub struct ModBuilder {
    meta: ModMeta,
}

impl ModBuilder {
    /// Starts a mod with the given package id.
    pub fn new(package_id_text: &str) -> Self {
        let id = package_id(package_id_text);
        let path = Utf8PathBuf::from(format!("/rs/mods/{}", id.as_str()));
        ModBuilder {
            meta: ModMeta::new(
                id.clone(),
                id.as_str().to_owned(),
                SourceId::game_mods(),
                path,
            ),
        }
    }

    /// Starts the n-th fictional mod (`rs.mod<n>`).
    pub fn numbered(n: usize) -> Self {
        ModBuilder::new(&format!("rs.mod{n}"))
    }

    /// Sets the display name.
    pub fn name(mut self, name: &str) -> Self {
        self.meta.name = name.to_owned();
        self
    }

    /// Sets the source id.
    pub fn source(mut self, source: SourceId) -> Self {
        self.meta.source = source;
        self
    }

    /// Sets the folder path.
    pub fn path(mut self, path: &str) -> Self {
        self.meta.path = Utf8PathBuf::from(path);
        self
    }

    /// Sets the Workshop id (and puts the mod in the first Workshop source).
    pub fn workshop(mut self, id: u64) -> Self {
        self.meta.workshop_id =
            Some(WorkshopId::new(id).expect("fixture workshop id must be valid"));
        self.meta.source = SourceId::workshop(0);
        self
    }

    /// Sets the supported versions as written (`1.5`, `1.6`).
    pub fn supports(mut self, versions: &[&str]) -> Self {
        self.meta.supported_versions = versions.iter().map(|v| (*v).to_owned()).collect();
        self
    }

    /// Adds `loadAfter` entries.
    pub fn load_after(mut self, ids: &[&str]) -> Self {
        self.meta
            .load_after
            .extend(ids.iter().map(|v| (*v).to_owned()));
        self
    }

    /// Adds `loadBefore` entries.
    pub fn load_before(mut self, ids: &[&str]) -> Self {
        self.meta
            .load_before
            .extend(ids.iter().map(|v| (*v).to_owned()));
        self
    }

    /// Adds `forceLoadAfter` entries.
    pub fn force_load_after(mut self, ids: &[&str]) -> Self {
        self.meta
            .force_load_after
            .extend(ids.iter().map(|v| (*v).to_owned()));
        self
    }

    /// Adds `forceLoadBefore` entries.
    pub fn force_load_before(mut self, ids: &[&str]) -> Self {
        self.meta
            .force_load_before
            .extend(ids.iter().map(|v| (*v).to_owned()));
        self
    }

    /// Adds `incompatibleWith` entries.
    pub fn incompatible_with(mut self, ids: &[&str]) -> Self {
        self.meta
            .incompatible_with
            .extend(ids.iter().map(|v| (*v).to_owned()));
        self
    }

    /// Adds a dependency on a package id.
    pub fn depends_on(mut self, package_id_text: &str) -> Self {
        self.meta.mod_dependencies.push(ModDependency {
            package_id: package_id_text.to_owned(),
            display_name: package_id_text.to_owned(),
            ..ModDependency::default()
        });
        self
    }

    /// Adds a per version block (`ByVersion` lists) for a version key such as `1.6`.
    pub fn by_version(mut self, version: &str, relations: VersionRelations) -> Self {
        self.meta.by_version.insert(version.to_owned(), relations);
        self
    }

    /// Adds folders directly inside the mod root (as the scanner would list them).
    pub fn root_dirs(mut self, dirs: &[&str]) -> Self {
        self.meta.root_dirs = dirs.iter().map(|d| (*d).to_owned()).collect();
        self
    }

    /// The finished metadata.
    pub fn build(self) -> ModMeta {
        self.meta
    }
}

impl From<ModBuilder> for ModMeta {
    fn from(b: ModBuilder) -> Self {
        b.build()
    }
}

/// A source set with the three usual built in sources rooted under `root`:
/// `game-data` at `<root>/Data`, `game-mods` at `<root>/Mods` and `workshop-0` at
/// `<root>/workshop`.
pub fn standard_sources(root: &str) -> SourceSet {
    let mut set = SourceSet::new();
    let root = root.trim_end_matches('/');
    set.insert(ModSource::new(
        SourceId::game_data(),
        SourceKind::GameData,
        Utf8PathBuf::from(format!("{root}/Data")),
    ));
    set.insert(ModSource::new(
        SourceId::game_mods(),
        SourceKind::GameMods,
        Utf8PathBuf::from(format!("{root}/Mods")),
    ));
    set.insert(ModSource::new(
        SourceId::workshop(0),
        SourceKind::Workshop,
        Utf8PathBuf::from(format!("{root}/workshop")),
    ));
    set
}

/// A custom folder with a deterministic id derived from `seed` (`cf_<8 hex>`).
pub fn custom_folder(seed: &str, path: &str) -> CustomFolder {
    CustomFolder::new(
        SourceId::custom_from_seed(seed.as_bytes()),
        Utf8PathBuf::from(path),
    )
}

/// Workspace settings holding the given custom folders and nothing else.
pub fn workspace_with_folders(folders: Vec<CustomFolder>) -> WorkspaceSettings {
    WorkspaceSettings {
        custom_mod_folders: folders,
        ..WorkspaceSettings::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_defaults_are_fictional_and_consistent() {
        let m = ModBuilder::new("rs.alpha").build();
        assert_eq!(m.package_id.as_str(), "rs.alpha");
        assert_eq!(m.name, "rs.alpha");
        assert_eq!(m.source, SourceId::game_mods());
        assert_eq!(m.path, "/rs/mods/rs.alpha");
        assert_eq!(rs_package_id(3).as_str(), "rs.mod3");
        assert_eq!(
            ModBuilder::numbered(7).build().package_id.as_str(),
            "rs.mod7"
        );
    }

    #[test]
    fn builder_sets_relations_and_versions() {
        let m = ModBuilder::new("rs.b")
            .name("RS_B")
            .supports(&["1.5", "1.6"])
            .load_after(&["rs.a"])
            .load_before(&["rs.c"])
            .force_load_after(&["rs.d"])
            .force_load_before(&["rs.e"])
            .incompatible_with(&["rs.f"])
            .depends_on("rs.g")
            .root_dirs(&["Defs"])
            .by_version(
                "1.6",
                VersionRelations {
                    load_after: vec!["rs.h".into()],
                    ..VersionRelations::default()
                },
            )
            .build();
        assert_eq!(m.name, "RS_B");
        assert!(m.supports(&game_version("1.6")));
        assert!(!m.supports(&game_version("1.4")));
        assert_eq!(
            m.effective_load_after(&game_version("1.6")),
            vec!["rs.a", "rs.h"]
        );
        assert_eq!(m.effective_load_after(&game_version("1.5")), vec!["rs.a"]);
        assert_eq!(m.load_before, vec!["rs.c"]);
        assert_eq!(m.force_load_after, vec!["rs.d"]);
        assert_eq!(m.force_load_before, vec!["rs.e"]);
        assert_eq!(m.incompatible_with, vec!["rs.f"]);
        assert_eq!(m.mod_dependencies[0].package_id, "rs.g");
        assert_eq!(m.root_dirs, vec!["Defs"]);
    }

    #[test]
    fn workshop_builder_moves_the_mod_to_a_workshop_source() {
        let m = ModBuilder::new("rs.w").workshop(1234).build();
        assert_eq!(m.workshop_id.map(WorkshopId::get), Some(1234));
        assert!(m.source.is_workshop());
    }

    #[test]
    fn index_does_not_depend_on_the_order_given() {
        let a = mod_index([
            ModBuilder::new("rs.b").build(),
            ModBuilder::new("rs.a").build(),
        ]);
        let b = mod_index([
            ModBuilder::new("rs.a").build(),
            ModBuilder::new("rs.b").build(),
        ]);
        let names = |i: &ModIndex| {
            i.iter()
                .map(|(_, m)| m.package_id.as_str().to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&a), names(&b));
        assert_eq!(a.len(), 2);
    }

    #[test]
    fn standard_sources_have_three_roots() {
        let s = standard_sources("/rs/game/");
        assert_eq!(s.len(), 3);
        assert_eq!(
            s.get(&SourceId::game_mods()).map(|x| x.path.as_str()),
            Some("/rs/game/Mods")
        );
    }

    #[test]
    fn custom_folder_ids_are_deterministic() {
        let a = custom_folder("one", "/rs/custom");
        let b = custom_folder("one", "/rs/custom");
        assert_eq!(a.id, b.id);
        assert!(a.id.is_custom());
        assert_ne!(a.id, custom_folder("two", "/rs/custom").id);
        let ws = workspace_with_folders(vec![a]);
        assert_eq!(ws.custom_mod_folders.len(), 1);
        assert_eq!(ws.custom_sources().len(), 1);
    }
}
