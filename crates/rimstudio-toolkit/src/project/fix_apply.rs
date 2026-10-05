//! `project_layout_fix_apply`: carries out the selected items of a fix plan.
//!
//! The plan is built again from the disk and its id compared with the one the caller reviewed, so a file
//! that changed since is refused (`designer.plan-stale`). Items are carried out in plan order. Every path of
//! every item goes through the guarded writer (inside the project root, no link that leaves it, outside the
//! protected folders, no case twin, safe names on every platform); a move never overwrites; the previous
//! content of `LoadFolders.xml` is backed up in the data root before it is replaced.
//!
//! An undo journal is written to the data root before the first change and rewritten before each edit and
//! after each item, so an interruption at any point leaves a journal that describes exactly what can be
//! reversed (see [`super::journal`]). The job observes cancellation between items; items done stay done and
//! can be undone. Afterwards the layout check runs again.

use std::collections::{BTreeMap, BTreeSet};

use camino::Utf8PathBuf;
use rimstudio_core::jobs::{CancelToken, Progress, ProgressSink, ProgressUnit};
use rimstudio_io::guard::{RootGuard, sanitize_name};
use rimstudio_io::rename::move_path;
use rimstudio_ipc_types::project::ProjectLayoutCheckRequest;
use rimstudio_ipc_types::project_fix::{
    LayoutFixDoneDto, LayoutFixEditedDto, LayoutFixItemKindDto, LayoutFixSkippedDto,
    ProjectLayoutFixApplyDto, ProjectLayoutFixApplyRequest,
};

use super::fix::{Op, PlanItem, build, gate_text, load_folders_rel, rename_entries};
use super::journal::{
    JOURNAL_SCHEMA, Journal, JournalItem, JournalKind, JournalState, JournalStatus,
    exact_entry_exists, hash_text, hash_tree, journal_path, write_journal,
};
use crate::error::{ToolkitError, ToolkitResult};
use crate::shared::env::ProjectEnv;
use crate::shared::writer::{Expect, GuardedWriter};

/// Test seams of an apply: a way to stop the run at the worst moment, as a crash would.
#[derive(Default)]
pub struct ApplyHooks<'a> {
    /// Called after item number `i` (zero based, among the carried out items) has changed the disk and
    /// before the journal records it. Returning `false` ends the run at once with an error, writing
    /// nothing more.
    pub after_change: Option<&'a dyn Fn(usize) -> bool>,
}

/// True when two paths share their folder and differ only by the letter case of the last name.
pub(crate) fn is_case_only(from: &str, to: &str) -> bool {
    let split = |p: &str| {
        p.rsplit_once('/')
            .map_or((String::new(), p.to_owned()), |(d, n)| {
                (d.to_owned(), n.to_owned())
            })
    };
    let ((fd, fname), (td, tname)) = (split(from), split(to));
    fd == td && fname != tname && fname.to_lowercase() == tname.to_lowercase()
}

/// Checks a source and a destination for a move through the writer. A destination that differs from the
/// source only by letter case is checked by its text, because the writer refuses a case twin.
pub(crate) fn check_move_paths(
    writer: &GuardedWriter,
    from: &str,
    to: &str,
) -> ToolkitResult<(Utf8PathBuf, Utf8PathBuf)> {
    let from_abs = writer.path_of(from)?;
    if is_case_only(from, to) {
        if !rimstudio_design::plan::is_safe_relative_path(to) {
            return Err(ToolkitError::PathRefused {
                path: to.chars().take(200).collect(),
                reason: "not a safe relative path".to_owned(),
            });
        }
        for segment in to.split('/') {
            sanitize_name(segment).map_err(|e| ToolkitError::PathRefused {
                path: to.chars().take(200).collect(),
                reason: e.to_string(),
            })?;
        }
        let to_abs = writer.root().join(to);
        return Ok((from_abs, to_abs));
    }
    let to_abs = writer.path_of(to)?;
    Ok((from_abs, to_abs))
}

fn journal_kind(kind: LayoutFixItemKindDto) -> JournalKind {
    match kind {
        LayoutFixItemKindDto::MoveFile => JournalKind::MoveFile,
        LayoutFixItemKindDto::MoveFolder => JournalKind::MoveFolder,
        LayoutFixItemKindDto::CreateFolder => JournalKind::CreateFolder,
        LayoutFixItemKindDto::EditLoadFolders => JournalKind::EditLoadFolders,
        LayoutFixItemKindDto::CreateLoadFolders => JournalKind::CreateLoadFolders,
    }
}

fn new_apply_id(env: &ProjectEnv, project_id: &str, plan_id: &str, ms: u64) -> String {
    let mut n = 0u32;
    loop {
        let hash = hash_text(&format!("{plan_id}:{ms}:{n}"));
        let id = format!("fix-{ms}-{}", hash.get(..8).unwrap_or("00000000"));
        if !journal_path(env, project_id, &id).as_std_path().exists() {
            return id;
        }
        n += 1;
    }
}

/// One chosen item: the plan item, the destination to use and its place in the plan.
struct Chosen<'a> {
    item: &'a PlanItem,
    to: String,
}

fn choose<'a>(
    built: &'a [PlanItem],
    req: &ProjectLayoutFixApplyRequest,
    skipped: &mut Vec<LayoutFixSkippedDto>,
) -> BTreeMap<usize, Chosen<'a>> {
    let mut chosen: BTreeMap<usize, Chosen<'a>> = BTreeMap::new();
    let skip = |skipped: &mut Vec<LayoutFixSkippedDto>, id: &str, reason: String| {
        skipped.push(LayoutFixSkippedDto {
            id: id.to_owned(),
            reason,
        });
    };
    for sel in &req.items {
        let Some((idx, item)) = built.iter().enumerate().find(|(_, i)| i.dto.id == sel.id) else {
            skip(skipped, &sel.id, "the item is not in the plan".to_owned());
            continue;
        };
        if chosen.contains_key(&idx) {
            continue;
        }
        if !item.dto.applicable {
            let why = item
                .dto
                .review_reason
                .clone()
                .unwrap_or_else(|| "the item needs review and is not applied".to_owned());
            skip(skipped, &sel.id, why);
            continue;
        }
        let mut to = item.dto.to.clone();
        if let Some(conflict) = &item.dto.conflict {
            match (&item.suggested_to, sel.rename_on_conflict) {
                (Some(other), true) => to.clone_from(other),
                (Some(other), false) => {
                    skip(
                        skipped,
                        &sel.id,
                        format!(
                            "the destination {} exists; select the numbered name {other} to move it anyway",
                            item.dto.to
                        ),
                    );
                    continue;
                }
                (None, _) => {
                    let _ = conflict;
                    skip(
                        skipped,
                        &sel.id,
                        format!("the destination {} exists", item.dto.to),
                    );
                    continue;
                }
            }
        }
        chosen.insert(idx, Chosen { item, to });
    }
    // an item that needs another item is dropped when the other is not selected
    loop {
        let missing: Vec<(usize, String)> = chosen
            .iter()
            .filter_map(|(idx, c)| {
                c.item
                    .dto
                    .requires
                    .iter()
                    .find(|r| {
                        !built
                            .iter()
                            .enumerate()
                            .any(|(j, other)| other.dto.id == **r && chosen.contains_key(&j))
                    })
                    .map(|r| (*idx, r.clone()))
            })
            .collect();
        if missing.is_empty() {
            break;
        }
        for (idx, needed) in missing {
            if let Some(c) = chosen.remove(&idx) {
                skip(
                    skipped,
                    &c.item.dto.id,
                    format!("it is applied together with {needed}, which was not selected"),
                );
            }
        }
    }
    chosen
}

fn new_item(c: &Chosen<'_>, root: &std::path::Path, writer: &GuardedWriter) -> JournalItem {
    let (kind, from, to) = match &c.item.op {
        Op::CreateFolder { path } => (JournalKind::CreateFolder, String::new(), path.clone()),
        Op::MoveFile { from, .. } => (JournalKind::MoveFile, from.clone(), c.to.clone()),
        Op::MoveFolder { from, .. } => (JournalKind::MoveFolder, from.clone(), c.to.clone()),
        Op::GateCe | Op::RenameEntry { .. } => (
            journal_kind(c.item.dto.kind),
            String::new(),
            c.item.dto.to.clone(),
        ),
        Op::Review => (journal_kind(c.item.dto.kind), String::new(), c.to.clone()),
    };
    let mut item = JournalItem {
        id: c.item.dto.id.clone(),
        kind,
        from,
        to,
        status: JournalStatus::Pending,
        hash: None,
        before_text: None,
        before_hash: None,
        after_hash: None,
        created_top: None,
        copied: false,
        reason: None,
    };
    if matches!(kind, JournalKind::MoveFile | JournalKind::MoveFolder) {
        item.hash = writer
            .path_of(&item.from)
            .ok()
            .and_then(|abs| hash_tree(abs.as_std_path()).ok());
    }
    // the topmost folder this item will create on the way, so an undo can remove the chain again
    let missing_top = |rel: &str| {
        let mut top = None;
        let mut probe = rel.to_owned();
        while let Some((parent, _)) = probe.rsplit_once('/') {
            if exact_entry_exists(&root.join(parent)) {
                break;
            }
            top = Some(parent.to_owned());
            probe = parent.to_owned();
        }
        top
    };
    match kind {
        JournalKind::CreateFolder => {
            item.from = missing_top(&item.to).unwrap_or_else(|| item.to.clone());
        }
        JournalKind::MoveFile | JournalKind::MoveFolder => {
            item.created_top = missing_top(&item.to);
        }
        _ => {}
    }
    item
}

/// What carrying out one item did.
enum Outcome {
    Done {
        copied: bool,
        edited: Option<LayoutFixEditedDto>,
    },
    Skipped(String),
}

struct Runner<'a> {
    env: &'a ProjectEnv,
    writer: &'a GuardedWriter,
    guard: RootGuard,
    view: &'a crate::shared::projectfs::ProjectView,
}

impl Runner<'_> {
    fn run(&self, op: &Op, ji: &mut JournalItem, journal: &mut Journal) -> ToolkitResult<Outcome> {
        match op {
            Op::CreateFolder { path } => {
                if exact_entry_exists(self.writer.root().join(path).as_std_path()) {
                    return Ok(Outcome::Skipped("the folder already exists".to_owned()));
                }
                self.writer.make_dir(path)?;
                Ok(Outcome::Done {
                    copied: false,
                    edited: None,
                })
            }
            Op::MoveFile { .. } | Op::MoveFolder { .. } => {
                let (from_abs, to_abs) = check_move_paths(self.writer, &ji.from, &ji.to)?;
                let now =
                    hash_tree(from_abs.as_std_path()).map_err(|e| ToolkitError::ApplyRefused {
                        reason: format!("{} cannot be read: {e}", ji.from),
                    })?;
                if matches!(op, Op::MoveFolder { .. }) {
                    // earlier items may have put files into the folder: what is moved is what is there now,
                    // and the journal knows it before the move
                    ji.hash = Some(now);
                    if let Some(slot) = journal.items.iter_mut().find(|i| i.id == ji.id) {
                        slot.hash.clone_from(&ji.hash);
                    }
                    write_journal(self.env, journal)?;
                } else if ji.hash.as_deref() != Some(now.as_str()) {
                    return Err(ToolkitError::ApplyRefused {
                        reason: format!("{} changed while the fix was applied", ji.from),
                    });
                }
                let moved = move_path(&self.guard, &from_abs, &to_abs).map_err(|e| {
                    ToolkitError::PathRefused {
                        path: ji.to.chars().take(200).collect(),
                        reason: e.to_string(),
                    }
                })?;
                Ok(Outcome::Done {
                    copied: moved.copied,
                    edited: None,
                })
            }
            Op::GateCe => {
                let rel = load_folders_rel(self.view);
                let (old, new) = gate_text(self.view, self.view.root.as_std_path())
                    .map_err(|reason| ToolkitError::ApplyRefused { reason })?;
                if old.as_deref() == Some(new.as_str()) {
                    return Ok(Outcome::Skipped(
                        "LoadFolders.xml already holds the entry".to_owned(),
                    ));
                }
                // read again: the text must be the one the gate was computed from
                let current = self.writer.read_text(&rel)?;
                if current != old {
                    return Err(ToolkitError::ApplyRefused {
                        reason: format!(
                            "{rel} changed on disk since the plan was made; plan again"
                        ),
                    });
                }
                self.edit(&rel, old.as_deref(), &new, ji, journal)
            }
            Op::RenameEntry { from, to } => {
                let rel = load_folders_rel(self.view);
                let Some(current) = self.writer.read_text(&rel)? else {
                    return Ok(Outcome::Skipped(
                        "LoadFolders.xml does not exist".to_owned(),
                    ));
                };
                let new = rename_entries(&current, from, to)
                    .map_err(|reason| ToolkitError::ApplyRefused { reason })?;
                if new == current {
                    return Ok(Outcome::Skipped(
                        "LoadFolders.xml has no such entry".to_owned(),
                    ));
                }
                self.edit(&rel, Some(&current), &new, ji, journal)
            }
            Op::Review => Ok(Outcome::Skipped("the item needs review".to_owned())),
        }
    }

    /// Records the edit in the journal, then writes the file with a backup.
    fn edit(
        &self,
        rel: &str,
        old: Option<&str>,
        new: &str,
        ji: &mut JournalItem,
        journal: &mut Journal,
    ) -> ToolkitResult<Outcome> {
        self.writer.preflight(rel, Expect::Previous(old))?;
        ji.to = rel.to_owned();
        ji.before_text = old.map(str::to_owned);
        ji.before_hash = old.map(hash_text);
        ji.after_hash = Some(hash_text(new));
        // the journal knows the edit before the file does
        if let Some(slot) = journal.items.iter_mut().find(|i| i.id == ji.id) {
            *slot = ji.clone();
        }
        write_journal(self.env, journal)?;
        let report = self
            .writer
            .write_expecting(rel, new, old.is_some(), Expect::Previous(old))?;
        Ok(Outcome::Done {
            copied: false,
            edited: Some(LayoutFixEditedDto {
                path: rel.to_owned(),
                created: old.is_none(),
                backup_path: report.backup.map(|p| p.to_string()),
            }),
        })
    }
}

/// `project_layout_fix_apply` (job): see the module documentation.
///
/// # Errors
///
/// [`ToolkitError::PlanStale`] when the plan changed, [`ToolkitError::Cancelled`] when cancelled before the
/// journal exists, [`ToolkitError::PathRefused`] for a project in a protected folder, store errors for the
/// journal. A failing item is not an error: it is reported as skipped and the others continue.
pub fn apply(
    env: &ProjectEnv,
    req: &ProjectLayoutFixApplyRequest,
    protected: &[Utf8PathBuf],
    progress: &dyn ProgressSink,
    cancel: &CancelToken,
) -> ToolkitResult<ProjectLayoutFixApplyDto> {
    apply_with(
        env,
        req,
        protected,
        progress,
        cancel,
        &ApplyHooks::default(),
    )
}

/// [`apply`] with test hooks.
///
/// # Errors
///
/// See [`apply`]; a hook that returns `false` ends the run with [`ToolkitError::Internal`].
pub fn apply_with(
    env: &ProjectEnv,
    req: &ProjectLayoutFixApplyRequest,
    protected: &[Utf8PathBuf],
    progress: &dyn ProgressSink,
    cancel: &CancelToken,
    hooks: &ApplyHooks<'_>,
) -> ToolkitResult<ProjectLayoutFixApplyDto> {
    let built = build(env, &req.project_id, protected)?;
    if built.plan_id != req.plan_id {
        return Err(ToolkitError::PlanStale {
            expected: req.plan_id.chars().take(64).collect(),
            found: built.plan_id,
        });
    }
    cancel.check()?;
    let view = &built.view;
    let writer = env.writer(&view.root, &req.project_id, protected)?;
    let guard = RootGuard::new([view.root.clone()])?;
    let mut skipped = Vec::new();
    let chosen = choose(&built.items, req, &mut skipped);
    let initial: Vec<JournalItem> = chosen
        .values()
        .map(|c| new_item(c, view.root.as_std_path(), &writer))
        .collect();
    let order: Vec<usize> = chosen.keys().copied().collect();
    let ms = env.clock().now_unix_ms();
    let apply_id = new_apply_id(env, &req.project_id, &built.plan_id, ms);
    let mut journal = Journal {
        schema: JOURNAL_SCHEMA,
        apply_id: apply_id.clone(),
        project_id: req.project_id.clone(),
        project_root: view.root.to_string(),
        plan_id: built.plan_id.clone(),
        created_ms: ms,
        undone_ms: None,
        state: JournalState::Started,
        items: initial,
        checksum: String::new(),
    };
    let mut done = Vec::new();
    let mut edited = Vec::new();
    let mut cancelled = false;
    if !journal.items.is_empty() {
        write_journal(env, &journal)?;
        let runner = Runner {
            env,
            writer: &writer,
            guard,
            view,
        };
        let total = order.len() as u64;
        let mut finished: BTreeSet<String> = BTreeSet::new();
        for (n, idx) in order.iter().enumerate() {
            let Some(c) = chosen.get(idx) else { continue };
            if cancel.is_cancelled() {
                cancelled = true;
                break;
            }
            progress.report(
                Progress::new("project.layout-fix", n as u64)
                    .with_total(total)
                    .with_unit(ProgressUnit::Files)
                    .with_detail(c.item.dto.id.clone()),
            );
            let Some(mut ji) = journal.items.get(n).cloned() else {
                continue;
            };
            // an item whose partner failed is not applied
            let earlier: BTreeSet<&str> = order
                .iter()
                .take(n)
                .filter_map(|j| chosen.get(j).map(|c| c.item.dto.id.as_str()))
                .collect();
            if let Some(missing) = c
                .item
                .dto
                .requires
                .iter()
                .find(|r| earlier.contains(r.as_str()) && !finished.contains(r.as_str()))
            {
                set_status(
                    &mut journal,
                    n,
                    JournalStatus::Skipped,
                    Some(format!("{missing} was not applied")),
                );
                skipped.push(LayoutFixSkippedDto {
                    id: ji.id.clone(),
                    reason: format!("{missing} was not applied"),
                });
                write_journal(env, &journal)?;
                continue;
            }
            match runner.run(&c.item.op, &mut ji, &mut journal) {
                Ok(Outcome::Done { copied, edited: e }) => {
                    if let Some(hook) = hooks.after_change
                        && !hook(n)
                    {
                        return Err(ToolkitError::internal(
                            "the apply was stopped by a test hook",
                        ));
                    }
                    ji.status = JournalStatus::Done;
                    ji.copied = copied;
                    if let Some(slot) = journal.items.get_mut(n) {
                        *slot = ji.clone();
                    }
                    write_journal(env, &journal)?;
                    finished.insert(ji.id.clone());
                    done.push(LayoutFixDoneDto {
                        id: ji.id.clone(),
                        kind: c.item.dto.kind,
                        from: ji.from.clone(),
                        to: ji.to.clone(),
                        copied,
                    });
                    edited.extend(e);
                }
                Ok(Outcome::Skipped(reason)) => {
                    set_status(
                        &mut journal,
                        n,
                        JournalStatus::Skipped,
                        Some(reason.clone()),
                    );
                    write_journal(env, &journal)?;
                    skipped.push(LayoutFixSkippedDto {
                        id: ji.id.clone(),
                        reason,
                    });
                }
                Err(e) => {
                    let reason = e.to_string();
                    // an interrupted edit may have reached the disk; the journal keeps its record
                    set_status(&mut journal, n, JournalStatus::Failed, Some(reason.clone()));
                    write_journal(env, &journal)?;
                    skipped.push(LayoutFixSkippedDto {
                        id: ji.id.clone(),
                        reason,
                    });
                }
            }
        }
        journal.state = if cancelled {
            JournalState::Cancelled
        } else {
            JournalState::Complete
        };
        write_journal(env, &journal)?;
    }
    let check = super::layout_check(
        env,
        &ProjectLayoutCheckRequest {
            project_id: req.project_id.clone(),
        },
    )?;
    Ok(ProjectLayoutFixApplyDto {
        project_id: req.project_id.clone(),
        apply_id,
        plan_id: built.plan_id,
        done,
        edited,
        skipped,
        cancelled,
        check,
    })
}

fn set_status(journal: &mut Journal, n: usize, status: JournalStatus, reason: Option<String>) {
    if let Some(slot) = journal.items.get_mut(n) {
        slot.status = status;
        slot.reason = reason;
    }
}
