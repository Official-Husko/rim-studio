//! Source use cases: list, add, update, remove and probe, with the overlap refusals.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use camino::Utf8PathBuf;
use common::Harness;
use rimstudio_core::mods::SourceKind;
use rimstudio_core::settings::FolderLayout;
use rimstudio_manager::error::ManagerError;
use rimstudio_manager::sources::{
    SourcesAddFolderRequest, SourcesListRequest, SourcesProbeFolderRequest, SourcesRemoveRequest,
    SourcesUpdateRequest, add_folder, list, probe_folder, remove, update,
};

fn add(h: &Harness, path: &str) -> Result<rimstudio_manager::sources::SourceRow, ManagerError> {
    add_folder(
        &h.ctx(),
        SourcesAddFolderRequest {
            path: path.to_owned(),
            ..SourcesAddFolderRequest::default()
        },
    )
    .map(|r| r.row)
}

fn make_dir(h: &Harness, name: &str) -> Utf8PathBuf {
    let p = h.install.root.join(name);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn rejected_codes(err: &ManagerError) -> Vec<String> {
    match err {
        ManagerError::FolderRejected { diagnostics, .. } => diagnostics
            .iter()
            .map(|d| d.code.as_str().to_owned())
            .collect(),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn the_list_shows_the_built_in_sources_with_their_status() {
    let h = Harness::with_overrides();
    let out = list(
        &h.ctx(),
        SourcesListRequest {
            estimate_mod_counts: true,
        },
    )
    .unwrap();
    let kinds: Vec<_> = out
        .rows
        .iter()
        .map(|r| (r.id.as_str().to_owned(), r.kind))
        .collect();
    assert_eq!(
        kinds,
        vec![
            ("game-data".to_owned(), SourceKind::GameData),
            ("game-mods".to_owned(), SourceKind::GameMods),
            ("workshop-0".to_owned(), SourceKind::Workshop),
        ]
    );
    assert!(
        out.rows
            .iter()
            .all(|r| r.status.exists && r.status.readable && !r.can_remove)
    );
    assert_eq!(out.rows[1].status.mod_count_estimate, Some(1));
    assert_eq!(out.rows[2].status.mod_count_estimate, Some(1));
    assert!(out.install_id.is_some());
    assert_eq!(out.game_version.as_deref(), Some("1.6.1000 rev100"));
}

#[test]
fn the_list_without_an_install_holds_only_custom_folders() {
    let h = Harness::bare();
    let out = list(&h.ctx(), SourcesListRequest::default()).unwrap();
    assert!(out.rows.is_empty());
    assert_eq!(out.install_id, None);
}

#[test]
fn any_number_of_custom_folders_can_be_added_and_are_stored() {
    let h = Harness::with_overrides();
    let a = add(&h, h.install.custom_dir.as_str()).unwrap();
    let b_dir = make_dir(&h, "second-folder");
    let c_dir = make_dir(&h, "third-folder");
    let b = add(&h, b_dir.as_str()).unwrap();
    let c = add(&h, c_dir.as_str()).unwrap();
    assert!(a.id.is_custom() && b.id.is_custom() && c.id.is_custom());
    assert_ne!(a.id, b.id);
    assert_eq!(a.label, "custom");
    assert_eq!(a.status.mod_count_estimate, Some(1));
    assert!(a.can_remove);
    assert_eq!(a.kind, SourceKind::Custom);
    let rows = list(&h.ctx(), SourcesListRequest::default()).unwrap().rows;
    assert_eq!(rows.len(), 6);
    assert_eq!(rows[3].id, a.id);
    assert_eq!(rows[5].id, c.id);
    let text = h.read(&h.workspace_path());
    assert!(text.contains(a.id.as_str()) && text.contains("second-folder"));
}

#[test]
fn adding_with_a_label_layout_and_depth_stores_them() {
    let h = Harness::with_overrides();
    let resp = add_folder(
        &h.ctx(),
        SourcesAddFolderRequest {
            path: h.install.custom_dir.to_string(),
            label: Some("  My mods  ".into()),
            layout: Some(FolderLayout::ModsRoot),
            scan_depth: Some(2),
            read_only: true,
        },
    )
    .unwrap();
    assert_eq!(resp.row.label, "My mods");
    assert_eq!(resp.row.layout, Some(FolderLayout::ModsRoot));
    assert_eq!(resp.row.scan_depth, Some(2));
    assert!(resp.row.read_only);
}

#[test]
fn a_duplicate_a_nested_and_an_enclosing_folder_are_refused() {
    let h = Harness::with_overrides();
    add(&h, h.install.custom_dir.as_str()).unwrap();
    let before = h.read(&h.workspace_path());

    let dup = add(&h, h.install.custom_dir.as_str()).unwrap_err();
    assert_eq!(rejected_codes(&dup), vec!["deploy.source-overlap"]);

    let nested = h.install.custom_dir.join("RS_Custom");
    let err = add(&h, nested.as_str()).unwrap_err();
    assert_eq!(rejected_codes(&err), vec!["deploy.source-overlap"]);

    let enclosing = h.install.root.clone();
    let err = add(&h, enclosing.as_str()).unwrap_err();
    assert!(rejected_codes(&err).contains(&"deploy.source-overlap".to_owned()));

    let spelled = format!("{}/./sub/..", h.install.custom_dir);
    let err = add(&h, &spelled).unwrap_err();
    assert_eq!(rejected_codes(&err), vec!["deploy.source-overlap"]);

    assert_eq!(h.read(&h.workspace_path()), before);
}

#[test]
fn a_folder_inside_or_equal_to_the_game_mods_folder_is_refused() {
    let h = Harness::with_overrides();
    let err = add(&h, h.install.mods_dir.as_str()).unwrap_err();
    assert_eq!(rejected_codes(&err), vec!["deploy.source-inside-mods"]);
    let inside = h.install.mods_dir.join("RS_ModsA");
    let err = add(&h, inside.as_str()).unwrap_err();
    assert_eq!(rejected_codes(&err), vec!["deploy.source-inside-mods"]);
    let workshop = add(&h, h.install.workshop_dir.as_str()).unwrap_err();
    assert_eq!(rejected_codes(&workshop), vec!["deploy.source-overlap"]);
}

#[test]
fn a_symbolic_link_to_a_known_source_is_refused_through_its_canonical_path() {
    let h = Harness::with_overrides();
    let link = h.install.root.join("alias-of-mods");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&h.install.mods_dir, &link).unwrap();
    #[cfg(unix)]
    {
        let err = add(&h, link.as_str()).unwrap_err();
        assert_eq!(rejected_codes(&err), vec!["deploy.source-inside-mods"]);
    }
    #[cfg(not(unix))]
    let _ = link;
}

#[test]
fn missing_files_and_relative_paths_are_refused() {
    let h = Harness::with_overrides();
    let missing = h.install.root.join("nowhere");
    let err = add(&h, missing.as_str()).unwrap_err();
    assert_eq!(err.code(), "manager.folder-rejected");
    let file = h.install.game_dir.join("Version.txt");
    let err = add(&h, file.as_str()).unwrap_err();
    assert_eq!(rejected_codes(&err), vec!["scan.path-not-directory"]);
    let err = add(&h, "relative/mods").unwrap_err();
    assert_eq!(err.code(), "core.invalid-path");
    let err = add(&h, "").unwrap_err();
    assert_eq!(err.code(), "core.invalid-path");
    let ok_dir = make_dir(&h, "depth-check");
    let err = add_folder(
        &h.ctx(),
        SourcesAddFolderRequest {
            path: ok_dir.to_string(),
            scan_depth: Some(9),
            ..SourcesAddFolderRequest::default()
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "manager.invalid-request");
    assert!(
        !h.workspace_path().exists() || !h.read(&h.workspace_path()).contains("customModFolders")
    );
}

#[test]
fn probing_reports_the_class_overlaps_and_whether_it_can_be_added() {
    let h = Harness::with_overrides();
    let ok = probe_folder(
        &h.ctx(),
        SourcesProbeFolderRequest {
            path: h.install.custom_dir.to_string(),
        },
    )
    .unwrap();
    assert!(ok.can_add);
    assert_eq!(ok.class.mod_count, 1);
    assert_eq!(ok.suggested_label, "custom");
    assert!(ok.overlaps.is_empty());
    assert!(!ok.removable_drive && ok.volume_hint.is_none());

    let bad = probe_folder(
        &h.ctx(),
        SourcesProbeFolderRequest {
            path: h.install.mods_dir.to_string(),
        },
    )
    .unwrap();
    assert!(!bad.can_add);
    assert_eq!(bad.overlaps.len(), 1);
    assert!(
        !h.workspace_path().exists() || !h.read(&h.workspace_path()).contains("customModFolders")
    );
}

#[test]
fn updating_a_custom_folder_changes_label_enabled_order_layout_and_depth() {
    let h = Harness::with_overrides();
    let a = add(&h, h.install.custom_dir.as_str()).unwrap();
    let b_dir = make_dir(&h, "b-folder");
    let b = add(&h, b_dir.as_str()).unwrap();
    let row = update(
        &h.ctx(),
        SourcesUpdateRequest {
            id: b.id.to_string(),
            label: Some("Second".into()),
            enabled: Some(false),
            order: Some(0),
            layout: Some(FolderLayout::SingleMod),
            scan_depth: Some(3),
            read_only: Some(true),
        },
    )
    .unwrap();
    assert_eq!(row.label, "Second");
    assert!(!row.enabled && row.read_only);
    assert_eq!(row.layout, Some(FolderLayout::SingleMod));
    assert_eq!(row.scan_depth, Some(3));
    let rows = list(&h.ctx(), SourcesListRequest::default()).unwrap().rows;
    let custom: Vec<_> = rows
        .iter()
        .filter(|r| r.can_remove)
        .map(|r| r.id.clone())
        .collect();
    assert_eq!(custom, vec![b.id.clone(), a.id.clone()]);
    let row = update(
        &h.ctx(),
        SourcesUpdateRequest {
            id: b.id.to_string(),
            order: Some(99),
            ..SourcesUpdateRequest::default()
        },
    )
    .unwrap();
    assert_eq!(row.id, b.id);
    let rows = list(&h.ctx(), SourcesListRequest::default()).unwrap().rows;
    assert_eq!(rows.last().unwrap().id, b.id);
}

#[test]
fn update_errors_leave_the_document_unchanged() {
    let h = Harness::with_overrides();
    let a = add(&h, h.install.custom_dir.as_str()).unwrap();
    let before = h.read(&h.workspace_path());
    let bad_depth = update(
        &h.ctx(),
        SourcesUpdateRequest {
            id: a.id.to_string(),
            scan_depth: Some(0),
            ..SourcesUpdateRequest::default()
        },
    )
    .unwrap_err();
    assert_eq!(bad_depth.code(), "manager.invalid-request");
    let empty = update(
        &h.ctx(),
        SourcesUpdateRequest {
            id: a.id.to_string(),
            label: Some("   ".into()),
            ..SourcesUpdateRequest::default()
        },
    )
    .unwrap_err();
    assert_eq!(empty.code(), "manager.invalid-request");
    let unknown = update(
        &h.ctx(),
        SourcesUpdateRequest {
            id: "cf_00000000".into(),
            enabled: Some(false),
            ..SourcesUpdateRequest::default()
        },
    )
    .unwrap_err();
    assert_eq!(unknown.code(), "manager.source-not-found");
    let builtin_label = update(
        &h.ctx(),
        SourcesUpdateRequest {
            id: "game-mods".into(),
            label: Some("x".into()),
            ..SourcesUpdateRequest::default()
        },
    )
    .unwrap_err();
    assert_eq!(builtin_label.code(), "manager.invalid-request");
    assert_eq!(h.read(&h.workspace_path()), before);
}

#[test]
fn a_built_in_source_can_be_disabled_and_enabled_again() {
    let h = Harness::with_overrides();
    let row = update(
        &h.ctx(),
        SourcesUpdateRequest {
            id: "workshop-0".into(),
            enabled: Some(false),
            ..SourcesUpdateRequest::default()
        },
    )
    .unwrap();
    assert!(!row.enabled);
    assert!(h.read(&h.workspace_path()).contains("modSources"));
    let row = update(
        &h.ctx(),
        SourcesUpdateRequest {
            id: "workshop-0".into(),
            enabled: Some(true),
            ..SourcesUpdateRequest::default()
        },
    )
    .unwrap();
    assert!(row.enabled);
}

#[test]
fn removing_a_custom_folder_never_touches_its_files() {
    let h = Harness::with_overrides();
    let a = add(&h, h.install.custom_dir.as_str()).unwrap();
    let out = remove(
        &h.ctx(),
        SourcesRemoveRequest {
            id: a.id.to_string(),
        },
    )
    .unwrap();
    assert_eq!(out.id, a.id);
    assert_eq!(out.path, h.install.custom_dir);
    assert_eq!(out.remaining, 0);
    assert!(
        h.install
            .custom_dir
            .join("RS_Custom/About/About.xml")
            .is_file()
    );
    let rows = list(&h.ctx(), SourcesListRequest::default()).unwrap().rows;
    assert!(rows.iter().all(|r| !r.can_remove));
    assert!(!h.read(&h.workspace_path()).contains(a.id.as_str()));
    // The same folder can be added again after removal.
    add(&h, h.install.custom_dir.as_str()).unwrap();
}

#[test]
fn removing_unknown_and_built_in_sources_is_refused() {
    let h = Harness::with_overrides();
    let unknown = remove(
        &h.ctx(),
        SourcesRemoveRequest {
            id: "cf_deadbeef".into(),
        },
    )
    .unwrap_err();
    assert_eq!(unknown.code(), "manager.source-not-found");
    let builtin = remove(
        &h.ctx(),
        SourcesRemoveRequest {
            id: "game-mods".into(),
        },
    )
    .unwrap_err();
    assert_eq!(builtin.code(), "manager.invalid-request");
}

#[test]
fn comments_in_the_workspace_file_survive_adding_and_removing_folders() {
    let h = Harness::with_overrides();
    let before = h.read(&h.workspace_path());
    let annotated = format!("// my setup, do not delete\n{before}");
    std::fs::write(h.workspace_path(), &annotated).unwrap();
    let a = add(&h, h.install.custom_dir.as_str()).unwrap();
    assert!(
        h.read(&h.workspace_path())
            .starts_with("// my setup, do not delete\n")
    );
    remove(
        &h.ctx(),
        SourcesRemoveRequest {
            id: a.id.to_string(),
        },
    )
    .unwrap();
    assert!(
        h.read(&h.workspace_path())
            .starts_with("// my setup, do not delete\n")
    );
}

#[test]
fn a_folder_on_a_removable_drive_gets_a_volume_hint() {
    let h = Harness::with_overrides();
    // The check is path based, so use a fake mount name for the probe only.
    assert!(rimstudio_manager::sources::looks_removable(
        "/run/media/me/stick/mods"
    ));
    let hint =
        rimstudio_manager::sources::volume_hint(camino::Utf8Path::new("/run/media/me/stick/mods"));
    assert_eq!(hint.mount, "/run/media/me/stick");
    assert_eq!(hint.label, "stick");
    let _ = h;
}
