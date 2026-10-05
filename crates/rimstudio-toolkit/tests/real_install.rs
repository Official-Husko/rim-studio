//! Tests against a real install. All are `#[ignore]`: they only read the install and write only into
//! temporary folders. Nothing is asserted about counts, only about structure; the output is for the
//! person running them and must not be committed.
//!
//! ```text
//! RIMSTUDIO_GAME_DIR=~/.steam/steam/steamapps/common/RimWorld \
//! RIMSTUDIO_CE_DIR=~/.steam/steam/steamapps/workshop/content/294100/2890901044 \
//! cargo test -p rimstudio-toolkit --release --test real_install -- --ignored --nocapture
//! ```
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::sync::Arc;
use std::time::Instant;

use camino::Utf8PathBuf;
use rimstudio_core::ids::SourceId;
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_core::mods::{ModIndex, ModSource, SourceKind, SourceSet};
use rimstudio_core::version::GameVersion;
use rimstudio_design::model::{DesignSpec, Draft, TechLevel};
use rimstudio_io::roots::DataRoots;
use rimstudio_ipc_types::designer::{
    DesignerCalibrateRequest, DesignerFitRequest, DesignerPreviewRequest,
    DesignerReferenceListRequest, ItemKindDto,
};
use rimstudio_library::scan::{ScanOptions, Scanner};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_toolkit::designer::dto::draft_to_dto;
use rimstudio_toolkit::designer::{
    Ctx, calibrate, fit, preview, reference_list, reference_list_ce, suggest_fill,
};
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
    let mut input_ops = None;
    if let Some(dir) = ce_dir {
        let meta = read_mod_meta(dir).unwrap().meta;
        options = options.with_mod(meta.package_id.as_str().to_owned());
        builder.push(meta);
        // the gun conversion operation of Combat Extended is registered by the caller
        input_ops = Some(rimstudio_design::ce::reader::custom_registry(
            &rimstudio_design::ce::reader::CeClassNames::default(),
            std::collections::BTreeMap::new(),
        ));
    }
    let index = builder.build();
    let reference = ReferenceSet::reference_for_designer(&index, game_dir, &options);
    let mut input = OpenInput::new(reference).with_game_dir(game_dir.clone());
    if let Some(ops) = input_ops {
        input = input.with_custom_ops(ops);
    }
    Arc::new(WorkspaceSession::open_simple(input).unwrap())
}

fn ctx_for(session: Arc<WorkspaceSession>) -> (Ctx, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let roots = DataRoots::under_base(&base);
    roots.ensure_all().unwrap();
    let ctx = Ctx::new(session, &roots, Arc::new(FakeClock::new(1_800_000_000_000))).unwrap();
    (ctx, tmp)
}

fn open(game_dir: &Utf8PathBuf) -> (Ctx, tempfile::TempDir) {
    ctx_for(session_for(game_dir, None))
}

#[test]
#[ignore = "reads the real install (RIMSTUDIO_GAME_DIR)"]
fn reference_sizes_and_a_preview_of_a_fictional_rifle_in_a_real_class() {
    let Some(game_dir) = dir_from_env("RIMSTUDIO_GAME_DIR") else {
        println!("RIMSTUDIO_GAME_DIR is not set, skipping");
        return;
    };
    let started = Instant::now();
    let (ctx, _tmp) = open(&game_dir);
    println!("open: {:?}", started.elapsed());

    for kind in [ItemKindDto::Ranged, ItemKindDto::Melee] {
        let list = reference_list(
            &ctx,
            DesignerReferenceListRequest {
                kind,
                limit: 500,
                ..DesignerReferenceListRequest::default()
            },
        )
        .unwrap();
        println!(
            "{kind:?}: {} reference weapons, {} stat pools, label {}",
            list.total,
            list.pools.len(),
            list.class_label
        );
    }
    println!(
        "combat extended list available: {}",
        reference_list_ce(&ctx, DesignerReferenceListRequest::default()).is_ok()
    );

    // a fictional rifle in the most common real ranged role at the industrial tier
    let engine = ctx.require_engine().unwrap();
    let pool = engine
        .pool(rimstudio_design::classes::ItemKind::Ranged)
        .unwrap();
    let mut roles = pool.roles();
    roles.sort_by_key(|r| std::cmp::Reverse(r.1));
    let role = roles.first().map(|r| r.0.clone());
    println!("ranged roles: {roles:?}");
    let mut spec = DesignSpec::new_ranged("RS_TestRifle", "test rifle");
    spec.tech_level = Some(TechLevel::Industrial);
    spec.role = role;
    let filled = suggest_fill(&ctx, draft_to_dto(&Draft::new(spec)).unwrap()).unwrap();
    let p = preview(
        &ctx,
        DesignerPreviewRequest {
            draft: filled.clone(),
        },
    )
    .unwrap();
    for r in &p.readouts {
        println!("readout {} = {:?}", r.key, r.value);
    }
    if let Some(e) = &p.estimate {
        println!("estimate: {} (strength {:.2})", e.class_label, e.strength);
    }
    let started = Instant::now();
    let runs = 200;
    for _ in 0..runs {
        let _ = preview(
            &ctx,
            DesignerPreviewRequest {
                draft: filled.clone(),
            },
        )
        .unwrap();
    }
    println!("preview: {:?} per call", started.elapsed() / runs);
    let r = fit(&ctx, DesignerFitRequest { draft: filled }).unwrap();
    println!(
        "fit: {} scored, {} typical, {} plausible, {} unusual",
        r.summary.scored, r.summary.typical, r.summary.plausible, r.summary.unusual
    );

    let started = Instant::now();
    let result = calibrate(
        &ctx,
        DesignerCalibrateRequest::default(),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    println!(
        "calibration of {} weapons took {:?}, {} variants",
        result.pool_size,
        started.elapsed(),
        result.variants.len()
    );
}

#[test]
#[ignore = "reads the real install and a Combat Extended folder (RIMSTUDIO_GAME_DIR, RIMSTUDIO_CE_DIR)"]
fn combat_extended_as_a_reference_mod_enables_only_the_optional_readouts() {
    let (Some(game_dir), Some(ce_dir)) = (
        dir_from_env("RIMSTUDIO_GAME_DIR"),
        dir_from_env("RIMSTUDIO_CE_DIR"),
    ) else {
        println!("RIMSTUDIO_GAME_DIR or RIMSTUDIO_CE_DIR is not set, skipping");
        return;
    };
    let started = Instant::now();
    // the vanilla session gives the pools and the twins, the Combat Extended session the CE model
    let (ctx, _tmp) = open(&game_dir);
    let ce_session = session_for(&game_dir, Some(&ce_dir));
    let ctx = ctx.with_ce_session(ce_session);
    println!("open both sessions: {:?}", started.elapsed());
    let engine = ctx.require_engine().unwrap();
    println!("combat extended present: {}", engine.ce_available());
    let ranged = reference_list(
        &ctx,
        DesignerReferenceListRequest {
            limit: 500,
            ..DesignerReferenceListRequest::default()
        },
    )
    .unwrap();
    println!("vanilla ranged list: {} weapons", ranged.total);
    match reference_list_ce(
        &ctx,
        DesignerReferenceListRequest {
            limit: 500,
            ..DesignerReferenceListRequest::default()
        },
    ) {
        Ok(ce) => println!("combat extended ranged list: {} weapons", ce.total),
        Err(e) => println!("combat extended list: {e}"),
    }
    let mut spec = DesignSpec::new_ranged("RS_TestRifle", "test rifle");
    spec.tech_level = Some(TechLevel::Industrial);
    spec.role = Some("Gun".into());
    let plain = preview(
        &ctx,
        DesignerPreviewRequest {
            draft: draft_to_dto(&Draft::new(spec.clone())).unwrap(),
        },
    )
    .unwrap();
    assert!(plain.readouts.iter().all(|r| !r.key.starts_with("ce-")));
    spec.ce = Some(rimstudio_design::model::CePatchSpec::default());
    let with_ce = preview(
        &ctx,
        DesignerPreviewRequest {
            draft: draft_to_dto(&Draft::new(spec)).unwrap(),
        },
    )
    .unwrap();
    for r in with_ce.readouts.iter().filter(|r| r.key.starts_with("ce-")) {
        println!("readout {} = {:?}", r.key, r.value);
    }
}
