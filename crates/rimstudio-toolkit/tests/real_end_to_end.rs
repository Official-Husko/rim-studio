//! End to end on a real install: design a fictional weapon in the class of a real one, into a temporary
//! project, once vanilla and once with the optional Combat Extended patch, and print both plans.
//!
//! All tests are `#[ignore]`: they only read the install and write only into temporary folders. The output
//! is for the person running them and must not be committed.
//!
//! ```text
//! RIMSTUDIO_GAME_DIR=~/.steam/steam/steamapps/common/RimWorld \
//! RIMSTUDIO_CE_DIR=~/.steam/steam/steamapps/workshop/content/294100/2890901044 \
//! cargo test -p rimstudio-toolkit --release --test real_end_to_end -- --ignored --nocapture
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
    CePatchSpec, CostEntry, DesignSpec, Draft, ParentRef, ProjectileChoice, ScalarField, Sourced,
    TechLevel, ValueSource,
};
use rimstudio_io::roots::DataRoots;
use rimstudio_ipc_types::designer::{
    DesignerApplyPlanRequest, DesignerConvertScanRequest, DesignerExportPlanRequest,
};
use rimstudio_library::scan::{ScanOptions, Scanner};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_toolkit::designer::dto::draft_to_dto;
use rimstudio_toolkit::designer::plan::export_plan;
use rimstudio_toolkit::designer::{Ctx, apply_plan, convert_scan, suggest_fill};
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

fn show(label: &str, plan: &rimstudio_ipc_types::designer::WritePlanDto) {
    println!(
        "==== {label}: {} files, errors: {} ====",
        plan.files.len(),
        plan.has_errors
    );
    for d in &plan.diagnostics {
        println!("diagnostic {} {:?} {}", d.code, d.severity, d.message);
    }
    for f in &plan.files {
        println!(
            "---- {} ({:?}, {:?}, {} bytes)",
            f.path, f.kind, f.action, f.bytes
        );
        println!("{}", f.rendered);
    }
}

#[test]
#[ignore = "reads the real install (RIMSTUDIO_GAME_DIR, RIMSTUDIO_CE_DIR)"]
fn a_fictional_weapon_in_a_real_class_with_and_without_the_combat_extended_patch() {
    let (Some(game_dir), ce_dir) = (
        dir_from_env("RIMSTUDIO_GAME_DIR"),
        dir_from_env("RIMSTUDIO_CE_DIR"),
    ) else {
        println!("RIMSTUDIO_GAME_DIR is not set, skipping");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let roots = DataRoots::under_base(&base.join("app"));
    roots.ensure_all().unwrap();
    let mut ctx = Ctx::new(
        session_for(&game_dir, None),
        &roots,
        Arc::new(FakeClock::new(1_800_000_000_000)),
    )
    .unwrap();
    if let Some(ce) = &ce_dir {
        ctx = ctx.with_ce_session(session_for(&game_dir, Some(ce)));
    }
    let engine = ctx.require_engine().unwrap();
    println!("combat extended available: {}", engine.ce_available());

    // the project: a fictional mod folder in a temporary directory
    let root = base.join("RS_EndToEnd");
    std::fs::create_dir_all(root.join("About").as_std_path()).unwrap();
    std::fs::write(
        root.join("About/About.xml").as_std_path(),
        "<ModMetaData><name>RS End To End</name><packageId>rs.endtoend</packageId><supportedVersions><li>1.6</li></supportedVersions></ModMetaData>",
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

    // a class of the real install: the most common ranged role at the industrial tier
    let pool = engine.pool(PoolKind::Ranged).unwrap();
    let mut roles = pool.roles();
    roles.sort_by_key(|r| std::cmp::Reverse(r.1));
    let role = roles.first().map(|r| r.0.clone()).unwrap();
    let reference = pool
        .items
        .iter()
        .find(|i| i.role == role)
        .or_else(|| pool.items.first())
        .unwrap();
    let record = engine.databases().get("ThingDef", &reference.id).unwrap();
    let parent = record.parents.first().map(|p| p.name.clone());
    let projectile = record
        .node
        .child("verbs")
        .and_then(|v| v.children_named("li").next())
        .and_then(|li| li.child_text("defaultProjectile"))
        .map(str::to_owned);
    let cost: Vec<CostEntry> = record
        .node
        .child("costList")
        .map(|c| {
            c.elements()
                .take(2)
                .map(|e| CostEntry::new(&e.tag, e.text_content().trim().parse().unwrap_or(10.0)))
                .collect()
        })
        .unwrap_or_default();
    println!(
        "class of {} (role {role}), parent {parent:?}, projectile {projectile:?}",
        reference.id
    );

    let mut spec = DesignSpec::new_ranged("RS_EndToEndRifle", "end to end rifle");
    spec.tech_level = Some(TechLevel::Industrial);
    spec.role = Some(role);
    spec.parent = parent.map(ParentRef::named);
    spec.cost_list = cost;
    let filled = suggest_fill(&ctx, draft_to_dto(&Draft::new(spec)).unwrap()).unwrap();
    let draft = rimstudio_toolkit::designer::dto::draft_from_dto(&filled).unwrap();
    let mut spec = draft.spec;
    // work to make is always asked: take the value of the reference weapon as the typed number
    spec.offer(
        ScalarField::WorkToMake,
        reference.stat("work").unwrap_or(9000.0),
        ValueSource::Typed,
    );
    if let (Some(r), Some(p)) = (spec.ranged.as_mut(), projectile) {
        r.projectile = Some(ProjectileChoice::Reference(p));
    }

    let request = |spec: &DesignSpec| DesignerExportPlanRequest {
        project_id: project_id.clone(),
        draft: draft_to_dto(&Draft::new(spec.clone())).unwrap(),
        convert: None,
        accept_suggestions: None,
    };
    let vanilla = export_plan(&ctx, request(&spec)).unwrap();
    show("vanilla", &vanilla);
    assert!(
        vanilla
            .files
            .iter()
            .all(|f| !f.rendered.contains("CombatExtended."))
    );

    if engine.ce_available() {
        let model = engine.ce();
        spec.ce = Some(CePatchSpec {
            ammo_set: model.ammo_sets.first().map(|a| a.def_name.clone()),
            default_projectile: model
                .ammo_sets
                .first()
                .and_then(|a| a.first())
                .map(|a| a.projectile.clone()),
            weapon_tag_class: model.ai_class_tags.first().cloned(),
            magazine_size: Some(Sourced::typed(30)),
            reload_time: Some(Sourced::typed(4.0)),
            bulk: Some(Sourced::typed(6.0)),
            sway_factor: Some(Sourced::typed(1.2)),
            shot_spread: Some(Sourced::typed(0.1)),
            ..CePatchSpec::default()
        });
        let with_ce = export_plan(&ctx, request(&spec)).unwrap();
        show("with the optional Combat Extended patch", &with_ce);
        for f in &with_ce.files {
            if f.rendered.contains("CombatExtended.") {
                assert!(f.path.starts_with("CE/"), "{}", f.path);
            }
        }
        if !with_ce.has_errors {
            let report = apply_plan(
                &ctx,
                DesignerApplyPlanRequest {
                    plan_id: with_ce.plan_id.clone(),
                    request: request(&spec),
                    backup: true,
                    dry_apply: true,
                },
                &NoopProgress,
                &CancelToken::new(),
            )
            .unwrap();
            println!(
                "applied {} files, dry apply ok: {:?}",
                report.written.len(),
                report.dry_apply_ok
            );
            for d in &report.diagnostics {
                println!("apply diagnostic {} {}", d.code, d.message);
            }
            let scan = convert_scan(
                &ctx,
                DesignerConvertScanRequest {
                    project_id: project_id.clone(),
                    include_converted: true,
                },
                &NoopProgress,
                &CancelToken::new(),
            )
            .unwrap();
            println!("convert scan: {:?}", scan.counts);
        }
    }
}

#[test]
#[ignore = "reads the real install, Combat Extended and the owner's mod folders (RIMSTUDIO_GAME_DIR, RIMSTUDIO_CE_DIR, RIMSTUDIO_CUSTOM_DIR)"]
fn the_convert_scan_over_the_owners_mod_folders_reads_only() {
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
    let base = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let roots = DataRoots::under_base(&base);
    roots.ensure_all().unwrap();
    let ctx = Ctx::new(
        session_for(&game_dir, None),
        &roots,
        Arc::new(FakeClock::new(1_800_000_000_000)),
    )
    .unwrap()
    .with_ce_session(session_for(&game_dir, Some(&ce_dir)));
    let mut folders: Vec<Utf8PathBuf> = std::fs::read_dir(custom.as_std_path())
        .unwrap()
        .flatten()
        .filter_map(|e| Utf8PathBuf::from_path_buf(e.path()).ok())
        .filter(|p| p.join("About/About.xml").is_file() && p.join("Defs").is_dir())
        .collect();
    folders.sort();
    for folder in folders.iter().take(12) {
        let id = match ctx.env().projects().open(folder) {
            Ok(r) => r.id.as_str().to_owned(),
            Err(e) => {
                println!("{folder}: cannot open: {e}");
                continue;
            }
        };
        let started = std::time::Instant::now();
        match convert_scan(
            &ctx,
            DesignerConvertScanRequest {
                project_id: id,
                include_converted: true,
            },
            &NoopProgress,
            &CancelToken::new(),
        ) {
            Ok(scan) => {
                println!(
                    "{}: {} weapons, {:?}, {:?}",
                    folder.file_name().unwrap_or(""),
                    scan.candidates.len(),
                    scan.counts,
                    started.elapsed()
                );
                for c in scan.candidates.iter().take(3) {
                    println!(
                        "    {} {:?} ({}) asks: {}",
                        c.def_name,
                        c.status,
                        c.reason,
                        c.asks
                            .iter()
                            .map(|a| a.field.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
            }
            Err(e) => println!("{folder}: scan failed: {e}"),
        }
    }
}
