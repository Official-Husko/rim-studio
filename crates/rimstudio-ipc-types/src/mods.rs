//! Compact mod list rows for the snapshot of the mod list.
//!
//! List rows carry list columns only (no descriptions, dependency lists or file listings): those come from a
//! detail call. A snapshot of 3,000 rows stays under 1 MB (see the size test).

use serde::{Deserialize, Serialize};

/// Bits of [`ModRowDto::flags`].
pub mod flags {
    /// The mod is in the active list.
    pub const ACTIVE: u32 = 1;
    /// The mod is the official game or an expansion.
    pub const OFFICIAL: u32 = 1 << 1;
    /// The mod came from the Steam Workshop folder.
    pub const WORKSHOP: u32 = 1 << 2;
    /// The mod supports the running game version.
    pub const VERSION_OK: u32 = 1 << 3;
    /// The mod has a preview image.
    pub const HAS_PREVIEW: u32 = 1 << 4;
    /// The mod is read only (a source marked read only).
    pub const READ_ONLY: u32 = 1 << 5;
}

/// One row of the mod list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ModRowDto {
    /// Session handle of the row.
    pub idx: u32,
    /// Stable mod id (`w<workshopId>` or `<source>:<packageId>`).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Authors joined with a comma and a space; empty when the mod names none.
    pub authors: String,
    /// Bit set of [`flags`].
    pub flags: u32,
    /// Bit mask of supported game minor versions, bit `n` for `1.n`.
    pub versions: u32,
    /// Position in the active list. Absent when the mod is not active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub load_index: Option<u32>,
    /// Number of error diagnostics.
    pub errors: u16,
    /// Number of warning diagnostics.
    pub warnings: u16,
    /// Size on disk in kibibytes, saturating.
    pub size_kib: u32,
    /// Last update as Unix seconds, zero when unknown.
    pub updated_s: u32,
}

/// The whole list at one revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ModsSnapshot {
    /// Revision of the list; deltas carry the revision they apply to.
    pub rev: u64,
    /// Running game version text. Absent when no install is selected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub game_version: Option<String>,
    /// The rows, in list order.
    pub rows: Vec<ModRowDto>,
}
