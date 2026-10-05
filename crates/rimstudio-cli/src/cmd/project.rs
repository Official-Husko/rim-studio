//! `project create|open`: mod folders as projects.

use std::fmt::Write as _;

use serde_json::{Value, json};

use super::project_fix as fix;
use super::project_layout as layout;
use super::project_link as link;
use crate::cli::{FixCmd, LinkCmd, ProjectCmd};
use crate::error::CliResult;
use crate::fmt::{arr_at, bool_at, count_diagnostics, render_diagnostics, str_at, u64_at};
use crate::session::{Session, absolute};

/// Opens (registers) a mod folder as a project and returns the summary.
///
/// # Errors
/// The envelope of `project_open` (`io.not-found` when the folder is not a mod).
pub(crate) fn open(s: &Session, path: &str) -> CliResult<Value> {
    let abs = absolute(path)?;
    Ok(s.call("project_open", json!({"path": abs}))?.value)
}

/// Renders a project summary.
#[must_use]
pub(crate) fn render_summary(v: &Value) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "project {} ({})",
        str_at(v, "/name"),
        str_at(v, "/projectId")
    );
    let _ = writeln!(out, "  folder: {}", str_at(v, "/path"));
    let _ = writeln!(
        out,
        "  package id: {}",
        if str_at(v, "/packageId").is_empty() {
            "-"
        } else {
            str_at(v, "/packageId")
        }
    );
    let versions: Vec<&str> = arr_at(v, "/supportedVersions")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let _ = writeln!(out, "  game versions: {}", versions.join(", "));
    let _ = writeln!(out, "  definition files: {}", u64_at(v, "/defFiles"));
    let _ = writeln!(
        out,
        "  LoadFolders.xml: {}{}",
        if bool_at(v, "/hasLoadFolders") {
            "present"
        } else {
            "absent"
        },
        if bool_at(v, "/hasCeGate") {
            ", already gates a Combat Extended folder"
        } else {
            ""
        }
    );
    out.push_str(&render_diagnostics(arr_at(v, "/diagnostics")));
    out
}

/// Runs a `project` command.
///
/// # Errors
/// The envelope of a failing command.
pub(crate) fn run(s: &Session, cmd: &ProjectCmd) -> CliResult {
    match cmd {
        ProjectCmd::Create {
            path,
            name,
            package_id,
            author,
            game_version,
            versioned,
            ce_folder,
            languages,
            assemblies,
            source,
            gitignore,
            ignore_source_art,
            readme,
            credits,
            no_textures,
            no_sounds,
        } => {
            let abs = absolute(path)?;
            let mut request = json!({
                "path": abs,
                "name": name,
                "packageId": package_id,
                "author": author,
                "versionedFolders": versioned,
                "cePatchFolder": ce_folder,
                "languagesFolder": languages,
                "assembliesFolder": assemblies,
                "sourceFolder": source,
                "gitignore": gitignore,
                "ignoreSourceArt": ignore_source_art,
                "readme": readme,
                "credits": credits,
                "texturesFolder": !no_textures,
                "soundsFolder": !no_sounds,
            });
            if !game_version.is_empty() {
                request["supportedVersions"] = json!(game_version);
            }
            let reply = s.call("project_create", request)?;
            s.emit(&reply.value, || {
                format!(
                    "created the mod folder {abs}\n{}",
                    render_summary(&reply.value)
                )
            });
        }
        ProjectCmd::Tree { path, depth, files } => layout::tree(s, path, *depth, *files)?,
        ProjectCmd::Check { path } => layout::check(s, path)?,
        ProjectCmd::ScaffoldMissing { path, dry_run } => {
            layout::scaffold_missing(s, path, *dry_run)?
        }
        ProjectCmd::Read {
            path,
            file,
            max_bytes,
        } => layout::read(s, path, file, *max_bytes)?,
        ProjectCmd::Fix(cmd) => match cmd {
            FixCmd::Plan { path, only } => fix::plan(s, path, only)?,
            FixCmd::Apply {
                path,
                items,
                all,
                rename_on_conflict,
                plan_id,
                yes,
            } => fix::apply(
                s,
                &fix::ApplyArgs {
                    path,
                    items,
                    all: *all,
                    rename_on_conflict: *rename_on_conflict,
                    plan_id: plan_id.as_deref(),
                    yes: *yes,
                },
            )?,
            FixCmd::Undo { path, apply_id } => fix::undo(s, path, apply_id)?,
            FixCmd::History { path } => fix::history(s, path)?,
        },
        ProjectCmd::Link(cmd) => match cmd {
            LinkCmd::Status { path } => link::status(s, path)?,
            LinkCmd::Create {
                path,
                mode,
                confirm_game_running,
                yes,
            } => link::create(s, path, *mode, *confirm_game_running, *yes)?,
            LinkCmd::Remove { path } => link::remove(s, path)?,
        },
        ProjectCmd::Open { path } => {
            let summary = open(s, path)?;
            s.emit(&summary, || render_summary(&summary));
            if count_diagnostics(arr_at(&summary, "/diagnostics")).warnings > 0 {
                s.note_warning();
            }
        }
    }
    Ok(())
}
