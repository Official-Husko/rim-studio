//! Workspace session tests: open, provenance, stale detection, rebuild, project overlay, search and
//! the type table built from assemblies. Everything is fictional; the real install tests at the end
//! are `#[ignore]` and only read.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

#[path = "../../rimstudio-defs/tests/assembly_support/mod.rs"]
mod assembly_support;
mod common;

use std::sync::Arc;

use assembly_support::{Extends, ImageBuilder};
use camino::Utf8PathBuf;
use common::{TAG, active, game, install, scan, thing, types, utf8};
use rimstudio_core::jobs::{CancelToken, CollectingProgress, NoopProgress};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_testing::install_tree::{InstallBuilder, ModFolder};
use rimstudio_workspace::defindex::{Page, Query};
use rimstudio_workspace::parse::ParseCache;
use rimstudio_workspace::refset::ReferenceSet;
use rimstudio_workspace::session::{OpenInput, WorkspaceSession};
use rimstudio_workspace::snapshot::DefRef;
use rimstudio_workspace::typetable::CacheConfig;

fn reference(inst: &rimstudio_testing::install_tree::BuiltInstall) -> ReferenceSet {
    ReferenceSet::resolve(
        &scan(inst),
        &active(&["ludeon.rimworld", "rs.base", "rs.gated"]),
        &game(),
    )
}

fn open(inst: &rimstudio_testing::install_tree::BuiltInstall) -> WorkspaceSession {
    WorkspaceSession::open_simple(OpenInput::new(reference(inst)).with_type_table(types())).unwrap()
}

#[test]
fn open_loads_defs_with_overrides_patches_and_provenance() {
    let inst = install();
    let session = open(&inst);
    let snap = session.current();
    assert_eq!(snap.revision, 1);
    let db = session.snapshot();

    // the later mod overrides the core def; the winner is the mod's
    let rifle = db.get(TAG, "RS_CoreRifle").unwrap();
    assert_eq!(rifle.node.child_text("label"), Some("modded rifle"));
    let view = session
        .resolve_def(&DefRef::new(TAG, "RS_CoreRifle"))
        .unwrap();
    assert_eq!(view.provenance.pack.as_ref().unwrap().package_id, "rs.base");
    assert!(
        view.provenance
            .file
            .as_ref()
            .unwrap()
            .path
            .ends_with("RS_Base/Defs/RS_Base.xml")
    );
    // the core copy can be asked for explicitly
    let core_copy = session
        .resolve_def(&DefRef::new(TAG, "RS_CoreRifle").in_pack("LUDEON.RIMWORLD"))
        .unwrap();
    assert_eq!(core_copy.node.child_text("label"), Some("core rifle"));

    // inheritance and the patch
    let pistol = session.resolve_def(&DefRef::new(TAG, "RS_Pistol")).unwrap();
    assert_eq!(pistol.node.child_text("label"), Some("patched pistol"));
    assert_eq!(
        pistol
            .node
            .child("statBases")
            .and_then(|s| s.child_text("RS_Mass")),
        Some("2")
    );
    assert_eq!(pistol.provenance.parents.len(), 1);
    assert_eq!(pistol.provenance.parents[0].name, "RS_BaseGun");
    assert_eq!(pistol.provenance.patched_by.len(), 1);
    let patch = &pistol.provenance.patched_by[0];
    assert_eq!(patch.class, "PatchOperationReplace");
    assert_eq!(patch.pack.as_ref().unwrap().package_id, "rs.base");
    assert!(
        patch
            .file
            .as_ref()
            .unwrap()
            .relative
            .starts_with("Patches/")
    );

    // the gated folder loaded because rs.base is in the set
    assert!(db.get(TAG, "RS_GatedDef").is_some());
    // an abstract node is not a def
    assert!(
        session
            .resolve_def(&DefRef::new(TAG, "RS_BaseGun"))
            .is_none()
    );
    assert!(
        session
            .resolve_def(&DefRef::new("NoSuchDef", "x"))
            .is_none()
    );
}

#[test]
fn a_broken_defs_file_is_a_diagnostic_and_the_rest_loads() {
    let inst = install();
    let session = open(&inst);
    let snap = session.current();
    let code = rimstudio_core::diag::DiagCode::new("defs.xml-parse-error");
    assert_eq!(
        snap.diagnostics.count(&code),
        1,
        "{:?}",
        snap.diagnostics.counts
    );
    assert!(session.snapshot().get(TAG, "RS_ModdedGun").is_some());
}

#[test]
fn the_session_is_deterministic_for_any_thread_count() {
    let inst = install();
    let a = WorkspaceSession::open_simple(
        OpenInput::new(reference(&inst))
            .with_type_table(types())
            .with_threads(1),
    )
    .unwrap();
    let b = WorkspaceSession::open_simple(
        OpenInput::new(reference(&inst))
            .with_type_table(types())
            .with_threads(4),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_string(&*a.current().defs).unwrap(),
        serde_json::to_string(&*b.current().defs).unwrap()
    );
    let q = Query::default();
    assert_eq!(
        serde_json::to_string(&a.search(&q, Page::default())).unwrap(),
        serde_json::to_string(&b.search(&q, Page::default())).unwrap()
    );
}

#[test]
fn a_second_open_with_the_shared_cache_parses_zero_files() {
    let inst = install();
    let cache = Arc::new(ParseCache::new());
    let first = WorkspaceSession::open_simple(
        OpenInput::new(reference(&inst))
            .with_type_table(types())
            .with_parse_cache(Arc::clone(&cache)),
    )
    .unwrap();
    assert_eq!(first.stats().files_parsed, first.stats().files_total);
    assert!(first.stats().files_total >= 6);
    let second = WorkspaceSession::open_simple(
        OpenInput::new(reference(&inst))
            .with_type_table(types())
            .with_parse_cache(cache),
    )
    .unwrap();
    assert_eq!(second.stats().files_parsed, 0);
    assert_eq!(second.stats().files_cached, second.stats().files_total);
    assert_eq!(
        serde_json::to_string(&*first.current().defs).unwrap(),
        serde_json::to_string(&*second.current().defs).unwrap()
    );
}

#[test]
fn stale_detection_and_rebuild_changed_reparse_only_the_changed_file() {
    let inst = install();
    let mut session = open(&inst);
    assert!(!session.is_stale());
    let base_defs = inst.mods_dir.join("RS_Base/Defs/RS_Base.xml");
    std::fs::write(
        &base_defs,
        "<Defs><RS_ThingDef><defName>RS_CoreRifle</defName><label>rewritten rifle</label></RS_ThingDef></Defs>",
    )
    .unwrap();
    assert!(session.is_stale());

    let report = session
        .rebuild_changed(std::slice::from_ref(&base_defs))
        .unwrap();
    assert_eq!(report.revision, 2);
    assert_eq!(report.files_parsed, 1);
    assert_eq!(report.packs_refreshed, 1);
    assert!(!session.is_stale());
    assert_eq!(session.revision(), 2);
    let rifle = session
        .resolve_def(&DefRef::new(TAG, "RS_CoreRifle"))
        .unwrap();
    assert_eq!(rifle.node.child_text("label"), Some("rewritten rifle"));
    // the def that was only in the old file is gone
    assert!(session.snapshot().get(TAG, "RS_ModdedGun").is_none());

    // a deleted file is stale too and the rebuild forgets it
    std::fs::remove_file(&base_defs).unwrap();
    assert!(session.is_stale());
    let report = session.rebuild_changed(&[base_defs]).unwrap();
    assert_eq!(report.files_parsed, 0);
    assert!(!session.is_stale());
    assert_eq!(
        session
            .resolve_def(&DefRef::new(TAG, "RS_CoreRifle"))
            .unwrap()
            .node
            .child_text("label"),
        Some("core rifle")
    );
}

fn project_install() -> (
    rimstudio_testing::install_tree::TempInstall,
    tempfile::TempDir,
    Utf8PathBuf,
) {
    let inst = install();
    let projects = tempfile::tempdir().unwrap();
    let root = utf8(projects.path());
    let path = ModFolder::new("RS_Project", "rs.project")
        .name("RS Project")
        .defs_file(
            "RS_Mine.xml",
            vec![
                thing("RS_MyGun", "my gun"),
                thing("RS_CoreRifle", "project rifle"),
            ],
        )
        .write_to(&root)
        .unwrap();
    (inst, projects, path)
}

#[test]
fn the_project_overlay_is_the_last_pack_and_its_defs_are_in_the_snapshot() {
    let (inst, _projects, project) = project_install();
    let input = OpenInput::new(reference(&inst))
        .with_type_table(types())
        .with_project(&project)
        .unwrap();
    let session = WorkspaceSession::open_simple(input).unwrap();
    let packs = &session.reference().packs;
    assert_eq!(packs.last().unwrap().package_id, "rs.project");
    let mine = session.resolve_def(&DefRef::new(TAG, "RS_MyGun")).unwrap();
    assert_eq!(
        mine.provenance.pack.as_ref().unwrap().package_id,
        "rs.project"
    );
    // the project loads last, so it overrides the other copies
    let rifle = session
        .resolve_def(&DefRef::new(TAG, "RS_CoreRifle"))
        .unwrap();
    assert_eq!(rifle.node.child_text("label"), Some("project rifle"));
    let hits = session.search(&Query::default().in_pack("rs.project"), Page::default());
    assert_eq!(hits.total, 2);
}

#[test]
fn editing_the_project_makes_the_session_stale_and_new_files_are_seen() {
    let (inst, _projects, project) = project_install();
    let input = OpenInput::new(reference(&inst))
        .with_type_table(types())
        .with_project(&project)
        .unwrap();
    let mut session = WorkspaceSession::open_simple(input).unwrap();
    assert!(!session.is_stale());
    let new_file = project.join("Defs/RS_New.xml");
    std::fs::write(
        &new_file,
        "<Defs><RS_ThingDef><defName>RS_NewGun</defName><label>new</label></RS_ThingDef></Defs>",
    )
    .unwrap();
    assert!(session.is_stale(), "a new file in the project is a change");
    let report = session.rebuild_changed(&[new_file]).unwrap();
    assert_eq!(report.files_parsed, 1);
    assert!(
        session
            .resolve_def(&DefRef::new(TAG, "RS_NewGun"))
            .is_some()
    );
    assert!(!session.is_stale());
}

#[test]
fn search_finds_defs_by_text_type_and_pack() {
    let inst = install();
    let session = open(&inst);
    let r = session.search(&Query::text("rifle"), Page::default());
    assert_eq!(r.total, 2);
    assert_eq!(r.hits[0].def_name, "RS_CoreRifle");
    let r = session.search(&Query::text("base").concrete_only(), Page::default());
    assert_eq!(r.total, 0);
    let r = session.search(&Query::text("base"), Page::default());
    assert_eq!(r.total, 1);
    assert!(r.hits[0].is_abstract);
    assert_eq!(r.hits[0].name.as_deref(), Some("RS_BaseGun"));
    let r = session.search(&Query::default().in_pack("rs.gated"), Page::default());
    assert_eq!(r.total, 2);
    let index = session.index();
    assert_eq!(index.types()[0].def_type, TAG);
    assert!(index.find(Some(TAG), "RS_Pistol").len() == 1);
    // the broken file contributes nothing to the index
    assert!(index.find(None, "RS_Broken").is_empty());
}

#[test]
fn progress_is_reported_and_cancel_is_an_error() {
    let inst = install();
    let progress = CollectingProgress::new();
    let session = WorkspaceSession::open(
        OpenInput::new(reference(&inst)).with_type_table(types()),
        &progress,
        &CancelToken::new(),
    )
    .unwrap();
    let phases: Vec<String> = progress.records().into_iter().map(|p| p.phase).collect();
    for want in ["workspace.types", "workspace.parse", "workspace.load"] {
        assert!(phases.iter().any(|p| p == want), "{want} in {phases:?}");
    }
    assert!(session.stats().files_total > 0);

    let cancel = CancelToken::new();
    cancel.cancel();
    let err = WorkspaceSession::open(
        OpenInput::new(reference(&inst)).with_type_table(types()),
        &NoopProgress,
        &cancel,
    )
    .unwrap_err();
    assert_eq!(err.code(), "workspace.cancelled");
}

// ---------------------------------------------------------------- the type table from assemblies

fn game_dll() -> Vec<u8> {
    let mut b = ImageBuilder::new("RS_Game");
    let corlib = b.assembly_ref("mscorlib");
    let object = b.type_ref(corlib, "System", "Object");
    let editable = b.add_type("Verse", "RS_Editable", 0, Extends::Ref(object));
    let def = b.add_type("Verse", "Def", 0, Extends::Def(editable));
    b.add_type("Verse", TAG, 0, Extends::Def(def));
    b.build()
}

fn mod_dll(extra: bool) -> Vec<u8> {
    let mut b = ImageBuilder::new("RS_Mod");
    let game = b.assembly_ref("RS_Game");
    let thing_ref = b.type_ref(game, "Verse", TAG);
    b.add_type("RS_Mod", "RS_AmmoDef", 0, Extends::Ref(thing_ref));
    if extra {
        b.add_type("RS_Mod", "RS_ExtraDef", 0, Extends::Ref(thing_ref));
    }
    b.build()
}

struct AssemblyFixture {
    inst: rimstudio_testing::install_tree::TempInstall,
    cache_dir: tempfile::TempDir,
}

fn assembly_fixture() -> AssemblyFixture {
    let ammo = NodeLike::ammo("RS_Ammo");
    let inst = InstallBuilder::new()
        .core_def(thing("RS_CoreRifle", "core rifle"))
        .mod_folder(
            ModFolder::new("RS_Ammo", "rs.ammo")
                .name("RS Ammo")
                .def(ammo)
                .raw_file("Assemblies/RS_Mod.dll", mod_dll(false))
                .raw_file(
                    "Assemblies/native.dll",
                    b"this is not a managed library".to_vec(),
                )
                .raw_file("Assemblies/broken.dll", {
                    let mut bytes = mod_dll(true);
                    bytes.truncate(bytes.len() / 2);
                    bytes
                }),
        )
        .build_temp()
        .unwrap();
    let managed = inst.game_dir.join("RimWorldLinux_Data/Managed");
    std::fs::create_dir_all(&managed).unwrap();
    std::fs::write(managed.join("Assembly-CSharp.dll"), game_dll()).unwrap();
    std::fs::write(
        managed.join("mscorlib.dll"),
        b"runtime libraries are not read",
    )
    .unwrap();
    AssemblyFixture {
        inst,
        cache_dir: tempfile::tempdir().unwrap(),
    }
}

struct NodeLike;

impl NodeLike {
    fn ammo(name: &str) -> rimstudio_core::tree::Node {
        rimstudio_core::tree::NodeBuilder::new("RS_Mod.RS_AmmoDef")
            .text_elem("defName", name)
            .text_elem("label", "ammo")
            .build()
    }
}

fn assembly_session(f: &AssemblyFixture) -> WorkspaceSession {
    let reference = ReferenceSet::resolve(
        &scan(&f.inst),
        &active(&["ludeon.rimworld", "rs.ammo"]),
        &game(),
    );
    let cache = CacheConfig::new(utf8(f.cache_dir.path()), Arc::new(FakeClock::new(10)));
    WorkspaceSession::open_simple(
        OpenInput::new(reference)
            .with_game_dir(f.inst.game_dir.clone())
            .with_cache(cache),
    )
    .unwrap()
}

#[test]
fn the_type_table_is_built_from_game_and_mod_assemblies() {
    let f = assembly_fixture();
    let session = assembly_session(&f);
    let table = session.type_table();
    assert!(table.info("Verse.Def").is_some());
    assert_eq!(table.lookup("RS_Mod.RS_AmmoDef"), Some("RS_Mod.RS_AmmoDef"));
    assert!(table.is_a("RS_Mod.RS_AmmoDef", "Verse.Def"));
    let stats = session.stats();
    assert_eq!((stats.assemblies_read, stats.assemblies_skipped), (2, 2));
    assert!(!stats.type_table_cached);
    let diags = session.diagnostics();
    assert_eq!(
        diags.count(&rimstudio_core::diag::DiagCode::new(
            "workspace.assembly-not-managed"
        )),
        1
    );
    assert_eq!(
        diags.count(&rimstudio_core::diag::DiagCode::new(
            "workspace.assembly-skipped"
        )),
        1
    );
    // the mod's own def class resolves, so its def is a def of the mod's type
    let ammo = session
        .resolve_def(&DefRef::new("RS_Mod.RS_AmmoDef", "RS_Ammo"))
        .unwrap();
    assert_eq!(ammo.provenance.type_name, "RS_Mod.RS_AmmoDef");
    let rifle = session
        .resolve_def(&DefRef::new(TAG, "RS_CoreRifle"))
        .unwrap();
    assert_eq!(rifle.provenance.type_name, format!("Verse.{TAG}"));
    assert_eq!(session.current().stats.unknown_type, 0);
}

#[test]
fn the_type_table_cache_is_used_until_a_dll_changes() {
    let f = assembly_fixture();
    let first = assembly_session(&f);
    assert!(!first.stats().type_table_cached);
    let second = assembly_session(&f);
    assert!(second.stats().type_table_cached, "same files, same keys");
    assert_eq!(second.stats().assemblies_read, 0);
    assert_eq!(
        second.type_table().to_json_value(),
        first.type_table().to_json_value()
    );

    // a new mod DLL: rebuilt, the new class resolves, and the next open caches that table
    let dll = f.inst.mods_dir.join("RS_Ammo/Assemblies/RS_Mod.dll");
    std::fs::write(&dll, mod_dll(true)).unwrap();
    let third = assembly_session(&f);
    assert!(!third.stats().type_table_cached);
    assert_eq!(
        third.type_table().lookup("RS_Mod.RS_ExtraDef"),
        Some("RS_Mod.RS_ExtraDef")
    );
    assert!(assembly_session(&f).stats().type_table_cached);
}

#[test]
fn rebuild_changed_reads_assemblies_again_only_when_one_changed() {
    let f = assembly_fixture();
    let mut session = assembly_session(&f);
    let defs = f.inst.mods_dir.join("RS_Ammo/Defs/RS_Defs.xml");
    assert!(defs.exists(), "fixture layout");
    std::fs::write(
        &defs,
        "<Defs><RS_Mod.RS_AmmoDef><defName>RS_Ammo</defName><label>changed</label></RS_Mod.RS_AmmoDef></Defs>",
    )
    .unwrap();
    session.rebuild_changed(&[defs]).unwrap();
    assert!(session.stats().type_table_cached, "no DLL changed");
    let dll = f.inst.mods_dir.join("RS_Ammo/Assemblies/RS_Mod.dll");
    std::fs::write(&dll, mod_dll(true)).unwrap();
    assert!(session.is_stale());
    session.rebuild_changed(&[dll]).unwrap();
    assert!(!session.stats().type_table_cached);
    assert!(session.type_table().lookup("RS_Mod.RS_ExtraDef").is_some());
    assert!(!session.is_stale());
}

#[test]
fn without_a_game_folder_the_open_still_succeeds_with_diagnostics() {
    let inst = install();
    let session = WorkspaceSession::open_simple(OpenInput::new(reference(&inst))).unwrap();
    let diags = session.diagnostics();
    assert_eq!(
        diags.count(&rimstudio_core::diag::DiagCode::new(
            "workspace.no-game-assemblies"
        )),
        1
    );
    // every def is of an unknown type, none is created
    assert_eq!(session.current().stats.defs, 0);
    assert!(session.current().stats.unknown_type > 0);
    // the index does not depend on the type table
    assert!(session.index().len() >= 6);
}

#[test]
fn sessions_and_snapshots_can_be_shared_between_threads() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<WorkspaceSession>();
    assert_send_sync::<rimstudio_workspace::snapshot::Snapshot>();
    assert_send_sync::<rimstudio_workspace::defindex::DefIndex>();
    assert_send_sync::<ReferenceSet>();
    assert_send_sync::<ParseCache>();
}
