//! Flow C on a real install: clone a vanilla rifle and a vanilla melee weapon into a temporary project,
//! plan, apply into that project and compare the written definition with the source's key numbers.
//!
//! All tests are `#[ignore]`: they only read the install and write only into temporary folders. The output
//! is for the person running them and must not be committed.
//!
//! ```text
//! RIMSTUDIO_GAME_DIR=~/.steam/steam/steamapps/common/RimWorld \
//! cargo test -p rimstudio-toolkit --release --test real_clone -- --ignored --nocapture
//! ```
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::sync::Arc;

use camino::Utf8PathBuf;
use rimstudio_core::ids::SourceId;
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_core::mods::{ModIndex, ModSource, SourceKind, SourceSet};
use rimstudio_core::tree::Node;
use rimstudio_core::version::GameVersion;
use rimstudio_design::classes::ItemKind as PoolKind;
use rimstudio_io::roots::DataRoots;
use rimstudio_ipc_types::designer::{
    DesignerApplyPlanRequest, DesignerCloneDiffRequest, DesignerCloneRequest,
    DesignerExportPlanRequest,
};
use rimstudio_library::scan::{ScanOptions, Scanner};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_toolkit::designer::{Ctx, apply_plan, clone_diff, clone_draft, export_plan};
use rimstudio_workspace::refset::{DesignerReferenceOptions, ReferenceSet};
use rimstudio_workspace::session::{OpenInput, WorkspaceSession};
use rimstudio_xml::modes::ParseMode;
use rimstudio_xml::reader::parse_top_level;

fn dir_from_env(name: &str) -> Option<Utf8PathBuf> {
    let path = Utf8PathBuf::from(std::env::var(name).ok()?);
    path.is_dir().then_some(path)
}

/// The default reference session of the designer: the game and its expansions only.
fn vanilla_session(game_dir: &Utf8PathBuf) -> Arc<WorkspaceSession> {
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
    let options = DesignerReferenceOptions::new(game);
    let mut builder = ModIndex::builder();
    for (_, meta) in outcome.index.iter() {
        builder.push(meta.clone());
    }
    let reference = ReferenceSet::reference_for_designer(&builder.build(), game_dir, &options);
    Arc::new(
        WorkspaceSession::open_simple(OpenInput::new(reference).with_game_dir(game_dir.clone()))
            .unwrap(),
    )
}

fn number(node: Option<&Node>, tag: &str) -> Option<f64> {
    node?.child_text(tag)?.trim().parse().ok()
}

fn close(a: Option<f64>, b: Option<f64>, what: &str) {
    match (a, b) {
        (Some(x), Some(y)) => assert!((x - y).abs() < 1e-6, "{what}: written {x}, source {y}"),
        (None, None) => {}
        other => panic!("{what}: written and source disagree about presence: {other:?}"),
    }
}

struct World {
    _tmp: tempfile::TempDir,
    ctx: Ctx,
    root: Utf8PathBuf,
    project_id: String,
}

fn world(game_dir: &Utf8PathBuf) -> World {
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let roots = DataRoots::under_base(&base.join("app"));
    roots.ensure_all().unwrap();
    let ctx = Ctx::new(
        vanilla_session(game_dir),
        &roots,
        Arc::new(FakeClock::new(1_800_000_000_000)),
    )
    .unwrap();
    let root = base.join("RS_CloneProject");
    std::fs::create_dir_all(root.join("About").as_std_path()).unwrap();
    std::fs::write(
        root.join("About/About.xml").as_std_path(),
        "<ModMetaData><name>RS Clone</name><packageId>rs.clonetest</packageId><supportedVersions><li>1.6</li></supportedVersions></ModMetaData>",
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
    World {
        _tmp: tmp,
        ctx,
        root,
        project_id,
    }
}

/// Clones `source`, plans, applies into the temporary project and returns the written weapon node.
fn clone_plan_apply(w: &World, source: &str, name: &str) -> Node {
    let response = clone_draft(
        &w.ctx,
        DesignerCloneRequest {
            project_id: w.project_id.clone(),
            source: source.to_owned(),
            def_name: name.to_owned(),
            label: None,
            mod_prefix: None,
        },
    )
    .unwrap();
    println!("clone of {source}:");
    for n in &response.notes {
        println!("  note: {n}");
    }
    assert!(
        response.entry.draft.spec.ce.is_none(),
        "the clone is vanilla"
    );
    let diff = clone_diff(
        &w.ctx,
        DesignerCloneDiffRequest {
            draft: response.entry.draft.clone(),
        },
    )
    .unwrap();
    assert!(diff.changes.is_empty(), "{:?}", diff.changes);
    let request = DesignerExportPlanRequest {
        project_id: w.project_id.clone(),
        draft: response.entry.draft.clone(),
        convert: None,
        accept_suggestions: None,
    };
    let plan = export_plan(&w.ctx, request.clone()).unwrap();
    for d in &plan.diagnostics {
        println!(
            "  plan diagnostic {} {:?} {}",
            d.code, d.severity, d.message
        );
    }
    assert!(!plan.has_errors, "the clone of {source} must plan");
    assert!(
        plan.files
            .iter()
            .all(|f| !f.rendered.contains("CombatExtended."))
    );
    let report = apply_plan(
        &w.ctx,
        DesignerApplyPlanRequest {
            plan_id: plan.plan_id.clone(),
            request,
            backup: true,
            dry_apply: true,
        },
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    assert!(!report.written.is_empty());
    let path = w.root.join(format!("Defs/Weapons/{name}.xml"));
    let bytes = std::fs::read(path.as_std_path()).unwrap();
    println!("{}", String::from_utf8_lossy(&bytes));
    let parsed = parse_top_level(&bytes, ParseMode::Tolerant, "Defs").unwrap();
    parsed
        .nodes
        .into_iter()
        .find(|n| n.child_text("defName") == Some(name))
        .expect("the written file holds the clone")
}

#[test]
#[ignore = "reads the real install (RIMSTUDIO_GAME_DIR)"]
fn a_vanilla_rifle_clones_plans_and_writes_with_the_numbers_of_its_source() {
    let Some(game_dir) = dir_from_env("RIMSTUDIO_GAME_DIR") else {
        println!("RIMSTUDIO_GAME_DIR is not set, skipping");
        return;
    };
    let w = world(&game_dir);
    let engine = w.ctx.require_engine().unwrap();
    let pool = engine.pool(PoolKind::Ranged).unwrap();
    let source = pool
        .items
        .iter()
        .find(|i| i.role == "rifle")
        .or_else(|| pool.items.first())
        .unwrap()
        .id
        .clone();
    println!("source rifle: {source}");
    let written = clone_plan_apply(&w, &source, "RS_ClonedRifle");
    let record = engine.databases().get("ThingDef", &source).unwrap();

    // parent and projectile names
    assert_eq!(
        written.attr("ParentName"),
        record.parents.first().map(|p| p.name.as_str()),
        "parent"
    );
    let verb = |n: &Node| {
        n.child("verbs")
            .and_then(|v| v.children_named("li").next().cloned())
            .unwrap()
    };
    let (src_verb, out_verb) = (verb(&record.node), verb(&written));
    assert_eq!(
        out_verb.child_text("defaultProjectile"),
        src_verb.child_text("defaultProjectile"),
        "projectile"
    );
    close(
        number(Some(&out_verb), "warmupTime"),
        number(Some(&src_verb), "warmupTime"),
        "warmup",
    );
    close(
        number(Some(&out_verb), "range"),
        number(Some(&src_verb), "range"),
        "range",
    );
    // cooldown and mass from the stat bases
    let (src_stats, out_stats) = (record.node.child("statBases"), written.child("statBases"));
    for stat in ["RangedWeapon_Cooldown", "Mass", "WorkToMake"] {
        close(number(out_stats, stat), number(src_stats, stat), stat);
    }
    // damage lives in the projectile the clone points at: the spec's damage is the projectile's
    let projectile = src_verb.child_text("defaultProjectile").unwrap();
    let projectile_damage = engine
        .databases()
        .get("ThingDef", projectile)
        .and_then(|d| number(d.node.child("projectile"), "damageAmountBase"));
    let cloned = clone_draft(
        &w.ctx,
        DesignerCloneRequest {
            project_id: w.project_id.clone(),
            source: source.clone(),
            def_name: "RS_ClonedRifleAgain".to_owned(),
            label: None,
            mod_prefix: None,
        },
    )
    .unwrap();
    let damage = cloned
        .entry
        .draft
        .spec
        .ranged
        .as_ref()
        .and_then(|r| r.damage)
        .map(|d| d.value);
    close(damage, projectile_damage, "damage");
    // the written definition keeps the cost list of the source
    let cost = |n: &Node| {
        n.child("costList")
            .map(|c| {
                c.elements()
                    .map(|e| (e.tag.clone(), e.text_content().trim().to_owned()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    let (mut a, mut b) = (cost(&written), cost(&record.node));
    a.sort();
    b.sort();
    assert_eq!(a.len(), b.len(), "cost list size");
    for ((na, _), (nb, _)) in a.iter().zip(&b) {
        assert_eq!(na, nb, "cost list names");
    }
}

#[test]
#[ignore = "reads the real install (RIMSTUDIO_GAME_DIR)"]
fn a_vanilla_melee_weapon_clones_plans_and_writes_with_the_numbers_of_its_source() {
    let Some(game_dir) = dir_from_env("RIMSTUDIO_GAME_DIR") else {
        println!("RIMSTUDIO_GAME_DIR is not set, skipping");
        return;
    };
    let w = world(&game_dir);
    let engine = w.ctx.require_engine().unwrap();
    let pool = engine.pool(PoolKind::Melee).unwrap();
    // a weapon that can be crafted, so that the clone has the work and materials the designer requires
    let craftable = |id: &str| {
        engine.databases().get("ThingDef", id).is_some_and(|d| {
            number(d.node.child("statBases"), "WorkToMake").is_some()
                && (d.node.child("costList").is_some() || d.node.child("stuffCategories").is_some())
        })
    };
    let source = pool
        .items
        .iter()
        .find(|i| craftable(&i.id))
        .unwrap_or_else(|| pool.items.first().unwrap())
        .id
        .clone();
    println!("source melee weapon: {source}");
    let written = clone_plan_apply(&w, &source, "RS_ClonedBlade");
    let record = engine.databases().get("ThingDef", &source).unwrap();
    assert_eq!(
        written.attr("ParentName"),
        record.parents.first().map(|p| p.name.as_str()),
        "parent"
    );
    let (src_stats, out_stats) = (record.node.child("statBases"), written.child("statBases"));
    for stat in ["Mass", "WorkToMake"] {
        close(number(out_stats, stat), number(src_stats, stat), stat);
    }
    let tools = |n: &Node| {
        n.child("tools")
            .map(|t| t.children_named("li").cloned().collect::<Vec<_>>())
            .unwrap_or_default()
    };
    let (out_tools, src_tools) = (tools(&written), tools(&record.node));
    assert_eq!(out_tools.len(), src_tools.len(), "tool count");
    for (o, s) in out_tools.iter().zip(&src_tools) {
        assert_eq!(o.child_text("label"), s.child_text("label"), "tool label");
        close(
            number(Some(o), "power"),
            number(Some(s), "power"),
            "tool power",
        );
        close(
            number(Some(o), "cooldownTime"),
            number(Some(s), "cooldownTime"),
            "tool cooldown",
        );
    }
    close(
        number(Some(&written), "costStuffCount"),
        number(Some(&record.node), "costStuffCount"),
        "costStuffCount",
    );
}
