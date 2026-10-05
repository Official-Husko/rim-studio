//! Creating and removing the entry of a project in the game `Mods` folder.
//!
//! Every write goes through [`GameWriteFence`]; this module only decides whether to ask, and keeps the
//! ownership manifest in step. A refusal is a value ([`Refusal`]), not an error: the caller shows the
//! reason together with the manual command.

use std::sync::{Mutex, MutexGuard, PoisonError};

use rimstudio_core::diag::DiagCode;
use rimstudio_core::ports::Clock;
use rimstudio_io::GuardError;
use rimstudio_io::fence::{FenceOp, OwnedEntry, OwnedKind};

use super::codes;
use super::inspect::{EntryState, GameRunning, LinkQuery, LinkStatus, inspect};
use super::manifest::{EntryMode, LinkRecord, record_id};

/// One deploy call at a time: the fence shares its ownership set between calls.
static DEPLOY_LOCK: Mutex<()> = Mutex::new(());

fn deploy_lock() -> MutexGuard<'static, ()> {
    DEPLOY_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What the caller asks for when creating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateOptions {
    /// How to make the entry.
    pub mode: EntryMode,
    /// The person confirmed that the game may be running (a new mod is only seen after a restart).
    pub confirm_game_running: bool,
}

/// Why an operation did nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// A stable code from [`codes`].
    pub code: DiagCode,
    /// A plain sentence.
    pub message: String,
}

fn refusal(code: DiagCode, message: impl Into<String>) -> Refusal {
    Refusal {
        code,
        message: message.into(),
    }
}

/// The result of [`create`] or [`remove`].
#[derive(Debug, Clone)]
pub struct DeployOutcome {
    /// True when the file system changed.
    pub done: bool,
    /// Why nothing was done.
    pub refusal: Option<Refusal>,
    /// The state after the call.
    pub status: LinkStatus,
}

fn owned_entries(q: &LinkQuery<'_>, status: &LinkStatus) -> Vec<OwnedEntry> {
    q.manifest
        .list_for(&status.mods_folder)
        .unwrap_or_default()
        .into_iter()
        .map(|r| OwnedEntry {
            name: r.name,
            kind: if r.mode.is_link() {
                OwnedKind::Link
            } else {
                OwnedKind::Copy
            },
            target: r.target,
        })
        .collect()
}

fn looks_like_permission(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    [
        "permission denied",
        "read-only",
        "access is denied",
        "not permitted",
        "privilege",
    ]
    .iter()
    .any(|p| t.contains(p))
}

fn mode_refusal(status: &LinkStatus, mode: EntryMode) -> Option<Refusal> {
    let ok = match mode {
        EntryMode::Symlink => status.support.symlink,
        EntryMode::Junction => status.support.junction,
        EntryMode::Copy => true,
    };
    if ok {
        return None;
    }
    let what = match mode {
        EntryMode::Symlink if status.support.needs_privilege => {
            "symbolic links need a privilege this app does not have"
        }
        EntryMode::Symlink => "symbolic links are not available here",
        _ => "junctions are not available through this app here",
    };
    Some(refusal(codes::MODE_UNSUPPORTED, what))
}

/// Creates the entry for the project, or says why not. Nothing that exists is replaced.
#[must_use]
pub fn create(q: &LinkQuery<'_>, options: CreateOptions, clock: &dyn Clock) -> DeployOutcome {
    let _guard = deploy_lock();
    let status = inspect(q);
    let stop = |status: LinkStatus, r: Refusal| DeployOutcome {
        done: false,
        refusal: Some(r),
        status,
    };
    let Some(target) = status.target.clone() else {
        return stop(
            status,
            refusal(codes::TARGET_REFUSED, "the project folder does not exist"),
        );
    };
    match status.state {
        EntryState::NotLinked => {}
        EntryState::Linked | EntryState::LinkedByHand | EntryState::Copy | EntryState::InMods => {
            return stop(
                status,
                refusal(codes::ALREADY_LINKED, "the game already sees this project"),
            );
        }
        EntryState::Stale => {
            return stop(
                status,
                refusal(
                    codes::NAME_TAKEN,
                    "an old link of this project is still in the Mods folder; remove it first",
                ),
            );
        }
        EntryState::ForeignLink | EntryState::ForeignFolder => {
            return stop(
                status,
                refusal(
                    codes::NAME_TAKEN,
                    "something with this name already exists in the Mods folder; it is left as it is",
                ),
            );
        }
        EntryState::Unavailable => {
            let code = if status.mods_exists {
                codes::TARGET_REFUSED
            } else {
                codes::MODS_MISSING
            };
            let text = if status.mods_exists {
                "the project cannot be linked"
            } else {
                "the game's Mods folder does not exist"
            };
            return stop(status, refusal(code, text));
        }
    }
    if status.running == GameRunning::Running && !options.confirm_game_running {
        return stop(
            status,
            refusal(
                codes::GAME_RUNNING,
                "RimWorld is running; a new mod is only seen after the game is restarted",
            ),
        );
    }
    if let Some(r) = mode_refusal(&status, options.mode) {
        return stop(status, r);
    }
    let record = LinkRecord {
        id: record_id(&status.mods_folder, &status.name),
        mods_folder: status.mods_folder.clone(),
        name: status.name.clone(),
        mode: options.mode,
        target: target.clone(),
        project_id: q.project_id.map(str::to_owned),
        created_ms: clock.now_unix_ms(),
        extra: std::collections::BTreeMap::new(),
    };
    // Record first: a crash after this leaves a record without an entry (dropped next time), never an
    // entry that is not recorded.
    if let Err(e) = q.manifest.put(&record) {
        return stop(
            status,
            refusal(
                codes::MANIFEST,
                format!("the ownership record could not be written: {e}"),
            ),
        );
    }
    let mut owned = owned_entries(q, &status);
    owned.retain(|e| e.name != record.name);
    owned.push(OwnedEntry {
        name: record.name.clone(),
        kind: if options.mode.is_link() {
            OwnedKind::Link
        } else {
            OwnedKind::Copy
        },
        target: target.clone(),
    });
    q.fence.set_owned(owned);
    let op = if options.mode.is_link() {
        FenceOp::CreateOwnedLink {
            name: &record.name,
            target: &target,
        }
    } else {
        FenceOp::CreateOwnedCopy {
            name: &record.name,
            source: &target,
        }
    };
    let result = q.fence.apply(&op);
    match result {
        Ok(()) => DeployOutcome {
            done: true,
            refusal: None,
            status: inspect(q),
        },
        Err(e) => {
            let _ = q.manifest.delete(&record.mods_folder, &record.name);
            let text = e.to_string();
            let r = if looks_like_permission(&text) {
                refusal(
                    codes::MODS_READ_ONLY,
                    "the game's Mods folder cannot be written by this app",
                )
            } else if matches!(&e, GuardError::FenceRefused { .. }) {
                refusal(codes::TARGET_REFUSED, text)
            } else {
                refusal(codes::IO, text)
            };
            stop(inspect(q), r)
        }
    }
}

/// Removes the entry of the project when it is one RimStudio made. Never asks about the running game,
/// never touches the project, never removes a real folder that is not a verified marked copy.
#[must_use]
pub fn remove(q: &LinkQuery<'_>) -> DeployOutcome {
    let _guard = deploy_lock();
    let status = inspect(q);
    let stop = |status: LinkStatus, r: Refusal| DeployOutcome {
        done: false,
        refusal: Some(r),
        status,
    };
    let Some(record) = status.record.clone() else {
        return stop(
            status,
            refusal(
                codes::NOT_OURS,
                "RimStudio did not create this entry, so it is not removed",
            ),
        );
    };
    if !status.can_remove() {
        return stop(
            status,
            refusal(
                codes::NOT_OURS,
                "this entry is not a link or copy RimStudio made, so it is not removed",
            ),
        );
    }
    q.fence.set_owned(owned_entries(q, &status));
    let op = if record.mode.is_link() {
        FenceOp::RemoveOwnedLink { name: &record.name }
    } else {
        FenceOp::RemoveOwnedCopy { name: &record.name }
    };
    match q.fence.apply(&op) {
        Ok(()) => {
            let _ = q.manifest.delete(&record.mods_folder, &record.name);
            DeployOutcome {
                done: true,
                refusal: None,
                status: inspect(q),
            }
        }
        Err(e) => stop(inspect(q), refusal(codes::NOT_OURS, e.to_string())),
    }
}
