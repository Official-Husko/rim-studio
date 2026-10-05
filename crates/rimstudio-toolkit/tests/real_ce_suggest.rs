//! Prints the Combat Extended suggestions for a fictional rifle on a real install with Combat Extended, and
//! the plan that accepts them once the caliber and the tag class are answered.
//!
//! The test is `#[ignore]`: it only reads the install and writes only into temporary folders. The output is
//! for the person running it and must not be committed.
//!
//! ```text
//! RIMSTUDIO_GAME_DIR=~/.steam/steam/steamapps/common/RimWorld \
//! RIMSTUDIO_CE_DIR=~/.steam/steam/steamapps/workshop/content/294100/2890901044 \
//! cargo test -p rimstudio-toolkit --release --test real_ce_suggest -- --ignored --nocapture
//! ```
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::sync::Arc;

use camino::Utf8PathBuf;
use rimstudio_core::ids::SourceId;
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_core::mods::{ModIndex, ModSource, SourceKind, SourceSet};
use rimstudio_core::version::GameVersion;
use rimstudio_design::classes::ItemKind as PoolKind;
use rimstudio_design::model::{
    CePatchSpec, CostEntry, DesignSpec, Draft, ParentRef, ScalarField, Sourced, TechLevel,
    ValueSource,
};
use rimstudio_io::roots::DataRoots;
use rimstudio_ipc_types::designer::{
    AcceptSuggestionsDto, DesignerCeSuggestRequest, DesignerExportPlanRequest,
};
use rimstudio_library::scan::{ScanOptions, Scanner};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_toolkit::designer::dto::{draft_from_dto, draft_to_dto};
use rimstudio_toolkit::designer::plan::export_plan;
use rimstudio_toolkit::designer::{Ctx, ce_suggest, suggest_fill};
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
            std::collections::BTreeMap::new(),
        ));
    }
    let reference = ReferenceSet::reference_for_designer(&builder.build(), game_dir, &options);
    let mut input = OpenInput::new(reference).with_game_dir(game_dir.clone());
    if let Some(ops) = ops {
        input = input.with_custom_ops(ops);
    }
    Arc::new(WorkspaceSession::open_simple(input).unwrap())
}

#[test]
#[ignore = "reads the real install (RIMSTUDIO_GAME_DIR, RIMSTUDIO_CE_DIR)"]
fn the_suggestions_for_a_fictional_rifle_on_the_real_install() {
    let (Some(game_dir), Some(ce_dir)) = (
        dir_from_env("RIMSTUDIO_GAME_DIR"),
        dir_from_env("RIMSTUDIO_CE_DIR"),
    ) else {
        println!("RIMSTUDIO_GAME_DIR or RIMSTUDIO_CE_DIR is not set, skipping");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let roots = DataRoots::under_base(&base.join("app"));
    roots.ensure_all().unwrap();
    let ctx = Ctx::new(
        session_for(&game_dir, None),
        &roots,
        Arc::new(FakeClock::new(1_800_000_000_000)),
    )
    .unwrap()
    .with_ce_session(session_for(&game_dir, Some(&ce_dir)));
    let engine = ctx.require_engine().unwrap();
    assert!(engine.ce_available(), "Combat Extended is not loaded");

    // the class of the most common ranged role of the install, with the weapon tags of one of its members
    let pool = engine.pool(PoolKind::Ranged).unwrap();
    let mut roles = pool.roles();
    roles.sort_by_key(|r| std::cmp::Reverse(r.1));
    let role = roles.first().map(|r| r.0.clone()).unwrap();
    let reference = pool.items.iter().find(|i| i.role == role).unwrap();
    let record = engine.databases().get("ThingDef", &reference.id).unwrap();
    let tags: Vec<String> = record
        .node
        .child("weaponTags")
        .map(|t| {
            t.children_named("li")
                .map(|li| li.text_content().trim().to_owned())
                .collect()
        })
        .unwrap_or_default();
    let mut spec = DesignSpec::new_ranged("RS_SuggestRifle", "suggest rifle");
    spec.tech_level = Some(TechLevel::Industrial);
    spec.role = Some(role);
    spec.parent = record.parents.first().map(|p| ParentRef::named(&p.name));
    spec.weapon_tags = tags;
    spec.cost_list = vec![CostEntry::new("Steel", 40.0)];
    let filled = suggest_fill(&ctx, draft_to_dto(&Draft::new(spec)).unwrap()).unwrap();
    let mut spec = draft_from_dto(&filled).unwrap().spec;
    spec.offer(ScalarField::WorkToMake, 9000.0, ValueSource::Typed);

    // the toggle is off: suggestions only
    let suggestion = ce_suggest(
        &ctx,
        DesignerCeSuggestRequest {
            draft: draft_to_dto(&Draft::new(spec.clone())).unwrap(),
        },
    )
    .unwrap();
    println!("{}", serde_json::to_string_pretty(&suggestion).unwrap());
    assert!(suggestion.available && !suggestion.toggle_on);

    // answer the caliber and the tag class with the best candidates, accept the rest
    let first = |field: &str| {
        suggestion
            .choices
            .iter()
            .find(|c| c.field == field)
            .and_then(|c| c.candidates.first())
            .map(|c| c.name.clone())
    };
    spec.ce = Some(CePatchSpec {
        ammo_set: first("/ce/ammoSet"),
        weapon_tag_class: first("/ce/weaponTagClass"),
        bulk: Some(Sourced::typed(6.0)),
        ..CePatchSpec::default()
    });
    let root = base.join("RS_Suggest");
    std::fs::create_dir_all(root.join("About").as_std_path()).unwrap();
    std::fs::write(
        root.join("About/About.xml").as_std_path(),
        "<ModMetaData><name>RS Suggest</name><packageId>rs.suggest</packageId><supportedVersions><li>1.6</li></supportedVersions></ModMetaData>",
    )
    .unwrap();
    let project_id = ctx
        .env()
        .projects()
        .open(&root)
        .unwrap()
        .id
        .as_str()
        .to_owned();
    let plan = export_plan(
        &ctx,
        DesignerExportPlanRequest {
            project_id,
            draft: draft_to_dto(&Draft::new(spec)).unwrap(),
            convert: None,
            accept_suggestions: Some(AcceptSuggestionsDto::default()),
        },
    )
    .unwrap();
    println!("==== plan: errors {} ====", plan.has_errors);
    for d in &plan.diagnostics {
        println!("diagnostic {} {:?} {}", d.code, d.severity, d.message);
    }
    for f in plan.files.iter().filter(|f| f.path.contains("CE")) {
        println!("---- {}\n{}", f.path, f.rendered);
    }
}
