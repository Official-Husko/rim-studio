//! What a library scan reports beyond its statistics: counts, duplicate groups and the Combat Extended entry.
//!
//! These types are additive fields of [`crate::library::LibraryScanResult`]. Every number comes from the scan
//! index of the run: nothing is guessed from folder names.
//!
//! - `loadable` counts the mods the game can load as they are (a mod in the install `Data` or `Mods` folder,
//!   or in a Steam Workshop folder, or a custom folder mod that is linked into `Mods`).
//! - `customOnly` counts the mods that exist only in a custom folder and are not visible to the game until
//!   they are linked or copied into `Mods`.
//! - A duplicate group is a set of mods that share one package id (compared ignoring case).

use serde::{Deserialize, Serialize};

use crate::library::SourceKindDto;

/// Counts over the scanned library.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LibraryCountsDto {
    /// Mods in the index.
    pub mods: u32,
    /// Mods the game can load as they are.
    pub loadable: u32,
    /// Mods that exist only in a custom folder and need a link or a copy before the game sees them.
    pub custom_only: u32,
    /// Rows restored from the cache because their source is offline.
    pub unavailable: u32,
    /// Mods whose `About.xml` could not be parsed.
    pub unparsed_about: u32,
    /// Mods with a made up package id.
    pub synthetic_ids: u32,
    /// Definitions in the index.
    pub defs: u32,
    /// Package ids that appear more than once.
    pub duplicate_groups: u32,
}

/// Why a copy of a duplicated mod was not kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum DuplicateReasonDto {
    /// The other copies are unavailable (their source is offline).
    Available,
    /// The user pinned the kept copy.
    Pinned,
    /// The kept copy comes from a better source class or a higher priority source.
    SourcePriority,
    /// The kept copy supports the game version and this one does not.
    VersionMatch,
    /// The kept copy has the newer `About.xml`.
    Newer,
    /// Nothing else differed; the path order decided.
    PathOrder,
}

/// What the game does with a copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum DuplicateGameDto {
    /// The game loads the copy.
    Loaded,
    /// The game logs an error and ignores the copy.
    Rejected,
    /// The game does not see the folder (a custom folder that is not linked).
    NotVisible,
}

/// One copy of a duplicated mod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DuplicateEntryDto {
    /// The mod folder.
    pub path: String,
    /// The display name of the copy.
    pub name: String,
    /// The id of the source the copy was found in.
    pub source: String,
    /// The kind of that source.
    pub source_kind: SourceKindDto,
    /// What the game does with the copy.
    pub game: DuplicateGameDto,
    /// Why the copy was skipped, in English. Absent for the kept copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub why: Option<String>,
}

/// A set of mods that share a package id: the copy that is kept and the copies that are skipped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroupDto {
    /// The package id, spelled as the kept copy writes it.
    pub package_id: String,
    /// True when the copies are all in one source.
    pub same_source: bool,
    /// The rung of the choice ladder that decided which copy is kept.
    pub reason: DuplicateReasonDto,
    /// The copy that is kept.
    pub kept: DuplicateEntryDto,
    /// The copies that are skipped, each with its source and the reason.
    pub skipped: Vec<DuplicateEntryDto>,
}

/// The duplicate groups of a scan, capped to a readable number.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroupsDto {
    /// All groups of the scan, including the ones that are not listed.
    pub total: u32,
    /// All skipped copies of all groups.
    pub skipped_total: u32,
    /// The listed groups, sorted by package id. At most the cap of the backend; compare with `total`.
    pub groups: Vec<DuplicateGroupDto>,
}

/// Whether Combat Extended is in the scanned library, found by its package id in the scan index.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CeInLibraryDto {
    /// True when a mod with the Combat Extended package id is in the index.
    pub present: bool,
    /// The package id as the mod writes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub package_id: Option<String>,
    /// The display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub name: Option<String>,
    /// The mod folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub path: Option<String>,
    /// The id of the source it was found in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub source: Option<String>,
    /// The kind of that source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub source_kind: Option<SourceKindDto>,
    /// The `modVersion` of its `About.xml`. Absent when the file gives none or cannot be read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub version: Option<String>,
    /// True when the game can load this copy as it stands.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub loadable: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_combat_extended_serialises_to_present_false_only() {
        let text = serde_json::to_string(&CeInLibraryDto::default()).unwrap_or_default();
        assert_eq!(text, r#"{"present":false}"#);
    }

    #[test]
    fn counts_use_custom_only_on_the_wire() {
        let text = serde_json::to_string(&LibraryCountsDto::default()).unwrap_or_default();
        assert!(text.contains("\"customOnly\":0"));
        assert!(!text.contains("needsLink"));
    }

    #[test]
    fn duplicate_groups_round_trip() {
        let entry = |path: &str, why: Option<&str>| DuplicateEntryDto {
            path: path.into(),
            name: "RS Mod".into(),
            source: "workshop-1".into(),
            source_kind: SourceKindDto::Workshop,
            game: DuplicateGameDto::Loaded,
            why: why.map(str::to_owned),
        };
        let groups = DuplicateGroupsDto {
            total: 3,
            skipped_total: 4,
            groups: vec![DuplicateGroupDto {
                package_id: "rs.fictional.mod".into(),
                same_source: false,
                reason: DuplicateReasonDto::SourcePriority,
                kept: entry("/fiction/Mods/RS_Mod", None),
                skipped: vec![entry("/fiction/custom/RS_Mod", Some("a better source"))],
            }],
        };
        let text = serde_json::to_string(&groups).unwrap_or_default();
        assert!(text.contains("\"reason\":\"source-priority\""));
        let back: Result<DuplicateGroupsDto, _> = serde_json::from_str(&text);
        assert_eq!(back.ok(), Some(groups));
    }
}
