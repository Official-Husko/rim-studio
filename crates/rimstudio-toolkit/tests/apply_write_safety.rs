//! Crash, race and failure behaviour of applying a plan: a file is only ever the old or the new version,
//! a file that changed after planning is never overwritten, a failure in the middle is reported with what
//! was written, and the gate file is never written before its patch.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;
mod common_project;

use std::sync::Mutex;

use common_project::{Proj, fixture, project, ranged, ranged_ce, request};
use rimstudio_core::jobs::{CancelToken, NoopProgress, Progress, ProgressSink};
use rimstudio_ipc_types::designer::{DesignerApplyPlanRequest, DesignerExportPlanRequest};
use rimstudio_toolkit::designer::plan::{export_plan, prepare};
use rimstudio_toolkit::designer::{ApplyOptions, apply_built, apply_plan};
use rimstudio_toolkit::error::ToolkitError;
use rstest::rstest;

fn apply_request(req: &DesignerExportPlanRequest, plan_id: String) -> DesignerApplyPlanRequest {
    DesignerApplyPlanRequest {
        plan_id,
        request: req.clone(),
        backup: true,
        dry_apply: true,
    }
}

fn read(p: &Proj, rel: &str) -> Option<String> {
    std::fs::read_to_string(p.root.join(rel).as_std_path()).ok()
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

fn no_temp_files(p: &Proj) {
    fn walk(dir: &std::path::Path, found: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.contains(".rstmp-") || name.ends_with(".bak") || name == "backups" {
                found.push(name);
            }
            if e.file_type().unwrap().is_dir() {
                walk(&e.path(), found);
            }
        }
    }
    let mut found = Vec::new();
    walk(p.root.as_std_path(), &mut found);
    assert!(found.is_empty(), "stray files in the mod folder: {found:?}");
}

#[rstest]
#[case(0)]
#[case(1)]
#[case(2)]
#[case(3)]
fn a_stop_after_any_file_leaves_whole_files_and_the_gate_only_after_its_patch(#[case] after: u64) {
    let f = fixture(true);
    let p = project(&f, &[]);
    let req = request(&p, &ranged_ce());
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    let token = CancelToken::new();
    let sink = CancelAfter {
        token: token.clone(),
        after,
        seen: Mutex::new(Vec::new()),
    };
    let report = apply_plan(
        &f.ctx,
        apply_request(&req, plan.plan_id.clone()),
        &sink,
        &token,
    );
    let written: Vec<String> = match &report {
        Ok(r) => r.written.iter().map(|w| w.path.clone()).collect(),
        Err(e) => panic!("{e:?}"),
    };
    for file in &plan.files {
        let on_disk = read(&p, &file.path);
        let was_written = written.contains(&file.path);
        assert_eq!(on_disk.is_some(), was_written, "{} {written:?}", file.path);
        if let Some(text) = on_disk {
            assert_eq!(text, file.rendered, "{} is whole", file.path);
        }
    }
    let gate = read(&p, "LoadFolders.xml").is_some();
    let patch_paths: Vec<&str> = plan
        .files
        .iter()
        .filter(|x| x.path.contains("Compat/CombatExtended/"))
        .map(|x| x.path.as_str())
        .collect();
    if gate {
        assert!(
            patch_paths.iter().all(|path| read(&p, path).is_some()),
            "the gate exists, so every patch it enables exists"
        );
    }
    no_temp_files(&p);
    // a rerun finishes the job
    let plan2 = export_plan(&f.ctx, req.clone()).unwrap();
    apply_plan(
        &f.ctx,
        apply_request(&req, plan2.plan_id),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    for file in &plan.files {
        assert_eq!(
            read(&p, &file.path).as_deref(),
            Some(file.rendered.as_str())
        );
    }
    no_temp_files(&p);
}

#[test]
fn a_stop_in_the_middle_of_an_update_leaves_each_file_old_or_new() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let req = request(&p, &ranged_ce());
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    apply_plan(
        &f.ctx,
        apply_request(&req, plan.plan_id),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    let old: Vec<(String, String)> = plan
        .files
        .iter()
        .map(|x| (x.path.clone(), read(&p, &x.path).unwrap()))
        .collect();
    let mut changed = ranged_ce();
    changed.identity.label = "renamed rifle".into();
    let req2 = request(&p, &changed);
    let plan2 = export_plan(&f.ctx, req2.clone()).unwrap();
    let token = CancelToken::new();
    let sink = CancelAfter {
        token: token.clone(),
        after: 0,
        seen: Mutex::new(Vec::new()),
    };
    apply_plan(
        &f.ctx,
        apply_request(&req2, plan2.plan_id.clone()),
        &sink,
        &token,
    )
    .unwrap();
    for file in &plan2.files {
        let now = read(&p, &file.path).unwrap();
        let before = &old.iter().find(|(path, _)| *path == file.path).unwrap().1;
        assert!(
            now == *before || now == file.rendered,
            "{} is neither the old nor the new version",
            file.path
        );
    }
    no_temp_files(&p);
}

#[test]
fn a_file_that_changed_on_disk_after_planning_is_not_overwritten() {
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
    let mut changed = ranged();
    changed.identity.label = "renamed rifle".into();
    let prepared = prepare(&f.ctx, &request(&p, &changed)).unwrap();
    // the person edits the file in another program between planning and applying
    let edited = format!(
        "{}<!-- hand edit -->\n",
        read(
            &p,
            "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_NewRifle.xml"
        )
        .unwrap()
    );
    common_project::put(
        &p.root,
        "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_NewRifle.xml",
        &edited,
    );
    let err = apply_built(
        &f.ctx,
        &prepared.writer,
        &prepared.plan,
        ApplyOptions::default(),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.apply-failed", "{err:?}");
    assert!(err.to_string().contains("changed"), "{err}");
    assert_eq!(
        read(
            &p,
            "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_NewRifle.xml"
        )
        .as_deref(),
        Some(edited.as_str())
    );
}

#[test]
fn a_file_that_appeared_after_planning_is_not_overwritten_by_a_create() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let prepared = prepare(&f.ctx, &request(&p, &ranged())).unwrap();
    common_project::put(
        &p.root,
        "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_NewRifle.xml",
        "<Defs/>\n",
    );
    let err = apply_built(
        &f.ctx,
        &prepared.writer,
        &prepared.plan,
        ApplyOptions::default(),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "designer.apply-failed", "{err:?}");
    assert_eq!(
        read(
            &p,
            "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_NewRifle.xml"
        )
        .as_deref(),
        Some("<Defs/>\n")
    );
}

#[test]
fn two_plan_paths_that_differ_only_by_case_refuse_the_whole_plan() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let mut prepared = prepare(&f.ctx, &request(&p, &ranged_ce())).unwrap();
    let mut twin = prepared.plan.files[0].clone();
    twin.path = twin.path.to_uppercase();
    prepared.plan.files.push(twin);
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
    assert!(err.to_string().contains("case"), "{err}");
    assert!(!p.root.join("Defs").exists(), "nothing was written");
}

#[test]
fn a_plan_path_that_is_a_folder_of_another_plan_path_refuses_the_whole_plan() {
    let f = fixture(true);
    let p = project(&f, &[]);
    let mut prepared = prepare(&f.ctx, &request(&p, &ranged())).unwrap();
    let mut clash = prepared.plan.files[0].clone();
    clash.path = format!("{}/inner.xml", prepared.plan.files[0].path);
    prepared.plan.files.push(clash);
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
    assert!(!p.root.join("Defs").exists(), "nothing was written");
}

#[cfg(unix)]
#[test]
fn a_write_error_in_the_middle_is_reported_with_the_files_already_written() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture(true);
    let p = project(&f, &[]);
    let prepared = prepare(&f.ctx, &request(&p, &ranged_ce())).unwrap();
    let patch = prepared
        .plan
        .files
        .iter()
        .find(|x| x.path.contains("Compat/CombatExtended/"))
        .unwrap();
    let dir = p.root.join(patch.path.rsplit_once('/').unwrap().0);
    std::fs::create_dir_all(dir.as_std_path()).unwrap();
    std::fs::set_permissions(dir.as_std_path(), std::fs::Permissions::from_mode(0o555)).unwrap();
    // a process that ignores permission bits (root) cannot run this test
    let probe = dir.join("probe");
    if std::fs::write(probe.as_std_path(), "x").is_ok() {
        std::fs::remove_file(probe.as_std_path()).unwrap();
        std::fs::set_permissions(dir.as_std_path(), std::fs::Permissions::from_mode(0o755))
            .unwrap();
        return;
    }
    let report = apply_built(
        &f.ctx,
        &prepared.writer,
        &prepared.plan,
        ApplyOptions::default(),
        &NoopProgress,
        &CancelToken::new(),
    );
    std::fs::set_permissions(dir.as_std_path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    let report = report.expect("a partial result is a report, not an error");
    assert!(!report.written.is_empty(), "the definitions came first");
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "designer.apply-write-failed"),
        "{:?}",
        report.diagnostics
    );
    assert_eq!(report.dry_apply_ok, None, "no dry apply after a failure");
    assert!(
        read(&p, "LoadFolders.xml").is_none(),
        "the gate is not written when its patch failed"
    );
    for w in &report.written {
        let planned = prepared.plan.file(&w.path).unwrap();
        assert_eq!(read(&p, &w.path).as_deref(), Some(planned.text.as_str()));
    }
    no_temp_files(&p);
}

#[cfg(unix)]
#[test]
fn a_write_error_on_the_first_file_is_an_error_and_writes_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture(true);
    let p = project(&f, &[]);
    let prepared = prepare(&f.ctx, &request(&p, &ranged())).unwrap();
    let first = &prepared.plan.files[0];
    let dir = p.root.join(first.path.rsplit_once('/').unwrap().0);
    std::fs::create_dir_all(dir.as_std_path()).unwrap();
    std::fs::set_permissions(dir.as_std_path(), std::fs::Permissions::from_mode(0o555)).unwrap();
    let probe = dir.join("probe");
    if std::fs::write(probe.as_std_path(), "x").is_ok() {
        std::fs::remove_file(probe.as_std_path()).unwrap();
        std::fs::set_permissions(dir.as_std_path(), std::fs::Permissions::from_mode(0o755))
            .unwrap();
        return;
    }
    let r = apply_built(
        &f.ctx,
        &prepared.writer,
        &prepared.plan,
        ApplyOptions::default(),
        &NoopProgress,
        &CancelToken::new(),
    );
    std::fs::set_permissions(dir.as_std_path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(r.is_err(), "{r:?}");
    assert!(read(&p, &first.path).is_none());
    no_temp_files(&p);
}

#[cfg(unix)]
#[test]
fn a_read_only_file_in_the_plan_refuses_before_the_first_write() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture(true);
    let p = project(&f, &[]);
    let req = request(&p, &ranged_ce());
    let plan = export_plan(&f.ctx, req.clone()).unwrap();
    apply_plan(
        &f.ctx,
        apply_request(&req, plan.plan_id),
        &NoopProgress,
        &CancelToken::new(),
    )
    .unwrap();
    let mut changed = ranged_ce();
    changed.identity.label = "renamed rifle".into();
    let prepared = prepare(&f.ctx, &request(&p, &changed)).unwrap();
    let locked = p.root.join("LoadFolders.xml");
    let before = std::fs::read_to_string(locked.as_std_path()).unwrap();
    // lock the file the plan reaches last; nothing may be written, not even the definition before it
    let changed_last = prepared
        .plan
        .files
        .iter()
        .rfind(|x| x.action != rimstudio_design::plan::FileAction::Unchanged)
        .unwrap()
        .path
        .clone();
    let victim = p.root.join(&changed_last);
    let victim_before = std::fs::read_to_string(victim.as_std_path()).unwrap();
    std::fs::set_permissions(victim.as_std_path(), std::fs::Permissions::from_mode(0o444)).unwrap();
    let defs_before = read(
        &p,
        "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_NewRifle.xml",
    )
    .unwrap();
    let r = apply_built(
        &f.ctx,
        &prepared.writer,
        &prepared.plan,
        ApplyOptions::default(),
        &NoopProgress,
        &CancelToken::new(),
    );
    std::fs::set_permissions(victim.as_std_path(), std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(r.is_err(), "{r:?}");
    assert_eq!(
        std::fs::read_to_string(victim.as_std_path()).unwrap(),
        victim_before
    );
    assert_eq!(
        read(
            &p,
            "Defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_NewRifle.xml"
        )
        .unwrap(),
        defs_before
    );
    assert_eq!(
        std::fs::read_to_string(locked.as_std_path()).unwrap(),
        before
    );
}

#[test]
fn backups_of_an_apply_never_land_in_the_mod_folder() {
    let f = fixture(true);
    let p = project(&f, &[]);
    for label in ["one", "two", "three"] {
        let mut spec = ranged_ce();
        spec.identity.label = label.into();
        let req = request(&p, &spec);
        let plan = export_plan(&f.ctx, req.clone()).unwrap();
        let report = apply_plan(
            &f.ctx,
            apply_request(&req, plan.plan_id),
            &NoopProgress,
            &CancelToken::new(),
        )
        .unwrap();
        for w in &report.written {
            if let Some(b) = &w.backup_path {
                assert!(!b.starts_with(p.root.as_str()), "{b}");
            }
        }
    }
    no_temp_files(&p);
}
