//! The facts of a scan (loadable and custom only counts, duplicate groups, Combat Extended) as DTOs.
//!
//! Plain mappings of the types of `rimstudio_manager::facts`; the numbers are the manager's.

use rimstudio_core::mods::SourceKind;
use rimstudio_ipc_types::library::SourceKindDto;
use rimstudio_ipc_types::library_facts::{
    CeInLibraryDto, DuplicateEntryDto, DuplicateGameDto, DuplicateGroupDto, DuplicateGroupsDto,
    DuplicateReasonDto, LibraryCountsDto,
};
use rimstudio_library::duplicates::{ChoiceReason, GameTreatment};
use rimstudio_manager::facts::{CeFact, DuplicateEntry, DuplicateFacts};
use rimstudio_manager::scan::LibraryCounts;

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn kind_dto(kind: SourceKind) -> SourceKindDto {
    match kind {
        SourceKind::GameData => SourceKindDto::GameData,
        SourceKind::GameMods => SourceKindDto::GameMods,
        SourceKind::Workshop => SourceKindDto::Workshop,
        SourceKind::Custom => SourceKindDto::Custom,
    }
}

fn reason_dto(reason: ChoiceReason) -> DuplicateReasonDto {
    match reason {
        ChoiceReason::Available => DuplicateReasonDto::Available,
        ChoiceReason::Pinned => DuplicateReasonDto::Pinned,
        ChoiceReason::SourcePriority => DuplicateReasonDto::SourcePriority,
        ChoiceReason::VersionMatch => DuplicateReasonDto::VersionMatch,
        ChoiceReason::Newer => DuplicateReasonDto::Newer,
        ChoiceReason::PathOrder => DuplicateReasonDto::PathOrder,
    }
}

fn game_dto(game: &GameTreatment) -> DuplicateGameDto {
    match game {
        GameTreatment::Loaded { .. } => DuplicateGameDto::Loaded,
        GameTreatment::Rejected => DuplicateGameDto::Rejected,
        GameTreatment::NotVisible => DuplicateGameDto::NotVisible,
    }
}

fn entry_dto(e: &DuplicateEntry) -> DuplicateEntryDto {
    DuplicateEntryDto {
        path: e.path.clone(),
        name: e.name.clone(),
        source: e.source.clone(),
        source_kind: kind_dto(e.kind),
        game: game_dto(&e.game),
        why: e.why.clone(),
    }
}

/// The counts of a scan as the contract shows them.
#[must_use]
pub fn counts_dto(c: &LibraryCounts) -> LibraryCountsDto {
    LibraryCountsDto {
        mods: count(c.mods),
        loadable: count(c.loadable),
        custom_only: count(c.needs_link),
        unavailable: count(c.unavailable),
        unparsed_about: count(c.unparsed_about),
        synthetic_ids: count(c.synthetic_ids),
        defs: count(c.defs),
        duplicate_groups: count(c.duplicate_groups),
    }
}

/// The duplicate groups of a scan.
#[must_use]
pub fn duplicates_dto(d: &DuplicateFacts) -> DuplicateGroupsDto {
    DuplicateGroupsDto {
        total: count(d.total),
        skipped_total: count(d.skipped_total),
        groups: d
            .groups
            .iter()
            .map(|g| DuplicateGroupDto {
                package_id: g.package_id.clone(),
                same_source: g.same_source,
                reason: reason_dto(g.reason),
                kept: entry_dto(&g.kept),
                skipped: g.skipped.iter().map(entry_dto).collect(),
            })
            .collect(),
    }
}

/// The Combat Extended entry of a scan.
#[must_use]
pub fn ce_dto(ce: &CeFact) -> CeInLibraryDto {
    CeInLibraryDto {
        present: ce.present,
        package_id: ce.package_id.clone(),
        name: ce.name.clone(),
        path: ce.path.clone(),
        source: ce.source.clone(),
        source_kind: ce.source_kind.map(kind_dto),
        version: ce.version.clone(),
        loadable: ce.loadable,
    }
}
