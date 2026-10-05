//! Applying a write plan (`designer_apply_plan`, a job).
//!
//! The job rebuilds the plan from the same request, refuses when its content hash differs from the one the
//! user reviewed ([`ToolkitError::PlanStale`]) or when it carries an error diagnostic, checks every path
//! with the guarded writer before the first byte is written (a path outside the project root, under the game
//! install or config folders, or through a link that leaves the project is refused for the whole plan), then
//! writes the files in order: definitions, patches, `LoadFolders.xml` last. A replaced file is backed up
//! first, written atomically and read back.
//!
//! Before the first byte every pending file is preflighted: its path, a read only flag, and that it still
//! holds exactly the text the plan was made from (an edit made by another program after planning is never
//! overwritten; a file that appeared since is never replaced by a create). Two plan paths that differ only by
//! letter case, or one that is the folder of another, refuse the whole plan. A write error after at least one
//! file was written does not throw the report away: the report lists what was written and carries an error
//! diagnostic, and the files after the failing one (the `LoadFolders.xml` gate comes last) are not written.
//!
//! Cancellation is observed between files. A file is either written whole or not at all, and the gated
//! folder is only listed in `LoadFolders.xml` after its patches exist, so a cancelled job leaves a project
//! that is consistent (it rolls forward: what was written stays, and the report names it).
//!
//! After writing, the generated Combat Extended patch is dry applied by the def engine on a scratch copy of
//! the definitions with the gun conversion operation simulated; every top level operation that would fail in
//! the game is reported (IT-056).

use rimstudio_core::diag::{DiagCode, Diagnostic, Severity};
use rimstudio_core::jobs::{CancelToken, Progress, ProgressSink, ProgressUnit};
use rimstudio_design::ce::patchgen::dry_apply;
use rimstudio_design::plan::{FileAction, FileKind};
use rimstudio_ipc_types::designer::{AppliedFileDto, ApplyReportDto, DesignerApplyPlanRequest};
use rimstudio_ipc_types::diagnostic::diagnostics_to_dtos;
use rimstudio_xml::modes::ParseMode;
use rimstudio_xml::reader::parse_document;

use super::ctx::Ctx;
use super::dto::count;
use super::plan::{BuiltPlan, action_dto, prepare, refusal};
use crate::error::{ToolkitError, ToolkitResult};
use crate::shared::writer::{Expect, GuardedWriter};

/// The diagnostic code of a job that stopped between files.
pub const APPLY_CANCELLED: DiagCode = DiagCode::new("designer.apply-cancelled");
/// The diagnostic code (error) of a write that failed after earlier files of the plan were written.
pub const APPLY_WRITE_FAILED: DiagCode = DiagCode::new("designer.apply-write-failed");

/// What the apply step does beyond writing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplyOptions {
    /// Back up files that are replaced.
    pub backup: bool,
    /// Dry apply the generated patch after writing.
    pub dry_apply: bool,
}

impl Default for ApplyOptions {
    fn default() -> Self {
        Self {
            backup: true,
            dry_apply: true,
        }
    }
}

/// The result of the dry apply of a plan's patch files.
#[derive(Debug, Clone, PartialEq)]
pub struct DryReport {
    /// True when every top level operation applied.
    pub ok: bool,
    /// One error per failed operation, plus the warnings of the simulation.
    pub diagnostics: Vec<Diagnostic>,
}

/// Dry applies the Combat Extended patch files of a plan, reading each file back from the project.
///
/// Returns `None` when the plan has no patch file or no Combat Extended data is loaded.
///
/// # Errors
///
/// A path refusal or a store error when a written file cannot be read back.
pub fn dry_run(
    ctx: &Ctx,
    writer: &GuardedWriter,
    plan: &BuiltPlan,
) -> ToolkitResult<Option<DryReport>> {
    let Some(engine) = ctx.engine().filter(|e| e.ce_available()) else {
        return Ok(None);
    };
    let mut patches = Vec::new();
    let mut diagnostics = Vec::new();
    for file in plan.files.iter().filter(|f| f.kind == FileKind::CePatch) {
        let text = writer
            .read_text(&file.path)?
            .unwrap_or_else(|| file.text.clone());
        match parse_document(text.as_bytes(), ParseMode::Game) {
            Ok(doc) => patches.push(doc.root),
            Err(e) => diagnostics.push(
                Diagnostic::new(
                    DiagCode::new("ce.dry-run-failed"),
                    Severity::Error,
                    format!("{} cannot be parsed: {e}", file.path),
                )
                .with_arg("path", &file.path),
            ),
        }
    }
    if patches.is_empty() && diagnostics.is_empty() {
        return Ok(None);
    }
    let run = dry_apply(&plan.scratch_defs, &patches, engine.ce());
    let ok = run.is_clean() && diagnostics.is_empty();
    diagnostics.extend(run.diagnostics);
    Ok(Some(DryReport { ok, diagnostics }))
}

/// Refuses a plan whose paths collide: the same path twice, two paths that differ only by letter case (one
/// disk would hold two files, another one a single file), or a path that is the folder of another path.
fn check_plan_paths(plan: &BuiltPlan) -> ToolkitResult<()> {
    let keys: Vec<(String, &str)> = plan
        .files
        .iter()
        .map(|f| (f.path.to_lowercase(), f.path.as_str()))
        .collect();
    let refuse = |path: &str, reason: &str| ToolkitError::PathRefused {
        path: path.chars().take(200).collect(),
        reason: reason.to_owned(),
    };
    for (i, (key, path)) in keys.iter().enumerate() {
        for (other_key, other) in keys.iter().skip(i + 1) {
            if key == other_key {
                let reason = if path == other {
                    "the plan lists this path twice"
                } else {
                    "two planned paths differ only by letter case"
                };
                return Err(refuse(path, reason));
            }
            if other_key.starts_with(&format!("{key}/")) {
                return Err(refuse(
                    path,
                    "a planned file is also the folder of another one",
                ));
            }
            if key.starts_with(&format!("{other_key}/")) {
                return Err(refuse(
                    other,
                    "a planned file is also the folder of another one",
                ));
            }
        }
    }
    Ok(())
}

fn first_error(plan: &BuiltPlan) -> String {
    plan.diagnostics
        .iter()
        .find(|d| d.severity == Severity::Error)
        .map_or_else(
            || "the plan has an error".to_owned(),
            |d| format!("{}: {}", d.code, d.message),
        )
}

/// Writes a built plan through the guarded writer. See the module documentation for the order, the checks
/// and the cancellation rule.
///
/// # Errors
///
/// [`ToolkitError::ApplyRefused`] for a plan with an error diagnostic, [`ToolkitError::PathRefused`] for a
/// path the writer refuses (nothing is written then), [`ToolkitError::Cancelled`] when cancelled before the
/// first file, and store errors from the writes.
pub fn apply_built(
    ctx: &Ctx,
    writer: &GuardedWriter,
    plan: &BuiltPlan,
    options: ApplyOptions,
    progress: &dyn ProgressSink,
    cancel: &CancelToken,
) -> ToolkitResult<ApplyReportDto> {
    if plan.has_errors() {
        return Err(refusal(first_error(plan)));
    }
    // every path is checked before the first write
    check_plan_paths(plan)?;
    for file in &plan.files {
        writer.path_of(&file.path)?;
    }
    let pending: Vec<_> = plan
        .files
        .iter()
        .filter(|f| f.action != FileAction::Unchanged)
        .collect();
    for file in &pending {
        writer.preflight(&file.path, Expect::Previous(file.previous.as_deref()))?;
    }
    let total = pending.len() as u64;
    let mut written = Vec::new();
    let mut diagnostics: Vec<Diagnostic> = plan
        .diagnostics
        .iter()
        .filter(|d| d.severity != Severity::Error)
        .cloned()
        .collect();
    let mut stopped = false;
    for (done, file) in pending.iter().enumerate() {
        if cancel.is_cancelled() {
            if written.is_empty() {
                return Err(ToolkitError::Cancelled);
            }
            diagnostics.push(Diagnostic::new(
                APPLY_CANCELLED,
                Severity::Warning,
                format!(
                    "cancelled after {} of {total} files; the written files stay in the project",
                    written.len()
                ),
            ));
            stopped = true;
            break;
        }
        progress.report(
            Progress::new("designer.apply", done as u64)
                .with_total(total)
                .with_unit(ProgressUnit::Files)
                .with_detail(file.path.clone()),
        );
        let result = writer.write_expecting(
            &file.path,
            &file.text,
            options.backup,
            Expect::Previous(file.previous.as_deref()),
        );
        let report = match result {
            Ok(report) => report,
            Err(e) if !written.is_empty() => {
                diagnostics.push(
                    Diagnostic::new(
                        APPLY_WRITE_FAILED,
                        Severity::Error,
                        format!(
                            "{} could not be written ({e}); {} of {total} files were written before it \
                             and stay in the project, the rest were not written",
                            file.path,
                            written.len()
                        ),
                    )
                    .with_arg("path", &file.path),
                );
                stopped = true;
                break;
            }
            Err(e) => return Err(e),
        };
        written.push(AppliedFileDto {
            path: file.path.clone(),
            action: action_dto(file.action),
            bytes: count(report.bytes),
            backup_path: report.backup.map(|p| p.to_string()),
            verified: report.verified,
        });
    }
    progress.report(
        Progress::new("designer.apply", written.len() as u64)
            .with_total(total)
            .with_unit(ProgressUnit::Files),
    );
    let mut dry_apply_ok = None;
    if options.dry_apply
        && !stopped
        && let Some(report) = dry_run(ctx, writer, plan)?
    {
        dry_apply_ok = Some(report.ok);
        diagnostics.extend(report.diagnostics);
    }
    Ok(ApplyReportDto {
        plan_id: plan.plan_id.clone(),
        written,
        unchanged: plan
            .files
            .iter()
            .filter(|f| f.action == FileAction::Unchanged)
            .map(|f| f.path.clone())
            .collect(),
        dry_apply_ok,
        diagnostics: diagnostics_to_dtos(&diagnostics),
    })
}

/// `designer_apply_plan`: rebuilds the plan of the request, compares its hash with the reviewed one and
/// writes it (see the module documentation).
///
/// # Errors
///
/// [`ToolkitError::PlanStale`] when the rebuilt plan differs from the reviewed one, and the errors of
/// [`apply_built`] and of planning.
pub fn apply_plan(
    ctx: &Ctx,
    req: DesignerApplyPlanRequest,
    progress: &dyn ProgressSink,
    cancel: &CancelToken,
) -> ToolkitResult<ApplyReportDto> {
    cancel.check()?;
    let prepared = prepare(ctx, &req.request)?;
    if prepared.plan.plan_id != req.plan_id {
        return Err(ToolkitError::PlanStale {
            expected: req.plan_id,
            found: prepared.plan.plan_id,
        });
    }
    apply_built(
        ctx,
        &prepared.writer,
        &prepared.plan,
        ApplyOptions {
            backup: req.backup,
            dry_apply: req.dry_apply,
        },
        progress,
        cancel,
    )
}
