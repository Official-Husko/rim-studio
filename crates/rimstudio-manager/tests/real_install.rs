//! Read only runs against the owner's machine. All tests are ignored by default; set
//! RIMSTUDIO_GAME_DIR, RIMSTUDIO_WORKSHOP_DIR and RIMSTUDIO_CUSTOM_DIR and run with `--ignored`.
//! They never write inside those folders: settings, the workspace document, the report cache and
//! the manifest go to temporary data roots.

#![allow(clippy::unwrap_used, clippy::expect_used, unreachable_pub)]

mod common;

use common::Harness;
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_manager::detect::{DetectSetOverrideRequest, OverrideField, set_override};
use rimstudio_manager::scan::{LibraryScanRequest, library_scan};
use rimstudio_manager::sources::{SourcesAddFolderRequest, add_folder};
use rimstudio_testing::install_tree::InstallBuilder;

fn env_dir(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

#[test]
#[ignore = "reads the real install named by RIMSTUDIO_GAME_DIR, RIMSTUDIO_WORKSHOP_DIR and RIMSTUDIO_CUSTOM_DIR"]
fn real_install_detects_adds_the_custom_folder_and_scans() {
    let Some(game) = env_dir("RIMSTUDIO_GAME_DIR") else {
        return;
    };
    let h = Harness::with_install(InstallBuilder::new().build_temp().unwrap());
    let ctx = h.ctx();
    let out = set_override(
        &ctx,
        DetectSetOverrideRequest {
            field: OverrideField::GameInstall,
            path: Some(game),
            pinned: true,
        },
    )
    .unwrap();
    println!("selected install: {:?}", out.selected_install);
    if let Some(ws) = env_dir("RIMSTUDIO_WORKSHOP_DIR") {
        set_override(
            &ctx,
            DetectSetOverrideRequest {
                field: OverrideField::WorkshopDir,
                path: Some(ws),
                pinned: false,
            },
        )
        .unwrap();
    }
    if let Some(custom) = env_dir("RIMSTUDIO_CUSTOM_DIR") {
        let added = add_folder(
            &ctx,
            SourcesAddFolderRequest {
                path: custom,
                ..SourcesAddFolderRequest::default()
            },
        )
        .unwrap();
        println!(
            "custom folder: {} mods estimated, depth {:?}",
            added.row.status.mod_count_estimate.unwrap_or(0),
            added.row.scan_depth
        );
    }
    let scan = library_scan(
        &ctx,
        LibraryScanRequest::default(),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    println!("counts: {:?}", scan.counts);
    println!("timings: {:?}", scan.timings);
    assert!(scan.counts.mods > 0);

    let facts = &scan.facts;
    println!(
        "loadable {} custom only {} duplicate groups {} (skipped copies {})",
        scan.counts.loadable,
        scan.counts.needs_link,
        facts.duplicates.total,
        facts.duplicates.skipped_total
    );
    for s in &facts.per_source {
        println!(
            "source {}: {} mods, {} loadable, {} custom only",
            s.source, s.mods, s.loadable, s.custom_only
        );
    }
    for g in facts.duplicates.groups.iter().take(8) {
        println!(
            "duplicate {}: kept {} ({:?}); skipped {}",
            g.package_id,
            g.kept.path,
            g.reason,
            g.skipped
                .iter()
                .map(|e| format!("{} [{}]", e.path, e.source))
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
    println!("combat extended in library: {:?}", facts.ce);
    let per_source: usize = facts.per_source.iter().map(|s| s.mods).sum();
    assert_eq!(per_source, scan.counts.mods);
    assert_eq!(facts.duplicates.total, scan.counts.duplicate_groups);
}
