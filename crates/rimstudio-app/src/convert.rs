//! Conversion of engine and manager types into the DTOs of `rimstudio-ipc-types`.
//!
//! The manager and the steam crate return their own serde types; the contract has its own shapes (paths
//! as strings, kebab-case enum strings, sizes below 2^53). These are plain mappings with no rules in
//! them, kept in one place so the handlers stay short.

use std::time::Duration;

use rimstudio_core::mods::SourceKind;
use rimstudio_core::os::Os;
use rimstudio_core::settings::WorkspaceSettings;
use rimstudio_ipc_types::diagnostic::{DiagnosticSummaryDto, diagnostics_to_dtos};
use rimstudio_ipc_types::library::{
    ConfidenceDto, DetectionReportDto, DetectionWarningDto, DirCheckDto, FolderKindDto,
    FolderWarningDto, HealthDto, HowDto, InstallDto, InstallKindDto, LaunchDto,
    LibraryScanResult as LibraryScanResultDto, OsDto, OverlapRelationDto, ScanStatsDto,
    ScanTimingsDto, SelectedDto, SourceDto, SourceKindDto, SourceOverlapDto, SourceReportDto,
    SourceStatusDto, SourcesProbeFolderResponse as ProbeDto, SteamLibraryDto, SteamRootDto,
    UserDirDto, UserDirKindDto, VersionDto, WorkshopDirDto,
};
use rimstudio_ipc_types::settings::{FolderLayoutDto, SettingsDto};
use rimstudio_library::scan::{ScanStats, ScanTimings, SourceStatus};
use rimstudio_library::sources::{FolderKind, FolderWarning, OverlapRelation};
use rimstudio_manager::scan::LibraryScanResult;
use rimstudio_manager::settings::SettingsView;
use rimstudio_manager::sources::{SourceRow, SourcesProbeFolderResponse};
use rimstudio_steam::report::{
    Confidence, DetectionReport, Health, How, Install, InstallKind, UserDirKind, VersionInfo,
};

/// The `how` of a candidate as the contract spells it.
#[must_use]
pub fn how_dto(how: How) -> HowDto {
    match how {
        How::Override => HowDto::Override,
        How::RegistryHkcu => HowDto::RegistryHkcu,
        How::RegistryHklm => HowDto::RegistryHklm,
        How::XdgDataHome => HowDto::XdgDataHome,
        How::SymlinkSteam => HowDto::SymlinkSteam,
        How::Flatpak => HowDto::Flatpak,
        How::Snap => HowDto::Snap,
        How::LibraryFoldersVdf => HowDto::LibraryFoldersVdf,
        How::Appmanifest => HowDto::Appmanifest,
        How::DirectoryProbe => HowDto::DirectoryProbe,
    }
}

fn confidence_dto(c: Confidence) -> ConfidenceDto {
    match c {
        Confidence::Low => ConfidenceDto::Low,
        Confidence::Medium => ConfidenceDto::Medium,
        Confidence::High => ConfidenceDto::High,
    }
}

fn os_dto(os: Os) -> OsDto {
    match os {
        Os::Windows => OsDto::Windows,
        Os::MacOs => OsDto::Macos,
        Os::Linux => OsDto::Linux,
        Os::Other => OsDto::Other,
    }
}

fn install_kind_dto(k: InstallKind) -> InstallKindDto {
    match k {
        InstallKind::Steam => InstallKindDto::Steam,
        InstallKind::Gog => InstallKindDto::Gog,
        InstallKind::Manual => InstallKindDto::Manual,
        InstallKind::Override => InstallKindDto::Override,
    }
}

fn health_dto(h: Health) -> HealthDto {
    match h {
        Health::Installed => HealthDto::Installed,
        Health::UpdatePending => HealthDto::UpdatePending,
        Health::NeedsVerify => HealthDto::NeedsVerify,
    }
}

fn user_dir_kind_dto(k: UserDirKind) -> UserDirKindDto {
    match k {
        UserDirKind::Native => UserDirKindDto::Native,
        UserDirKind::Proton => UserDirKindDto::Proton,
        UserDirKind::Flatpak => UserDirKindDto::Flatpak,
        UserDirKind::Snap => UserDirKindDto::Snap,
        UserDirKind::Macos => UserDirKindDto::Macos,
        UserDirKind::Windows => UserDirKindDto::Windows,
        UserDirKind::Override => UserDirKindDto::Override,
    }
}

fn version_dto(v: &VersionInfo) -> VersionDto {
    VersionDto {
        raw: v.raw.clone(),
        major: v.major,
        minor: v.minor,
        build: v.build,
        rev: v.rev,
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn install_dto(i: &Install) -> InstallDto {
    InstallDto {
        id: i.id.clone(),
        kind: install_kind_dto(i.kind),
        path: i.path.as_str().to_owned(),
        game_root: i.game_root.as_str().to_owned(),
        how: how_dto(i.how),
        confidence: confidence_dto(i.confidence),
        version: i.version.as_ref().map(version_dto),
        build_id: i.build_id.clone(),
        target_build_id: i.target_build_id.clone(),
        state_flags: i.state_flags.and_then(|f| u32::try_from(f).ok()),
        health: health_dto(i.health),
        launch: LaunchDto {
            steam_url: i.launch.steam_url.clone(),
            executable: i.launch.executable.as_ref().map(|p| p.as_str().to_owned()),
            working_dir: i.launch.working_dir.as_str().to_owned(),
            args: i.launch.args.clone(),
            steam_only: i.launch.steam_only,
        },
        mods_dir: DirCheckDto {
            path: i.mods_dir.path.as_str().to_owned(),
            exists: i.mods_dir.exists,
            readable: i.mods_dir.readable,
        },
        data_dirs: i.data_dirs.clone(),
        proton: i.proton,
        library: i.library.as_ref().map(|p| p.as_str().to_owned()),
        is_symlink: i.is_symlink,
        workshop: i
            .workshop
            .iter()
            .map(|w| WorkshopDirDto {
                library: w.library.as_ref().map(|p| p.as_str().to_owned()),
                content_dir: w.content_dir.as_str().to_owned(),
                acf: w.acf.as_ref().map(|p| p.as_str().to_owned()),
                items_on_disk: count(w.items_on_disk),
                items_in_acf: w.items_in_acf.map(count),
                how: how_dto(w.how),
            })
            .collect(),
    }
}

/// The detection report as the contract shows it.
#[must_use]
pub fn detection_report_dto(r: &DetectionReport) -> DetectionReportDto {
    DetectionReportDto {
        schema: r.schema,
        generated_at_ms: r.generated_at_ms,
        os: os_dto(r.os),
        steam_roots: r
            .steam_roots
            .iter()
            .map(|s| SteamRootDto {
                path: s.path.as_str().to_owned(),
                canonical: s.canonical.as_str().to_owned(),
                how: how_dto(s.how),
                confidence: confidence_dto(s.confidence),
                exists: s.checks.exists,
                has_steamapps: s.checks.has_steamapps,
                has_library_folders: s.checks.has_library_folders,
            })
            .collect(),
        libraries: r
            .libraries
            .iter()
            .map(|l| SteamLibraryDto {
                path: l.path.as_str().to_owned(),
                canonical: l.canonical.as_str().to_owned(),
                how: how_dto(l.how),
                root: l.root.as_str().to_owned(),
                online: l.online,
                timed_out: l.timed_out,
                label: l.label.clone(),
                has_app: l.has_app,
                stale: l.stale,
            })
            .collect(),
        installs: r.installs.iter().map(install_dto).collect(),
        user_dirs: r
            .user_dirs
            .iter()
            .map(|u| UserDirDto {
                path: u.path.as_str().to_owned(),
                kind: user_dir_kind_dto(u.kind),
                mods_config_exists: u.mods_config.exists,
                mods_config_mtime_ms: u.mods_config.mtime_ms,
                game_version: u.mods_config.game_version.clone(),
                player_log: u.player_log.as_ref().map(|p| p.as_str().to_owned()),
                prefs: u.prefs.as_ref().map(|p| p.as_str().to_owned()),
                newest: u.newest,
            })
            .collect(),
        selected: SelectedDto {
            install: r.selected.install.clone(),
            user_dir: r.selected.user_dir.as_ref().map(|p| p.as_str().to_owned()),
        },
        warnings: r
            .warnings
            .iter()
            .map(|w| DetectionWarningDto {
                code: w.code.clone(),
                message: w.message.clone(),
                paths: w.paths.iter().map(|p| p.as_str().to_owned()).collect(),
            })
            .collect(),
    }
}

fn source_kind_dto(k: SourceKind) -> SourceKindDto {
    match k {
        SourceKind::GameData => SourceKindDto::GameData,
        SourceKind::GameMods => SourceKindDto::GameMods,
        SourceKind::Workshop => SourceKindDto::Workshop,
        SourceKind::Custom => SourceKindDto::Custom,
    }
}

fn source_status_dto(s: SourceStatus) -> SourceStatusDto {
    match s {
        SourceStatus::Ready => SourceStatusDto::Ready,
        SourceStatus::Disabled => SourceStatusDto::Disabled,
        SourceStatus::Offline => SourceStatusDto::Offline,
        SourceStatus::NotDirectory => SourceStatusDto::NotDirectory,
    }
}

/// One row of the sources list. `mod_count` is the count of the last scan when there was one.
#[must_use]
pub fn source_dto(row: &SourceRow, scanned_mods: Option<u32>) -> SourceDto {
    let status = if !row.enabled {
        SourceStatusDto::Disabled
    } else if !row.status.exists {
        SourceStatusDto::NotDirectory
    } else if !row.status.readable {
        SourceStatusDto::Offline
    } else {
        SourceStatusDto::Ready
    };
    SourceDto {
        id: row.id.to_string(),
        kind: source_kind_dto(row.kind),
        path: row.path.as_str().to_owned(),
        label: row.label.clone(),
        enabled: row.enabled,
        status,
        mod_count: scanned_mods.or_else(|| row.status.mod_count_estimate.map(count)),
        layout: row
            .layout
            .map_or(FolderLayoutDto::Auto, FolderLayoutDto::from),
        scan_depth: row.scan_depth.unwrap_or(1),
        read_only: row.read_only,
    }
}

fn folder_kind_dto(k: FolderKind) -> FolderKindDto {
    match k {
        FolderKind::Missing => FolderKindDto::Missing,
        FolderKind::NotDirectory => FolderKindDto::NotDirectory,
        FolderKind::GameData => FolderKindDto::GameData,
        FolderKind::GameMods => FolderKindDto::GameMods,
        FolderKind::Workshop => FolderKindDto::Workshop,
        FolderKind::SingleMod => FolderKindDto::SingleMod,
        FolderKind::ModsRoot => FolderKindDto::ModsRoot,
        FolderKind::Empty => FolderKindDto::Empty,
    }
}

fn folder_warning_dto(w: FolderWarning) -> FolderWarningDto {
    match w {
        FolderWarning::NoModsFound => FolderWarningDto::NoModsFound,
        FolderWarning::ModsFoundDeeper => FolderWarningDto::ModsFoundDeeper,
        FolderWarning::AmbiguousLayout => FolderWarningDto::AmbiguousLayout,
        FolderWarning::PathIsLink => FolderWarningDto::PathIsLink,
        FolderWarning::Unreachable => FolderWarningDto::Unreachable,
    }
}

fn overlap_relation_dto(r: OverlapRelation) -> OverlapRelationDto {
    match r {
        OverlapRelation::Same => OverlapRelationDto::Same,
        OverlapRelation::Inside => OverlapRelationDto::Inside,
        OverlapRelation::Contains => OverlapRelationDto::Contains,
    }
}

/// The probe of a folder as the contract shows it.
#[must_use]
pub fn probe_dto(p: &SourcesProbeFolderResponse) -> ProbeDto {
    ProbeDto {
        kind: folder_kind_dto(p.class.kind),
        mod_count: count(p.class.mod_count),
        suggested_depth: p.class.suggested_depth,
        suggested_layout: FolderLayoutDto::from(p.class.suggested_layout),
        warnings: p
            .class
            .warnings
            .iter()
            .copied()
            .map(folder_warning_dto)
            .collect(),
        overlaps: p
            .overlaps
            .iter()
            .map(|o| SourceOverlapDto {
                with: o.with.to_string(),
                relation: overlap_relation_dto(o.relation),
                code: o.code.clone(),
                hard: o.hard,
            })
            .collect(),
        diagnostics: diagnostics_to_dtos(&p.diagnostics),
        can_save: p.can_add,
    }
}

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

fn stats_dto(s: &ScanStats) -> ScanStatsDto {
    ScanStatsDto {
        folders_probed: s.folders_probed,
        mods_found: s.mods_found,
        mods_indexed: s.mods_indexed,
        about_parsed: s.about_parsed,
        about_reused: s.about_reused,
        load_folders_parsed: s.load_folders_parsed,
        load_folders_reused: s.load_folders_reused,
        def_files_parsed: s.def_files_parsed,
        def_files_reused: s.def_files_reused,
        defs: s.defs,
        files_hashed: s.files_hashed,
        bytes_read: s.bytes_read,
        dir_errors: s.dir_errors,
        non_utf8: s.non_utf8,
    }
}

fn timings_dto(t: &ScanTimings) -> ScanTimingsDto {
    ScanTimingsDto {
        discover_ms: millis(t.discover),
        metadata_ms: millis(t.metadata),
        definitions_ms: millis(t.definitions),
        total_ms: millis(t.total),
    }
}

/// The per source reports of a scan.
#[must_use]
pub fn source_reports_dto(result: &LibraryScanResult) -> Vec<SourceReportDto> {
    result
        .sources
        .iter()
        .map(|s| {
            let counts = result.facts.source(s.id.as_str());
            SourceReportDto {
                id: s.id.to_string(),
                kind: source_kind_dto(s.kind),
                path: s.path.as_str().to_owned(),
                status: source_status_dto(s.status),
                mods: count(s.mods),
                loadable: counts.map(|c| count(c.loadable)),
                custom_only: counts.map(|c| count(c.custom_only)),
            }
        })
        .collect()
}

/// The result of a scan as the contract shows it; `rev` is the library revision after the scan.
#[must_use]
pub fn scan_result_dto(result: &LibraryScanResult, rev: u64) -> LibraryScanResultDto {
    LibraryScanResultDto {
        rev,
        stats: stats_dto(&result.stats),
        timings: timings_dto(&result.timings),
        sources: source_reports_dto(result),
        diagnostics: DiagnosticSummaryDto::from(&result.diagnostics),
        cancelled: result.cancelled,
        counts: Some(crate::convert_facts::counts_dto(&result.counts)),
        duplicates: Some(crate::convert_facts::duplicates_dto(
            &result.facts.duplicates,
        )),
        ce_in_library: Some(crate::convert_facts::ce_dto(&result.facts.ce)),
    }
}

/// A revision number below 2^53 from the fingerprint text of the manager.
#[must_use]
pub fn rev_of_fingerprint(fingerprint: &str) -> u64 {
    let head: String = fingerprint.chars().take(13).collect();
    u64::from_str_radix(&head, 16).unwrap_or(0)
}

/// The settings view and the workspace document as the contract shows them.
#[must_use]
pub fn settings_dto(view: &SettingsView, workspace: &WorkspaceSettings) -> SettingsDto {
    SettingsDto::from_core(&view.settings, workspace, rev_of_fingerprint(&view.rev))
}

#[cfg(test)]
mod tests {
    use rimstudio_core::ids::SourceId;
    use rimstudio_core::settings::FolderLayout;
    use rimstudio_manager::sources::FolderStatus;

    use super::*;

    fn row(enabled: bool, exists: bool, readable: bool) -> SourceRow {
        SourceRow {
            id: SourceId::new("cf_abc").unwrap_or_else(|_| SourceId::game_data()),
            kind: SourceKind::Custom,
            label: "RS_Folder".into(),
            path: "/fiction/mods".into(),
            enabled,
            can_remove: true,
            order: 0,
            layout: Some(FolderLayout::ModsRoot),
            scan_depth: Some(2),
            read_only: false,
            link: None,
            status: FolderStatus {
                exists,
                readable,
                removable_drive: false,
                mod_count_estimate: Some(3),
                kind: None,
                warnings: Vec::new(),
            },
        }
    }

    #[test]
    fn a_source_row_maps_status_layout_and_depth() {
        let ready = source_dto(&row(true, true, true), None);
        assert_eq!(ready.status, SourceStatusDto::Ready);
        assert_eq!(ready.layout, FolderLayoutDto::ModsRoot);
        assert_eq!(ready.scan_depth, 2);
        assert_eq!(ready.mod_count, Some(3));
        assert_eq!(
            source_dto(&row(false, true, true), None).status,
            SourceStatusDto::Disabled
        );
        assert_eq!(
            source_dto(&row(true, false, false), None).status,
            SourceStatusDto::NotDirectory
        );
        assert_eq!(
            source_dto(&row(true, true, false), None).status,
            SourceStatusDto::Offline
        );
    }

    #[test]
    fn the_scanned_count_wins_over_the_estimate() {
        assert_eq!(
            source_dto(&row(true, true, true), Some(9)).mod_count,
            Some(9)
        );
    }

    #[test]
    fn revisions_from_fingerprints_stay_below_two_to_the_fifty_third() {
        let rev = rev_of_fingerprint("ffffffffffffffff");
        assert!(rev < (1_u64 << 53));
        assert_eq!(rev_of_fingerprint("0000000000000001"), 0);
        assert_eq!(rev_of_fingerprint("not hex at all"), 0);
        assert_eq!(rev_of_fingerprint("zz00000000000000"), 0);
    }

    #[test]
    fn the_library_folders_spelling_is_kebab_case_on_the_contract() {
        assert_eq!(how_dto(How::LibraryFoldersVdf), HowDto::LibraryFoldersVdf);
        let text = serde_json::to_string(&how_dto(How::LibraryFoldersVdf)).unwrap_or_default();
        assert_eq!(text, "\"library-folders-vdf\"");
    }
}
