//! Golden JSON of the most important DTOs.
//!
//! The files under `tests/golden` are the contract the webview types are generated against. A change to a
//! shape shows up here as a diff. Run with `RIMSTUDIO_UPDATE_GOLDEN=1` to rewrite the files, then review
//! the diff.

mod common;

use std::path::PathBuf;

use serde::Serialize;

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(format!("{name}.json"))
}

fn check<T: Serialize>(name: &str, value: &T) {
    let mut text = serde_json::to_string_pretty(value).unwrap_or_default();
    text.push('\n');
    let path = golden_path(name);
    if std::env::var_os("RIMSTUDIO_UPDATE_GOLDEN").is_some() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&path, &text) {
            panic!("cannot write {}: {e}", path.display());
        }
        return;
    }
    let expected = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => panic!(
            "missing golden file {} ({e}); run with RIMSTUDIO_UPDATE_GOLDEN=1",
            path.display()
        ),
    };
    assert_eq!(
        text.replace("\r\n", "\n"),
        expected.replace("\r\n", "\n"),
        "golden mismatch for {name}"
    );
}

#[test]
fn golden_api_error() {
    check("api-error", &common::api_error());
}

#[test]
fn golden_design_spec_ranged_without_ce() {
    check("design-spec-ranged", &common::ranged_spec());
}

#[test]
fn golden_design_spec_with_carried_fields() {
    check("design-spec-carried", &common::carried_spec());
}

#[test]
fn golden_design_spec_melee_with_ce() {
    check("design-spec-melee-ce", &common::melee_spec_with_ce());
}

#[test]
fn golden_preview() {
    check("preview", &common::preview());
}

#[test]
fn golden_write_plan() {
    check("write-plan", &common::write_plan());
}

#[test]
fn golden_write_plan_with_copied_assets() {
    check("write-plan-assets", &common::asset_plan());
}

#[test]
fn golden_design_spec_with_asset_imports() {
    check("design-spec-assets", &common::asset_spec());
}

#[test]
fn golden_asset_info() {
    check("asset-info", &common::asset_info());
}

#[test]
fn golden_lint_files() {
    check("lint-files", &common::lint_files_result());
}

#[test]
fn golden_convert_scan() {
    check("convert-scan", &common::convert_scan());
}

#[test]
fn golden_ce_suggestion() {
    check("ce-suggestion", &common::ce_suggestion());
}

#[test]
fn golden_clone_diff() {
    check("clone-diff", &common::clone_diff());
}

#[test]
fn golden_fit_report() {
    check("fit-report", &common::fit_report());
}

#[test]
fn golden_job_result_envelope() {
    use rimstudio_ipc_types::jobs::JobResultEnvelope;
    let envelope = JobResultEnvelope::Finished {
        job_id: "j1".into(),
        result: common::scan_result(),
        elapsed_ms: 40,
        diagnostics: vec![],
    };
    check("job-finished-scan", &envelope);
}

#[test]
fn golden_project_tree() {
    check("project-tree", &common::project_tree());
}

#[test]
fn golden_project_layout_check() {
    check("project-layout-check", &common::layout_check());
}

#[test]
fn golden_project_file() {
    check("project-file", &common::project_file());
}
