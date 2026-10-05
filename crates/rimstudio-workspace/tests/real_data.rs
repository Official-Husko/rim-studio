//! Tests against a real install. All are `#[ignore]`: they only read, never write, and print what
//! they find. Nothing is asserted about counts, only about structure.
//!
//! Run them with the folders of your machine:
//!
//! ```text
//! RIMSTUDIO_GAME_DIR=~/.steam/steam/steamapps/common/RimWorld \
//! RIMSTUDIO_CE_DIR=~/.steam/steam/steamapps/workshop/content/294100/2890901044 \
//! RIMSTUDIO_CUSTOM_DIR="/path/to/owner/mods" \
//! cargo test -p rimstudio-workspace --release --test real_data -- --ignored --nocapture
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::sync::Arc;
use std::time::Instant;

use camino::Utf8PathBuf;
use rimstudio_core::ids::SourceId;
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_core::mods::{ModIndex, ModSource, SourceKind, SourceSet};
use rimstudio_core::version::GameVersion;
use rimstudio_library::scan::{ScanOptions, Scanner};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_workspace::defindex::{Page, Query};
use rimstudio_workspace::project::read_mod_meta;
use rimstudio_workspace::refset::{DesignerReferenceOptions, ReferenceSet};
use rimstudio_workspace::session::{OpenInput, WorkspaceSession};
use rimstudio_workspace::typetable::CacheConfig;

fn dir_from_env(name: &str) -> Option<Utf8PathBuf> {
    let value = std::env::var(name).ok()?;
    let path = Utf8PathBuf::from(value);
    path.is_dir().then_some(path)
}

fn game_version(game_dir: &Utf8PathBuf) -> GameVersion {
    let text = std::fs::read_to_string(game_dir.join("Version.txt")).unwrap();
    GameVersion::parse(text.trim()).unwrap()
}

fn game_index(game_dir: &Utf8PathBuf, game: &GameVersion) -> ModIndex {
    let mut set = SourceSet::new();
    set.insert(ModSource::new(
        SourceId::game_data(),
        SourceKind::GameData,
        game_dir.join("Data"),
    ));
    let outcome = Scanner::scan(
        &set,
        &ScanOptions::default().with_game_version(game.clone()),
        &NoopProgress,
        &CancelToken::new(),
    );
    let index: &ModIndex = &outcome.index;
    index.clone()
}

fn report(label: &str, session: &WorkspaceSession) {
    let s = session.stats();
    let snap = session.current();
    println!(
        "{label}: {} packs, {} files ({} parsed), {} index rows, {} defs, {} unknown type, {} patch ops",
        session.reference().packs.len(),
        s.files_total,
        s.files_parsed,
        s.index_entries,
        snap.stats.defs,
        snap.stats.unknown_type,
        snap.stats.patch_ops
    );
    println!(
        "{label}: types {} us (cached {}), parse {} us, load {} us, index {} us, total {} us",
        s.timings.types,
        s.type_table_cached,
        s.timings.parse,
        s.timings.load,
        s.timings.index,
        s.timings.total
    );
    println!(
        "{label}: type table has {} def types, {} assemblies read, {} skipped",
        session.type_table().len(),
        s.assemblies_read,
        s.assemblies_skipped
    );
}

#[test]
#[ignore = "reads the real install (RIMSTUDIO_GAME_DIR)"]
fn open_the_real_game_with_expansions() {
    let Some(game_dir) = dir_from_env("RIMSTUDIO_GAME_DIR") else {
        println!("RIMSTUDIO_GAME_DIR is not set, skipping");
        return;
    };
    let game = game_version(&game_dir);
    let index = game_index(&game_dir, &game);
    let reference = ReferenceSet::reference_for_designer(
        &index,
        &game_dir,
        &DesignerReferenceOptions::new(game),
    );
    assert!(
        reference.packs[0]
            .package_id
            .eq_ignore_ascii_case("ludeon.rimworld")
    );
    let cache_dir = tempfile::tempdir().unwrap();
    let cache = CacheConfig::new(
        Utf8PathBuf::from_path_buf(cache_dir.path().to_path_buf()).unwrap(),
        Arc::new(FakeClock::new(1)),
    );
    let input = OpenInput::new(reference)
        .with_game_dir(game_dir.clone())
        .with_cache(cache);
    let started = Instant::now();
    let session = WorkspaceSession::open_simple(input.clone()).unwrap();
    println!("cold open: {:?}", started.elapsed());
    report("game", &session);
    assert!(session.current().stats.defs > 0);
    assert!(session.type_table().info("Verse.Def").is_some());
    assert!(session.index().len() >= session.current().stats.defs as usize);

    let q = Query::text("rifle").concrete_only();
    let started = Instant::now();
    let found = session.search(&q, Page::default());
    println!(
        "search 'rifle': {} hits in {:?}",
        found.total,
        started.elapsed()
    );
    assert!(found.total > 0);

    // a second session with the same parse cache parses nothing and reads the table from the cache
    let again =
        WorkspaceSession::open_simple(input.with_parse_cache(session.parse_cache())).unwrap();
    report("game again", &again);
    assert_eq!(again.stats().files_parsed, 0);
    assert!(again.stats().type_table_cached);
}

#[test]
#[ignore = "reads the real install and a Combat Extended folder (RIMSTUDIO_GAME_DIR, RIMSTUDIO_CE_DIR)"]
fn open_the_real_game_with_combat_extended_as_a_reference() {
    let (Some(game_dir), Some(ce_dir)) = (
        dir_from_env("RIMSTUDIO_GAME_DIR"),
        dir_from_env("RIMSTUDIO_CE_DIR"),
    ) else {
        println!("RIMSTUDIO_GAME_DIR or RIMSTUDIO_CE_DIR is not set, skipping");
        return;
    };
    let game = game_version(&game_dir);
    let base = game_index(&game_dir, &game);
    let ce_meta = read_mod_meta(&ce_dir).unwrap().meta;
    let ce_id = ce_meta.package_id.as_str().to_owned();
    let mut builder = ModIndex::builder();
    for (_, meta) in base.iter() {
        builder.push(meta.clone());
    }
    builder.push(ce_meta);
    let index = builder.build();
    let reference = ReferenceSet::reference_for_designer(
        &index,
        &game_dir,
        &DesignerReferenceOptions::new(game).with_mod(ce_id.clone()),
    );
    assert_eq!(reference.packs.last().unwrap().package_id, ce_id);
    println!(
        "CE pack loads {:?}: {} defs files, {} patch files, {} assemblies",
        reference.packs.last().unwrap().plan.relative_folders(),
        reference.packs.last().unwrap().defs.len(),
        reference.packs.last().unwrap().patches.len(),
        reference.packs.last().unwrap().assemblies.len()
    );
    let session =
        WorkspaceSession::open_simple(OpenInput::new(reference).with_game_dir(game_dir)).unwrap();
    report("game + CE", &session);
    // the CE assembly brings its own def classes into the table
    let ce_types = session
        .type_table()
        .iter()
        .filter(|(_, info)| {
            info.assembly
                .as_deref()
                .is_some_and(|a| a.starts_with("Combat"))
        })
        .count();
    println!("def types from the CE assembly: {ce_types}");
    assert!(ce_types > 0);
}

#[test]
#[ignore = "reads the real install and the owner's mod folder (RIMSTUDIO_GAME_DIR, RIMSTUDIO_CUSTOM_DIR)"]
fn open_a_real_custom_mod_as_the_project_overlay() {
    let (Some(game_dir), Some(custom)) = (
        dir_from_env("RIMSTUDIO_GAME_DIR"),
        dir_from_env("RIMSTUDIO_CUSTOM_DIR"),
    ) else {
        println!("RIMSTUDIO_GAME_DIR or RIMSTUDIO_CUSTOM_DIR is not set, skipping");
        return;
    };
    let game = game_version(&game_dir);
    let index = game_index(&game_dir, &game);
    let reference = ReferenceSet::reference_for_designer(
        &index,
        &game_dir,
        &DesignerReferenceOptions::new(game),
    );
    let mut folders: Vec<Utf8PathBuf> = std::fs::read_dir(&custom)
        .unwrap()
        .flatten()
        .filter_map(|e| Utf8PathBuf::from_path_buf(e.path()).ok())
        .filter(|p| p.is_dir() && read_mod_meta(p).is_ok())
        .collect();
    folders.sort();
    let Some(project) = folders.first() else {
        println!("no mod folder found in {custom}, skipping");
        return;
    };
    println!("project overlay: {project}");
    let input = OpenInput::new(reference)
        .with_game_dir(game_dir)
        .with_project(project)
        .unwrap();
    let session = WorkspaceSession::open_simple(input).unwrap();
    report("game + project", &session);
    let last = session.reference().packs.last().unwrap();
    assert_eq!(last.kind, rimstudio_workspace::refset::PackKind::Project);
    println!(
        "project pack: {} defs files, {} patch files",
        last.defs.len(),
        last.patches.len()
    );
    assert!(!session.is_stale());
}
