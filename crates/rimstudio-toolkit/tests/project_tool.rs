//! The project tool: open, close, create from the scaffold plan, the load folders summary.
#![cfg(feature = "tool-project")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::sync::Arc;

use camino::Utf8PathBuf;
use rimstudio_io::roots::DataRoots;
use rimstudio_ipc_types::project::{ProjectCloseRequest, ProjectOpenRequest};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_toolkit::project::{close, create, load_folders_summary, open, open_project};
use rimstudio_toolkit::shared::env::ProjectEnv;
use rimstudio_workspace::scaffold::ScaffoldSpec;

struct World {
    _tmp: tempfile::TempDir,
    base: Utf8PathBuf,
    env: ProjectEnv,
}

fn world() -> World {
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let roots = DataRoots::under_base(&base.join("app"));
    roots.ensure_all().unwrap();
    let env = ProjectEnv::new(
        &roots,
        Arc::new(FakeClock::new(FakeClock::DEFAULT_START_MS)),
    )
    .unwrap();
    World {
        _tmp: tmp,
        base,
        env,
    }
}

fn tree(root: &Utf8PathBuf) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(dir: &std::path::Path, prefix: &str, out: &mut Vec<String>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().collect();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for e in entries {
            let name = format!("{prefix}{}", e.file_name().to_string_lossy());
            out.push(name.clone());
            if e.path().is_dir() {
                walk(&e.path(), &format!("{name}/"), out);
            }
        }
    }
    walk(root.as_std_path(), "", &mut out);
    out
}

fn mod_folder(w: &World, with_gate: bool) -> Utf8PathBuf {
    let root = w.base.join("RS_Existing");
    std::fs::create_dir_all(root.join("About").as_std_path()).unwrap();
    std::fs::write(
        root.join("About/About.xml").as_std_path(),
        "<ModMetaData><name>RS Existing</name><packageId>rs.existing</packageId><supportedVersions><li>1.6</li></supportedVersions></ModMetaData>",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("Defs").as_std_path()).unwrap();
    std::fs::write(root.join("Defs/A.xml").as_std_path(), "<Defs/>").unwrap();
    if with_gate {
        std::fs::write(
            root.join("LoadFolders.xml").as_std_path(),
            "<loadFolders><v1.6><li>/</li><li IfModActive=\"ceteam.combatextended\">CE</li></v1.6></loadFolders>",
        )
        .unwrap();
    }
    root
}

#[test]
fn opening_registers_the_project_and_writes_nothing_into_the_folder() {
    let w = world();
    let root = mod_folder(&w, true);
    let before = tree(&root);
    let summary = open(
        &w.env,
        ProjectOpenRequest {
            path: root.to_string(),
        },
    )
    .unwrap();
    assert!(summary.project_id.starts_with("p-"));
    assert_eq!(summary.name, "RS Existing");
    assert_eq!(summary.package_id.as_deref(), Some("rs.existing"));
    assert_eq!(summary.supported_versions, vec!["1.6".to_owned()]);
    assert!(summary.has_about && summary.has_load_folders && summary.has_ce_gate);
    assert_eq!(summary.def_files, 1);
    assert_eq!(tree(&root), before);
    // the same folder is the same project
    let again = open(
        &w.env,
        ProjectOpenRequest {
            path: root.to_string(),
        },
    )
    .unwrap();
    assert_eq!(again.project_id, summary.project_id);
    assert!(w.env.record(&summary.project_id).is_ok());
}

#[test]
fn a_folder_without_about_is_not_a_project() {
    let w = world();
    let empty = w.base.join("RS_Empty");
    std::fs::create_dir_all(empty.as_std_path()).unwrap();
    let err = open(
        &w.env,
        ProjectOpenRequest {
            path: empty.to_string(),
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "io.not-found");
}

#[test]
fn closing_reports_whether_the_id_was_known() {
    let w = world();
    let root = mod_folder(&w, false);
    let summary = open(
        &w.env,
        ProjectOpenRequest {
            path: root.to_string(),
        },
    )
    .unwrap();
    assert!(
        close(
            &w.env,
            ProjectCloseRequest {
                project_id: summary.project_id.clone()
            }
        )
        .unwrap()
        .closed
    );
    assert!(
        !close(
            &w.env,
            ProjectCloseRequest {
                project_id: "p-ffffffff".into()
            }
        )
        .unwrap()
        .closed
    );
    assert!(
        !close(
            &w.env,
            ProjectCloseRequest {
                project_id: "not an id".into()
            }
        )
        .unwrap()
        .closed
    );
}

#[test]
fn the_load_folders_summary_lists_blocks_and_the_gate() {
    let w = world();
    let root = mod_folder(&w, true);
    let opened = open_project(&w.env, &root).unwrap();
    let summary = load_folders_summary(&opened.view);
    assert!(summary.exists && summary.ce_gate);
    assert_eq!(summary.blocks.len(), 1);
    assert_eq!(summary.blocks[0].0, "v1.6");
    assert_eq!(
        summary.blocks[0].1,
        vec!["/".to_owned(), "CE [if ceteam.combatextended]".to_owned()]
    );
    let plain = mod_folder_without(&w);
    assert!(!load_folders_summary(&open_project(&w.env, &plain).unwrap().view).exists);
}

fn mod_folder_without(w: &World) -> Utf8PathBuf {
    let root = w.base.join("RS_Plain");
    std::fs::create_dir_all(root.join("About").as_std_path()).unwrap();
    std::fs::write(
        root.join("About/About.xml").as_std_path(),
        "<ModMetaData><name>Plain</name><packageId>rs.plain</packageId></ModMetaData>",
    )
    .unwrap();
    root
}

#[test]
fn create_writes_the_scaffold_through_the_guarded_writer_and_opens_it() {
    let w = world();
    let target = w.base.join("RS_NewMod");
    let mut spec = ScaffoldSpec::new(target.clone(), "RS New Mod", "rs.newmod");
    spec.ce_patch_folder = true;
    let report = create(&w.env, &spec, &[]).unwrap();
    assert!(report.written.iter().any(|p| p == "About/About.xml"));
    assert!(report.written.iter().any(|p| p == "LoadFolders.xml"));
    assert!(target.join("Defs").is_dir());
    assert!(report.folders.iter().any(|f| f == "Defs"));
    assert!(report.project.summary.has_ce_gate);
    assert_eq!(
        report.project.summary.package_id.as_deref(),
        Some("rs.newmod")
    );
    // the Combat Extended folder is only ever listed behind the gate
    let lf = std::fs::read_to_string(target.join("LoadFolders.xml").as_std_path()).unwrap();
    assert!(lf.contains("IfModActive"));
}

#[test]
fn a_scaffold_never_overwrites() {
    let w = world();
    let target = w.base.join("RS_Twice");
    let spec = ScaffoldSpec::new(target.clone(), "RS Twice", "rs.twice");
    create(&w.env, &spec, &[]).unwrap();
    let about = target.join("About/About.xml");
    std::fs::write(about.as_std_path(), "changed by hand").unwrap();
    let err = create(&w.env, &spec, &[]).unwrap_err();
    assert_eq!(err.code(), "designer.apply-failed");
    assert_eq!(
        std::fs::read_to_string(about.as_std_path()).unwrap(),
        "changed by hand"
    );
}

#[test]
fn an_invalid_spec_and_a_protected_target_are_refused_before_any_write() {
    let w = world();
    let bad = ScaffoldSpec::new(w.base.join("RS_Bad"), "", "rs.bad");
    assert_eq!(
        create(&w.env, &bad, &[]).unwrap_err().code(),
        "designer.apply-failed"
    );
    assert!(!w.base.join("RS_Bad").exists());

    let game = w.base.join("RS_Game");
    std::fs::create_dir_all(game.as_std_path()).unwrap();
    let inside = ScaffoldSpec::new(game.join("Mods/RS_In"), "RS In", "rs.in");
    let err = create(&w.env, &inside, std::slice::from_ref(&game)).unwrap_err();
    assert_eq!(err.code(), "project.path-outside-root");
    assert!(!game.join("Mods").exists());
}
