//! The project tool (`tool-project`, minimal): open a mod folder and register it, create a new mod from the
//! scaffold plan, and summarise `LoadFolders.xml`.
//!
//! Opening reads the folder (About, `LoadFolders.xml`, the layout) and registers it in the project store of
//! the app data root; nothing is written into the mod folder (D-083). Creating a mod turns the scaffold plan
//! of `rimstudio-workspace` into files through the same guarded writer the designer uses: every path must
//! stay inside the new folder and outside the game install and config folders, and a scaffold never
//! overwrites a file that exists.
//!
//! The layout side of the tool (`docs/features/mod-layout.md`): [`scan`] reads a folder and gives every entry
//! its role, [`check`] lists what differs from the layout with a suggested fix, [`tree`] builds the annotated
//! tree, [`scaffold_missing`] creates the missing standard folders and [`read`] reads one file for the
//! viewer. Nothing there moves, replaces or deletes a file.
//!
//! The tool does not use the designer module (ADR 0004). The pieces both need (the project view, the writer,
//! the merging of `LoadFolders.xml`) live in [`crate::shared`].

pub mod about;
pub mod about_apply;
pub mod about_lint;
pub mod about_preview;
pub mod check;
pub mod fix;
pub mod fix_apply;
pub mod history;
pub mod journal;
pub mod link;
pub mod load_folders;
pub mod read;
pub mod scaffold_missing;
pub mod scaffold_preview;
pub mod scan;
pub mod tree;
pub mod undo;
pub mod versions;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::diag::Severity;
use rimstudio_design::ce::reader::CE_PACKAGE_ID;
use rimstudio_io::guard::sanitize_name;
use rimstudio_ipc_types::diagnostic::diagnostics_to_dtos;
use rimstudio_ipc_types::project::{
    ProjectCloseRequest, ProjectCloseResponse, ProjectLayoutCheckDto, ProjectLayoutCheckRequest,
    ProjectOpenRequest, ProjectSummaryDto, ProjectTreeDto, ProjectTreeRequest,
};
use rimstudio_workspace::project::{ProjectRecord, project_id_for};
use rimstudio_workspace::scaffold::{ScaffoldPlan, ScaffoldSpec, plan as scaffold_plan};
use rimstudio_xml::render::RenderOpts;

use crate::error::{ToolkitError, ToolkitResult};
use crate::shared::env::ProjectEnv;
use crate::shared::projectfs::{ProjectView, count_def_files, read_project};
use crate::shared::writer::Expect;

/// An opened project: the registered record, the read only view and the summary DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenedProject {
    /// The record in the project store (its id is the `ProjectId` of later calls).
    pub record: ProjectRecord,
    /// The view of the folder.
    pub view: ProjectView,
    /// The summary of `project_open`.
    pub summary: ProjectSummaryDto,
}

/// The blocks of a project's `LoadFolders.xml`, for display and for the Combat Extended plan.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LoadFoldersSummary {
    /// The project has a readable `LoadFolders.xml`.
    pub exists: bool,
    /// The blocks in file order: the block name and its entries (folder text, with `[if <ids>]` appended
    /// for a gated entry).
    pub blocks: Vec<(String, Vec<String>)>,
    /// An entry is gated on the Combat Extended package id.
    pub ce_gate: bool,
}

/// Summarises the `LoadFolders.xml` of a project view.
#[must_use]
pub fn load_folders_summary(view: &ProjectView) -> LoadFoldersSummary {
    let Some(file) = &view.load_folders else {
        return LoadFoldersSummary::default();
    };
    let blocks = file
        .root
        .elements()
        .map(|block| {
            let entries = block
                .children_named("li")
                .map(|li| {
                    let folder = li.text_content().trim().to_owned();
                    match li.attr("IfModActive") {
                        Some(ids) => format!("{folder} [if {ids}]"),
                        None => folder,
                    }
                })
                .collect();
            (block.tag.clone(), entries)
        })
        .collect();
    LoadFoldersSummary {
        exists: true,
        blocks,
        ce_gate: view.gates_on(CE_PACKAGE_ID),
    }
}

/// The summary DTO of a project.
#[must_use]
pub fn summary_of(record: &ProjectRecord, view: &ProjectView) -> ProjectSummaryDto {
    ProjectSummaryDto {
        project_id: record.id.as_str().to_owned(),
        name: view.name.clone(),
        path: view.root.to_string(),
        package_id: view.package_id.clone(),
        supported_versions: view.supported_versions.clone(),
        has_about: true,
        has_load_folders: view.load_folders.is_some(),
        has_ce_gate: view.gates_on(CE_PACKAGE_ID),
        def_files: count_def_files(&view.root),
        diagnostics: diagnostics_to_dtos(&view.diagnostics),
    }
}

/// Opens the mod folder at `path`: validates that it has an About file, registers it in the project store
/// (or refreshes its record) and returns the record, the view and the summary.
///
/// # Errors
///
/// [`ToolkitError::ProjectInvalid`] when the folder is not a mod, [`ToolkitError::Workspace`] for a path or
/// store problem.
pub fn open_project(env: &ProjectEnv, path: &Utf8Path) -> ToolkitResult<OpenedProject> {
    let view = read_project(path)?;
    let record = env.projects().open(&view.root)?;
    let summary = summary_of(&record, &view);
    Ok(OpenedProject {
        record,
        view,
        summary,
    })
}

/// `project_open`: [`open_project`] with the request and response DTOs.
///
/// # Errors
///
/// See [`open_project`].
pub fn open(env: &ProjectEnv, req: ProjectOpenRequest) -> ToolkitResult<ProjectSummaryDto> {
    Ok(open_project(env, Utf8Path::new(&req.path))?.summary)
}

/// `project_close`: releases a project. The toolkit keeps no session per project, so this only reports
/// whether the id was known; the record stays in the project store (it is the recent projects list).
///
/// # Errors
///
/// [`ToolkitError::Workspace`] for a store problem.
pub fn close(env: &ProjectEnv, req: ProjectCloseRequest) -> ToolkitResult<ProjectCloseResponse> {
    Ok(ProjectCloseResponse {
        closed: env.record(&req.project_id).is_ok(),
    })
}

/// The scaffold plan of a new mod (a pure function; nothing is written).
#[must_use]
pub fn create_plan(spec: &ScaffoldSpec) -> ScaffoldPlan {
    scaffold_plan(spec)
}

/// What [`create`] did.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateReport {
    /// The files written, relative to the new folder, in write order.
    pub written: Vec<String>,
    /// The folders created, relative to the new folder.
    pub folders: Vec<String>,
    /// The new project, opened and registered.
    pub project: OpenedProject,
}

/// `project_create`: applies the scaffold plan of `spec` through the guarded writer and opens the new
/// mod. `protected` lists folders that must not be written (the game install and config folders); the
/// environment's own protected folders always apply.
///
/// # Errors
///
/// [`ToolkitError::ApplyRefused`] for an invalid spec or when a planned file already exists (a scaffold never
/// overwrites), [`ToolkitError::PathRefused`] for a target inside a protected folder, and store errors.
pub fn create(
    env: &ProjectEnv,
    spec: &ScaffoldSpec,
    protected: &[Utf8PathBuf],
) -> ToolkitResult<CreateReport> {
    let plan = scaffold_plan(spec);
    if !plan.is_valid() {
        let reasons: Vec<String> = plan
            .problems
            .iter()
            .filter(|p| p.severity == Severity::Error)
            .map(|p| p.message.clone())
            .collect();
        return Err(ToolkitError::ApplyRefused {
            reason: reasons.join("; "),
        });
    }
    let folder_name = plan.root.file_name().unwrap_or("");
    if let Err(e) = sanitize_name(folder_name) {
        return Err(ToolkitError::PathRefused {
            path: plan.root.as_str().chars().take(200).collect(),
            reason: format!("the folder name is not usable on every platform: {e}"),
        });
    }
    let id = project_id_for(&plan.root);
    let writer = env.writer(&plan.root, id.as_str(), protected)?;
    // About.xml goes last: the folder only looks like a mod once everything else exists
    let mut rendered = plan.render(&RenderOpts::default());
    rendered.sort_by_key(|f| f.path == "About/About.xml");
    // a scaffold never overwrites: a file that exists must hold exactly the planned text (a stopped
    // run being completed), anything else refuses the call before the first write
    let mut pending = Vec::new();
    for file in &rendered {
        match writer.read_text(&file.path)? {
            None => pending.push(file),
            // About.xml is written last: when it exists the scaffold is complete and this is a real mod
            Some(existing) if existing == file.text && file.path != "About/About.xml" => {}
            Some(_) => {
                return Err(ToolkitError::ApplyRefused {
                    reason: format!("{} already exists in {}", file.path, plan.root),
                });
            }
        }
    }
    let mut folders = Vec::new();
    for dir in plan.dirs() {
        writer.make_dir(dir)?;
        folders.push(dir.to_owned());
    }
    let mut written = Vec::new();
    for file in pending {
        writer.write_expecting(&file.path, &file.text, false, Expect::Previous(None))?;
        written.push(file.path.clone());
    }
    let project = open_project(env, &plan.root)?;
    Ok(CreateReport {
        written,
        folders,
        project,
    })
}

/// `project_tree`: the annotated tree of a registered project, with its layout issues.
///
/// # Errors
///
/// [`ToolkitError::ProjectNotOpen`] for an unknown project, [`ToolkitError::ProjectInvalid`] when the folder
/// is no longer a mod.
pub fn tree(env: &ProjectEnv, req: &ProjectTreeRequest) -> ToolkitResult<ProjectTreeDto> {
    let (_, view) = env.view(&req.project_id)?;
    let max = req.max_nodes.map_or(tree::DEFAULT_MAX_NODES, |n| {
        usize::try_from(n).unwrap_or(usize::MAX)
    });
    Ok(tree::project_tree_of(&view, &req.project_id, max))
}

/// `project_layout_check`: the layout issues of a registered project, most serious first.
///
/// # Errors
///
/// See [`tree`].
pub fn layout_check(
    env: &ProjectEnv,
    req: &ProjectLayoutCheckRequest,
) -> ToolkitResult<ProjectLayoutCheckDto> {
    use rimstudio_ipc_types::diagnostic::SeverityDto;
    let (_, view) = env.view(&req.project_id)?;
    let scanned = scan::scan(&view);
    let issues = check::check(&view, &scanned);
    let count = |sev: SeverityDto| {
        u32::try_from(issues.iter().filter(|i| i.severity == sev).count()).unwrap_or(u32::MAX)
    };
    Ok(ProjectLayoutCheckDto {
        project_id: req.project_id.clone(),
        profile: tree::profile_dto(view.layout.profile),
        errors: count(SeverityDto::Error),
        warnings: count(SeverityDto::Warning),
        infos: count(SeverityDto::Info),
        auto_fixable: u32::try_from(issues.iter().filter(|i| i.fix.automatic).count())
            .unwrap_or(u32::MAX),
        issues,
    })
}
