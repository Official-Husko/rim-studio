//! Lints the owner's hand written Combat Extended patches on the real install: the Gewehr 41 patch and the
//! Lone Wolf weapon package patches, and prints a summary.
//!
//! The test is `#[ignore]`: it only reads the install and the owner's mod folders. The XML files of the mods
//! are copied into a temporary folder first, so nothing is registered or written next to the originals. The
//! output is for the person running it and must not be committed.
//!
//! ```text
//! RIMSTUDIO_GAME_DIR=~/.steam/steam/steamapps/common/RimWorld \
//! RIMSTUDIO_CE_DIR=~/.steam/steam/steamapps/workshop/content/294100/2890901044 \
//! RIMSTUDIO_CUSTOM_DIR="/path/to/RimWorld Mods" \
//! cargo test -p rimstudio-toolkit --release --test real_lint_files -- --ignored --nocapture
//! ```
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use camino::Utf8PathBuf;
use rimstudio_core::ids::SourceId;
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_core::mods::{ModIndex, ModSource, SourceKind, SourceSet};
use rimstudio_core::version::GameVersion;
use rimstudio_io::roots::DataRoots;
use rimstudio_ipc_types::designer::{DesignerLintFilesRequest, LintFileStatusDto};
use rimstudio_library::scan::{ScanOptions, Scanner};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_toolkit::designer::{Ctx, lint_files};
use rimstudio_workspace::project::read_mod_meta;
use rimstudio_workspace::refset::{DesignerReferenceOptions, ReferenceSet};
use rimstudio_workspace::session::{OpenInput, WorkspaceSession};

fn dir_from_env(name: &str) -> Option<Utf8PathBuf> {
    let path = Utf8PathBuf::from(std::env::var(name).ok()?);
    path.is_dir().then_some(path)
}

fn session_for(game_dir: &Utf8PathBuf, ce_dir: Option<&Utf8PathBuf>) -> Arc<WorkspaceSession> {
    let text = std::fs::read_to_string(game_dir.join("Version.txt")).unwrap();
    let game = GameVersion::parse(text.trim()).unwrap();
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
    let mut options = DesignerReferenceOptions::new(game);
    let mut builder = ModIndex::builder();
    for (_, meta) in outcome.index.iter() {
        builder.push(meta.clone());
    }
    let mut ops = None;
    if let Some(dir) = ce_dir {
        let meta = read_mod_meta(dir).unwrap().meta;
        options = options.with_mod(meta.package_id.as_str().to_owned());
        builder.push(meta);
        ops = Some(rimstudio_design::ce::reader::custom_registry(
            &rimstudio_design::ce::reader::CeClassNames::default(),
            BTreeMap::new(),
        ));
    }
    let reference = ReferenceSet::reference_for_designer(&builder.build(), game_dir, &options);
    let mut input = OpenInput::new(reference).with_game_dir(game_dir.clone());
    if let Some(ops) = ops {
        input = input.with_custom_ops(ops);
    }
    Arc::new(WorkspaceSession::open_simple(input).unwrap())
}

/// Copies the XML files and the About folder of a mod (read only on the source side).
fn copy_text(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let target = to.join(&name);
        if path.is_dir() {
            if matches!(
                name.as_str(),
                "Textures" | "Sounds" | "Source" | "Raw Assets" | "Assemblies" | "Languages"
            ) {
                continue;
            }
            std::fs::create_dir_all(&target).unwrap();
            copy_text(&path, &target);
        } else if name.to_ascii_lowercase().ends_with(".xml")
            && entry.metadata().map(|m| m.len()).unwrap_or(u64::MAX) < 8_000_000
        {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

fn lint_mod(ctx: &Ctx, custom: &Utf8PathBuf, folder: &str, tmp: &Path) {
    let source = custom.join(folder);
    if !source.is_dir() {
        println!("{folder}: not found, skipped");
        return;
    }
    let copy = tmp.join(folder.replace(['[', ']', ' '], "_"));
    std::fs::create_dir_all(&copy).unwrap();
    copy_text(source.as_std_path(), &copy);
    let root = Utf8PathBuf::from_path_buf(copy).unwrap();
    let record = ctx.env().projects().open(&root).unwrap();
    let result = lint_files(
        ctx,
        &DesignerLintFilesRequest {
            project_id: record.id.as_str().to_owned(),
            paths: Vec::new(),
        },
    )
    .unwrap();
    println!(
        "{folder}: {} files, {} checked, {} errors, {} warnings, {} notes (Combat Extended data: {}, game version {})",
        result.counts.files,
        result.counts.checked,
        result.counts.errors,
        result.counts.warnings,
        result.counts.notes,
        result.ce_data,
        result.game_version
    );
    for file in &result.files {
        println!(
            "  {} [{:?}] {} operations, {} findings",
            file.path,
            file.status,
            file.operations,
            file.findings.len()
        );
        assert_ne!(file.status, LintFileStatusDto::Unreadable, "{}", file.path);
        for f in &file.findings {
            println!(
                "    {:?} {} op {:?} {}: {}",
                f.severity,
                f.rule_id.as_deref().unwrap_or(&f.code),
                f.operation,
                f.xpath.as_deref().unwrap_or("-"),
                f.message
            );
        }
    }
    for f in &result.project {
        println!("  project: {:?} {}", f.severity, f.message);
    }
    for n in &result.not_checked {
        println!("  not checked {}: {}", n.rule_id, n.reason);
    }
    assert!(result.counts.files > 0, "{folder} has no patch file");
    assert!(result.counts.checked > 0, "{folder}: no file was readable");
}

#[test]
#[ignore = "reads the real install and the owner's mod folders (RIMSTUDIO_GAME_DIR, RIMSTUDIO_CE_DIR, RIMSTUDIO_CUSTOM_DIR)"]
fn the_owners_hand_written_patches_are_linted_on_the_real_install() {
    let (Some(game_dir), Some(ce_dir), Some(custom)) = (
        dir_from_env("RIMSTUDIO_GAME_DIR"),
        dir_from_env("RIMSTUDIO_CE_DIR"),
        dir_from_env("RIMSTUDIO_CUSTOM_DIR"),
    ) else {
        println!(
            "RIMSTUDIO_GAME_DIR, RIMSTUDIO_CE_DIR or RIMSTUDIO_CUSTOM_DIR is not set, skipping"
        );
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().join("app")).unwrap();
    let roots = DataRoots::under_base(&base);
    roots.ensure_all().unwrap();
    let ctx = Ctx::new(
        session_for(&game_dir, Some(&ce_dir)),
        &roots,
        Arc::new(FakeClock::new(1_800_000_000_000)),
    )
    .unwrap();
    assert!(ctx.require_engine().unwrap().ce_available());
    for folder in ["[OH] Gewehr 41", "[OH] The Lone Wolf Weapon Package"] {
        lint_mod(&ctx, &custom, folder, tmp.path());
    }
}
