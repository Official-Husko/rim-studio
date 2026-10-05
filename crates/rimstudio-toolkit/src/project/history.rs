//! `project_layout_fix_history`: the undo journals of a project, newest first, with whether each can still
//! be undone.
//!
//! A journal that cannot be read or whose checksum does not hold is listed too, so the user sees it; it is
//! never undoable.

use camino::Utf8PathBuf;
use rimstudio_ipc_types::project_fix::{
    LayoutFixJournalDto, ProjectLayoutFixHistoryDto, ProjectLayoutFixHistoryRequest,
};

use super::journal::{JournalState, JournalStatus, list_apply_ids, read_journal};
use super::undo::blocker;
use crate::error::ToolkitResult;
use crate::shared::env::ProjectEnv;

/// The most journals listed.
pub const MAX_JOURNALS: usize = 200;

/// `project_layout_fix_history`: lists the journals of a project. Read only.
///
/// # Errors
///
/// [`crate::error::ToolkitError::ProjectNotOpen`] or [`crate::error::ToolkitError::ProjectInvalid`], and
/// [`crate::error::ToolkitError::PathRefused`] when the project lies in a protected folder.
pub fn history(
    env: &ProjectEnv,
    req: &ProjectLayoutFixHistoryRequest,
    protected: &[Utf8PathBuf],
) -> ToolkitResult<ProjectLayoutFixHistoryDto> {
    let (record, view) = env.view(&req.project_id)?;
    let writer = env.writer(&view.root, record.id.as_str(), protected)?;
    let mut journals = Vec::new();
    for apply_id in list_apply_ids(env, record.id.as_str()) {
        match read_journal(env, record.id.as_str(), &apply_id) {
            Ok(j) => {
                let count = |f: &dyn Fn(JournalStatus) -> bool| {
                    u32::try_from(j.items.iter().filter(|i| f(i.status)).count())
                        .unwrap_or(u32::MAX)
                };
                let undone = j.state == JournalState::Undone;
                let undo_blocker = if undone {
                    Some("already undone".to_owned())
                } else if j.project_root != view.root.as_str() {
                    Some("the journal belongs to another folder".to_owned())
                } else {
                    blocker(&j, &writer)
                };
                journals.push(LayoutFixJournalDto {
                    apply_id: j.apply_id.clone(),
                    plan_id: j.plan_id.clone(),
                    created_ms: j.created_ms,
                    items_total: u32::try_from(j.items.len()).unwrap_or(u32::MAX),
                    items_done: count(&|s| s == JournalStatus::Done),
                    items_skipped: count(&|s| s != JournalStatus::Done),
                    undone,
                    undo_possible: undo_blocker.is_none(),
                    undo_blocker,
                });
            }
            Err(e) => journals.push(LayoutFixJournalDto {
                apply_id,
                plan_id: String::new(),
                created_ms: 0,
                items_total: 0,
                items_done: 0,
                items_skipped: 0,
                undone: false,
                undo_possible: false,
                undo_blocker: Some(e.to_string()),
            }),
        }
    }
    journals.sort_by(|a, b| {
        b.created_ms
            .cmp(&a.created_ms)
            .then_with(|| b.apply_id.cmp(&a.apply_id))
    });
    journals.truncate(MAX_JOURNALS);
    Ok(ProjectLayoutFixHistoryDto {
        project_id: req.project_id.clone(),
        journals,
    })
}
