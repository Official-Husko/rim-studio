//! Scans of real folders, read only. Every test is ignored and reads its folder from an
//! environment variable; nothing is written and nothing is committed.
//!
//! Run with:
//! `RIMSTUDIO_GAME_DIR=<install> RIMSTUDIO_WORKSHOP_DIR=<content dir> RIMSTUDIO_CUSTOM_DIR=<folder>
//!  cargo test -p rimstudio-library --release --test real_data -- --ignored --nocapture --test-threads=1`

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::sync::Arc;

use camino::Utf8PathBuf;
use rimstudio_core::ids::SourceId;
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_core::mods::{ModSource, SourceKind, SourceSet};
use rimstudio_core::settings::{CustomFolder, FolderLayout};
use rimstudio_library::duplicates::{DuplicatePolicy, resolve};
use rimstudio_library::scan::{ScanLevel, ScanOptions, ScanOutcome, Scanner};

fn env_dir(name: &str) -> Option<Utf8PathBuf> {
    std::env::var(name)
        .ok()
        .filter(|v| !v.is_empty())
        .map(Utf8PathBuf::from)
}

fn real_sources() -> (SourceSet, ScanOptions) {
    let mut set = SourceSet::new();
    let mut opts = ScanOptions::default();
    if let Some(game) = env_dir("RIMSTUDIO_GAME_DIR") {
        set.insert(ModSource::new(
            SourceId::game_data(),
            SourceKind::GameData,
            game.join("Data"),
        ));
        set.insert(ModSource::new(
            SourceId::game_mods(),
            SourceKind::GameMods,
            game.join("Mods"),
        ));
    }
    if let Some(ws) = env_dir("RIMSTUDIO_WORKSHOP_DIR") {
        set.insert(ModSource::new(
            SourceId::workshop(0),
            SourceKind::Workshop,
            ws,
        ));
    }
    if let Some(custom) = env_dir("RIMSTUDIO_CUSTOM_DIR") {
        let mut f = CustomFolder::new(SourceId::new("cf_00000001").unwrap(), custom);
        f.layout = FolderLayout::Auto;
        f.scan_depth = 2;
        set.insert(f.to_source(0));
        opts = opts.with_custom_folders(&[f]);
    }
    (set, opts)
}

fn run(set: &SourceSet, opts: &ScanOptions) -> ScanOutcome {
    Scanner::scan(set, opts, &NoopProgress, &CancelToken::new())
}

fn describe(label: &str, out: &ScanOutcome) {
    let t = &out.timings;
    let s = &out.stats;
    println!(
        "{label}: mods {} defs {} | parsed: about {} lf {} defs {} | reused: about {} defs {} | \
         discover {:?} level0 {:?} level1 {:?} total {:?}",
        s.mods_indexed,
        s.defs,
        s.about_parsed,
        s.load_folders_parsed,
        s.def_files_parsed,
        s.about_reused,
        s.def_files_reused,
        t.discover,
        t.metadata,
        t.definitions,
        t.total
    );
}

#[test]
#[ignore = "reads real folders from RIMSTUDIO_* environment variables"]
fn real_library_scan_counts_and_timings() {
    let (set, opts) = real_sources();
    if set.is_empty() {
        println!("no RIMSTUDIO_* folder set, nothing to scan");
        return;
    }
    let level0 = opts.clone().with_level(ScanLevel::Metadata);
    let cold = run(&set, &level0);
    describe("level 0 first run ", &cold);
    let warm = run(&set, &level0);
    describe("level 0 warm      ", &warm);
    for r in &warm.sources {
        println!(
            "  source {} {:?} {:?} mods {}",
            r.id, r.kind, r.status, r.mods
        );
    }

    let full = run(&set, &opts);
    describe("level 1 no cache  ", &full);
    let again = run(
        &set,
        &opts.clone().with_previous(Arc::new(full.manifest.clone())),
    );
    describe("level 1 with cache", &again);
    assert_eq!(
        again.stats.files_parsed(),
        0,
        "an unchanged library must not be parsed again"
    );
    assert_eq!(full.index.len(), again.index.len());

    let bytes = full.manifest.to_json_bytes().unwrap();
    println!(
        "manifest: {} mods, {} files, {} defs, {} bytes",
        full.manifest.mod_count(),
        full.manifest.file_count(),
        full.manifest.def_count(),
        bytes.len()
    );
    println!("diagnostics: {:?}", full.diagnostics.counts);

    let report = resolve(&full.index, &DuplicatePolicy::from_sources(&set));
    println!("duplicate groups: {}", report.group_count());
    for g in &report.groups {
        println!(
            "  {} members {} reason {:?} scope {:?} steam note {}",
            g.key,
            g.members.len(),
            g.reason,
            g.scope,
            g.steam_note.is_some()
        );
    }
    assert!(!full.index.is_empty());
}

#[test]
#[ignore = "reads RIMSTUDIO_GAME_DIR"]
fn real_game_data_contains_the_core_mod() {
    let Some(game) = env_dir("RIMSTUDIO_GAME_DIR") else {
        println!("RIMSTUDIO_GAME_DIR not set");
        return;
    };
    let mut set = SourceSet::new();
    set.insert(ModSource::new(
        SourceId::game_data(),
        SourceKind::GameData,
        game.join("Data"),
    ));
    let out = run(&set, &ScanOptions::default());
    describe("game data", &out);
    let (_, core) = out.index.first_by_package_id("ludeon.rimworld").unwrap();
    println!("core defs files: {}", core.root_dirs.len());
    assert!(out.index.def_count() > 1000);
}
