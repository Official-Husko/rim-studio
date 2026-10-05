//! Applying a plan: guarded paths, backups, read back, cancellation between files, the dry apply.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;
mod common_project;

use std::sync::Mutex;

use common_project::{Proj, fixture, project, ranged, ranged_ce, request};
use rimstudio_core::jobs::{CancelToken, NoopProgress, Progress, ProgressSink};
use rimstudio_ipc_types::designer::{
    DesignerApplyPlanRequest, DesignerExportPlanRequest, FileActionDto,
};
use rimstudio_toolkit::designer::plan::{export_plan, prepare};
use rimstudio_toolkit::designer::{ApplyOptions, apply_built, apply_plan};
use rimstudio_toolkit::error::ToolkitError;
use rimstudio_toolkit::shared::env::ProjectEnv;

fn apply_request(req: &DesignerExportPlanRequest, plan_id: String) -> DesignerApplyPlanRequest {
    DesignerApplyPlanRequest {
        plan_id,
        request: req.clone(),
        backup: true,
        dry_apply: true,
    }
}

fn read(p: &Proj, rel: &str) -> String {
    std::fs::read_to_string(p.root.join(rel).as_std_path()).unwrap()
}

#[test]
fn apply_writes_the_planned_text_and_reports_every_file_verified() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let req = request(&p, &ranged_ce());
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    let report = apply_plan(
        &f.ctx,
        apply_request(&req, plan.plan_id.clone()),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(report.plan_id, plan.plan_id);
    assert_eq!(report.written.len(), plan.files.len());
    assert!(report.written.iter().all(|w| w.verified));
    for file in &plan.files {
        assert_eq!(read(&p, &file.path), file.rendered, "{}", file.path);
    }
    // vanilla definitions come first, LoadFolders.xml is written last
    let order: Vec<&str> = report.written.iter().map(|w| w.path.as_str()).collect();
    assert_eq!(order.last(), Some(&"LoadFolders.xml"));
    assert_eq!(report.dry_apply_ok, Some(true), "{:?}", report.diagnostics);
}

#[test]
fn a_replaced_file_is_backed_up_and_the_new_text_is_read_back() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let req = request(&p, &ranged());
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    apply_plan(
        &f.ctx,
        apply_request(&req, plan.plan_id),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    let old = read(&p, "Defs/Weapons/RS_NewRifle.xml");
    let mut changed = ranged();
    changed.identity.label = "renamed rifle".into();
    let req2 = request(&p, &changed);
    let plan2 = export_plan(&f.ctx, req2.clone()).unwrap();
    let file = plan2
        .files
        .iter()
        .find(|x| x.path == "Defs/Weapons/RS_NewRifle.xml")
        .unwrap();
    assert_eq!(file.action, FileActionDto::UpdateRegion);
    assert!(
        file.diff
            .as_deref()
            .unwrap()
            .contains("+    <label>renamed rifle</label>")
    );
    let report = apply_plan(
        &f.ctx,
        apply_request(&req2, plan2.plan_id),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    let applied = report
        .written
        .iter()
        .find(|w| w.path == "Defs/Weapons/RS_NewRifle.xml")
        .unwrap();
    let backup = applied.backup_path.as_deref().expect("a backup path");
    assert_eq!(std::fs::read_to_string(backup).unwrap(), old);
    assert!(
        !backup.starts_with(p.root.as_str()),
        "backups stay out of the mod folder"
    );
    assert!(read(&p, "Defs/Weapons/RS_NewRifle.xml").contains("renamed rifle"));
    assert!(applied.verified);
}

#[test]
fn a_plan_that_changed_on_disk_since_review_is_stale_and_nothing_is_written() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let req = request(&p, &ranged());
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    common_project::put(&p.root, "Defs/Weapons/RS_NewRifle.xml", "<Defs>\n</Defs>\n");
    let err = apply_plan(
        &f.ctx,
        apply_request(&req, plan.plan_id),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.plan-stale");
    assert!(!p.root.join("Defs/Projectiles").exists());
}

#[test]
fn a_plan_with_an_error_is_refused() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let mut spec = ranged();
    spec.identity.def_name = "RS_Gun00".into();
    let req = request(&p, &spec);
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    assert!(plan.has_errors);
    let err = apply_plan(
        &f.ctx,
        apply_request(&req, plan.plan_id),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.apply-failed");
}

#[test]
fn a_path_outside_the_project_root_refuses_the_whole_plan_before_any_write() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let mut prepared = prepare(&f.ctx, &request(&p, &ranged())).unwrap();
    let last = prepared.plan.files.len() - 1;
    prepared.plan.files[last].path = "../evil.xml".into();
    let err = apply_built(
        &f.ctx,
        &prepared.writer,
        &prepared.plan,
        ApplyOptions::default(),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap_err();
    assert!(matches!(err, ToolkitError::PathRefused { .. }), "{err:?}");
    assert!(
        !p.root.join("Defs").exists(),
        "nothing was written (IT-004)"
    );
    assert!(!p.root.parent().unwrap().join("evil.xml").exists());
}

#[test]
fn a_protected_folder_inside_the_project_refuses_the_write() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let env = ProjectEnv::new(&f.roots, std::sync::Arc::new(f.clock.clone()))
        .unwrap()
        .with_protected([p.root.join("Defs/Weapons")]);
    let ctx = rimstudio_toolkit::designer::Ctx::new(
        f.session.clone(),
        &f.roots,
        std::sync::Arc::new(f.clock.clone()),
    )
    .unwrap()
    .with_env(env);
    let req = request(&p, &ranged());
    let plan = export_plan(&ctx, req.clone()).unwrap();
    assert!(
        plan.has_errors
            && plan
                .diagnostics
                .iter()
                .any(|d| d.code == "designer.path-refused"),
        "{:?}",
        plan.diagnostics
    );
    let err = apply_plan(
        &ctx,
        apply_request(&req, plan.plan_id),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.apply-failed");
    assert!(!p.root.join("Defs").exists());
}

#[test]
fn a_project_inside_the_game_folder_cannot_be_written() {
    let f = fixture(true);
    // a mod folder below the fictional install is inside the protected install
    let inside = f.install.game_dir.join("Mods").join("RS_Inside");
    std::fs::create_dir_all(inside.join("About").as_std_path()).unwrap();
    std::fs::write(
        inside.join("About/About.xml").as_std_path(),
        common_project::about("rs.inside", &["1.6"]),
    )
    .unwrap();
    let record = f.ctx.env().projects().open(&inside).unwrap();
    let mut req = request(
        &Proj {
            tmp: tempfile::tempdir().unwrap(),
            root: inside.clone(),
            id: record.id.as_str().to_owned(),
        },
        &ranged(),
    );
    req.project_id = record.id.as_str().to_owned();
    let err = export_plan(&f.ctx, req).unwrap_err();
    assert_eq!(err.code(), "project.path-outside-root", "{err:?}");
    assert!(!inside.join("Defs").exists());
}

struct CancelAfter {
    token: CancelToken,
    after: u64,
    seen: Mutex<Vec<String>>,
}

impl ProgressSink for CancelAfter {
    fn report(&self, progress: Progress) {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(format!("{}:{}", progress.phase, progress.done));
        }
        if progress.phase == "designer.apply" && progress.done == self.after {
            self.token.cancel();
        }
    }
}

#[test]
fn cancelling_between_files_rolls_forward_and_a_rerun_completes() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let req = request(&p, &ranged_ce());
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    assert_eq!(
        plan.files.len(),
        4,
        "{:?}",
        plan.files.iter().map(|x| &x.path).collect::<Vec<_>>()
    );
    let token = CancelToken::new();
    let sink = CancelAfter {
        token: token.clone(),
        after: 1,
        seen: Mutex::new(Vec::new()),
    };
    let report = apply_plan(&f.ctx, apply_request(&req, plan.plan_id), &sink, &token).unwrap();
    assert_eq!(
        report.written.len(),
        2,
        "two files were written before the stop"
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "designer.apply-cancelled")
    );
    assert_eq!(report.dry_apply_ok, None);
    // the gate is only added after the patch exists: LoadFolders.xml is not there yet
    assert!(!p.root.join("LoadFolders.xml").exists());
    for w in &report.written {
        assert!(p.root.join(&w.path).exists());
    }
    // the written files are whole files, equal to the plan
    for w in &report.written {
        let planned = plan.files.iter().find(|x| x.path == w.path).unwrap();
        assert_eq!(read(&p, &w.path), planned.rendered);
    }
    // a second run only writes what is missing
    let plan2 = export_plan(&f.ctx, req.clone()).unwrap();
    let unchanged = plan2
        .files
        .iter()
        .filter(|x| x.action == FileActionDto::Unchanged)
        .count();
    assert_eq!(unchanged, 2);
    let report2 = apply_plan(
        &f.ctx,
        apply_request(&req, plan2.plan_id),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(report2.written.len(), 2);
    assert_eq!(report2.unchanged.len(), 2);
    assert!(p.root.join("LoadFolders.xml").exists());
    assert_eq!(
        report2.dry_apply_ok,
        Some(true),
        "{:?}",
        report2.diagnostics
    );
}

#[test]
fn a_job_cancelled_before_the_first_file_is_cancelled_and_writes_nothing() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let req = request(&p, &ranged());
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    let token = CancelToken::new();
    token.cancel();
    let err = apply_plan(
        &f.ctx,
        apply_request(&req, plan.plan_id),
        &NoopProgress,
        &token,
    )
    .unwrap_err();
    assert_eq!(err.code(), "job.cancelled");
    assert!(!p.root.join("Defs").exists());
}

#[test]
fn the_dry_apply_reports_an_operation_that_would_fail() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let mut prepared = prepare(&f.ctx, &request(&p, &ranged_ce())).unwrap();
    // replace the generated patch by one whose operation matches nothing
    let broken = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<Patch>\n  <Operation Class=\"PatchOperationReplace\">\n    <xpath>Defs/ThingDef[defName=\"RS_NewRifle\"]/statBases/RS_Missing</xpath>\n    <value><RS_Missing>1</RS_Missing></value>\n  </Operation>\n</Patch>\n";
    for file in &mut prepared.plan.files {
        if file.path.contains("Weapons_Ranged") {
            file.text = broken.to_owned();
        }
    }
    let report = apply_built(
        &f.ctx,
        &prepared.writer,
        &prepared.plan,
        ApplyOptions::default(),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(report.dry_apply_ok, Some(false));
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "ce.dry-run-failed"),
        "{:?}",
        report.diagnostics
    );
}
