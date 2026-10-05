//! The write plan of the designer (`designer_export_plan`): the files a design would write, rendered and
//! compared with the project, without writing anything.
//!
//! The vanilla definition is always planned ([`rimstudio_design::plan::export_vanilla_plan`]). The optional
//! Combat Extended patch joins the plan only when the spec carries a Combat Extended block and Combat
//! Extended data is loaded; it is a separate set of files in the gated CE folder plus the `LoadFolders.xml`
//! edit, never a change of the vanilla definition (owner rule D-085). Without the block the plan holds no
//! Combat Extended file and no Combat Extended class. When the block is on but Combat Extended is not
//! available, the plan stays vanilla and a warning says why.
//!
//! Planning is a pure function of the draft, the project folder and the reference snapshot: the engine
//! builds node trees and diagnostics, [`realize`] renders them with `rimstudio-xml` (a new file) or merges
//! them into the existing file by byte span edits, computes the unified diff and the plan id (a content
//! hash). Static validation (the design validator, the reference checks and the Combat Extended lint) runs
//! here, so a plan with an error diagnostic is visible before anything is written.

use std::collections::BTreeSet;

use rimstudio_core::diag::{DiagCode, Diagnostic, Severity};
use rimstudio_core::tree::Node;
use rimstudio_design::ce::lint::{LintContext, LintFile, run_files};
use rimstudio_design::ce::patchgen::{CeProjectState, export_ce_plan_with, gate_violations};
use rimstudio_design::model::DesignSpec;
use rimstudio_design::plan::{
    CopyPlan, FileAction, FileKind, PlanBuilder, PlannedFile, TextEdit, WritePlan,
    export_vanilla_plan_with,
};
use rimstudio_design::validation::{has_errors, validate_refs};
use rimstudio_ipc_types::designer::{
    CopyPlanDto, DesignerExportPlanRequest, FileActionDto, FileKindDto, PlannedFileDto,
    WritePlanDto,
};
use rimstudio_ipc_types::diagnostic::diagnostics_to_dtos;
use rimstudio_workspace::project::ProjectRecord;

use super::ce::accept_for_plan;
use super::convert::convert_plan;
use super::ctx::{Ctx, Engine};
use super::dto::{count, draft_from_dto};
use crate::error::{ToolkitError, ToolkitResult};
use crate::shared::assets::facts_of;
use crate::shared::diff::unified_diff;
use crate::shared::merge::{changed_span, merge_into, merge_load_folders, render_new};
use crate::shared::projectfs::ProjectView;
use crate::shared::writer::GuardedWriter;

/// The diagnostic code of a planned file that cannot be merged into the file on disk.
pub const MERGE_FAILED: DiagCode = DiagCode::new("designer.merge-failed");
/// The diagnostic code of a planned path the writer refuses.
pub const PATH_REFUSED: DiagCode = DiagCode::new("designer.path-refused");
/// The diagnostic code of an open question of the conversion flow.
pub const CONVERT_ASK: DiagCode = DiagCode::new("designer.convert-needs-answer");
/// The diagnostic code (info) that reports a number of a conversion the user did not give: it was predicted
/// from the user's own conversions or copied from the vanilla definition.
pub const CONVERT_DERIVED: DiagCode = DiagCode::new("designer.convert-derived");
/// The diagnostic code of a Combat Extended block on a machine without Combat Extended.
pub const CE_UNAVAILABLE: DiagCode = DiagCode::new("designer.ce-unavailable");

/// One file of a built plan: the final text and what happens to the file.
#[derive(Debug, Clone, PartialEq)]
pub struct BuiltFile {
    /// Path relative to the project root, with `/` separators.
    pub path: String,
    /// What the file is for.
    pub kind: FileKind,
    /// What applying does to it.
    pub action: FileAction,
    /// The complete text the file has after the plan is applied.
    pub text: String,
    /// The text on disk before, `None` for a new file.
    pub previous: Option<String>,
    /// Unified diff against the file on disk, for [`FileAction::UpdateRegion`].
    pub diff: Option<String>,
    /// The byte span edits that turn `previous` into `text`.
    pub edits: Vec<TextEdit>,
    /// For a copied asset ([`FileKind::Copy`]): the source and what is at the target now. `text` is empty.
    pub copy: Option<BuiltCopy>,
}

/// The copy part of a built file: where the bytes come from and what the target holds now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltCopy {
    /// What the plan says about the source (path as written, hash, size, dimensions).
    pub plan: CopyPlan,
    /// SHA-256 of the file at the target at planning time, `None` when there is none. Apply refuses when
    /// the target holds something else by then.
    pub existing_sha256: Option<String>,
}

/// A plan with final texts, ready to show or to apply.
#[derive(Debug, Clone, PartialEq)]
pub struct BuiltPlan {
    /// The files in write order: definitions, then patches, then `LoadFolders.xml`.
    pub files: Vec<BuiltFile>,
    /// Validation, lint and planning diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// Content hash of the files (path, action, text).
    pub plan_id: String,
    /// The def nodes a dry apply of the patch runs against: the new definitions of a design, the raw
    /// definitions of the project for a conversion.
    pub scratch_defs: Vec<Node>,
}

impl BuiltPlan {
    /// True when any diagnostic is an error; such a plan cannot be applied.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        has_errors(&self.diagnostics)
    }

    /// The file with this path.
    #[must_use]
    pub fn file(&self, path: &str) -> Option<&BuiltFile> {
        self.files.iter().find(|f| f.path == path)
    }

    /// The paths of the files, in write order.
    #[must_use]
    pub fn paths(&self) -> Vec<&str> {
        self.files.iter().map(|f| f.path.as_str()).collect()
    }

    /// The plan as the DTO of `designer_export_plan`.
    #[must_use]
    pub fn to_dto(&self) -> WritePlanDto {
        WritePlanDto {
            plan_id: self.plan_id.clone(),
            files: self
                .files
                .iter()
                .map(|f| PlannedFileDto {
                    path: f.path.clone(),
                    kind: kind_dto(f.kind),
                    action: action_dto(f.action),
                    rendered: f.text.clone(),
                    diff: f.diff.clone(),
                    bytes: match &f.copy {
                        Some(c) => count(usize::try_from(c.plan.bytes).unwrap_or(usize::MAX)),
                        None => count(f.text.len()),
                    },
                    copy: f.copy.as_ref().map(|c| CopyPlanDto {
                        source: c.plan.source.clone(),
                        sha256: c.plan.sha256.clone(),
                        bytes: count(usize::try_from(c.plan.bytes).unwrap_or(usize::MAX)),
                        width: c.plan.width,
                        height: c.plan.height,
                        existing_sha256: c.existing_sha256.clone().filter(|h| *h != c.plan.sha256),
                    }),
                })
                .collect(),
            diagnostics: diagnostics_to_dtos(&self.diagnostics),
            has_errors: self.has_errors(),
        }
    }
}

/// The wire kind of a file kind.
#[must_use]
pub fn kind_dto(kind: FileKind) -> FileKindDto {
    match kind {
        FileKind::CePatch => FileKindDto::CePatch,
        FileKind::CeDefs => FileKindDto::CeDefs,
        FileKind::LoadFolders => FileKindDto::LoadFolders,
        FileKind::About => FileKindDto::About,
        FileKind::Copy => FileKindDto::Copy,
        _ => FileKindDto::VanillaDefs,
    }
}

/// The wire action of a file action.
#[must_use]
pub fn action_dto(action: FileAction) -> FileActionDto {
    match action {
        FileAction::Create => FileActionDto::Create,
        FileAction::UpdateRegion => FileActionDto::UpdateRegion,
        FileAction::Unchanged => FileActionDto::Unchanged,
        FileAction::Replace => FileActionDto::Replace,
    }
}

/// Everything a plan is built from and applied with: the registered project, its view and the writer.
#[derive(Debug)]
pub struct Prepared {
    /// The registered project.
    pub record: ProjectRecord,
    /// The read only view of the project folder.
    pub view: ProjectView,
    /// The guarded writer of the project.
    pub writer: GuardedWriter,
    /// The built plan.
    pub plan: BuiltPlan,
}

fn error_diag(code: &DiagCode, message: String) -> Diagnostic {
    Diagnostic::new(code.clone(), Severity::Error, message)
}

/// The write order: copied assets first (a definition never points at a file that is not there yet), then
/// definitions, then patches, then `LoadFolders.xml`.
fn write_rank(kind: FileKind) -> u8 {
    match kind {
        FileKind::Copy => 0,
        FileKind::VanillaDefs => 1,
        FileKind::CeDefs => 2,
        FileKind::CePatch => 3,
        FileKind::LoadFolders => 4,
        _ => 5,
    }
}

/// Renders a design plan against the project on disk: a new file is rendered whole, an existing file is
/// merged by byte span edits, the diff and the plan id are computed.
///
/// A file that cannot be merged, or whose path the writer refuses, is dropped from the result and reported
/// as an error diagnostic, so the plan cannot be applied.
#[must_use]
pub fn realize(
    view: &ProjectView,
    writer: &GuardedWriter,
    design: &WritePlan,
    extra: Vec<Diagnostic>,
) -> BuiltPlan {
    let mut diagnostics: Vec<Diagnostic> = design.diagnostics.clone();
    diagnostics.extend(extra);
    let mut files: Vec<BuiltFile> = Vec::new();
    let mut scratch_defs: Vec<Node> = Vec::new();
    for file in &design.files {
        if file.kind == FileKind::Copy {
            if let Some(built) = realize_copy(writer, file, &mut diagnostics) {
                files.push(built);
            }
            continue;
        }
        if file.kind == FileKind::VanillaDefs
            && let Some(tree) = &file.tree
        {
            scratch_defs.extend(tree.elements().cloned());
        }
        let rel = match (&file.kind, &view.load_folders) {
            (FileKind::LoadFolders, Some(existing)) => existing.rel.clone(),
            _ => file.path.clone(),
        };
        let previous = match writer.read_text(&rel) {
            Ok(p) => p,
            Err(e) => {
                // a path the writer refuses is a path problem; a file that cannot be read as UTF-8 text
                // (another encoding, binary content) is a content problem and is never overwritten
                let diag = match e {
                    ToolkitError::PathRefused { .. } => error_diag(&PATH_REFUSED, e.to_string()),
                    _ => error_diag(
                        &MERGE_FAILED,
                        format!("{rel} cannot be read as UTF-8 text and is left alone: {e}"),
                    ),
                };
                diagnostics.push(diag.with_arg("path", &rel));
                continue;
            }
        };
        let merged: Result<String, String> = match (&previous, file.kind, &file.tree) {
            (None, _, _) => Ok(render_new(file)),
            (Some(old), FileKind::LoadFolders, Some(tree)) => merge_load_folders(old, tree),
            (Some(old), _, _) => merge_into(old, file),
        };
        let text = match merged {
            Ok(t) => t,
            Err(reason) => {
                diagnostics.push(
                    error_diag(&MERGE_FAILED, format!("{rel} cannot be updated: {reason}"))
                        .with_arg("path", &rel)
                        .with_arg("reason", reason),
                );
                continue;
            }
        };
        let (action, diff, edits) = match &previous {
            None => (FileAction::Create, None, Vec::new()),
            Some(old) if *old == text => (FileAction::Unchanged, None, Vec::new()),
            Some(old) => (
                FileAction::UpdateRegion,
                Some(unified_diff(&rel, old, &text)).filter(|d| !d.is_empty()),
                changed_span(old, &text),
            ),
        };
        files.push(BuiltFile {
            path: rel,
            kind: file.kind,
            action,
            text,
            previous,
            diff,
            edits,
            copy: None,
        });
    }
    files.sort_by(|a, b| {
        write_rank(a.kind)
            .cmp(&write_rank(b.kind))
            .then_with(|| a.path.cmp(&b.path))
    });
    let plan_id = plan_id_of(&files);
    BuiltPlan {
        files,
        diagnostics,
        plan_id,
        scratch_defs,
    }
}

/// Builds the file of a planned copy: compares the hash of what is at the target with the source's.
fn realize_copy(
    writer: &GuardedWriter,
    file: &PlannedFile,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<BuiltFile> {
    let plan = file.copy.clone()?;
    let existing = match writer.existing_file_hash(&file.path) {
        Ok(e) => e.map(|(_, hash)| hash),
        Err(e) => {
            let diag = match e {
                ToolkitError::PathRefused { .. } => error_diag(&PATH_REFUSED, e.to_string()),
                _ => error_diag(
                    &MERGE_FAILED,
                    format!("{} cannot be compared and is left alone: {e}", file.path),
                ),
            };
            diagnostics.push(diag.with_arg("path", &file.path));
            return None;
        }
    };
    let action = match &existing {
        None => FileAction::Create,
        Some(hash) if *hash == plan.sha256 => FileAction::Unchanged,
        Some(_) => FileAction::Replace,
    };
    Some(BuiltFile {
        path: file.path.clone(),
        kind: FileKind::Copy,
        action,
        text: String::new(),
        previous: None,
        diff: None,
        edits: Vec::new(),
        copy: Some(BuiltCopy {
            plan,
            existing_sha256: existing,
        }),
    })
}

/// The content hash of a plan: path, kind, action and final text of every file, in write order. A copied
/// file adds its source, the hash of the source and the hash of what is at the target.
#[must_use]
pub fn plan_id_of(files: &[BuiltFile]) -> String {
    let mut hasher = blake3::Hasher::new();
    for f in files {
        hasher.update(f.path.as_bytes());
        hasher.update(&[0]);
        hasher.update(format!("{:?}{:?}", f.kind, f.action).as_bytes());
        hasher.update(&[0]);
        hasher.update(f.text.as_bytes());
        hasher.update(&[0]);
        if let Some(c) = &f.copy {
            hasher.update(c.plan.source.as_bytes());
            hasher.update(&[0]);
            hasher.update(c.plan.sha256.as_bytes());
            hasher.update(&[0]);
            hasher.update(c.existing_sha256.as_deref().unwrap_or("-").as_bytes());
            hasher.update(&[0]);
        }
    }
    hasher
        .finalize()
        .to_hex()
        .as_str()
        .chars()
        .take(32)
        .collect()
}

/// The game version a Combat Extended plan serves: the newest supported version of the project, else the
/// version of the reference set.
fn serve_version(ctx: &Ctx, view: &ProjectView) -> String {
    let fallback = ctx
        .session()
        .map(|s| {
            let v = &s.reference().game_version;
            format!("{}.{}", v.major, v.minor)
        })
        .unwrap_or_else(|| "1.6".to_owned());
    view.game_version(&fallback)
}

/// The Combat Extended patch plan of a spec and the lint of its files, merged into `builder`.
///
/// Returns the diagnostics of the part. Nothing is added when the spec has no Combat Extended block.
fn plan_ce(
    ctx: &Ctx,
    engine: Option<&Engine>,
    view: &ProjectView,
    spec: &DesignSpec,
    builder: &mut PlanBuilder,
) -> Vec<Diagnostic> {
    if spec.ce.is_none() {
        return Vec::new();
    }
    let Some(engine) = engine.filter(|e| e.ce_available()) else {
        return vec![
            Diagnostic::new(
                CE_UNAVAILABLE,
                Severity::Warning,
                "the Combat Extended patch was asked for, but no Combat Extended data is loaded; \
                 only the vanilla definition is planned",
            )
            .with_arg("field", "/ce"),
        ];
    };
    let state = CeProjectState {
        load_folders: view.load_folders.as_ref().map(|f| f.root.clone()),
        supported_versions: view.supported_versions.clone(),
        game_version: serve_version(ctx, view),
        container: None,
    };
    let plan = export_ce_plan_with(spec, engine.ce(), &view.layout, &state);
    finish_ce(engine, view, &state, plan, &spec_known_defs(spec), builder)
}

/// The def names the lint may treat as existing besides the Combat Extended data: the new item's own
/// projectile.
fn spec_known_defs(spec: &DesignSpec) -> BTreeSet<String> {
    let mut known = BTreeSet::new();
    if let Some(ce) = &spec.ce {
        known.extend(ce.default_projectile.clone());
    }
    known.extend(super::ammo::custom_known_defs(spec));
    known
}

/// Lints the patch files of a Combat Extended plan and merges the plan into `builder`. Shared by the design
/// and the conversion flow.
pub(crate) fn finish_ce(
    engine: &Engine,
    view: &ProjectView,
    state: &CeProjectState,
    plan: WritePlan,
    known_defs: &BTreeSet<String>,
    builder: &mut PlanBuilder,
) -> Vec<Diagnostic> {
    let mut diagnostics = plan.diagnostics.clone();
    let patch_files: Vec<&PlannedFile> = plan
        .files
        .iter()
        .filter(|f| f.kind == FileKind::CePatch)
        .collect();
    if !patch_files.is_empty() {
        let lint_files: Vec<LintFile> = patch_files
            .iter()
            .filter_map(|f| {
                f.tree
                    .clone()
                    .map(|t| LintFile::parsed(Some(f.path.clone()), t))
            })
            .collect();
        let load_folders = plan
            .files
            .iter()
            .find(|f| f.kind == FileKind::LoadFolders)
            .and_then(|f| f.tree.clone())
            .or_else(|| state.load_folders.clone());
        let ctx = LintContext {
            paths: lint_files.iter().filter_map(|f| f.path.clone()).collect(),
            load_folders,
            game_version: state.game_version.clone(),
            known_defs: known_defs.clone(),
            ce_types: None,
            known_fields: None,
        };
        diagnostics.extend(run_files(&lint_files, engine.ce(), &ctx));
    }
    let violations = gate_violations(&plan, &view.layout);
    for path in violations {
        diagnostics.push(
            error_diag(
                &DiagCode::new("designer.ce-outside-gate"),
                format!("{path} holds a Combat Extended class outside the gated folder"),
            )
            .with_arg("path", path),
        );
    }
    if !has_errors(&diagnostics) {
        for file in plan.files {
            builder.add_file(file);
        }
    }
    diagnostics
}

/// Plans a new design: the vanilla files always, the Combat Extended patch only when the spec asks for it.
/// Pure: reads the project view and the reference snapshot, writes nothing.
#[must_use]
pub fn plan_design(
    ctx: &Ctx,
    view: &ProjectView,
    spec: &DesignSpec,
) -> (WritePlan, Vec<Diagnostic>) {
    let engine = ctx.engine();
    let mut builder = PlanBuilder::new();
    let mut extra: Vec<Diagnostic> = Vec::new();
    if let Some(engine) = &engine {
        extra.extend(validate_refs(spec, &engine.lookup()));
    }
    if let (Some(engine), Some(custom)) = (
        &engine,
        spec.ce.as_ref().and_then(|c| c.custom_ammo.as_ref()),
    ) {
        extra.extend(super::ammo::ref_diagnostics(spec, custom, &engine.lookup()));
    }
    let facts = facts_of(spec, &view.root);
    let vanilla = export_vanilla_plan_with(spec, &view.layout, &facts);
    let vanilla_ok = !vanilla.has_errors();
    extra.extend(vanilla.diagnostics.iter().cloned());
    for file in vanilla.files {
        builder.add_file(file);
    }
    if vanilla_ok {
        extra.extend(plan_ce(ctx, engine.as_deref(), view, spec, &mut builder));
    }
    (builder.build(), extra)
}

/// Builds the plan of a request against a prepared project.
///
/// # Errors
///
/// [`ToolkitError::ProjectNotOpen`], [`ToolkitError::ProjectInvalid`], [`ToolkitError::InvalidDraft`] and
/// the errors of the conversion flow. Content problems are diagnostics of the plan.
pub fn prepare(ctx: &Ctx, req: &DesignerExportPlanRequest) -> ToolkitResult<Prepared> {
    let (record, view) = ctx.env().view(&req.project_id)?;
    let writer = ctx.writer_for(&req.project_id, &view.root)?;
    let plan = match &req.convert {
        Some(convert) => {
            let outcome = convert_plan(ctx, &record, &view, convert)?;
            let mut built = realize(&view, &writer, &outcome.plan, outcome.diagnostics);
            built.scratch_defs = outcome.scratch_defs;
            built
        }
        None => {
            let draft = draft_from_dto(&req.draft)?;
            // An opt in to suggestions fills the empty fields of the Combat Extended block (never typed
            // values, never the toggle) before the patch is planned.
            let (spec, mut notes) = match &req.accept_suggestions {
                Some(accept) => accept_for_plan(ctx, &draft.spec, accept),
                None => (draft.spec.clone(), Vec::new()),
            };
            let (plan, extra) = plan_design(ctx, &view, &spec);
            notes.extend(extra);
            realize(&view, &writer, &plan, notes)
        }
    };
    let mut plan = plan;
    plan.diagnostics.extend(
        view.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .cloned(),
    );
    Ok(Prepared {
        record,
        view,
        writer,
        plan,
    })
}

/// `designer_export_plan`: the files a design (or a conversion) would write, rendered, with diffs and
/// diagnostics. Reads the project and the reference snapshot, writes nothing (IT-014).
///
/// # Errors
///
/// See [`prepare`].
pub fn export_plan(ctx: &Ctx, req: DesignerExportPlanRequest) -> ToolkitResult<WritePlanDto> {
    Ok(prepare(ctx, &req)?.plan.to_dto())
}

/// A plan refused for a reason that is not a content problem.
#[must_use]
pub fn refusal(reason: impl Into<String>) -> ToolkitError {
    ToolkitError::ApplyRefused {
        reason: reason.into(),
    }
}
