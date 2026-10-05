//! Shared fixtures of the integration tests: a fictional install, its mod index and a type table.
//!
//! Everything is fictional (names start with `RS_`). Nothing here reads a real install.

#![allow(dead_code, unreachable_pub)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::sync::Arc;

use camino::Utf8PathBuf;
use rimstudio_core::ids::{PackageId, SourceId};
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_core::load_plan::{LoadEntry, LoadFoldersSpec};
use rimstudio_core::mods::{ActiveList, ModIndex, ModSource, SourceKind, SourceSet};
use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_core::version::GameVersion;
use rimstudio_defs::{TypeInfo, TypeTable};
use rimstudio_library::scan::{ScanOptions, Scanner};
use rimstudio_testing::install_tree::{BuiltInstall, InstallBuilder, ModFolder, TempInstall};

pub const TAG: &str = "RS_ThingDef";

pub fn utf8(p: &std::path::Path) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(p.to_path_buf()).unwrap()
}

pub fn thing(name: &str, label: &str) -> Node {
    NodeBuilder::new(TAG)
        .text_elem("defName", name)
        .text_elem("label", label)
        .build()
}

pub fn abstract_base(name: &str) -> Node {
    NodeBuilder::new(TAG)
        .attr("Name", name)
        .attr("Abstract", "True")
        .text_elem("label", "base")
        .child(NodeBuilder::new("statBases").text_elem("RS_Mass", "2"))
        .build()
}

pub fn child(name: &str, parent: &str) -> Node {
    NodeBuilder::new(TAG)
        .attr("ParentName", parent)
        .text_elem("defName", name)
        .text_elem("label", "child")
        .build()
}

pub fn replace_label(def_name: &str, label: &str) -> Node {
    NodeBuilder::new("Operation")
        .attr("Class", "PatchOperationReplace")
        .text_elem("xpath", format!("Defs/{TAG}[defName=\"{def_name}\"]/label"))
        .child(NodeBuilder::new("value").text_elem("label", label))
        .build()
}

pub fn types() -> Arc<TypeTable> {
    Arc::new(
        TypeTable::new(vec![
            (
                "Verse.Def".to_owned(),
                TypeInfo::with_base("Verse.Editable"),
            ),
            (format!("Verse.{TAG}"), TypeInfo::with_base("Verse.Def")),
            (
                "RS_Mod.RS_AmmoDef".to_owned(),
                TypeInfo::with_base(format!("Verse.{TAG}")),
            ),
        ])
        .unwrap(),
    )
}

pub fn game() -> GameVersion {
    GameVersion::parse("1.6.1000").unwrap()
}

/// The fictional install:
///
/// - Core: `RS_CoreRifle`, abstract `RS_BaseGun`, child `RS_Pistol`.
/// - Royalty (expansion): `RS_RoyalGun`.
/// - `rs.base` (Mods): overrides `RS_CoreRifle`, adds `RS_ModdedGun`, patches the label of
///   `RS_Pistol`, and has one broken Defs file.
/// - `rs.gated` (Mods): `LoadFolders.xml` with the root and a folder `CE` listed only when
///   `rs.base` is active; `CE/Defs` defines `RS_GatedDef`.
pub fn install() -> TempInstall {
    InstallBuilder::new()
        .core_defs_file(
            "RS_Core.xml",
            vec![
                thing("RS_CoreRifle", "core rifle"),
                abstract_base("RS_BaseGun"),
                child("RS_Pistol", "RS_BaseGun"),
            ],
        )
        .dlc(
            ModFolder::new("Royalty", "ludeon.rimworld.royalty")
                .name("Royalty")
                .defs_file("RS_Royal.xml", vec![thing("RS_RoyalGun", "royal gun")]),
        )
        .mod_folder(
            ModFolder::new("RS_Base", "rs.base")
                .name("RS Base")
                .defs_file(
                    "RS_Base.xml",
                    vec![
                        thing("RS_CoreRifle", "modded rifle"),
                        thing("RS_ModdedGun", "modded gun"),
                    ],
                )
                .patch_file(
                    "RS_Patch.xml",
                    vec![replace_label("RS_Pistol", "patched pistol")],
                )
                .raw_file("Defs/RS_Broken.xml", b"<Defs><RS_ThingDef></Defs>".to_vec()),
        )
        .mod_folder(
            ModFolder::new("RS_Gated", "rs.gated")
                .name("RS Gated")
                .defs_file("RS_Gated.xml", vec![thing("RS_GatedPlain", "plain")])
                .defs_file_in("CE", "RS_GatedCE.xml", vec![thing("RS_GatedDef", "gated")])
                .load_folders(LoadFoldersSpec::from_blocks([(
                    "1.6",
                    vec![
                        LoadEntry::root(),
                        LoadEntry::dir("CE").if_active(["rs.base"]),
                    ],
                )])),
        )
        .build_temp()
        .unwrap()
}

pub fn sources(b: &BuiltInstall) -> SourceSet {
    let mut set = SourceSet::new();
    set.insert(ModSource::new(
        SourceId::game_data(),
        SourceKind::GameData,
        b.data_dir.clone(),
    ));
    set.insert(ModSource::new(
        SourceId::game_mods(),
        SourceKind::GameMods,
        b.mods_dir.clone(),
    ));
    set.insert(ModSource::new(
        SourceId::workshop(0),
        SourceKind::Workshop,
        b.workshop_dir.clone(),
    ));
    set
}

/// Scans the install into a mod index (level 0, About and folder names).
pub fn scan(b: &BuiltInstall) -> ModIndex {
    let outcome = Scanner::scan(
        &sources(b),
        &ScanOptions::default().with_game_version(game()),
        &NoopProgress,
        &CancelToken::new(),
    );
    let index: &ModIndex = &outcome.index;
    index.clone()
}

pub fn active(ids: &[&str]) -> ActiveList {
    ActiveList::from_ids(ids.iter().map(|i| PackageId::parse(i).unwrap()))
}
