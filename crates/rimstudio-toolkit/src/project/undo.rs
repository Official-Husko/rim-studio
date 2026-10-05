//! `project_layout_fix_undo`: reverses an apply from its journal.
//!
//! The undo first checks everything and only then changes anything. It refuses, with a sentence that names
//! the path, when the journal does not hold (checksum, project, paths), when a moved path no longer holds
//! the bytes the journal recorded, when something exists at the place a file would return to, or when
//! `LoadFolders.xml` was edited again. The check also copes with an apply that was interrupted: an item the
//! journal still lists as pending counts as done when its destination holds the recorded bytes and its
//! source is gone.
//!
//! Items are reversed from the last to the first. A folder that an apply moved after other items had put
//! files into it is found at its new place when the earlier items are checked, and moved back first.

use camino::Utf8PathBuf;
use rimstudio_io::guard::RootGuard;
use rimstudio_io::rename::{move_path, remove_empty_dir, remove_file};
use rimstudio_ipc_types::project::ProjectLayoutCheckRequest;
use rimstudio_ipc_types::project_fix::{ProjectLayoutFixUndoDto, ProjectLayoutFixUndoRequest};

use super::fix_apply::{check_move_paths, is_case_only};
use super::journal::{
    Journal, JournalItem, JournalKind, JournalState, JournalStatus, exact_entry_exists, hash_text,
    hash_tree, read_journal, write_journal,
};
use crate::error::{ToolkitError, ToolkitResult};
use crate::shared::env::ProjectEnv;
use crate::shared::writer::{Expect, GuardedWriter};

/// One reversal, in the order it is carried out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
    /// Move `to` back to `from`; `hash` is what must be found.
    MoveBack {
        from: String,
        to: String,
        hash: String,
        folder: bool,
        /// The topmost folder the move created, removed again (while empty) after the move back.
        created_top: Option<String>,
    },
    /// Put the previous text back; `now` is the hash of the text that must be there.
    Restore {
        rel: String,
        text: String,
        now: String,
    },
    /// Delete the created file; `now` is the hash it must have.
    RemoveFile { rel: String, now: String },
    /// Remove the folders the apply created, from `to` up to `top`, as long as they are empty.
    RemoveFolders { to: String, top: String },
}

fn refuse(reason: impl Into<String>) -> ToolkitError {
    ToolkitError::FixUndoRefused {
        reason: reason.into(),
    }
}

fn damaged(journal: &Journal, reason: impl Into<String>) -> ToolkitError {
    ToolkitError::FixJournalDamaged {
        id: journal.apply_id.clone(),
        reason: reason.into(),
    }
}

/// Replaces the leading `from` of `path` by `to` (whole segments, letter case ignored).
fn rebase(path: &str, from: &str, to: &str) -> Option<String> {
    let (lp, lf) = (path.to_lowercase(), from.to_lowercase());
    if lp == lf {
        return Some(to.to_owned());
    }
    let rest = lp.strip_prefix(&format!("{lf}/"))?;
    path.get(path.len() - rest.len()..)
        .map(|tail| format!("{to}/{tail}"))
}

fn current_text_hash(writer: &GuardedWriter, rel: &str) -> ToolkitResult<Option<String>> {
    let abs = writer.path_of(rel)?;
    match std::fs::read(abs.as_std_path()) {
        Ok(bytes) => Ok(Some(blake3::hash(&bytes).to_hex().to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(refuse(format!("{rel} cannot be read: {e}"))),
    }
}

fn is_load_folders_name(rel: &str) -> bool {
    !rel.contains('/') && rel.eq_ignore_ascii_case("LoadFolders.xml")
}

/// Checks a journal against the disk and returns the steps of the undo, or the reason it cannot be undone.
pub(crate) fn preflight(journal: &Journal, writer: &GuardedWriter) -> ToolkitResult<Vec<Step>> {
    if journal.state == JournalState::Undone {
        return Err(refuse("this apply was already undone"));
    }
    if journal.project_root != writer.root().as_str() {
        return Err(damaged(
            journal,
            "the journal belongs to another project folder",
        ));
    }
    let mut steps = Vec::new();
    // folder moves undone so far (processing runs from the last item to the first)
    let mut later: Vec<(String, String)> = Vec::new();
    // the state of LoadFolders.xml as it will be after the steps so far
    let mut lf_state: Option<Option<String>> = None;
    for item in journal.items.iter().rev() {
        if item.status == JournalStatus::Skipped {
            continue;
        }
        match item.kind {
            JournalKind::MoveFile | JournalKind::MoveFolder => {
                if let Some(step) = move_step(journal, item, writer, &later)? {
                    if let Step::MoveBack {
                        from, to, folder, ..
                    } = &step
                        && *folder
                    {
                        later.push((from.clone(), to.clone()));
                    }
                    steps.push(step);
                }
            }
            JournalKind::CreateFolder => {
                if !rimstudio_design::plan::is_safe_relative_path(&item.to)
                    || !rimstudio_design::plan::is_safe_relative_path(&item.from)
                    || rebase(&item.to, &item.from, "").is_none()
                {
                    return Err(damaged(
                        journal,
                        format!("the folder {} is not valid", item.to),
                    ));
                }
                writer.path_of(&item.to)?;
                // an apply that stopped before it recorded the folder may still have made it; an empty
                // folder at this place is removed, anything inside keeps it
                if matches!(item.status, JournalStatus::Done | JournalStatus::Pending) {
                    steps.push(Step::RemoveFolders {
                        to: item.to.clone(),
                        top: item.from.clone(),
                    });
                }
            }
            JournalKind::EditLoadFolders | JournalKind::CreateLoadFolders => {
                if !is_load_folders_name(&item.to) {
                    return Err(damaged(
                        journal,
                        format!("{} is not LoadFolders.xml", item.to),
                    ));
                }
                let now = match &lf_state {
                    Some(state) => state.clone(),
                    None => current_text_hash(writer, &item.to)?,
                };
                let after = item.after_hash.clone();
                if item.after_hash.is_none() {
                    // never written: the journal was cut before the edit
                    lf_state = Some(now);
                    continue;
                }
                let applied = now.is_some() && now == after;
                if applied {
                    match item.kind {
                        JournalKind::EditLoadFolders => {
                            let Some(text) = &item.before_text else {
                                return Err(damaged(journal, "an edit has no previous text"));
                            };
                            if item.before_hash.as_deref() != Some(hash_text(text).as_str()) {
                                return Err(damaged(
                                    journal,
                                    "the previous text does not match its hash",
                                ));
                            }
                            steps.push(Step::Restore {
                                rel: item.to.clone(),
                                text: text.clone(),
                                now: now.clone().unwrap_or_default(),
                            });
                            lf_state = Some(item.before_hash.clone());
                        }
                        _ => {
                            steps.push(Step::RemoveFile {
                                rel: item.to.clone(),
                                now: now.clone().unwrap_or_default(),
                            });
                            lf_state = Some(None);
                        }
                    }
                } else if now == item.before_hash
                    || (item.kind == JournalKind::CreateLoadFolders && now.is_none())
                {
                    // the edit never reached the disk, or the created file is gone already
                    lf_state = Some(now);
                } else {
                    return Err(refuse(format!(
                        "{} was changed after the fix; undo it by hand or restore the backup",
                        item.to
                    )));
                }
            }
        }
    }
    Ok(steps)
}

fn move_step(
    journal: &Journal,
    item: &JournalItem,
    writer: &GuardedWriter,
    later: &[(String, String)],
) -> ToolkitResult<Option<Step>> {
    let Some(expected) = item.hash.clone() else {
        return Err(damaged(
            journal,
            format!("{} has no recorded hash", item.to),
        ));
    };
    let (from_abs, to_abs) = check_move_paths(writer, &item.to, &item.from)
        .map(|(a, b)| (b, a))
        .map_err(|e| damaged(journal, e.to_string()))?;
    let _ = from_abs;
    // where the destination lives now: a folder moved later took it along
    let mut loc = item.to.clone();
    for (a, b) in later.iter().rev() {
        if let Some(moved) = rebase(&loc, a, b) {
            loc = moved;
        }
    }
    let loc_abs = if loc == item.to {
        to_abs
    } else {
        writer.root().join(&loc)
    };
    let from_present = exact_entry_exists(writer.root().join(&item.from).as_std_path());
    let at_destination = exact_entry_exists(loc_abs.as_std_path());
    let folder = item.kind == JournalKind::MoveFolder;
    if at_destination {
        let found = hash_tree(loc_abs.as_std_path())
            .map_err(|e| refuse(format!("{loc} cannot be read: {e}")))?;
        if found == expected {
            if from_present && !is_case_only(&item.from, &item.to) {
                return Err(refuse(format!(
                    "{} exists again at the original place; move or remove it first",
                    item.from
                )));
            }
            return Ok(Some(Step::MoveBack {
                from: item.from.clone(),
                to: item.to.clone(),
                hash: expected,
                folder,
                created_top: item.created_top.clone(),
            }));
        }
        if item.status == JournalStatus::Done || !from_present {
            return Err(refuse(format!(
                "{loc} was changed after the fix; its content is no longer what was moved"
            )));
        }
    } else if item.status == JournalStatus::Done {
        return Err(refuse(format!(
            "{loc} is no longer there; it was moved or deleted after the fix"
        )));
    }
    // the move did not happen (pending or failed): the source is where it was and there is nothing to
    // reverse. Its content is not compared: an earlier item of the same apply may have changed a folder.
    if from_present {
        return Ok(None);
    }
    Err(refuse(format!(
        "neither {} nor {} holds the moved content",
        item.from, item.to
    )))
}

/// Removes the folders from `start` up to `top` (inclusive) as long as they exist and are empty.
fn remove_chain(
    writer: &GuardedWriter,
    guard: &RootGuard,
    start: &str,
    top: &str,
    removed: &mut Vec<String>,
) -> ToolkitResult<()> {
    let mut current = start.to_owned();
    loop {
        let abs = writer.path_of(&current)?;
        if !exact_entry_exists(abs.as_std_path()) {
            break;
        }
        let gone = remove_empty_dir(guard, &abs).map_err(|e| ToolkitError::PathRefused {
            path: current.chars().take(200).collect(),
            reason: e.to_string(),
        })?;
        if !gone {
            break;
        }
        removed.push(current.clone());
        if current.eq_ignore_ascii_case(top) {
            break;
        }
        let Some((parent, _)) = current.rsplit_once('/') else {
            break;
        };
        current = parent.to_owned();
    }
    Ok(())
}

/// Carries out checked steps.
fn execute(
    steps: &[Step],
    writer: &GuardedWriter,
    guard: &RootGuard,
    moved_back: &mut Vec<String>,
    restored: &mut Vec<String>,
    removed_folders: &mut Vec<String>,
) -> ToolkitResult<()> {
    for step in steps {
        match step {
            Step::MoveBack {
                from,
                to,
                hash,
                created_top,
                ..
            } => {
                let (to_abs, from_abs) = check_move_paths(writer, to, from)?;
                let found = hash_tree(to_abs.as_std_path())
                    .map_err(|e| refuse(format!("{to} cannot be read: {e}")))?;
                if found != *hash {
                    return Err(refuse(format!("{to} changed while the undo ran")));
                }
                move_path(guard, &to_abs, &from_abs).map_err(|e| ToolkitError::PathRefused {
                    path: from.chars().take(200).collect(),
                    reason: e.to_string(),
                })?;
                moved_back.push(from.clone());
                if let (Some(top), Some((parent, _))) = (created_top, to.rsplit_once('/')) {
                    remove_chain(writer, guard, parent, top, removed_folders)?;
                }
            }
            Step::Restore { rel, text, now } => {
                let current = writer.read_text(rel)?.unwrap_or_default();
                if hash_text(&current) != *now {
                    return Err(refuse(format!("{rel} changed while the undo ran")));
                }
                writer.write_expecting(rel, text, true, Expect::Previous(Some(&current)))?;
                restored.push(rel.clone());
            }
            Step::RemoveFile { rel, now } => {
                let abs = writer.path_of(rel)?;
                let found = current_text_hash(writer, rel)?;
                if found.as_deref() != Some(now.as_str()) {
                    return Err(refuse(format!("{rel} changed while the undo ran")));
                }
                remove_file(guard, &abs).map_err(|e| ToolkitError::PathRefused {
                    path: rel.chars().take(200).collect(),
                    reason: e.to_string(),
                })?;
                restored.push(rel.clone());
            }
            Step::RemoveFolders { to, top } => {
                remove_chain(writer, guard, to, top, removed_folders)?;
            }
        }
    }
    Ok(())
}

/// Why an undo would be refused now, `None` when it would go ahead. Used by the history.
pub(crate) fn blocker(journal: &Journal, writer: &GuardedWriter) -> Option<String> {
    match preflight(journal, writer) {
        Ok(_) => None,
        Err(e) => Some(e.to_string()),
    }
}

/// `project_layout_fix_undo`: see the module documentation.
///
/// # Errors
///
/// [`ToolkitError::FixNotFound`], [`ToolkitError::FixJournalDamaged`] for a journal that does not hold,
/// [`ToolkitError::FixUndoRefused`] when the disk no longer matches the journal (nothing is changed), and
/// the usual path and store errors.
pub fn undo(
    env: &ProjectEnv,
    req: &ProjectLayoutFixUndoRequest,
    protected: &[Utf8PathBuf],
) -> ToolkitResult<ProjectLayoutFixUndoDto> {
    let (record, view) = env.view(&req.project_id)?;
    let mut journal = read_journal(env, record.id.as_str(), &req.apply_id)?;
    let writer = env.writer(&view.root, record.id.as_str(), protected)?;
    let guard = RootGuard::new([view.root.clone()])?;
    let steps = preflight(&journal, &writer)?;
    let mut moved_back = Vec::new();
    let mut restored = Vec::new();
    let mut removed_folders = Vec::new();
    execute(
        &steps,
        &writer,
        &guard,
        &mut moved_back,
        &mut restored,
        &mut removed_folders,
    )?;
    journal.state = JournalState::Undone;
    journal.undone_ms = Some(env.clock().now_unix_ms());
    write_journal(env, &journal)?;
    let check = super::layout_check(
        env,
        &ProjectLayoutCheckRequest {
            project_id: req.project_id.clone(),
        },
    )?;
    Ok(ProjectLayoutFixUndoDto {
        project_id: req.project_id.clone(),
        apply_id: req.apply_id.clone(),
        moved_back,
        restored,
        removed_folders,
        check,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebase_replaces_whole_segments_only() {
        assert_eq!(
            rebase("CE/Patches/a.xml", "CE", "Compat/CombatExtended").as_deref(),
            Some("Compat/CombatExtended/Patches/a.xml")
        );
        assert_eq!(rebase("CE", "ce", "X").as_deref(), Some("X"));
        assert_eq!(rebase("CEX/a.xml", "CE", "X"), None);
        assert_eq!(rebase("a/CE/b", "CE", "X"), None);
    }

    #[test]
    fn only_the_standard_name_is_a_load_folders_file() {
        assert!(is_load_folders_name("LoadFolders.xml"));
        assert!(is_load_folders_name("loadfolders.xml"));
        assert!(!is_load_folders_name("a/LoadFolders.xml"));
        assert!(!is_load_folders_name("About/About.xml"));
    }
}
