//! Library scan use case over a fictional install with a custom folder.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::Harness;
use rimstudio_core::jobs::{CancelToken, CollectingProgress, NoopProgress};
use rimstudio_library::index::Loadability;
use rimstudio_library::scan::{ScanLevel, SourceStatus};
use rimstudio_manager::scan::{LibraryScanRequest, ManifestState, library_scan};
use rimstudio_manager::sources::{
    SourcesAddFolderRequest, SourcesUpdateRequest, add_folder, update,
};

fn add_custom(h: &Harness) -> String {
    add_folder(
        &h.ctx(),
        SourcesAddFolderRequest {
            path: h.install.custom_dir.to_string(),
            ..SourcesAddFolderRequest::default()
        },
    )
    .unwrap()
    .row
    .id
    .to_string()
}

fn scan(h: &Harness, req: LibraryScanRequest) -> rimstudio_manager::scan::LibraryScanResult {
    library_scan(&h.ctx(), req, &NoopProgress, &CancelToken::new()).unwrap()
}

#[test]
fn a_scan_covers_game_workshop_and_custom_sources() {
    let h = Harness::with_overrides();
    add_custom(&h);
    let out = scan(&h, LibraryScanRequest::default());
    assert!(!out.cancelled);
    assert_eq!(out.counts.mods, 4);
    assert_eq!(out.counts.defs, 4);
    assert_eq!(out.counts.duplicate_groups, 0);
    assert_eq!(out.sources.len(), 4);
    assert!(out.sources.iter().all(|s| s.status == SourceStatus::Ready));
    assert_eq!(
        out.game_version.as_ref().map(|v| v.to_string()),
        Some("1.6.1000 rev100".to_owned())
    );
    assert!(out.index.first_by_package_id("rs.mods.a").is_some());
    assert!(out.index.first_by_package_id("rs.workshop.one").is_some());
}

#[test]
fn a_mod_only_in_a_custom_folder_is_not_loadable_until_it_is_linked() {
    let h = Harness::with_overrides();
    add_custom(&h);
    let out = scan(&h, LibraryScanRequest::default());
    let (idx, _) = out.index.first_by_package_id("rs.custom.one").unwrap();
    assert_eq!(
        out.index.info(idx).unwrap().loadable,
        Loadability::NeedsLink
    );
    assert_eq!(out.counts.needs_link, 1);
    assert_eq!(out.counts.loadable, 3);

    #[cfg(unix)]
    {
        let target = h.install.custom_dir.join("RS_Custom");
        std::os::unix::fs::symlink(&target, h.install.mods_dir.join("RS_Custom")).unwrap();
        let out = scan(
            &h,
            LibraryScanRequest {
                full: true,
                ..LibraryScanRequest::default()
            },
        );
        let (idx, _) = out.index.first_by_package_id("rs.custom.one").unwrap();
        assert_eq!(out.index.info(idx).unwrap().loadable, Loadability::Loadable);
        assert_eq!(out.counts.needs_link, 0);
        assert_eq!(
            out.counts.mods, 4,
            "one folder reached by two sources is listed once"
        );
    }
}

#[test]
fn the_manifest_is_saved_and_a_rescan_parses_nothing() {
    let h = Harness::with_overrides();
    add_custom(&h);
    let first = scan(&h, LibraryScanRequest::default());
    assert_eq!(first.manifest_before, ManifestState::Missing);
    assert!(first.manifest_saved);
    assert!(first.stats.files_parsed() > 0);
    assert!(h.roots.cache.read_dir().is_ok());

    let second = scan(&h, LibraryScanRequest::default());
    assert_eq!(second.manifest_before, ManifestState::Used);
    assert_eq!(second.stats.files_parsed(), 0);
    assert_eq!(second.counts, first.counts);

    let full = scan(
        &h,
        LibraryScanRequest {
            full: true,
            ..LibraryScanRequest::default()
        },
    );
    assert_eq!(full.manifest_before, ManifestState::Skipped);
    assert!(full.stats.files_parsed() > 0);
}

#[test]
fn a_damaged_manifest_is_ignored_and_replaced() {
    let h = Harness::with_overrides();
    add_custom(&h);
    scan(&h, LibraryScanRequest::default());
    let path = rimstudio_library::cache::manifest_path(&h.roots);
    std::fs::write(&path, "garbage").unwrap();
    let out = scan(&h, LibraryScanRequest::default());
    assert_eq!(out.manifest_before, ManifestState::Ignored);
    assert!(out.manifest_saved);
    assert_eq!(
        scan(&h, LibraryScanRequest::default()).manifest_before,
        ManifestState::Used
    );
}

#[test]
fn a_disabled_custom_folder_is_not_scanned() {
    let h = Harness::with_overrides();
    let id = add_custom(&h);
    update(
        &h.ctx(),
        SourcesUpdateRequest {
            id,
            enabled: Some(false),
            ..SourcesUpdateRequest::default()
        },
    )
    .unwrap();
    let out = scan(&h, LibraryScanRequest::default());
    assert!(out.index.first_by_package_id("rs.custom.one").is_none());
    assert_eq!(out.counts.mods, 3);
    assert!(
        out.sources
            .iter()
            .any(|s| s.status == SourceStatus::Disabled)
    );
}

#[test]
fn a_disabled_built_in_source_is_not_scanned() {
    let h = Harness::with_overrides();
    update(
        &h.ctx(),
        SourcesUpdateRequest {
            id: "workshop-0".into(),
            enabled: Some(false),
            ..SourcesUpdateRequest::default()
        },
    )
    .unwrap();
    let out = scan(&h, LibraryScanRequest::default());
    assert!(out.index.first_by_package_id("rs.workshop.one").is_none());
}

#[test]
fn a_metadata_only_scan_reads_no_definitions() {
    let h = Harness::with_overrides();
    let out = scan(
        &h,
        LibraryScanRequest {
            level: ScanLevel::Metadata,
            ..LibraryScanRequest::default()
        },
    );
    assert_eq!(out.counts.defs, 0);
    assert_eq!(out.counts.mods, 3);
}

#[test]
fn a_missing_custom_folder_is_reported_offline_and_the_scan_still_succeeds() {
    let h = Harness::with_overrides();
    add_custom(&h);
    std::fs::remove_dir_all(&h.install.custom_dir).unwrap();
    let out = scan(&h, LibraryScanRequest::default());
    assert!(
        out.sources
            .iter()
            .any(|s| s.status == SourceStatus::Offline)
    );
    assert_eq!(out.counts.mods, 3);
}

#[test]
fn a_cancelled_scan_reports_it_and_does_not_save_the_manifest() {
    let h = Harness::with_overrides();
    let cancel = CancelToken::new();
    cancel.cancel();
    let out = library_scan(
        &h.ctx(),
        LibraryScanRequest::default(),
        &NoopProgress,
        &cancel,
    )
    .unwrap();
    assert!(out.cancelled);
    assert!(!out.manifest_saved);
    assert!(!rimstudio_library::cache::manifest_path(&h.roots).exists());
}

#[test]
fn progress_records_reach_the_sink() {
    let h = Harness::with_overrides();
    let sink = CollectingProgress::new();
    library_scan(
        &h.ctx(),
        LibraryScanRequest::default(),
        &sink,
        &CancelToken::new(),
    )
    .unwrap();
    assert!(!sink.is_empty());
}

#[test]
fn scanning_without_any_source_is_an_error() {
    let h = Harness::bare();
    let err = library_scan(
        &h.ctx(),
        LibraryScanRequest::default(),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "manager.no-sources");
}

#[test]
fn a_scan_writes_only_inside_the_data_roots() {
    let h = Harness::with_overrides();
    add_custom(&h);
    let snapshot = |dir: &camino::Utf8Path| {
        let mut files = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap() {
                let p = camino::Utf8PathBuf::from_path_buf(e.unwrap().path()).unwrap();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    let m = std::fs::metadata(&p).unwrap();
                    files.push((p, m.len(), m.modified().unwrap()));
                }
            }
        }
        files.sort();
        files
    };
    let before = snapshot(&h.install.root);
    scan(&h, LibraryScanRequest::default());
    scan(
        &h,
        LibraryScanRequest {
            full: true,
            ..LibraryScanRequest::default()
        },
    );
    assert_eq!(snapshot(&h.install.root), before);
}
