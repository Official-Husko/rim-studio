//! `lint ce PATH...`: checks the Combat Extended conversions of mod folders.
//!
//! The lint rules run inside the patch generator, so this command scans each folder and re-plans the
//! conversions that already exist (update mode), which lints the files RimStudio owns. It reports every
//! diagnostic and writes nothing.

use std::fmt::Write as _;

use serde_json::{Value, json};

use crate::cli::{KindArg, LintCmd};
use crate::cmd::convert::scan_project;
use crate::cmd::drafts::project_id;
use crate::cmd::project;
use crate::draftops::new_draft;
use crate::error::{CliError, CliResult};
use crate::fmt::{arr_at, count_diagnostics, render_diagnostics, str_at};
use crate::session::Session;

/// Runs a `lint` command.
///
/// # Errors
/// The envelope of a failing command; a reported failure (exit 1) when an error diagnostic was found.
pub(crate) fn run(s: &Session, cmd: &LintCmd) -> CliResult {
    let LintCmd::Ce { paths } = cmd;
    let filler = new_draft(KindArg::Ranged, "RS_Unused", None).map_err(CliError::failed)?;
    let mut results: Vec<Value> = Vec::new();
    let mut text = String::new();
    let mut errors = 0;
    let mut warnings = 0;
    for path in paths {
        let summary = project::open(s, path)?;
        let scan = scan_project(s, &summary, true)?;
        let mut diagnostics: Vec<Value> = arr_at(&scan, "/diagnostics").to_vec();
        for c in arr_at(&scan, "/candidates") {
            if str_at(c, "/status") != "already-ce" {
                continue;
            }
            let request = json!({
                "projectId": project_id(&summary),
                "draft": filler,
                "convert": {"defName": str_at(c, "/defName"), "answers": {}},
            });
            let plan = s.call("designer_export_plan", request)?.value;
            diagnostics.extend(arr_at(&plan, "/diagnostics").iter().cloned());
        }
        let counts = count_diagnostics(&diagnostics);
        errors += counts.errors;
        warnings += counts.warnings;
        let _ = writeln!(
            text,
            "{}: {} errors, {} warnings, {} diagnostics",
            str_at(&summary, "/path"),
            counts.errors,
            counts.warnings,
            diagnostics.len()
        );
        text.push_str(&render_diagnostics(&diagnostics));
        results.push(json!({"path": str_at(&summary, "/path"), "diagnostics": diagnostics}));
    }
    s.emit(
        &json!({"results": results, "errors": errors, "warnings": warnings}),
        || text.clone(),
    );
    if errors > 0 {
        return Err(CliError::Reported);
    }
    if warnings > 0 {
        s.note_warning();
    }
    Ok(())
}
