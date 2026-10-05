//! Real data: the About files of the owner's install, workshop and own mods, read only. Run with
//! `RIMSTUDIO_GAME_DIR=<install> RIMSTUDIO_WORKSHOP_DIR=<content dir> RIMSTUDIO_CUSTOM_DIR=<folder>
//!  cargo test -p rimstudio-toolkit --release --test real_about -- --ignored --nocapture`.
//!
//! The first test scans the sources, reads every About file and prints the findings summary across the
//! library. The second applies a no-op update (every field set to what it already is) to temporary copies of
//! ten mods and checks that the bytes stay identical. Nothing is ever written into the originals.
#![cfg(feature = "tool-project")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::collections::BTreeMap;
use std::sync::Arc;

use camino::Utf8PathBuf;
use rimstudio_core::ids::SourceId;
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_core::mods::{ModSource, SourceKind, SourceSet};
use rimstudio_core::settings::{CustomFolder, FolderLayout};
use rimstudio_core::version::GameVersion;
use rimstudio_design::assets::{Detected, detect};
use rimstudio_io::roots::DataRoots;
use rimstudio_ipc_types::project_about::{
    AboutChangeDto, AboutListFieldDto, AboutTextFieldDto, ProjectAboutGetRequest,
    ProjectAboutUpdateRequest,
};
use rimstudio_library::scan::{ScanOptions, Scanner};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_toolkit::project::about::{self, AboutContext};
use rimstudio_toolkit::project::about_lint::{LintContext, PreviewFacts, lint};
use rimstudio_toolkit::project::open_project;
use rimstudio_toolkit::shared::env::ProjectEnv;
use rimstudio_workspace::project::find_about_file;
use rimstudio_xml::about::read_lenient;
use rimstudio_xml::edit::SpanEditor;

fn env_dir(name: &str) -> Option<Utf8PathBuf> {
    std::env::var(name)
        .ok()
        .filter(|v| !v.is_empty())
        .map(Utf8PathBuf::from)
}

fn sources() -> (SourceSet, ScanOptions) {
    let mut set = SourceSet::new();
    let mut opts =
        ScanOptions::default().with_game_version(GameVersion::parse("1.6.4630").unwrap());
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

fn preview_facts(root: &Utf8PathBuf) -> PreviewFacts {
    let path = root.join("About/Preview.png");
    let Ok(bytes) = std::fs::read(&path) else {
        return PreviewFacts::default();
    };
    PreviewFacts {
        exists: true,
        other_spelling: None,
        bytes: bytes.len() as u64,
        dimensions: match detect(&bytes) {
            Detected::Png(p) => Some((p.width, p.height)),
            _ => None,
        },
    }
}

#[test]
#[ignore = "reads the owner's mod folders (RIMSTUDIO_*_DIR)"]
fn the_findings_across_the_library() {
    let (set, opts) = sources();
    assert!(
        set.iter().next().is_some(),
        "set RIMSTUDIO_GAME_DIR, RIMSTUDIO_WORKSHOP_DIR or RIMSTUDIO_CUSTOM_DIR"
    );
    let out = Scanner::scan(&set, &opts, &NoopProgress, &CancelToken::new());
    let game = GameVersion::parse("1.6.4630").unwrap();
    let mut by_code: BTreeMap<String, (usize, String)> = BTreeMap::new();
    let (mut files, mut unparsed) = (0usize, 0usize);
    for (_, meta, _, _) in out.index.iter_full() {
        let Some(about_path) = find_about_file(&meta.path) else {
            continue;
        };
        let bytes = std::fs::read(&about_path).unwrap();
        files += 1;
        let read = read_lenient(&bytes);
        if !read.parsed {
            unparsed += 1;
            continue;
        }
        let present = std::str::from_utf8(&bytes)
            .ok()
            .and_then(|t| SpanEditor::open(t).ok())
            .map(|ed| {
                let root = format!("/{}", ed.root_tag());
                ed.children(&root)
                    .unwrap()
                    .into_iter()
                    .map(|c| c.tag)
                    .collect()
            })
            .unwrap_or_default();
        let preview = preview_facts(&meta.path);
        let icon = |_: &str| true;
        let findings = lint(
            &read.about,
            &present,
            &LintContext {
                library: Some((&out.index, meta.path.as_path())),
                game_version: Some(&game),
                preview: &preview,
                icon_exists: &icon,
            },
        );
        for d in findings {
            let entry = by_code
                .entry(d.code.as_str().to_owned())
                .or_insert((0, format!("{}: {}", meta.name, d.message)));
            entry.0 += 1;
        }
    }
    println!("{files} About files, {unparsed} not parsed");
    let mut rows: Vec<_> = by_code.into_iter().collect();
    rows.sort_by_key(|row| std::cmp::Reverse(row.1.0));
    for (code, (n, sample)) in rows {
        let cut: String = sample.chars().take(150).collect();
        println!("{n:>5}  {code:<42} e.g. {cut}");
    }
    assert!(files > 0);
}

#[test]
#[ignore = "reads the owner's mod folders (RIMSTUDIO_*_DIR)"]
fn a_no_op_update_keeps_the_bytes_of_ten_real_files() {
    let (set, opts) = sources();
    let out = Scanner::scan(&set, &opts, &NoopProgress, &CancelToken::new());
    let mut candidates: Vec<(String, Utf8PathBuf)> = Vec::new();
    for (_, meta, _, _) in out.index.iter_full() {
        if let Some(p) = find_about_file(&meta.path)
            && std::fs::read_to_string(&p).is_ok_and(|t| SpanEditor::open(t).is_ok())
            && read_lenient(&std::fs::read(&p).unwrap()).parsed
        {
            candidates.push((meta.name.clone(), p));
        }
    }
    assert!(candidates.len() >= 10, "{} candidates", candidates.len());
    let step = candidates.len() / 10;
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().canonicalize().unwrap()).unwrap();
    let roots = DataRoots::under_base(&base.join("app"));
    roots.ensure_all().unwrap();
    let env = ProjectEnv::new(
        &roots,
        Arc::new(FakeClock::new(FakeClock::DEFAULT_START_MS)),
    )
    .unwrap();
    let mut checked = 0;
    for (n, (name, original)) in candidates.iter().step_by(step.max(1)).take(10).enumerate() {
        let root = base.join(format!("copy{n}"));
        std::fs::create_dir_all(root.join("About")).unwrap();
        let bytes = std::fs::read(original).unwrap();
        std::fs::write(root.join("About/About.xml"), &bytes).unwrap();
        let id = open_project(&env, &root)
            .unwrap()
            .record
            .id
            .as_str()
            .to_owned();
        let model = about::get(
            &env,
            &ProjectAboutGetRequest {
                project_id: id.clone(),
                include_preview_image: false,
            },
            &AboutContext::default(),
            &[],
        )
        .unwrap();
        // every field set to what the file already says
        let text = String::from_utf8(bytes.clone()).unwrap();
        let ed = SpanEditor::open(text.as_str()).unwrap();
        let rootp = format!("/{}", ed.root_tag());
        let raw = |tag: &str| {
            ed.exists(&format!("{rootp}/{tag}"))
                .then(|| ed.element_text(&format!("{rootp}/{tag}")).unwrap())
        };
        let mut changes: Vec<AboutChangeDto> = Vec::new();
        for (field, tag) in [
            (AboutTextFieldDto::Name, "name"),
            (AboutTextFieldDto::Author, "author"),
            (AboutTextFieldDto::PackageId, "packageId"),
            (AboutTextFieldDto::Url, "url"),
            (AboutTextFieldDto::ModVersion, "modVersion"),
            (AboutTextFieldDto::Description, "description"),
        ] {
            // a field with padding or a line break is trimmed by a set, which is not a no-op
            if let Some(t) = raw(tag)
                && !t.is_empty()
                && (tag == "description" || (t == t.trim() && !t.contains('\n')))
            {
                changes.push(AboutChangeDto::Set { field, value: t });
            }
        }
        for (field, items) in [
            (
                AboutListFieldDto::SupportedVersions,
                &model.supported_versions,
            ),
            (AboutListFieldDto::LoadAfter, &model.load_after),
            (AboutListFieldDto::LoadBefore, &model.load_before),
            (
                AboutListFieldDto::IncompatibleWith,
                &model.incompatible_with,
            ),
        ] {
            if items.present && !items.items.is_empty() {
                changes.push(AboutChangeDto::ListSet {
                    field,
                    items: items.items.clone(),
                });
            }
        }
        let changed_before = changes.len();
        let result = about::update(
            &env,
            &ProjectAboutUpdateRequest {
                project_id: id,
                changes,
                expected_hash: Some(model.file_hash.clone()),
            },
            &AboutContext::default(),
            &[],
        )
        .unwrap();
        let after = std::fs::read(root.join("About/About.xml")).unwrap();
        println!(
            "{name}: {changed_before} no-op changes, written {}, {} bytes",
            result.written,
            after.len()
        );
        assert_eq!(after, bytes, "{name}: the bytes changed\n{}", result.diff);
        assert!(!result.written);
        checked += 1;
    }
    assert_eq!(checked, 10);
}
