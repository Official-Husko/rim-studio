//! Scanner integration tests on fictional installs written to temporary folders.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::diag::DiagCode;
use rimstudio_core::ids::SourceId;
use rimstudio_core::jobs::{CancelToken, NoopProgress, Progress, ProgressSink};
use rimstudio_core::load_plan::{LoadEntry, LoadFoldersSpec};
use rimstudio_core::mods::{ModMeta, ModSource, SourceKind, SourceSet, VersionRelations};
use rimstudio_core::settings::{CustomFolder, FolderLayout};
use rimstudio_core::tree::NodeBuilder;
use rimstudio_core::version::GameVersion;
use rimstudio_library::cache::{LoadStatus, Manifest};
use rimstudio_library::duplicates::{DuplicatePolicy, resolve};
use rimstudio_library::index::{AboutStatus, Loadability};
use rimstudio_library::scan::{ScanLevel, ScanOptions, ScanOutcome, Scanner, SourceStatus};
use rimstudio_testing::install_tree::{BuiltInstall, InstallBuilder, ModFolder};

fn def(name: &str) -> rimstudio_core::tree::Node {
    NodeBuilder::new("ThingDef")
        .text_elem("defName", name)
        .text_elem("label", "rs test")
        .build()
}

fn custom_id() -> SourceId {
    SourceId::new("cf_aaaaaaaa").unwrap()
}

fn sources(b: &BuiltInstall) -> SourceSet {
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
    set.insert(ModSource::new(
        custom_id(),
        SourceKind::Custom,
        b.custom_dir.clone(),
    ));
    set
}

fn scan(set: &SourceSet, opts: &ScanOptions) -> ScanOutcome {
    Scanner::scan(set, opts, &NoopProgress, &CancelToken::new())
}

fn opts() -> ScanOptions {
    ScanOptions::default().with_game_version(GameVersion::parse("1.6.1000").unwrap())
}

fn code(text: &'static str) -> DiagCode {
    DiagCode::new(text)
}

fn find<'a>(out: &'a ScanOutcome, package: &str) -> (&'a ModMeta, rimstudio_core::ids::ModIdx) {
    let (idx, meta) = out.index.first_by_package_id(package).unwrap();
    (meta, idx)
}

fn basic_install() -> rimstudio_testing::install_tree::TempInstall {
    InstallBuilder::new()
        .core_def(def("RS_CoreThing"))
        .mod_folder(
            ModFolder::new("RS_ModsA", "rs.mods.a")
                .name("RS Mods A")
                .author("Tester")
                .supports(&["1.6"])
                .def(def("RS_A1"))
                .def(def("RS_A2")),
        )
        .workshop_mod(
            111,
            ModFolder::new("ignored", "rs.workshop.one")
                .name("RS Workshop One")
                .def(def("RS_W1")),
        )
        .custom_mod(ModFolder::new("RS_Custom", "rs.custom.one").name("RS Custom One"))
        .build_temp()
        .unwrap()
}

#[test]
fn all_source_kinds_are_scanned_with_the_right_identity_and_visibility() {
    let b = basic_install();
    let out = scan(&sources(&b), &opts());
    assert!(!out.cancelled);
    assert_eq!(out.index.len(), 4);
    let (core, _) = find(&out, "ludeon.rimworld");
    assert_eq!(core.source, SourceId::game_data());
    let (w, widx) = find(&out, "rs.workshop.one");
    assert_eq!(w.workshop_id.map(|x| x.get()), Some(111));
    assert_eq!(
        out.index.info(widx).unwrap().loadable,
        Loadability::Loadable
    );
    let (c, cidx) = find(&out, "rs.custom.one");
    assert_eq!(c.source, custom_id());
    assert_eq!(c.workshop_id, None);
    assert_eq!(
        out.index.info(cidx).unwrap().loadable,
        Loadability::NeedsLink
    );
    assert_eq!(out.index.loadable_count(), 3);
    let (a, aidx) = find(&out, "rs.mods.a");
    assert_eq!(a.name, "RS Mods A");
    assert_eq!(a.authors, vec!["Tester".to_owned()]);
    assert_eq!(out.index.info(aidx).unwrap().about, AboutStatus::Parsed);
    assert_eq!(out.index.content(aidx).unwrap().def_count(), 2);
    assert_eq!(out.stats.defs, 4);
    for r in &out.sources {
        assert_eq!(r.status, SourceStatus::Ready);
    }
    assert_eq!(out.sources.iter().map(|r| r.mods).sum::<usize>(), 4);
    // Search works through the outcome.
    let hits = out.index.search("workshop one", 5);
    assert_eq!(hits.len(), 1);
}

#[test]
fn the_five_lowercase_about_cases_are_all_found() {
    let b = InstallBuilder::new()
        .mod_folder(ModFolder::new("RS_Canon", "rs.canon"))
        .mod_folder(ModFolder::new("RS_LowDir", "rs.lowdir").lowercase_about_dir())
        .mod_folder(ModFolder::new("RS_LowFile", "rs.lowfile").about_file_name("about.xml"))
        .mod_folder(
            ModFolder::new("RS_LowBoth", "rs.lowboth")
                .lowercase_about_dir()
                .about_file_name("about.xml"),
        )
        .mod_folder(
            ModFolder::new("RS_Bom", "rs.bom")
                .lowercase_about_dir()
                .about_file_name("ABOUT.XML")
                .bom()
                .crlf(),
        )
        .build_temp()
        .unwrap();
    let out = scan(&sources(&b), &opts());
    for id in [
        "rs.canon",
        "rs.lowdir",
        "rs.lowfile",
        "rs.lowboth",
        "rs.bom",
    ] {
        let (meta, idx) = find(&out, id);
        assert_eq!(meta.package_id.as_str(), id);
        assert_eq!(
            out.index.info(idx).unwrap().about,
            AboutStatus::Parsed,
            "{id}"
        );
    }
    // Only the lowercase folder spellings are reported: three mods.
    assert_eq!(out.diagnostics.count(&code("scan.about-dir-case")), 3);
    assert!(out.index.iter().all(|(_, m)| m.icon_path.is_none()));
}

#[test]
fn by_version_relations_survive_the_scan() {
    let rel = VersionRelations {
        load_after: vec!["rs.other".to_owned()],
        ..VersionRelations::default()
    };
    let b = InstallBuilder::new()
        .mod_folder(
            ModFolder::new("RS_Rel", "rs.rel")
                .load_after(&["rs.base"])
                .by_version("1.6", rel),
        )
        .build_temp()
        .unwrap();
    let out = scan(&sources(&b), &opts());
    let (meta, _) = find(&out, "rs.rel");
    let v = GameVersion::parse("1.6.1000").unwrap();
    assert_eq!(meta.effective_load_after(&v), vec!["rs.base", "rs.other"]);
    let old = GameVersion::parse("1.4.3000").unwrap();
    assert_eq!(meta.effective_load_after(&old), vec!["rs.base"]);
}

fn load_folder_install() -> rimstudio_testing::install_tree::TempInstall {
    let spec = LoadFoldersSpec::from_blocks([
        (
            "1.6",
            vec![
                LoadEntry::dir("Common"),
                LoadEntry::dir("v16"),
                LoadEntry::dir("WithOther").if_active(["rs.other"]),
            ],
        ),
        ("1.4", vec![LoadEntry::dir("v14")]),
    ]);
    InstallBuilder::new()
        .mod_folder(ModFolder::new("RS_Other", "rs.other"))
        .mod_folder(
            ModFolder::new("RS_Lf", "rs.lf")
                .load_folders(spec)
                .defs_file_in("Common", "c.xml", vec![def("RS_Common")])
                .defs_file_in("v16", "a.xml", vec![def("RS_V16")])
                .defs_file_in("v14", "a.xml", vec![def("RS_V14")])
                .defs_file_in("WithOther", "a.xml", vec![def("RS_Cond")])
                .defs_file_in("NotListed", "a.xml", vec![def("RS_Unlisted")]),
        )
        .mod_folder(
            ModFolder::new("RS_Implicit", "rs.implicit")
                .defs_file_in("1.6", "a.xml", vec![def("RS_I16")])
                .defs_file_in("1.4", "a.xml", vec![def("RS_I14")])
                .defs_file_in("Common", "c.xml", vec![def("RS_ICommon")])
                .defs_file("root.xml", vec![def("RS_IRoot")]),
        )
        .build_temp()
        .unwrap()
}

fn folders_of(out: &ScanOutcome, package: &str) -> Vec<String> {
    let (_, idx) = find(out, package);
    let mut f = out.index.content(idx).unwrap().folders.clone();
    f.sort();
    f
}

#[test]
fn load_folders_and_implicit_rules_narrow_level_one_by_game_version() {
    let b = load_folder_install();
    let set = sources(&b);
    let out = scan(&set, &opts());
    // The conditional entry is covered (all installed mods active) and the 1.4 block is not.
    assert_eq!(
        folders_of(&out, "rs.lf"),
        vec!["Common", "WithOther", "v16"]
    );
    assert_eq!(folders_of(&out, "rs.implicit"), vec!["", "1.6", "Common"]);
    let (_, idx) = find(&out, "rs.lf");
    let files: Vec<&str> = out
        .index
        .content(idx)
        .unwrap()
        .def_files
        .iter()
        .map(|f| f.rel_path.as_str())
        .collect();
    assert_eq!(
        files,
        vec![
            "Common/Defs/c.xml",
            "WithOther/Defs/a.xml",
            "v16/Defs/a.xml"
        ]
    );
    // Without a version every block and every version folder is searched.
    let all = scan(&set, &ScanOptions::default());
    assert_eq!(
        folders_of(&all, "rs.lf"),
        vec!["Common", "WithOther", "v14", "v16"]
    );
    assert_eq!(
        folders_of(&all, "rs.implicit"),
        vec!["", "1.4", "1.6", "Common"]
    );
}

#[test]
fn metadata_level_reads_no_definitions() {
    let b = load_folder_install();
    let out = scan(&sources(&b), &opts().with_level(ScanLevel::Metadata));
    assert_eq!(out.stats.def_files_parsed, 0);
    assert_eq!(out.stats.defs, 0);
    assert!(out.index.iter_full().all(|(_, _, _, c)| !c.complete));
    assert_eq!(out.manifest.file_count(), 0);
    assert!(out.timings.definitions.is_zero());
}

#[test]
fn markers_for_patches_languages_and_assemblies_are_recorded() {
    let b = InstallBuilder::new()
        .mod_folder(
            ModFolder::new("RS_Markers", "rs.markers")
                .patch_file("p.xml", vec![NodeBuilder::new("Operation").build()])
                .raw_file("Languages/English/Keyed/k.xml", "<LanguageData/>")
                .raw_file("Assemblies/RS.dll", "x")
                .raw_file("Assemblies/readme.txt", "x")
                .raw_file("Assemblies/sub/Inner.dll", "x")
                .def(def("RS_M")),
        )
        .build_temp()
        .unwrap();
    let out = scan(&sources(&b), &opts());
    let (_, idx) = find(&out, "rs.markers");
    let c = out.index.content(idx).unwrap();
    assert_eq!(c.patch_files, vec!["Patches/p.xml"]);
    assert_eq!(c.languages, vec!["Languages/English"]);
    assert_eq!(c.assemblies, vec!["Assemblies/RS.dll"]);
    assert!(c.has_assemblies());
    assert!(c.complete);
}

#[test]
fn duplicate_mixes_resolve_with_the_steam_rule() {
    let b = InstallBuilder::new()
        .mod_folder(ModFolder::new("RS_Tree", "Rs.Tree").name("local"))
        .workshop_mod(222, ModFolder::new("x", "rs.tree").name("workshop"))
        .custom_mod(ModFolder::new("RS_Tree_Copy", "rs.tree").name("custom copy"))
        .mod_folder(ModFolder::new("RS_One", "rs.one"))
        .mod_folder(ModFolder::new("RS_One_Copy", "rs.one"))
        .build_temp()
        .unwrap();
    let set = sources(&b);
    let out = scan(&set, &opts());
    let report = resolve(&out.index, &DuplicatePolicy::from_sources(&set));
    assert_eq!(report.group_count(), 2);
    let tree = report.group_for("rs.tree").unwrap();
    assert_eq!(tree.members.len(), 3);
    assert!(tree.spelling_differs);
    // The custom copy wins by source rank; the workshop copy is the one the game calls _steam.
    assert_eq!(out.index.get(tree.effective).unwrap().name, "custom copy");
    assert_eq!(
        tree.steam_note.as_ref().unwrap().effective_id,
        "rs.tree_steam"
    );
    let one = report.group_for("rs.one").unwrap();
    assert!(one.game_order_uncertain);
    assert!(one.steam_note.is_none());
}

#[test]
fn a_mod_with_no_package_id_gets_a_standin_and_a_warning() {
    let b = InstallBuilder::new()
        .mod_folder(ModFolder::new("RS_NoId", "x").raw_about(
            "<ModMetaData><name>RS Unnamed Id</name><author>Tester</author></ModMetaData>",
        ))
        .mod_folder(ModFolder::new("RS_Garbage", "y").raw_about("<<<not xml at all"))
        .build_temp()
        .unwrap();
    let out = scan(&sources(&b), &opts());
    let no_id = out
        .index
        .iter_full()
        .find(|(_, m, _, _)| m.name == "RS Unnamed Id")
        .unwrap();
    assert!(no_id.2.synthetic_package_id);
    assert!(no_id.1.package_id.as_str().starts_with("Tester."));
    assert!(out.diagnostics.count(&code("scan.package-id-missing")) >= 1);
    let garbage = out
        .index
        .iter_full()
        .find(|(_, m, _, _)| m.path.as_str().ends_with("RS_Garbage"))
        .unwrap();
    assert_eq!(garbage.2.about, AboutStatus::Unparsed);
    assert!(out.diagnostics.count(&code("scan.about-unparsed")) >= 1);
}

#[test]
fn broken_definition_files_are_diagnostics_and_keep_their_early_records() {
    let b = InstallBuilder::new()
        .mod_folder(
            ModFolder::new("RS_Broken", "rs.broken")
                .def(def("RS_Good"))
                .raw_file(
                    "Defs/truncated.xml",
                    "<Defs><ThingDef><defName>RS_Early</defName></ThingDef><ThingDef><defName>RS_Late",
                ),
        )
        .build_temp()
        .unwrap();
    let out = scan(&sources(&b), &opts());
    let (_, idx) = find(&out, "rs.broken");
    let c = out.index.content(idx).unwrap();
    assert!(c.def_count() >= 2);
    assert_eq!(out.diagnostics.count(&code("scan.def-file-broken")), 1);
    // Broken files are not cached, good ones are.
    assert_eq!(out.manifest.file_count(), 1);
    let sample = &out.diagnostics.samples[0];
    assert_eq!(sample.mod_idx, Some(idx));
}

#[test]
fn folders_without_about_are_reported_in_builtin_sources_only() {
    let b = InstallBuilder::new()
        .mod_folder(ModFolder::new("RS_Real", "rs.real"))
        .mod_folder(ModFolder::new("RS_Stray", "rs.stray").no_about())
        .build_temp()
        .unwrap();
    fs_err::create_dir_all(b.custom_dir.join("grouping/Nothing")).unwrap();
    fs_err::create_dir_all(b.mods_dir.join(".git")).unwrap();
    let out = scan(&sources(&b), &opts());
    assert_eq!(out.diagnostics.count(&code("scan.no-about")), 1);
    assert!(out.index.by_package_id("rs.stray").is_empty());
}

#[test]
fn missing_sources_are_offline_and_never_fatal() {
    let b = basic_install();
    let mut set = sources(&b);
    set.insert(ModSource::new(
        SourceId::new("cf_bbbbbbbb").unwrap(),
        SourceKind::Custom,
        b.root.join("does-not-exist"),
    ));
    let out = scan(&set, &opts());
    assert_eq!(out.index.len(), 4);
    let r = out.sources.last().unwrap();
    assert_eq!(r.status, SourceStatus::Offline);
    assert_eq!(out.diagnostics.count(&code("deploy.source-offline")), 1);
}

#[test]
fn cached_rows_of_an_offline_source_come_back_as_unavailable() {
    let b = basic_install();
    let set = sources(&b);
    let first = scan(&set, &opts());
    // Take the custom folder away.
    let moved = b.root.join("custom-gone");
    fs_err::rename(&b.custom_dir, &moved).unwrap();
    let second = scan(
        &set,
        &opts().with_previous(Arc::new(first.manifest.clone())),
    );
    let (meta, idx) = find(&second, "rs.custom.one");
    assert_eq!(meta.source, custom_id());
    assert!(!second.index.info(idx).unwrap().available);
    assert_eq!(second.index.len(), 4);
    // Without the previous manifest the row is simply gone.
    let third = scan(&set, &opts());
    assert!(third.index.by_package_id("rs.custom.one").is_empty());
    // The offline row stays in the manifest for the next time.
    assert!(second.manifest.mods.contains_key(meta.path.as_str()));
}

#[test]
fn rescan_of_an_unchanged_library_parses_nothing() {
    let b = load_folder_install();
    let set = sources(&b);
    let first = scan(&set, &opts());
    assert!(first.stats.files_parsed() > 0);
    let second = scan(
        &set,
        &opts().with_previous(Arc::new(first.manifest.clone())),
    );
    assert_eq!(second.stats.files_parsed(), 0);
    assert_eq!(second.stats.about_reused, first.stats.about_parsed);
    assert_eq!(second.stats.def_files_reused, first.stats.def_files_parsed);
    assert_eq!(second.manifest, first.manifest);
    for ((_, a), (_, b)) in first.index.iter().zip(second.index.iter()) {
        assert_eq!(a, b);
    }
    assert_eq!(first.index.def_count(), second.index.def_count());
}

fn rewrite_same_size_with_new_mtime(path: &Utf8Path) {
    let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(7200);
    file.set_modified(later).unwrap();
}

#[test]
fn only_changed_files_are_parsed_again() {
    let b = load_folder_install();
    let set = sources(&b);
    let first = scan(&set, &opts());
    let prev = Arc::new(first.manifest.clone());

    // A different size.
    let target = b.mod_path("RS_Lf").unwrap().join("v16/Defs/a.xml");
    fs_err::write(
        &target,
        "<Defs><ThingDef><defName>RS_V16</defName></ThingDef><ThingDef><defName>RS_V16b</defName></ThingDef></Defs>",
    )
    .unwrap();
    let second = scan(&set, &opts().with_previous(prev.clone()));
    assert_eq!(second.stats.def_files_parsed, 1);
    assert_eq!(second.stats.about_parsed, 0);
    let (_, idx) = find(&second, "rs.lf");
    assert!(
        second
            .index
            .content(idx)
            .unwrap()
            .def_files
            .iter()
            .any(|f| f
                .records
                .iter()
                .any(|r| r.def_name.as_deref() == Some("RS_V16b")))
    );

    // The same size with another time stamp counts as changed on a normal volume.
    let prev2 = Arc::new(second.manifest.clone());
    rewrite_same_size_with_new_mtime(&target);
    let third = scan(&set, &opts().with_previous(prev2));
    assert_eq!(third.stats.def_files_parsed, 1);

    // A changed About.xml updates exactly that mod.
    let about = b.mod_path("RS_Other").unwrap().join("About/About.xml");
    let text = fs_err::read_to_string(&about).unwrap();
    fs_err::write(&about, text.replace("RS_Other", "RS_Renamed")).unwrap();
    let fourth = scan(
        &set,
        &opts().with_previous(Arc::new(third.manifest.clone())),
    );
    assert_eq!(fourth.stats.about_parsed, 1);
    assert_eq!(fourth.stats.def_files_parsed, 0);
}

#[test]
fn a_damaged_manifest_file_is_ignored_and_the_scan_still_works() {
    let b = basic_install();
    let set = sources(&b);
    let first = scan(&set, &opts());
    let dir = tempfile::tempdir().unwrap();
    let path = Utf8PathBuf::from_path_buf(dir.path().join("manifest.json")).unwrap();
    first.manifest.save(&path).unwrap();
    let bytes = fs_err::read(&path).unwrap();
    fs_err::write(&path, &bytes[..bytes.len() / 2]).unwrap();
    let loaded = Manifest::load(&path);
    assert!(matches!(loaded.status, LoadStatus::Invalid(_)));
    let again = scan(&set, &opts().with_previous(Arc::new(loaded.manifest)));
    assert_eq!(again.stats.about_reused, 0);
    assert_eq!(again.index.len(), first.index.len());
    assert_eq!(again.stats.files_parsed(), first.stats.files_parsed());
}

struct CancelOnDefinitions(CancelToken);

impl ProgressSink for CancelOnDefinitions {
    fn report(&self, p: Progress) {
        if p.phase == "library.scan.definitions" {
            self.0.cancel();
        }
    }
}

#[test]
fn cancelling_level_one_keeps_the_list_and_cancelling_early_is_clean() {
    let b = basic_install();
    let set = sources(&b);
    let token = CancelToken::new();
    let sink = CancelOnDefinitions(token.clone());
    let out = Scanner::scan(&set, &opts(), &sink, &token);
    assert!(out.cancelled);
    assert_eq!(out.index.len(), 4);
    assert_eq!(out.stats.def_files_parsed, 0);
    assert!(out.index.iter_full().all(|(_, _, _, c)| !c.complete));
    assert_eq!(out.diagnostics.count(&code("job.cancelled")), 1);
    // About results of the cancelled scan are cached.
    assert_eq!(out.manifest.mod_count(), 4);

    let token = CancelToken::new();
    token.cancel();
    let out = Scanner::scan(&set, &opts(), &NoopProgress, &token);
    assert!(out.cancelled);
    assert!(out.index.is_empty());
}

fn signature(out: &ScanOutcome) -> String {
    let mut s = String::new();
    for (idx, meta, info, content) in out.index.iter_full() {
        s.push_str(&format!(
            "{} {:?} {:?} {:?}\n",
            idx.get(),
            serde_json::to_string(meta).unwrap(),
            serde_json::to_string(info).unwrap(),
            serde_json::to_string(content).unwrap(),
        ));
    }
    s.push_str(&serde_json::to_string(&out.diagnostics).unwrap());
    s.push_str(&serde_json::to_string(&out.stats).unwrap());
    s.push_str(&String::from_utf8(out.manifest.to_json_bytes().unwrap()).unwrap());
    s
}

#[test]
fn output_is_identical_at_one_and_eight_workers() {
    let b = load_folder_install();
    fs_err::create_dir_all(b.custom_dir.join("group/RS_Deep")).unwrap();
    let mut set = sources(&b);
    set.insert(ModSource::new(
        SourceId::new("cf_cccccccc").unwrap(),
        SourceKind::Custom,
        b.custom_dir.clone(),
    ));
    let one = scan(&set, &opts().with_workers(1));
    let eight = scan(&set, &opts().with_workers(8));
    assert_eq!(signature(&one), signature(&eight));
    let two = scan(&set, &opts().with_workers(2));
    assert_eq!(signature(&one), signature(&two));
}

fn write_mod(dir: &Utf8Path, package: &str) {
    ModFolder::new(dir.file_name().unwrap(), package)
        .write_to(dir.parent().unwrap())
        .unwrap();
}

fn custom_set(path: &Utf8Path, layout: FolderLayout, depth: u8) -> (SourceSet, ScanOptions) {
    let mut f = CustomFolder::new(custom_id(), path.to_owned());
    f.layout = layout;
    f.scan_depth = depth;
    let mut set = SourceSet::new();
    set.insert(f.to_source(0));
    (set, opts().with_custom_folders(&[f]))
}

#[test]
fn custom_layouts_and_depths_follow_the_folder_settings() {
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    write_mod(&root.join("RS_Flat"), "rs.flat");
    write_mod(&root.join("Weapons/RS_Nested"), "rs.nested");
    write_mod(&root.join("Weapons/Deeper/RS_TooDeep"), "rs.toodeep");
    fs_err::create_dir_all(root.join("template/Defs")).unwrap();
    fs_err::create_dir_all(root.join(".git/objects")).unwrap();
    write_mod(&root.join(".git/RS_Hidden"), "rs.hidden");

    let ids = |out: &ScanOutcome| {
        let mut v: Vec<String> = out
            .index
            .iter()
            .map(|(_, m)| m.package_id.as_str().to_owned())
            .collect();
        v.sort();
        v
    };
    let (set, o) = custom_set(&root, FolderLayout::ModsRoot, 1);
    assert_eq!(ids(&scan(&set, &o)), vec!["rs.flat"]);
    let (set, o) = custom_set(&root, FolderLayout::ModsRoot, 2);
    assert_eq!(ids(&scan(&set, &o)), vec!["rs.flat", "rs.nested"]);
    let (set, o) = custom_set(&root, FolderLayout::Auto, 3);
    assert_eq!(
        ids(&scan(&set, &o)),
        vec!["rs.flat", "rs.nested", "rs.toodeep"]
    );
    // A single mod path: the folder itself is the mod, whatever the depth.
    let (set, o) = custom_set(&root.join("RS_Flat"), FolderLayout::SingleMod, 1);
    assert_eq!(ids(&scan(&set, &o)), vec!["rs.flat"]);
    let (set, o) = custom_set(&root.join("RS_Flat"), FolderLayout::Auto, 1);
    assert_eq!(ids(&scan(&set, &o)), vec!["rs.flat"]);
    // Hidden folders are never entered, so a depth that would reach them still finds nothing there.
    let (set, o) = custom_set(&root, FolderLayout::ModsRoot, 4);
    assert!(!ids(&scan(&set, &o)).contains(&"rs.hidden".to_owned()));
}

#[cfg(unix)]
mod links {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn links_are_followed_at_the_first_level_only_and_loops_do_not_hang() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let outside = root.join("outside");
        write_mod(&outside.join("RS_Linked"), "rs.linked");
        let lib = root.join("lib");
        fs_err::create_dir_all(lib.join("group")).unwrap();
        write_mod(&lib.join("RS_Real"), "rs.real");
        symlink(outside.join("RS_Linked"), lib.join("RS_LinkAtTop")).unwrap();
        symlink(outside.join("RS_Linked"), lib.join("group/RS_LinkBelow")).unwrap();
        symlink(&lib, lib.join("RS_Loop")).unwrap();
        symlink(root.join("nowhere"), lib.join("RS_Dangling")).unwrap();
        let (set, o) = custom_set(&lib, FolderLayout::ModsRoot, 3);
        let out = scan(&set, &o);
        let mut ids: Vec<String> = out
            .index
            .iter()
            .map(|(_, m)| m.package_id.as_str().to_owned())
            .collect();
        ids.sort();
        assert_eq!(ids, vec!["rs.linked", "rs.real"]);
        let (_, idx) = find(&out, "rs.linked");
        assert!(out.index.info(idx).unwrap().is_link);
        // The link below the first level and the loop link (a link to a folder that is no mod).
        assert_eq!(out.diagnostics.count(&code("scan.link-skipped")), 2);
        assert_eq!(out.diagnostics.count(&code("scan.dangling-link")), 1);
    }

    #[test]
    fn the_same_folder_through_two_sources_is_listed_once_under_the_first() {
        let b = InstallBuilder::new()
            .workshop_mod(333, ModFolder::new("x", "rs.shared"))
            .build_temp()
            .unwrap();
        let mut set = SourceSet::new();
        set.insert(ModSource::new(
            SourceId::workshop(0),
            SourceKind::Workshop,
            b.workshop_dir.clone(),
        ));
        // A custom folder that is the workshop folder itself, reached through a link.
        let alias = b.root.join("alias");
        symlink(&b.workshop_dir, &alias).unwrap();
        set.insert(ModSource::new(custom_id(), SourceKind::Custom, alias));
        let out = scan(&set, &opts());
        assert_eq!(out.index.len(), 1);
        let (meta, idx) = find(&out, "rs.shared");
        assert_eq!(meta.source, SourceId::workshop(0));
        assert_eq!(out.index.info(idx).unwrap().also_in, vec![custom_id()]);
        assert_eq!(out.diagnostics.count(&code("scan.same-folder")), 1);
        // The custom copy being first makes it the owner, and it is visible through Workshop.
        let mut set2 = SourceSet::new();
        set2.insert(ModSource::new(
            custom_id(),
            SourceKind::Custom,
            b.root.join("alias"),
        ));
        set2.insert(ModSource::new(
            SourceId::workshop(0),
            SourceKind::Workshop,
            b.workshop_dir.clone(),
        ));
        let out2 = scan(&set2, &opts());
        let (meta2, idx2) = find(&out2, "rs.shared");
        assert_eq!(meta2.source, custom_id());
        assert_eq!(
            out2.index.info(idx2).unwrap().loadable,
            Loadability::Loadable
        );
    }
}

#[test]
fn progress_is_reported_for_both_levels() {
    let b = basic_install();
    let sink = rimstudio_core::jobs::CollectingProgress::new();
    let _ = Scanner::scan(&sources(&b), &opts(), &sink, &CancelToken::new());
    let phases: Vec<String> = sink.records().into_iter().map(|p| p.phase).collect();
    assert!(phases.contains(&"library.scan.metadata".to_owned()));
    assert!(phases.contains(&"library.scan.definitions".to_owned()));
    let last = sink.last().unwrap();
    assert_eq!(last.done, last.total.unwrap());
}
