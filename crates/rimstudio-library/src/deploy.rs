//! Making one project visible to the game: a link (or an explicit, marked copy) of the project folder
//! inside `<game install>/Mods`.
//!
//! The game reads only its own `Mods` folder, `Data` and the Steam workshop, so a mod made in a custom
//! folder is invisible until something puts it in `Mods`. This module is the small, safe part of the
//! planned deploy feature that the project page needs for that (invariant I-05):
//!
//! - [`inspect::inspect`] reads the state of the entry `Mods/<name>` without writing anything;
//! - [`ops::create`] creates the entry through the write fence of `rimstudio-io`, never over anything
//!   that exists, after recording it in the ownership manifest;
//! - [`ops::remove`] removes the entry only when it is ours: a link that points at the recorded target
//!   is unlinked (never a recursive delete, the target is untouched) and a marked copy is removed by the
//!   fence after it verified the marker.
//!
//! The ownership manifest ([`manifest::LinkManifest`]) is one JSON document per entry in the data root
//! (`game-links`), written atomically and before the entry is created, so a crash leaves either nothing,
//! or a record without an entry (dropped at the next create), never an entry that is not recorded.
//!
//! Still deferred: the whole link farm for a load order, dry run plans and the `ModsConfig.xml` write.

pub mod command;
pub mod inspect;
pub mod manifest;
pub mod ops;

use rimstudio_core::diag::DiagCode;

/// Diagnostic and refusal codes of the deploy module.
pub mod codes {
    use super::DiagCode;

    /// No game install is known, so there is no `Mods` folder to link into.
    pub const GAME_NOT_FOUND: DiagCode = DiagCode::new("deploy.game-not-found");
    /// The `Mods` folder of the install does not exist.
    pub const MODS_MISSING: DiagCode = DiagCode::new("deploy.mods-missing");
    /// The `Mods` folder cannot be written by this user.
    pub const MODS_READ_ONLY: DiagCode = DiagCode::new("deploy.mods-read-only");
    /// The game is running; a new mod is only seen after a restart.
    pub const GAME_RUNNING: DiagCode = DiagCode::new("deploy.game-running");
    /// Something with the link name already exists in `Mods` and is not ours.
    pub const NAME_TAKEN: DiagCode = DiagCode::new("deploy.name-taken");
    /// The project is already linked.
    pub const ALREADY_LINKED: DiagCode = DiagCode::new("deploy.already-linked");
    /// The entry is not one RimStudio created, so it is never removed.
    pub const NOT_OURS: DiagCode = DiagCode::new("deploy.not-ours");
    /// The requested way of linking is not available on this system.
    pub const MODE_UNSUPPORTED: DiagCode = DiagCode::new("deploy.mode-unsupported");
    /// The project folder cannot be linked (missing, not a folder, or inside the game folder).
    pub const TARGET_REFUSED: DiagCode = DiagCode::new("deploy.target-refused");
    /// The project folder is already inside `Mods`; the game sees it.
    pub const IN_MODS: DiagCode = DiagCode::new("deploy.project-in-mods");
    /// The ownership manifest could not be read or written.
    pub const MANIFEST: DiagCode = DiagCode::new("deploy.manifest");
    /// The file system refused the operation for another reason.
    pub const IO: DiagCode = DiagCode::new("deploy.io");
}
