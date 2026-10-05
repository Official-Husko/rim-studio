//! The layout fix plan: what the layout check suggests, as a list of changes the tool can carry out.
//!
//! The plan is a pure read of the project. Each item is one change: move a weapon definition file to the
//! place the layout names, move a Combat Extended patch into the gated folder (and gate that folder in
//! `LoadFolders.xml`), rename an older Combat Extended folder to `Compat/CombatExtended` (and its
//! `LoadFolders.xml` entry), correct the letter case of a standard folder, or create a missing standard
//! folder. An item has a risk. A **safe** item can be applied and undone. An item that **needs review** is
//! only listed: textures and sounds (definitions refer to them by path), a move whose old path other files
//! mention, a file that cannot be moved whole (several weapons of different categories, a patch that also
//! holds operations that do not use Combat Extended), and anything the path rules refuse.
//!
//! A conflict (the destination exists) is shown with a numbered name next to it; the apply refuses the item
//! unless the caller accepts that name. The plan id is a hash of every item, the state of every file the
//! items touch and the text of `LoadFolders.xml`, so an apply built from an older plan is refused.
//!
//! The edits of `LoadFolders.xml` use the helpers of `rimstudio-xml` and the merge of the designer's Combat
//! Extended plan, so an entry is added by a span edit that keeps every other byte of the file. A project
//! without the file gets one that keeps loading what the game loaded without it (the root, `Common`, the
//! version folder) plus the gated folder.

use std::collections::BTreeSet;
use std::path::Path;

use camino::Utf8PathBuf;
use rimstudio_core::load_plan::{LoadEntry, LoadFoldersSpec};
use rimstudio_core::paths;
use rimstudio_core::tree::Node;
use rimstudio_design::ce::patchgen::load_folders_plan;
use rimstudio_ipc_types::project::{LayoutIssueDto, NodeRoleDto};
use rimstudio_ipc_types::project_fix::{
    LayoutFixConflictDto, LayoutFixItemDto, LayoutFixItemKindDto, LayoutFixReferenceDto,
    LayoutFixRiskDto, ProjectLayoutFixPlanDto, ProjectLayoutFixPlanRequest,
};
use rimstudio_xml::edit::SpanEditor;
use rimstudio_xml::load_folders;
use rimstudio_xml::modes::ParseMode;
use rimstudio_xml::reader::parse_document;
use rimstudio_xml::render::RenderOpts;

use super::check::check;
use super::journal::{hash_text, hash_tree};
use super::scan::{Scan, scan as scan_project};
use crate::error::ToolkitResult;
use crate::shared::diff::unified_diff;
use crate::shared::env::ProjectEnv;
use crate::shared::merge::merge_load_folders;
use crate::shared::projectfs::ProjectView;
use crate::shared::writer::GuardedWriter;

/// The most items a plan lists.
pub const MAX_ITEMS: usize = 500;
/// The most references listed for one item.
pub const MAX_REFERENCES: usize = 20;
/// The largest file read for the references, in bytes.
const MAX_REFERENCE_FILE_BYTES: usize = 2_000_000;
/// The most files read for the references.
const MAX_REFERENCE_FILES: usize = 4000;
/// The most bytes of file text kept for the references.
const MAX_REFERENCE_TOTAL_BYTES: usize = 64 * 1024 * 1024;
/// The numbered names tried for a conflict.
const SUFFIX_TRIES: u32 = 9;

/// What an item does when it is applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Op {
    /// Create an empty folder.
    CreateFolder { path: String },
    /// Move a file.
    MoveFile { from: String, to: String },
    /// Move or rename a folder.
    MoveFolder { from: String, to: String },
    /// Gate the Combat Extended folder in `LoadFolders.xml` (edit the file or create it).
    GateCe,
    /// Rename the `LoadFolders.xml` entries of the old Combat Extended folder.
    RenameEntry { from: String, to: String },
    /// Listed for review only.
    Review,
}

/// One item of a built plan with what the apply needs besides the DTO.
#[derive(Debug, Clone)]
pub(crate) struct PlanItem {
    /// The DTO shown to the caller.
    pub dto: LayoutFixItemDto,
    /// What applying does.
    pub op: Op,
    /// The numbered name offered for a conflict.
    pub suggested_to: Option<String>,
}

/// A plan built from the disk, with the project read it was built from.
#[derive(Debug, Clone)]
pub(crate) struct Built {
    /// The plan id.
    pub plan_id: String,
    /// Every item, in the order they are carried out.
    pub items: Vec<PlanItem>,
    /// The view of the project the plan was built from.
    pub view: ProjectView,
}

struct Draft {
    kind: LayoutFixItemKindDto,
    code: &'static str,
    from: String,
    to: String,
    why: String,
    risk: LayoutFixRiskDto,
    review_reason: Option<String>,
    conflict: bool,
    suggested: Option<String>,
    references: Vec<LayoutFixReferenceDto>,
    op: Op,
    needs_gate: bool,
    pairs_with_rename: bool,
    diff: Option<String>,
}

impl Draft {
    fn new(kind: LayoutFixItemKindDto, code: &'static str, op: Op) -> Self {
        Self {
            kind,
            code,
            from: String::new(),
            to: String::new(),
            why: String::new(),
            risk: LayoutFixRiskDto::Safe,
            review_reason: None,
            conflict: false,
            suggested: None,
            references: Vec::new(),
            op,
            needs_gate: false,
            pairs_with_rename: false,
            diff: None,
        }
    }

    fn review(mut self, reason: impl Into<String>) -> Self {
        self.set_review(reason);
        self
    }

    fn set_review(&mut self, reason: impl Into<String>) {
        self.risk = LayoutFixRiskDto::NeedsReview;
        self.review_reason = Some(reason.into());
        self.op = Op::Review;
    }
}

/// The lower cased text of the definition and patch files, for the reference search.
struct RefIndex {
    files: Vec<(String, Vec<String>)>,
}

impl RefIndex {
    fn build(writer: &GuardedWriter, scan: &Scan) -> Self {
        let mut files = Vec::new();
        let mut total = 0usize;
        for e in scan.entries.iter().filter(|e| {
            !e.is_dir
                && e.name.to_ascii_lowercase().ends_with(".xml")
                && matches!(
                    e.role,
                    NodeRoleDto::Defs
                        | NodeRoleDto::DefsWeapons
                        | NodeRoleDto::DefsSounds
                        | NodeRoleDto::Patches
                        | NodeRoleDto::CeCompat
                )
        }) {
            if files.len() >= MAX_REFERENCE_FILES || total >= MAX_REFERENCE_TOTAL_BYTES {
                break;
            }
            let Ok(Some(read)) = writer.read_limited(&e.rel, MAX_REFERENCE_FILE_BYTES) else {
                continue;
            };
            total = total.saturating_add(read.bytes.len());
            let text = String::from_utf8_lossy(&read.bytes).into_owned();
            files.push((
                e.rel.clone(),
                text.lines().map(str::to_owned).collect::<Vec<_>>(),
            ));
        }
        Self { files }
    }

    fn find(&self, needles: &[String], skip: &str) -> Vec<LayoutFixReferenceDto> {
        let needles: Vec<String> = needles
            .iter()
            .filter(|n| !n.is_empty())
            .map(|n| n.to_ascii_lowercase())
            .collect();
        let mut out = Vec::new();
        if needles.is_empty() {
            return out;
        }
        for (rel, lines) in &self.files {
            if rel == skip {
                continue;
            }
            for (i, line) in lines.iter().enumerate() {
                let lower = line.to_ascii_lowercase();
                if needles.iter().any(|n| lower.contains(n.as_str())) {
                    out.push(LayoutFixReferenceDto {
                        path: rel.clone(),
                        line: u32::try_from(i + 1).unwrap_or(u32::MAX),
                        text: line.trim().chars().take(200).collect(),
                    });
                    if out.len() >= MAX_REFERENCES {
                        return out;
                    }
                }
            }
        }
        out
    }
}

fn is_plain_version(name: &str) -> bool {
    let mut parts = name.split('.');
    let (Some(a), Some(b), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !a.is_empty()
        && !b.is_empty()
        && a.bytes().all(|c| c.is_ascii_digit())
        && b.bytes().all(|c| c.is_ascii_digit())
}

fn version_key(name: &str) -> (u32, u32) {
    let mut parts = name.split('.');
    let major = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    (major, minor)
}

fn normal(text: &str) -> String {
    text.replace('\\', "/").trim().trim_matches('/').to_owned()
}

/// The path of `LoadFolders.xml` as it is on disk, the standard name when the project has none.
pub(crate) fn load_folders_rel(view: &ProjectView) -> String {
    view.load_folders
        .as_ref()
        .map_or_else(|| "LoadFolders.xml".to_owned(), |f| f.rel.clone())
}

/// True when `LoadFolders.xml` has an entry for `folder` that is gated on Combat Extended.
fn gate_covers(view: &ProjectView, folder: &str) -> bool {
    let Some(file) = &view.load_folders else {
        return false;
    };
    file.root.elements().any(|block| {
        block.children_named("li").any(|li| {
            normal(&li.text_content()).eq_ignore_ascii_case(&normal(folder))
                && li.attr("IfModActive").is_some_and(|ids| {
                    ids.split(',')
                        .any(|id| id.trim().eq_ignore_ascii_case(paths::CE_PACKAGE_ID))
                })
        })
    })
}

/// True when the project's `LoadFolders.xml` lists a folder that holds `path` (or there is no file).
fn load_folders_cover(view: &ProjectView, path: &str) -> bool {
    let Some(file) = &view.load_folders else {
        return true;
    };
    let lower = path.to_ascii_lowercase();
    file.root.elements().any(|block| {
        block.children_named("li").any(|li| {
            let entry = normal(&li.text_content()).to_ascii_lowercase();
            entry.is_empty() || lower.starts_with(&format!("{entry}/"))
        })
    })
}

/// The text of `LoadFolders.xml` after the gate item: `(previous text, new text)`; the previous text is
/// `None` when the file is created.
///
/// # Errors
///
/// A plain sentence when the existing file cannot be edited.
pub(crate) fn gate_text(
    view: &ProjectView,
    root: &Path,
) -> Result<(Option<String>, String), String> {
    let game = view.game_version("1.6");
    if let Some(file) = &view.load_folders {
        // an entry for the folder that has no condition yet is gated in place, not listed twice
        let text = gate_existing_entries(&file.text, &view.layout.ce_dir())?;
        let root = parse_document(text.as_bytes(), ParseMode::Tolerant)
            .map_err(|e| format!("LoadFolders.xml cannot be edited: {e}"))?
            .root;
        let outcome = load_folders_plan(Some(&root), &view.layout, &game, paths::CE_PACKAGE_ID);
        let Some(planned) = outcome.file else {
            let why = outcome.diagnostics.first().map_or_else(
                || "the file cannot be edited".to_owned(),
                |d| d.message.clone(),
            );
            return Err(format!("LoadFolders.xml cannot be edited: {why}"));
        };
        let tree = planned
            .tree
            .ok_or_else(|| "LoadFolders.xml cannot be edited".to_owned())?;
        let new = merge_load_folders(&text, &tree)?;
        return Ok((Some(file.text.clone()), new));
    }
    Ok((None, created_text(view, root, &game)))
}

/// Gives every entry for `folder` that has no `IfModActive` condition the Combat Extended condition, by span
/// edits.
fn gate_existing_entries(text: &str, folder: &str) -> Result<String, String> {
    let mut ed = SpanEditor::open(text).map_err(|e| format!("LoadFolders.xml: {e}"))?;
    let root = format!("/{}", ed.root_tag());
    let mut targets = Vec::new();
    for block in ed.children(&root).map_err(|e| e.to_string())? {
        for li in ed.children(&block.path).map_err(|e| e.to_string())? {
            if li.tag != "li" {
                continue;
            }
            let info = ed.element(&li.path).map_err(|e| e.to_string())?;
            let conditional = info
                .attrs
                .iter()
                .any(|(k, _)| k == "IfModActive" || k == "IfModActiveAll");
            if !conditional && normal(&info.text).eq_ignore_ascii_case(&normal(folder)) {
                targets.push(li.path);
            }
        }
    }
    for path in targets {
        ed.set_attr(&path, "IfModActive", paths::CE_PACKAGE_ID)
            .map_err(|e| e.to_string())?;
    }
    Ok(ed.into_text())
}

/// A new `LoadFolders.xml` that keeps loading what the game loads without one and gates the Combat Extended
/// folder.
fn created_text(view: &ProjectView, root: &Path, game: &str) -> String {
    let mut versions: BTreeSet<(u32, u32, String)> = BTreeSet::new();
    let mut has_common = false;
    if let Ok(read) = std::fs::read_dir(root) {
        for e in read.flatten() {
            let Ok(kind) = e.file_type() else { continue };
            let Ok(name) = e.file_name().into_string() else {
                continue;
            };
            if !kind.is_dir() {
                continue;
            }
            if name == "Common" {
                has_common = true;
            } else if is_plain_version(&name) {
                let (a, b) = version_key(&name);
                versions.insert((a, b, name));
            }
        }
    }
    let ce_block = match &view.layout.version_folder {
        Some(v) if is_plain_version(v) => v.clone(),
        _ => game.to_owned(),
    };
    let (a, b) = version_key(&ce_block);
    versions.insert((a, b, ce_block.clone()));
    let on_disk: BTreeSet<String> = std::fs::read_dir(root)
        .map(|r| {
            r.flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    let base = |folder: Option<&str>| {
        let mut entries = vec![LoadEntry::root()];
        if has_common {
            entries.push(LoadEntry::dir("Common"));
        }
        if let Some(f) = folder.filter(|f| on_disk.contains(*f)) {
            entries.push(LoadEntry::dir(f));
        }
        entries
    };
    let mut spec = LoadFoldersSpec::new();
    for (_, _, key) in &versions {
        let mut entries = base(Some(key));
        if *key == ce_block {
            entries.push(LoadEntry::dir(view.layout.ce_dir()).if_active([paths::CE_PACKAGE_ID]));
        }
        spec.add_block(key, entries);
    }
    // the fallback block follows the newest block, as the designer's Combat Extended plan does
    let mut fallback = base(None);
    if versions.iter().all(|(a2, b2, _)| (*a2, *b2) <= (a, b)) {
        fallback.push(LoadEntry::dir(view.layout.ce_dir()).if_active([paths::CE_PACKAGE_ID]));
    }
    spec.add_block("default", fallback);
    load_folders::create(&spec, &RenderOpts::default())
}

/// Renames the `LoadFolders.xml` entries that name `from` to `to`, by span edits.
///
/// # Errors
///
/// A plain sentence when the text cannot be edited.
pub(crate) fn rename_entries(text: &str, from: &str, to: &str) -> Result<String, String> {
    let mut ed = SpanEditor::open(text).map_err(|e| format!("LoadFolders.xml: {e}"))?;
    let root = format!("/{}", ed.root_tag());
    let mut targets = Vec::new();
    for block in ed.children(&root).map_err(|e| e.to_string())? {
        for li in ed.children(&block.path).map_err(|e| e.to_string())? {
            if li.tag != "li" {
                continue;
            }
            let info = ed.element(&li.path).map_err(|e| e.to_string())?;
            if normal(&info.text).eq_ignore_ascii_case(&normal(from)) {
                targets.push(li.path);
            }
        }
    }
    for path in targets {
        ed.replace_text(&path, to).map_err(|e| e.to_string())?;
    }
    Ok(ed.into_text())
}

fn mentions_ce(node: &Node) -> bool {
    let hit = |s: &str| s.contains("CombatExtended") || s.contains("Combat Extended");
    node.attrs.iter().any(|(_, v)| hit(v))
        || node.tag.contains("CombatExtended")
        || node.elements().any(mentions_ce)
        || node.children.iter().filter_map(|c| c.as_text()).any(hit)
}

/// What a Combat Extended patch file holds: `Err` with the reason when it cannot be moved whole.
fn ce_patch_moves_whole(writer: &GuardedWriter, rel: &str) -> Result<(), String> {
    let read = match writer.read_limited(rel, MAX_REFERENCE_FILE_BYTES) {
        Ok(Some(r)) if !r.truncated => r,
        Ok(_) => return Err("the file is too large to check".to_owned()),
        Err(e) => return Err(format!("the file cannot be read: {e}")),
    };
    let doc = parse_document(&read.bytes, ParseMode::Game)
        .map_err(|e| format!("the file is not valid XML: {e}"))?;
    let others = doc
        .root
        .children_named("Operation")
        .filter(|op| !mentions_ce(op))
        .count();
    if others == 0 {
        Ok(())
    } else {
        Err(format!(
            "the file also holds {others} operation(s) that do not use Combat Extended; moving it would load them only when Combat Extended is active"
        ))
    }
}

fn short(parts: &[&str]) -> String {
    let mut hasher = blake3::Hasher::new();
    for p in parts {
        hasher.update(p.as_bytes());
        hasher.update(&[0]);
    }
    hasher.finalize().to_hex().chars().take(6).collect()
}

fn numbered(to: &str, n: u32) -> String {
    let (dir, name) = to.rsplit_once('/').map_or(("", to), |(d, n)| (d, n));
    let (stem, ext) = name.rsplit_once('.').map_or((name, ""), |(s, e)| (s, e));
    let file = if ext.is_empty() {
        format!("{stem}_{n}")
    } else {
        format!("{stem}_{n}.{ext}")
    };
    if dir.is_empty() {
        file
    } else {
        format!("{dir}/{file}")
    }
}

struct Ctx<'a> {
    view: &'a ProjectView,
    scan: &'a Scan,
    writer: &'a GuardedWriter,
    refs: RefIndex,
    claimed: BTreeSet<String>,
}

impl Ctx<'_> {
    fn taken(&self, rel: &str) -> bool {
        self.scan.has_ci(rel) || self.claimed.contains(&rel.to_ascii_lowercase())
    }

    /// Fills the destination, the conflict and the path checks of a move draft.
    fn place(&mut self, d: &mut Draft, folder: bool) {
        if let Some(reason) = container_change(&d.from, &d.to) {
            d.set_review(reason);
            return;
        }
        if let Err(e) = self.writer.path_of(&d.to) {
            d.set_review(format!("the destination is refused: {e}"));
            return;
        }
        if self.taken(&d.to) {
            d.conflict = true;
            if !folder {
                d.suggested = (2..2 + SUFFIX_TRIES)
                    .map(|n| numbered(&d.to, n))
                    .find(|c| !self.taken(c) && self.writer.path_of(c).is_ok());
            }
            if folder || d.suggested.is_none() {
                d.review_reason = Some(
                    "the destination exists and no free name is offered; move or rename it first"
                        .to_owned(),
                );
            }
        } else {
            self.claimed.insert(d.to.to_ascii_lowercase());
        }
    }

    fn references_for(&self, rel: &str) -> Vec<LayoutFixReferenceDto> {
        let mut needles = vec![rel.to_owned()];
        let lower = rel.to_ascii_lowercase();
        for area in ["textures/", "sounds/"] {
            if let Some(pos) = lower.find(area) {
                let below = rel.get(pos + area.len()..).unwrap_or("");
                let stem = below.rsplit_once('.').map_or(below, |(s, _)| s);
                needles.push(stem.to_owned());
            }
        }
        self.refs.find(&needles, rel)
    }
}

/// The game version folder a path lives in (`1.4`), `Common`, or `None` for the project root.
fn container_of(rel: &str) -> Option<&str> {
    let first = rel.split('/').next().unwrap_or("");
    (first == "Common" || is_plain_version(first)).then_some(first)
}

/// Why a move between two content folders changes what the game loads, `None` when it does not. The root and
/// `Common` load for every game version; a version folder loads for that version only.
fn container_change(from: &str, to: &str) -> Option<String> {
    let (a, b) = (container_of(from), container_of(to));
    fn version(c: Option<&str>) -> Option<&str> {
        c.filter(|c| *c != "Common")
    }
    if version(a) == version(b) {
        return None;
    }
    Some(match (version(a), version(b)) {
        (Some(x), Some(y)) => {
            format!("the file belongs to the game version folder {x} and the destination is in {y}")
        }
        (Some(x), None) => format!(
            "the file loads for game version {x} only and the destination loads for every version"
        ),
        (None, Some(y)) => {
            format!("the file loads for every game version and the destination loads for {y} only")
        }
        (None, None) => return None,
    })
}

fn is_texture_or_sound(rel: &str) -> bool {
    let lower = rel.to_ascii_lowercase();
    lower.starts_with("textures")
        || lower.starts_with("sounds")
        || lower.contains("/textures")
        || lower.contains("/sounds")
}

fn issue_for<'a>(issues: &'a [LayoutIssueDto], code: &str) -> Vec<&'a LayoutIssueDto> {
    issues.iter().filter(|i| i.code == code).collect()
}

fn weapon_drafts(ctx: &mut Ctx<'_>, issues: &[LayoutIssueDto], out: &mut Vec<Draft>) {
    use rimstudio_design::plan::LayoutProfile;
    let layout = &ctx.view.layout;
    let mut done: BTreeSet<String> = BTreeSet::new();
    for code in ["layout.weapon-misplaced", "layout.def-wrong-category"] {
        let static_code: &'static str = code;
        for issue in issue_for(issues, code) {
            if !done.insert(issue.path.clone()) {
                continue;
            }
            let Some(target) = issue.fix.targets.first().cloned() else {
                continue;
            };
            let Some(facts) = ctx.scan.facts.iter().find(|f| f.rel == issue.path) else {
                continue;
            };
            let mut d = Draft::new(
                LayoutFixItemKindDto::MoveFile,
                static_code,
                Op::MoveFile {
                    from: issue.path.clone(),
                    to: target.clone(),
                },
            );
            d.from = issue.path.clone();
            d.to = target.clone();
            d.why = issue.message.clone();
            let many = facts.weapons.len() > 1;
            let whole_file = match layout.profile {
                LayoutProfile::Flat => true,
                LayoutProfile::Rimstudio => !many,
                LayoutProfile::CoreStyle => {
                    // one shared category file: the whole file moves only when every weapon belongs there
                    let cats: BTreeSet<&str> = facts
                        .weapons
                        .iter()
                        .map(|w| super::check::expected_category(w).name())
                        .collect();
                    cats.len() == 1 && code == "layout.weapon-misplaced"
                }
            };
            if !whole_file {
                d = d.review(format!(
                    "the file defines {} weapons; moving one of them means splitting the file, which RimStudio does not do",
                    facts.weapons.len()
                ));
                out.push(d);
                continue;
            }
            if !load_folders_cover(ctx.view, &target) {
                d = d.review("LoadFolders.xml does not list the folder of the destination");
                out.push(d);
                continue;
            }
            let refs = ctx.references_for(&issue.path);
            if !refs.is_empty() {
                d.references = refs;
                d = d.review("other files mention the old path");
                out.push(d);
                continue;
            }
            ctx.place(&mut d, false);
            out.push(d);
        }
    }
}

fn ce_drafts(
    ctx: &mut Ctx<'_>,
    issues: &[LayoutIssueDto],
    out: &mut Vec<Draft>,
    gate_wanted: &mut Option<&'static str>,
) {
    let ce_dir = ctx.view.layout.ce_dir();
    let gated = gate_covers(ctx.view, &ce_dir);
    for issue in issue_for(issues, "layout.ce-outside-gate") {
        let Some(target) = issue.fix.targets.first().cloned() else {
            continue;
        };
        let mut d = Draft::new(
            LayoutFixItemKindDto::MoveFile,
            "layout.ce-outside-gate",
            Op::MoveFile {
                from: issue.path.clone(),
                to: target.clone(),
            },
        );
        d.from = issue.path.clone();
        d.to = target;
        d.why = format!(
            "{} uses Combat Extended classes outside the gated folder; it moves into {ce_dir}/Patches.",
            issue.path
        );
        if let Some(facts) = ctx.scan.facts.iter().find(|f| f.rel == issue.path)
            && let Some(why) = &facts.unparsable
        {
            d = d.review(format!("the file is not valid XML: {why}"));
            out.push(d);
            continue;
        }
        if let Err(reason) = ce_patch_moves_whole(ctx.writer, &issue.path) {
            d = d.review(reason);
            out.push(d);
            continue;
        }
        ctx.place(&mut d, false);
        if d.risk == LayoutFixRiskDto::Safe && !gated {
            d.needs_gate = true;
            *gate_wanted = Some("layout.ce-outside-gate");
        }
        out.push(d);
    }
    if !issue_for(issues, "layout.ce-folder-ungated").is_empty() && !gated {
        *gate_wanted = Some("layout.ce-folder-ungated");
    }
}

fn gate_draft(ctx: &Ctx<'_>, code: &'static str) -> Draft {
    let rel = load_folders_rel(ctx.view);
    let created = ctx.view.load_folders.is_none();
    let kind = if created {
        LayoutFixItemKindDto::CreateLoadFolders
    } else {
        LayoutFixItemKindDto::EditLoadFolders
    };
    let mut d = Draft::new(kind, code, Op::GateCe);
    d.to = rel.clone();
    d.why = format!(
        "{} lists {} so that it loads only when Combat Extended is active.",
        if created {
            "A new LoadFolders.xml"
        } else {
            "LoadFolders.xml"
        },
        ctx.view.layout.ce_dir()
    );
    if !created && ctx.scan.has_ci(&rel) && ctx.view.load_folders.is_none() {
        return d.review("LoadFolders.xml cannot be read");
    }
    match gate_text(ctx.view, ctx.view.root.as_std_path()) {
        Ok((old, new)) => {
            d.diff = Some(unified_diff(&rel, old.as_deref().unwrap_or(""), &new));
            if old.as_deref() == Some(new.as_str()) {
                return d.review("LoadFolders.xml already holds the entry");
            }
            d
        }
        Err(reason) => d.review(reason),
    }
}

fn legacy_drafts(
    ctx: &mut Ctx<'_>,
    issues: &[LayoutIssueDto],
    gate_first: bool,
    out: &mut Vec<Draft>,
    base_text: Option<&str>,
) {
    if issue_for(issues, "layout.ce-legacy-folder").is_empty() {
        return;
    }
    let from = ctx.view.layout.ce_dir();
    let to = ctx.view.layout.in_version(paths::CE_COMPAT_DIR);
    let mut rename = Draft::new(
        LayoutFixItemKindDto::MoveFolder,
        "layout.ce-legacy-folder",
        Op::MoveFolder {
            from: from.clone(),
            to: to.clone(),
        },
    );
    rename.from = from.clone();
    rename.to = to.clone();
    rename.why = format!(
        "The Combat Extended folder {from} moves to the standard name {to}; nothing in it changes."
    );
    // an unreadable LoadFolders.xml may name the old folder, so the folder stays where it is
    if ctx.view.load_folders.is_none() && ctx.scan.has_ci("LoadFolders.xml") {
        out.push(rename.review("LoadFolders.xml cannot be read, so its entry cannot be updated"));
        return;
    }
    if let Err(e) = ctx.writer.path_of(&to) {
        out.push(rename.review(format!("the destination is refused: {e}")));
        return;
    }
    if ctx.taken(&to) {
        rename.conflict = true;
        rename.review_reason = Some(
            "the standard folder exists; merging two folders is not done automatically".to_owned(),
        );
        out.push(rename);
        return;
    }
    ctx.claimed.insert(to.to_ascii_lowercase());
    let listed = ctx.view.load_folders.as_ref().is_some_and(|f| {
        f.root.elements().any(|b| {
            b.children_named("li")
                .any(|li| normal(&li.text_content()).eq_ignore_ascii_case(&normal(&from)))
        })
    });
    // the text the entry rename starts from: the file as it is, or as the gate item leaves it
    let start = base_text.map(str::to_owned);
    if listed || (gate_first && start.is_some()) {
        let mut edit = Draft::new(
            LayoutFixItemKindDto::EditLoadFolders,
            "layout.ce-legacy-folder",
            Op::RenameEntry {
                from: from.clone(),
                to: to.clone(),
            },
        );
        edit.to = load_folders_rel(ctx.view);
        edit.why = format!("The LoadFolders.xml entry {from} becomes {to}.");
        match start.as_deref().map(|t| rename_entries(t, &from, &to)) {
            Some(Ok(new)) => {
                edit.diff = Some(unified_diff(&edit.to, start.as_deref().unwrap_or(""), &new));
                edit.pairs_with_rename = true;
                rename.pairs_with_rename = true;
                out.push(rename);
                out.push(edit);
            }
            Some(Err(e)) => out.push(rename.review(e)),
            None => out.push(rename.review("LoadFolders.xml cannot be read")),
        }
    } else {
        out.push(rename);
    }
}

fn case_drafts(ctx: &mut Ctx<'_>, issues: &[LayoutIssueDto], out: &mut Vec<Draft>) {
    for issue in issue_for(issues, "layout.folder-case") {
        let Some(to) = issue.fix.targets.first().cloned() else {
            continue;
        };
        let mut d = Draft::new(
            LayoutFixItemKindDto::MoveFolder,
            "layout.folder-case",
            Op::MoveFolder {
                from: issue.path.clone(),
                to: to.clone(),
            },
        );
        d.from = issue.path.clone();
        d.to = to.clone();
        d.why = issue.message.clone();
        if is_texture_or_sound(&to) {
            d.references = ctx.references_for(&issue.path);
            d = d.review(
                "textures and sounds are referenced by path from definitions; rename the folder yourself and check the paths",
            );
            out.push(d);
            continue;
        }
        // another folder already has the exact name: a case twin that must be merged by hand
        if ctx.scan.entries.iter().any(|e| e.is_dir && e.rel == to) {
            d.conflict = true;
            d.review_reason = Some(
                "a folder with the correct name exists as well; merge them by hand".to_owned(),
            );
            out.push(d);
            continue;
        }
        out.push(d);
    }
}

/// Builds the full plan of a project: every fixable finding of the layout check, in the order the items are
/// carried out.
pub(crate) fn build(
    env: &ProjectEnv,
    project_id: &str,
    protected: &[Utf8PathBuf],
) -> ToolkitResult<Built> {
    let (record, view) = env.view(project_id)?;
    let scan = scan_project(&view);
    let issues = check(&view, &scan);
    let writer = env.writer(&view.root, record.id.as_str(), protected)?;
    let mut ctx = Ctx {
        view: &view,
        scan: &scan,
        writer: &writer,
        refs: RefIndex::build(&writer, &scan),
        claimed: BTreeSet::new(),
    };
    // 1. folders to create
    let mut creates = Vec::new();
    for issue in issue_for(&issues, "layout.missing-folder") {
        let Some(path) = issue.fix.targets.first() else {
            continue;
        };
        let mut d = Draft::new(
            LayoutFixItemKindDto::CreateFolder,
            "layout.missing-folder",
            Op::CreateFolder { path: path.clone() },
        );
        d.to = path.clone();
        d.why = issue.message.clone();
        if let Err(e) = writer.path_of(path) {
            d = d.review(format!("the folder is refused: {e}"));
        }
        creates.push(d);
    }
    // 2. moves of weapon files and of Combat Extended patches
    let mut weapons = Vec::new();
    weapon_drafts(&mut ctx, &issues, &mut weapons);
    let mut ce_moves = Vec::new();
    let mut gate_wanted = None;
    ce_drafts(&mut ctx, &issues, &mut ce_moves, &mut gate_wanted);
    // 3. the gate in LoadFolders.xml
    let mut gate = gate_wanted.map(|code| gate_draft(&ctx, code));
    // the text the later LoadFolders edit starts from
    let after_gate = match &gate {
        Some(g) if g.risk == LayoutFixRiskDto::Safe => gate_text(&view, view.root.as_std_path())
            .ok()
            .map(|(_, new)| new),
        _ => view.load_folders.as_ref().map(|f| f.text.clone()),
    };
    let mut legacy = Vec::new();
    legacy_drafts(
        &mut ctx,
        &issues,
        gate.as_ref()
            .is_some_and(|g| g.risk == LayoutFixRiskDto::Safe),
        &mut legacy,
        after_gate.as_deref(),
    );
    let mut cases = Vec::new();
    case_drafts(&mut ctx, &issues, &mut cases);
    // the CE patches only move when the gate item is applicable
    if gate
        .as_ref()
        .is_some_and(|g| g.risk != LayoutFixRiskDto::Safe)
    {
        for d in &mut ce_moves {
            if d.needs_gate && d.risk == LayoutFixRiskDto::Safe {
                let reason = gate
                    .as_ref()
                    .and_then(|g| g.review_reason.clone())
                    .unwrap_or_default();
                d.set_review(format!("the folder cannot be gated: {reason}"));
            }
        }
    }
    let mut ordered: Vec<Draft> = Vec::new();
    ordered.extend(creates);
    let gate_index = gate.take().map(|g| {
        ordered.push(g);
        ordered.len() - 1
    });
    ordered.extend(weapons);
    ordered.extend(ce_moves);
    ordered.extend(legacy);
    ordered.extend(cases);
    ordered.truncate(MAX_ITEMS);
    Ok(finish(&view, ordered, gate_index))
}

fn finish(view: &ProjectView, drafts: Vec<Draft>, gate_index: Option<usize>) -> Built {
    // ids first, so `requires` can point at them
    let ids: Vec<String> = drafts
        .iter()
        .enumerate()
        .map(|(i, d)| {
            format!(
                "fix-{:03}-{}",
                i + 1,
                short(&[&format!("{:?}", d.kind), &d.from, &d.to])
            )
        })
        .collect();
    let gate_id = gate_index.and_then(|i| ids.get(i).cloned());
    let rename_ids: Vec<(usize, String)> = drafts
        .iter()
        .enumerate()
        .filter(|(_, d)| d.pairs_with_rename)
        .map(|(i, _)| (i, ids.get(i).cloned().unwrap_or_default()))
        .collect();
    let mut hasher = blake3::Hasher::new();
    let mut items = Vec::new();
    for (i, d) in drafts.into_iter().enumerate() {
        let id = ids.get(i).cloned().unwrap_or_default();
        let mut requires = Vec::new();
        if d.needs_gate
            && let Some(g) = &gate_id
        {
            requires.push(g.clone());
        }
        if d.pairs_with_rename {
            requires.extend(
                rename_ids
                    .iter()
                    .filter(|(j, _)| *j != i)
                    .map(|(_, id)| id.clone()),
            );
        }
        let source_hash = match &d.op {
            Op::MoveFile { from, .. } | Op::MoveFolder { from, .. } => {
                hash_tree(view.root.join(from).as_std_path()).ok()
            }
            _ => None,
        };
        let applicable = d.risk == LayoutFixRiskDto::Safe
            && !(d.conflict && d.suggested.is_none())
            && !matches!(d.op, Op::Review);
        let conflict = d.conflict.then(|| LayoutFixConflictDto {
            destination_exists: true,
            suggested_to: d.suggested.clone(),
        });
        let review_reason = if applicable {
            None
        } else {
            d.review_reason.clone()
        };
        let dto = LayoutFixItemDto {
            id: id.clone(),
            kind: d.kind,
            issue_code: d.code.to_owned(),
            from: d.from.clone(),
            to: d.to.clone(),
            why: d.why.clone(),
            risk: d.risk,
            applicable,
            review_reason,
            diff: d.diff.clone(),
            conflict,
            references: d.references.clone(),
            requires,
        };
        hasher.update(serde_json::to_string(&dto).unwrap_or_default().as_bytes());
        hasher.update(source_hash.as_deref().unwrap_or("-").as_bytes());
        hasher.update(&[0]);
        items.push(PlanItem {
            dto,
            op: d.op,
            suggested_to: d.suggested,
        });
    }
    if let Some(f) = &view.load_folders {
        hasher.update(hash_text(&f.text).as_bytes());
    }
    let plan_id: String = hasher.finalize().to_hex().chars().take(32).collect();
    Built {
        plan_id,
        items,
        view: view.clone(),
    }
}

/// `project_layout_fix_plan`: the fixable findings of the layout check as a list of changes. Read only.
///
/// # Errors
///
/// [`crate::error::ToolkitError::ProjectNotOpen`] for an unknown project,
/// [`crate::error::ToolkitError::ProjectInvalid`] when the folder is no longer a mod, and
/// [`crate::error::ToolkitError::PathRefused`] when the project lies in a protected folder.
pub fn plan(
    env: &ProjectEnv,
    req: &ProjectLayoutFixPlanRequest,
    protected: &[Utf8PathBuf],
) -> ToolkitResult<ProjectLayoutFixPlanDto> {
    let built = build(env, &req.project_id, protected)?;
    let mut items: Vec<LayoutFixItemDto> = built.items.iter().map(|i| i.dto.clone()).collect();
    if let Some(codes) = &req.fixes {
        let keep: BTreeSet<&str> = codes.iter().map(String::as_str).collect();
        let wanted: BTreeSet<String> = items
            .iter()
            .filter(|i| keep.contains(i.issue_code.as_str()))
            .flat_map(|i| std::iter::once(i.id.clone()).chain(i.requires.iter().cloned()))
            .collect();
        items.retain(|i| wanted.contains(&i.id));
    }
    let count = |f: &dyn Fn(&LayoutFixItemDto) -> bool| {
        u32::try_from(items.iter().filter(|i| f(i)).count()).unwrap_or(u32::MAX)
    };
    Ok(ProjectLayoutFixPlanDto {
        project_id: req.project_id.clone(),
        plan_id: built.plan_id,
        safe: count(&|i| i.applicable),
        needs_review: count(&|i| i.risk == LayoutFixRiskDto::NeedsReview),
        conflicts: count(&|i| i.conflict.is_some()),
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbered_names_keep_the_extension() {
        assert_eq!(numbered("a/b/c.xml", 2), "a/b/c_2.xml");
        assert_eq!(numbered("c.xml", 3), "c_3.xml");
        assert_eq!(numbered("a/Folder", 2), "a/Folder_2");
    }

    #[test]
    fn entries_are_renamed_by_span_edits() {
        let text = "<loadFolders>\n  <v1.6>\n    <li>/</li>\n    <li IfModActive=\"ceteam.combatextended\">CE</li>\n    <li>Other</li>\n  </v1.6>\n  <default>\n    <li>\\CE\\</li>\n  </default>\n</loadFolders>\n";
        let out = rename_entries(text, "CE", "Compat/CombatExtended").unwrap();
        assert_eq!(out.matches("Compat/CombatExtended").count(), 2);
        assert!(out.contains("<li>Other</li>"));
        assert_eq!(rename_entries(&out, "CE", "x").unwrap(), out);
    }

    #[test]
    fn a_patch_operation_that_names_combat_extended_is_one() {
        let doc = parse_document(
            br#"<Patch><Operation Class="PatchOperationAdd"><value><li Class="CombatExtended.ToolCE"/></value></Operation>
                <Operation Class="PatchOperationAdd"><xpath>/Defs/ThingDef</xpath></Operation></Patch>"#,
            ParseMode::Game,
        )
        .unwrap();
        let ops: Vec<bool> = doc
            .root
            .children_named("Operation")
            .map(mentions_ce)
            .collect();
        assert_eq!(ops, [true, false]);
    }

    #[test]
    fn a_move_may_not_change_the_game_versions_a_file_loads_for() {
        assert_eq!(
            container_change("Patches/a.xml", "Common/Compat/a.xml"),
            None
        );
        assert_eq!(
            container_change("1.4/Patches/a.xml", "1.4/Compat/a.xml"),
            None
        );
        assert_eq!(container_change("Common/Defs/a.xml", "Defs/a.xml"), None);
        assert!(
            container_change("1.2/Patches/a.xml", "1.4/Compat/a.xml")
                .unwrap()
                .contains("1.2")
        );
        assert!(container_change("Patches/a.xml", "1.4/Compat/a.xml").is_some());
        assert!(container_change("1.4/Patches/a.xml", "Compat/a.xml").is_some());
    }

    #[test]
    fn version_names_are_plain_numbers() {
        assert!(is_plain_version("1.6"));
        assert!(!is_plain_version("1.6.2"));
        assert!(!is_plain_version("Common"));
        assert_eq!(version_key("1.10"), (1, 10));
    }
}
