# RimStudio command catalog

This catalog is the authoritative list of every command, broadcast event and channel that the RimStudio webview and the CLI can use. It consolidates the registry rows of [IPC and state](ipc-and-state.md) and every command and event that the ten feature specifications under `docs/features/` name or propose, resolves duplicates and conflicting spellings with the `<area>_<verb>` rule, and records for each row its kind, DTOs, owning crate, milestone, requirement ids, root class, aliases found in other documents and status. It does not redesign anything: kinds, payloads and milestones come from the source documents, and every name change is recorded in the [decision register](decision-register.md#12-integration-decisions) (D-071 to D-073, D-081, D-082).

Status: draft, section 4.12 records the handlers built for 0.1.0 | Last updated: 2026-10-06

Milestone numbering follows the [roadmap, section 1.1](../roadmap.md#11-mapping-to-the-milestone-names-in-the-register) (older mentions of M3 to M6 use the decision register's numbering).

## Contents

1. [How to read the catalog](#1-how-to-read-the-catalog)
2. [Reconciliation rules](#2-reconciliation-rules)
3. [Names that were renamed or merged](#3-names-that-were-renamed-or-merged)
4. [The catalog](#4-the-catalog) (4.12 lists the handlers built for 0.1.0)
5. [Broadcast events](#5-broadcast-events)
6. [Channels and message unions](#6-channels-and-message-unions)
7. [Root classes](#7-root-classes)
8. [Counts](#8-counts)
9. [Adding or changing a command](#9-adding-or-changing-a-command)
10. [Open points](#10-open-points)

## 1. How to read the catalog

The catalog has 158 rows: the 153 rows of the tables of section 4 plus the five extra rows of section 4.12. 40 of them are registered in [IPC and state](ipc-and-state.md) (the first 40 commands, milestone M0 to M4 entries of section 11 there); the others are proposed by a feature spec or by a note of the architecture documents and must become registry rows in `crates/rimstudio-app/src/registry.rs` when their milestone starts. A row marked "family" stands for a group of commands that a spec names only by a wildcard (for example `history_*`); the concrete names are fixed when the milestone starts and each then gets its own row.

| Column | Meaning |
|---|---|
| Command | Canonical wire name in snake case. The TypeScript name is the camelCase form exported from `rimstudio-ipc-types`. |
| Kind | `query`, `action`, `stream` or `job` as defined in [IPC and state section 2.1](ipc-and-state.md#21-declaration). A kind written with a note (promotable to a job) records a handler whose Rust work may exceed the inline budget. |
| DTOs and result | `X` means `XRequest` and `XResponse` as in the registry. A job's response is `JobHandleDto` and the result type is carried by its terminal `JobEvent<R>`. Rows proposed by a spec show the conventional stem when the spec gives no DTO name; the stem is a placeholder until the DTO is written in `rimstudio-ipc-types`. |
| Owning crate | The crate whose `api` module holds the handler that the registry row points to, as in the sketch `manager::api::mods_get_detail` of [IPC and state](ipc-and-state.md). An engine or service crate that does the work is named in brackets. The registry itself is always in `rimstudio-app`, and no shell crate owns a command ([crate catalog](crate-catalog.md) section 7). |
| Milestone | Roadmap numbering (M0 to M7). For registered rows it is the value of IPC and state. For proposed rows it is the milestone of the first requirement that uses the row (MVP maps to M2 and v1 to M3 for the manager, the feature milestones of the roadmap for the toolkit, designer and publisher). |
| Requirements | Owner requirement ids R1 to R12 and the feature ids (MM, LO, GD, CD, WS, IT, WP, CE) that use the row. A bare spec prefix such as `WS` means the whole feature area of that spec. |
| Root class | The path root class that the registry attribute declares for the path fields of the request, as defined in [security and privacy section 5](security-and-privacy.md#5-path-root-validation): `LibraryRoot`, `ProjectRoot`, `DataRoot`, `GameRoot`, `UserPick` or `none`. The assignment per row is derived from that rule and from what the row touches; section 7 explains it. |
| Aliases | Other spellings found in research notes, early spec drafts or sibling specs. An alias is never accepted on the wire. |
| Status | `registered, row N` (the row number of the first 40 table) or `proposed by <document>`. |

## 2. Reconciliation rules

1. **Naming.** The name follows `<area>_<verb>` from [workspace layout section 2](workspace-layout.md#2-naming-and-layer-tag-conventions) and [IPC and state section 3](ipc-and-state.md#3-naming). The single reserved verb-first name is `cancel_job`. Names that already had the area first in a spec are kept unchanged.
2. **One name per operation.** When two specs describe the same operation (`logs_tail` in the workspace spec and `logs_follow` in the mod manager spec; `publish_stage` and `publish_dry_run`), the catalog keeps the name whose spec owns the operation in more detail and lists the other as an alias.
3. **Research aliases.** The research notes used `list_mods_snapshot`, `subscribe_mods` and `get_mod_detail`; the mod manager early draft used `library_snapshot`, `library_subscribe` and `library_mod_detail`, `setup_detect`, `list_enable` and `job_cancel`. All of them map to the registered names `mods_snapshot`, `mods_subscribe`, `mods_get_detail`, `detect_run`, `list_toggle` and `cancel_job`.
4. **Kinds.** A name has exactly one kind. Where a spec wrote a pair of kinds for one name, the kind that fits the work was chosen and the other becomes the promotion rule: `sort_preview`, `diagnostics_for_mod` and `patch_test_run` are queries that must stay promotable to jobs (a query whose Rust work exceeds 1 ms inline is promoted by changing its row kind, [IPC and state section 11](ipc-and-state.md#11-the-first-40-commands) note 5); `designer_material_matrix` is promoted if its bench exceeds 1 ms. `library_folder_size` is a job because it walks a directory tree.
5. **`sort_preview`.** It is a query in the registry (610 mods sort in about 3 ms in the mod manager spec). It must be implemented so that promotion needs no change of its DTOs: the response already carries a preview token, the progress shape of a job is the shared progress record, and the TypeScript wrapper is generated from the kind. The mod manager spec and the load order spec both state this need, and D-073 records it.
6. **Events versus channels.** Tauri events are limited to the low rate whole-app facts of [IPC and state section 3.1](ipc-and-state.md#31-broadcast-events). The dotted event names of the mod manager spec (`library.delta` and similar) are message kinds on a stream or job channel; section 5 and 6 give the mapping (D-081).
7. **Shared rows.** `launch_start` and `cancel_job` are reused by the workspace and publishing specs and are listed once. The `logs_*` rows are shared by the Player.log viewer and the project log tab (D-082).
8. **Families.** Wildcard names (`history_*`, `groups_*`, `usermeta_*`, `bisect_*`) stay as family rows until their milestone starts.
9. **Not commands.** Names that look like commands but are settings keys or diagnostic codes are not listed: `launch_via_steam_protocol`, `steam_client_integration`, `steam_mods_update_check`, `steam_apikey` and `game_folder` (settings keys of the [settings spec](../features/settings.md)), and `patch_failed` (a log classification code of the workspace spec).
10. **Functions that are not registry rows.** `rimstudio_app::boot` and `rimstudio_app::dispatch` are plain functions; so is `rimstudio-app::dispatch_to_file` (section 4.1 note).

## 3. Names that were renamed or merged

The wire forms below are retired; each is recorded in D-072 unless noted. The specs keep the old spelling only in alias sentences.

| Retired spelling | Canonical name | Found in |
|---|---|---|
| `list_mods_snapshot`, `library_snapshot` | `mods_snapshot` | research, mod manager early draft |
| `subscribe_mods`, `library_subscribe` | `mods_subscribe` | research, mod manager early draft |
| `get_mod_detail`, `library_mod_detail` | `mods_get_detail` | research, mod manager early draft |
| `setup_detect` | `detect_run` | mod manager early draft |
| `list_enable` | `list_toggle` | mod manager early draft |
| `job_cancel` | `cancel_job` | mod manager early draft |
| `validate_list` | `diagnostics_list` | mod manager, load order |
| `validate_mute` | `diagnostics_mute` | mod manager, load order |
| `logs_tail` | `logs_follow` | workspace |
| `publish_stage` | `publish_dry_run` | workspace |
| `save_preview` | `list_save_preview` | mod manager |
| `save_apply` | `list_save` | mod manager |
| `backups_list`, `backups_restore` | `list_backups_list`, `list_backups_restore` | mod manager |
| `import_detect`, `import_preview`, `import_apply` | `list_import_detect`, `list_import_preview`, `list_import_apply` | mod manager |
| `export_render` | `list_export_render` | mod manager |
| `import_rimsort_scan`, `import_rimsort_apply` | `settings_import_rimsort_scan`, `settings_import_rimsort_apply` | settings |
| `storage_summary` | `settings_storage_summary` | settings |
| `library.delta`, `library.diagnostics` | message kinds of `ModsStreamMsg` | mod manager |
| `job.progress`, `job.finished` | `JobEvent<R>` progress and terminal variants | mod manager |
| `game.state` | broadcast `game:running-changed` | mod manager |
| `game.list-changed` | broadcast `game:list-changed` (proposed) | mod manager |
| `datasets.status` | message kind of `DatasetsStreamMsg` | mod manager |
| `log.append` | message kind of `LogsStreamMsg` | mod manager |
| `job_list` | dropped (the registry has none) | mod manager early draft |

Three additions are required by the integration decisions and are added here although no spec names them as rows: `settings_set_window_state` (D-076), `rimstudio-app::dispatch_to_file` (D-075) and `app_webview_info` (named in the note of IPC and state section 11, specified here as the one command allowed before the feature probe passes; D-072).

## 4. The catalog

Rows are grouped by area. Within a group the registered rows come first in registry order, then the proposed rows.

### 4.1 Application and infrastructure

| Command | Kind | DTOs and result | Owning crate | Milestone | Requirements | Root class | Aliases | Status |
|---|---|---|---|---|---|---|---|---|
| `app_ping` | query | `AppPing`: echo for the performance lab and tests | `rimstudio-app` | M0 | R1, R5 | none | none | registered, row 1 |
| `app_get_info` | query | `AppGetInfo`: version, os, contract hash, portable flag, webview probe data | `rimstudio-app` | M0 | R1 | none | none | registered, row 2 |
| `app_list_tools` | query | `AppListTools`: `ToolDescriptor[]` with capability availability | `rimstudio-app` | M0 | R5 | none | none | registered, row 3 |
| `cancel_job` | action | `CancelJob`: job id, resulting state | `rimstudio-app` | M0 | R5, MM-033 | none | `job_cancel` (mod manager early draft) | registered, row 4 |
| `app_webview_info` | query | `AppWebviewInfo`: probe data for the fallback screen; the only command allowed before the feature probe passes | `rimstudio-app` | M0 | R1, R5 | none | none | proposed (ipc-and-state section 11 note 4, cross-platform) |
| `app_check_update` | action | `AppCheckUpdateRequest`, `AppCheckUpdateResponse` (by convention) | `rimstudio-app` | M7 | R1 | none | none | proposed (ipc-and-state section 11 note 4) |
| `log_event` | action | `LogEventRequest`: level, component, message; rate limited and length capped | `rimstudio-app` | M2 | R5 | none | none | proposed (error-handling-and-logging) |
| `applog_query` | query | `ApplogQueryRequest`, `ApplogQueryResponse` (by convention) | `rimstudio-app` | M2 | R5 | DataRoot | none | proposed (error-handling-and-logging) |
| `applog_follow` | stream | `AppLogFollow` with `AppLogStreamMsg` | `rimstudio-app` | M2 | R5 | DataRoot | none | proposed (error-handling-and-logging) |

### 4.2 Settings and diagnostics export

| Command | Kind | DTOs and result | Owning crate | Milestone | Requirements | Root class | Aliases | Status |
|---|---|---|---|---|---|---|---|---|
| `settings_get` | query | `SettingsGet`: optional sections, `SettingsDto` without secrets | `rimstudio-manager` | M1 | R10, MM-046 | none | none | registered, row 5 |
| `settings_update` | action | `SettingsUpdate`: section patches, returns `SettingsDto` and rev | `rimstudio-manager` | M1 | R10, MM-046 | none | none | registered, row 6 |
| `settings_set_window_state` | action | `SettingsSetWindowState`: geometry, maximised flag, monitor; debounced; written to the `window` key (D-076) | `rimstudio-manager` | M2 | R10, R5 | none | none | proposed (OD-08, D-076); added by this catalog |
| `settings_list_themes` | query | `SettingsListThemesRequest`, `SettingsListThemesResponse` (by convention) | `rimstudio-manager` | M2 | R8, R10 | DataRoot | none | proposed (ipc-and-state section 11 note 4) |
| `settings_get_theme` | query | `SettingsGetThemeRequest`, `SettingsGetThemeResponse` (by convention) | `rimstudio-manager` | M2 | R8, R10 | DataRoot | none | proposed (ipc-and-state section 11 note 4) |
| `settings_get_user_css` | query | `SettingsGetUserCssRequest`, `SettingsGetUserCssResponse` (by convention) | `rimstudio-manager` | M2 | R8, R10 | DataRoot | none | proposed (ipc-and-state section 11 note 4) |
| `settings_reset` | action | `SettingsReset`: key paths | `rimstudio-manager` | M2 | R10 | none | none | proposed by settings |
| `settings_backups_list` | query | `SettingsBackupsListRequest`, `SettingsBackupsListResponse` (by convention) | `rimstudio-manager` | M2 | R10 | DataRoot | none | proposed by settings |
| `settings_backup_restore` | action | `SettingsBackupRestoreRequest`, `SettingsBackupRestoreResponse` (by convention) | `rimstudio-manager` | M2 | R10 | DataRoot | none | proposed by settings |
| `settings_import_rimsort_scan` | query | candidates and counts from a RimSort install | `rimstudio-manager` | M3 | R6, MM-043 | UserPick | `import_rimsort_scan` (settings spec) | proposed by settings |
| `settings_import_rimsort_apply` | job | result: imported, merged and skipped counts | `rimstudio-manager` | M3 | R6, MM-043 | UserPick | `import_rimsort_apply` (settings spec) | proposed by settings |
| `settings_storage_summary` | query | `SettingsStorageSummaryRequest`, `SettingsStorageSummaryResponse` (by convention) | `rimstudio-manager` | M3 | R10 | DataRoot | `storage_summary` (settings spec) | proposed by settings |
| `diagnostics_export` | action | `DiagnosticsExportRequest`, `DiagnosticsExportResponse` (by convention) | `rimstudio-manager` | M3 | R5, R10 | UserPick | none | proposed by mod manager and settings |

### 4.3 Detection, sources and library

| Command | Kind | DTOs and result | Owning crate | Milestone | Requirements | Root class | Aliases | Status |
|---|---|---|---|---|---|---|---|---|
| `detect_run` | job | `DetectRun`; result `DetectionReportDto`: all candidates with how and confidence | `rimstudio-manager` | M1 | R3, MM-001 | GameRoot | `setup_detect` (mod manager early draft) | registered, row 7 |
| `detect_get_report` | query | `DetectGetReport`: last report, cached | `rimstudio-manager` | M1 | R3, MM-001 | none | none | registered, row 8 |
| `detect_set_override` | action | `DetectSetOverride`: path field and `UserPathDto` | `rimstudio-manager` | M1 | R3, MM-001 | UserPick | none | registered, row 9 |
| `sources_list` | query | `SourcesList`: install, workshop and custom folders with status | `rimstudio-manager` | M1 | R3, R4, MM-002 | none | none | registered, row 10 |
| `sources_add_folder` | action | `SourcesAddFolder`: `UserPathDto`, label; returns `SourceDto` | `rimstudio-manager` | M1 | R4, MM-002 | UserPick | none | registered, row 11 |
| `sources_update` | action | `SourcesUpdate`: label, enabled, order | `rimstudio-manager` | M1 | R4, MM-002 | none | none | registered, row 12 |
| `sources_remove` | action | `SourcesRemove`: `SourceId`; never touches files | `rimstudio-manager` | M1 | R4, MM-002 | none | none | registered, row 13 |
| `sources_probe_folder` | query | `SourcesProbeFolder`: looks like a mods folder, mod count estimate, removable drive flag, warnings | `rimstudio-manager` | M1 | R4, MM-002 | UserPick | none | registered, row 14 |
| `library_scan` | job | `LibraryScan`: `libraryId?`, `full?`; result `LibraryScanResult` | `rimstudio-manager` (engine `rimstudio-library`) | M1 | R3, R4, R5, MM-003 | LibraryRoot | none | registered, row 15 |
| `library_get_status` | query | `LibraryGetStatus`: session id, rev, scan state, game version | `rimstudio-manager` | M1 | R5, MM-003 | none | none | registered, row 16 |
| `library_query` | query | `LibraryQuery`: search text and filters; returns a `ModIdx` array and ranges | `rimstudio-manager` | M2 | R5, MM-009, MM-010 | none | none | proposed by mod manager |
| `library_folder_size` | job | `LibraryFolderSize`: `ModId`; disk walk, so a job | `rimstudio-manager` (engine `rimstudio-library`) | M3 | R5, MM-007 | LibraryRoot | none (the mod manager spec lists it as Q, J) | proposed by mod manager |

### 4.4 Mod list session

| Command | Kind | DTOs and result | Owning crate | Milestone | Requirements | Root class | Aliases | Status |
|---|---|---|---|---|---|---|---|---|
| `mods_snapshot` | query | `ModsSnapshot`: `sessionId`, `rev`, rows, active order, counts | `rimstudio-manager` | M2 | R5, MM-003 | none | `list_mods_snapshot` (research), `library_snapshot` (mod manager early draft) | registered, row 17 |
| `mods_subscribe` | stream | `ModsSubscribe`: `sessionId`, `sinceRev`; messages `ModsStreamMsg` | `rimstudio-manager` | M2 | R5, MM-003, MM-041 | none | `subscribe_mods` (research), `library_subscribe` | registered, row 18 |
| `mods_get_detail` | query | `ModsGetDetail`: `ModId`; response `ModDetailDto` | `rimstudio-manager` | M2 | R5, MM-007 | none | `get_mod_detail` (research), `library_mod_detail` | registered, row 19 |
| `list_toggle` | action | `ListToggle`: `ModIdx[]`, `active`, `expectedRev?`; response `ListEditResponse` | `rimstudio-manager` | M2 | R5, MM-012, MM-014 | none | `list_enable` (mod manager early draft) | registered, row 20 |
| `list_move` | action | `ListMove`: `ModIdx[]`, target or relative step, `expectedRev?`; response `ListEditResponse` | `rimstudio-manager` | M2 | R5, MM-012, MM-013, LO-017 | none | none | registered, row 21 |
| `list_set_active` | action | `ListSetActive`: full active order; response `ListEditResponse` | `rimstudio-manager` | M2 | R5, MM-014, MM-023 | none | none | registered, row 22 |
| `list_undo` | action | `ListUndo`; response `ListEditResponse` | `rimstudio-manager` | M2 | R5, MM-014 | none | none | registered, row 23 |
| `list_redo` | action | `ListRedo`; response `ListEditResponse` | `rimstudio-manager` | M2 | R5, MM-014 | none | none | registered, row 24 |
| `list_save` | action | `ListSave`: target `ModsConfig` write; response `ListSaveResponse` (backup path id, blocking diagnostics) | `rimstudio-manager` (engine `rimstudio-library::deploy`) | M2 | R5, MM-031 | GameRoot | `save_apply` (mod manager spec) | registered, row 25 |
| `list_clear` | action | `ListClearRequest`, `ListClearResponse` (by convention) | `rimstudio-manager` | M2 | R5, MM-014 | none | none | proposed by mod manager |
| `list_diff` | query | `ListDiffRequest`, `ListDiffResponse` (by convention) | `rimstudio-manager` | M2 | R5, MM-015 | none | none | proposed by mod manager |
| `list_reload_game_list` | action | `ListReloadGameListRequest`, `ListReloadGameListResponse` (by convention) | `rimstudio-manager` | M2 | R5, MM-015 | GameRoot | none | proposed by mod manager |
| `list_move_bounds` | query | allowed interval for a move of selected mods | `rimstudio-manager` | M2 | R5, LO-017 | none | none | proposed by load order |
| `list_save_preview` | query | link to create and list diff shown before a save | `rimstudio-manager` | M2 | R5, MM-031 | GameRoot | `save_preview` (mod manager spec) | proposed by mod manager |
| `list_backups_list` | query | `ListBackupsListRequest`, `ListBackupsListResponse` (by convention) | `rimstudio-manager` | M2 | R5, MM-031 | DataRoot | `backups_list` | proposed by mod manager |
| `list_backups_restore` | action | `ListBackupsRestoreRequest`, `ListBackupsRestoreResponse` (by convention) | `rimstudio-manager` | M2 | R5, MM-031 | GameRoot | `backups_restore` | proposed by mod manager |
| `list_import_detect` | query | format detection of a pasted or picked list | `rimstudio-manager` | M2 | R6, MM-023 | UserPick | `import_detect` | proposed by mod manager |
| `list_import_preview` | query | diff preview of an import | `rimstudio-manager` | M2 | R6, MM-023 | UserPick | `import_preview` | proposed by mod manager |
| `list_import_apply` | action | `ListImportApplyRequest`, `ListImportApplyResponse` (by convention) | `rimstudio-manager` | M2 | R6, MM-023 | none | `import_apply` | proposed by mod manager |
| `list_export_render` | query | rendered export text or file body | `rimstudio-manager` | M2 | R5, MM-024 | none | `export_render` | proposed by mod manager |

### 4.5 Sorting, diagnostics, profiles and other manager use cases

| Command | Kind | DTOs and result | Owning crate | Milestone | Requirements | Root class | Aliases | Status |
|---|---|---|---|---|---|---|---|---|
| `sort_preview` | query (must be promotable to a job) | `SortPreview`: mode canonical or game-style; response `SortPreviewResponse`: `ModIdx[]`, moves with reasons, cycles | `rimstudio-manager` (engine `rimstudio-sort`) | M2 | R5, MM-017, LO-025 | none | none | registered, row 26 |
| `sort_apply` | action | `SortApply`: preview token; response `ListEditResponse` | `rimstudio-manager` | M2 | R5, MM-017, LO-026 | none | none | registered, row 27 |
| `sort_explain` | query | `explain(x, y)` and `explain_position(x)` results | `rimstudio-manager` (engine `rimstudio-rules`) | M2 | R5, MM-018, LO-023, LO-024 | none | none | proposed by load order and mod manager |
| `diagnostics_for_mod` | query (promotable to a job) | `DiagnosticsForMod`: `ModId`; `Diagnostic[]` with provenance | `rimstudio-manager` (engine `rimstudio-validate`) | M2 | R5, MM-016, LO-019 | none | none | registered, row 28 |
| `diagnostics_list` | query | full diagnostics for the working list | `rimstudio-manager` (engine `rimstudio-validate`) | M2 | R5, MM-016, LO-019 | none | `validate_list` | proposed by load order and mod manager |
| `diagnostics_mute` | action | mute or unmute an ignore entry (mod id, code) | `rimstudio-manager` | M2 | R5, MM-016, LO-021 | none | `validate_mute` | proposed by load order and mod manager |
| `profiles_list` | query | `ProfilesList`; `ProfileDto[]` | `rimstudio-manager` | M2 | R5, MM-021 | none | none | registered, row 29 |
| `profiles_save` | action | `ProfilesSave`: name, from current list; `ProfileDto` | `rimstudio-manager` | M2 | R5, MM-021 | none | none | registered, row 30 |
| `profiles_load` | action | `ProfilesLoad`: `ProfileId`, missing mod policy; `ListEditResponse` plus missing ids | `rimstudio-manager` | M2 | R5, MM-021, MM-026 | none | none | registered, row 31 |
| `profiles_delete` | action | `ProfilesDeleteRequest`, `ProfilesDeleteResponse` (by convention) | `rimstudio-manager` | M3 | R5, MM-021 | none | none | proposed by mod manager |
| `history_*` | family (query, action) | snapshot, list, diff, restore, annotate (names fixed at M3) | `rimstudio-manager` | M3 | R5, MM-022 | none | none | proposed by mod manager (family) |
| `groups_*` | family (action) | group edits, user data writes (names fixed at M3) | `rimstudio-manager` | M3 | R5, MM-019 | none | none | proposed by mod manager (family) |
| `usermeta_*` | family (action) | tags, colours, notes (names fixed at M2 and M3) | `rimstudio-manager` | M2 | R5, MM-020 | none | none | proposed by mod manager (family) |
| `duplicates_list` | query | `DuplicatesListRequest`, `DuplicatesListResponse` (by convention) | `rimstudio-manager` | M2 | R5, MM-027 | none | none | proposed by mod manager |
| `duplicates_resolve` | action | `DuplicatesResolveRequest`, `DuplicatesResolveResponse` (by convention) | `rimstudio-manager` | M2 | R5, MM-027 | none | none | proposed by mod manager |
| `bisect_*` | family (action, query) | split, verdict, finish (names fixed at M3) | `rimstudio-manager` | M3 | R5, MM-032 | none | none | proposed by mod manager (family) |

### 4.6 Launch and deployment

| Command | Kind | DTOs and result | Owning crate | Milestone | Requirements | Root class | Aliases | Status |
|---|---|---|---|---|---|---|---|---|
| `launch_check` | job | pre-launch check outcome | `rimstudio-manager` | M2 | R5, MM-029, MM-030 | GameRoot | none | proposed by mod manager |
| `launch_start` | action | `LaunchStart`: launch arguments, via Steam or direct; `LaunchStartResponse`: pre-launch check outcome | `rimstudio-manager` | M2 | R5, MM-029, WS | GameRoot | none | registered, row 38 |
| `launch_state` | stream | `LaunchState` with `LaunchStreamMsg`: running, pid | `rimstudio-manager` | M2 | R5, MM-029 | none | event `game.state` | proposed by mod manager |
| `deploy_plan` | query | `DeployPlan`: active list; `DeployPlanResponse`: links to create and remove, fallback copies, blockers | `rimstudio-manager` (engine `rimstudio-library::deploy`) | M2 | R4, MM-002, MM-031, GD | LibraryRoot | none | registered, row 36 |
| `deploy_apply` | job | `DeployApply`: plan token; result links created and removed, manifest id | `rimstudio-manager` (engine `rimstudio-library::deploy`) | M2 | R4, MM-030, MM-031, GD | GameRoot | none | registered, row 37 |
| `deploy_unlink_all` | action | `DeployUnlinkAllRequest`, `DeployUnlinkAllResponse` (by convention) | `rimstudio-manager` (engine `rimstudio-library::deploy`) | M2 | R4, MM-002, MM-031 | GameRoot | none | proposed by discovery and mod manager |
| `deploy_audit` | query | `DeployAuditRequest`, `DeployAuditResponse` (by convention) | `rimstudio-manager` (engine `rimstudio-library::deploy`) | M2 | R4, GD | GameRoot | none | proposed by discovery |

### 4.7 Datasets and rules

| Command | Kind | DTOs and result | Owning crate | Milestone | Requirements | Root class | Aliases | Status |
|---|---|---|---|---|---|---|---|---|
| `datasets_status` | query | `DatasetsStatus`: `{ rev, items }` of `DatasetStatusDto` | `rimstudio-manager` (engine `rimstudio-datasets`) | M2 | R6, MM-039, CD-003 | none | none | registered, row 32 |
| `datasets_subscribe` | stream | `DatasetsSubscribe`: `sinceRev`; messages `DatasetsStreamMsg` | `rimstudio-manager` | M2 | R6, CD-003 | none | event `datasets.status` | registered, row 33 |
| `datasets_refresh` | job | `DatasetsRefresh`: `datasetIds?`, `force?`; result per dataset outcome | `rimstudio-manager` (engine `rimstudio-datasets`) | M2 | R6, CD-006 | DataRoot | none | registered, row 34 |
| `datasets_set_source` | action | `DatasetsSetSourceRequest`, `DatasetsSetSourceResponse` (by convention) | `rimstudio-manager` | M3 | R6, CD-005 | none | none | proposed by datasets and mod manager |
| `datasets_validate_source` | query | `DatasetsValidateSourceRequest`, `DatasetsValidateSourceResponse` (by convention) | `rimstudio-manager` | M3 | R6, CD-005 | none | none | proposed by datasets |
| `datasets_set_enabled` | action | `DatasetsSetEnabledRequest`, `DatasetsSetEnabledResponse` (by convention) | `rimstudio-manager` | M3 | R6, CD-006 | none | none | proposed by datasets |
| `datasets_revert` | action | `DatasetsRevertRequest`, `DatasetsRevertResponse` (by convention) | `rimstudio-manager` | M3 | R6, CD-004 | DataRoot | none | proposed by datasets and mod manager |
| `datasets_changelog` | job | `DatasetsChangelogRequest`, `DatasetsChangelogResponse` (by convention) | `rimstudio-manager` | M3 | R6, CD-004 | DataRoot | none | proposed by datasets |
| `rules_import_rimsort` | job | `RulesImportRimsort`: `UserPathDto` of the user rules file; result import counts | `rimstudio-manager` (engine `rimstudio-datasets`) | M2 | R6, CD-017, MM-043 | UserPick | none | registered, row 35 |
| `rules_user_get` | query | `RulesUserGetRequest`, `RulesUserGetResponse` (by convention) | `rimstudio-manager` (engine `rimstudio-rules`) | M3 | R6, CD-013, CD-014 | DataRoot | none | proposed by datasets |
| `rules_user_edit` | action | `RulesUserEditRequest`, `RulesUserEditResponse` (by convention) | `rimstudio-manager` (engine `rimstudio-rules`) | M3 | R6, CD-014, MM-045 | DataRoot | none | proposed by datasets |
| `rules_suppress` | action | `RulesSuppressRequest`, `RulesSuppressResponse` (by convention) | `rimstudio-manager` | M3 | R6, CD-015 | DataRoot | none | proposed by datasets |
| `rules_unsuppress` | action | `RulesUnsuppressRequest`, `RulesUnsuppressResponse` (by convention) | `rimstudio-manager` | M3 | R6, CD-015 | DataRoot | none | proposed by datasets |
| `rules_classify` | job | `RulesClassifyRequest`, `RulesClassifyResponse` (by convention) | `rimstudio-manager` (engine `rimstudio-rules`) | M3 | R6, CD-016 | none | none | proposed by datasets |
| `rules_export_patch` | job | `RulesExportPatchRequest`, `RulesExportPatchResponse` (by convention) | `rimstudio-manager` (engine `rimstudio-rules`) | M3 | R6, CD-018 | UserPick | none | proposed by datasets |

### 4.8 Logs

| Command | Kind | DTOs and result | Owning crate | Milestone | Requirements | Root class | Aliases | Status |
|---|---|---|---|---|---|---|---|---|
| `logs_open` | job | parse a log file with progress | `rimstudio-toolkit` (engine `rimstudio-validate`) | M3 | R5, MM-035, WS | UserPick | none | proposed by mod manager and workspace |
| `logs_page` | query | paged blocks | `rimstudio-toolkit` | M3 | R5, MM-035 | none | none | proposed by mod manager |
| `logs_follow` | stream | `LogsFollow` with `LogsStreamMsg`: new blocks, chunks of 250 to 1000 records | `rimstudio-toolkit` | M3 | R5, MM-035, WS | none | `logs_tail` (workspace spec), event `log.append` | proposed by mod manager and workspace |
| `logs_block` | query | one block with stack | `rimstudio-toolkit` | M4 | R5, WS | none | none | proposed by workspace |
| `logs_patterns` | query | classification catalogue info | `rimstudio-toolkit` | M4 | R5, WS | none | none | proposed by workspace |

### 4.9 Def explorer, patch tester and project tools

| Command | Kind | DTOs and result | Owning crate | Milestone | Requirements | Root class | Aliases | Status |
|---|---|---|---|---|---|---|---|---|
| `defs_open_session` | job | `DefsOpenSession`: reference set, optional project id; result `{ sessionId, defCount }` | `rimstudio-toolkit` (engine `rimstudio-workspace`) | M4 | R5, WS | GameRoot | none | registered, row 39 |
| `defs_search` | query | `DefsSearch`; `DefsSearchResponse`: `queryId`, total, items `DefRowDto[]` | `rimstudio-toolkit` (engine `rimstudio-defs`) | M4 | R5, WS | none | none | registered, row 40 |
| `defs_get_resolved` | query | resolved node tree as JSON, patch event list, element origins | `rimstudio-toolkit` | M4 | R5, WS | none | none | proposed (ipc-and-state section 6.4, workspace spec) |
| `defs_find_references` | query | paged inbound and outbound references | `rimstudio-toolkit` | M4 | R5, WS | none | none | proposed (ipc-and-state section 6.4, workspace spec) |
| `defs_close_session` | action | frees the snapshot | `rimstudio-toolkit` | M4 | R5, WS | none | none | proposed (ipc-and-state section 6.4) |
| `defs_build_snapshot` | job | full `DefDatabases` snapshot (OD-10: DTOs not yet specified) | `rimstudio-toolkit` (engine `rimstudio-workspace`) | M4 | R5, WS | none | none | proposed (ipc-and-state section 11 note 4, OD-10) |
| `defs_get_provenance` | query | def, patch event and element origin levels (D-017) | `rimstudio-toolkit` | M4 | R5, WS | none | none | proposed by workspace |
| `defs_tree` | query | children and parents | `rimstudio-toolkit` | M4 | R5, WS | none | none | proposed by workspace |
| `defs_compare` | query | `DefsCompareRequest`, `DefsCompareResponse` (by convention) | `rimstudio-toolkit` | M4 | R5, WS | none | none | proposed by workspace |
| `patch_test_run` | query (promotable to a job) | replay is a job; one operation under 1 ms stays inline | `rimstudio-toolkit` (engines `rimstudio-xpath`, `rimstudio-defs`) | M4 | R5, WS | none | none | proposed by workspace (spec says job for replay, query for one operation) |
| `patch_test_explain` | query | why a node did or did not match | `rimstudio-toolkit` | M4 | R5, WS | none | none | proposed by workspace |
| `project_templates_list` | query | `ProjectTemplatesListRequest`, `ProjectTemplatesListResponse` (by convention) | `rimstudio-toolkit` | M4 | R5, WS-001 | none | none | proposed by workspace |
| `project_scaffold_plan` | query | returns a write plan | `rimstudio-toolkit` | M4 | R5, WS-001 | ProjectRoot | none | proposed by workspace |
| `project_create` | action | applies the scaffold plan | `rimstudio-toolkit` | M4 | R5, WS-001 | UserPick | none | proposed by workspace |
| `project_open` | action | open a folder, create or load the record, return the project summary | `rimstudio-toolkit` | M4 | R5, WS | UserPick | none | proposed by workspace |
| `project_close` | action | release the session | `rimstudio-toolkit` | M4 | R5, WS | ProjectRoot | none | proposed by workspace |
| `project_tree` | query | the annotated folder tree of a project: role, size and issue count per entry, totals, layout profile (mod layout section 7) | `rimstudio-toolkit` | M4 | R5, WS-001 | ProjectRoot | none | as built (0.1.0 UI work, ADR 0041) |
| `project_layout_check` | query | the layout issues with a suggested fix each and whether the fix is automatic (mod layout section 8) | `rimstudio-toolkit` | M4 | R5, WS-001 | ProjectRoot | none | as built (0.1.0 UI work, ADR 0041) |
| `project_scaffold_missing` | action | creates the missing standard folders only, never a file, never overwrites; `dryRun` lists them | `rimstudio-toolkit` | M4 | R5, WS-001 | ProjectRoot | none | as built (0.1.0 UI work, ADR 0041) |
| `project_read_file` | query | one text file of the project by relative path through the guarded writer, size limited, for the file viewer | `rimstudio-toolkit` | M4 | R5, WS-001 | ProjectRoot | none | as built (0.1.0 UI work, ADR 0041) |
| `project_layout_fix_plan` | query | the fixable findings of the layout check as items (kind, from, to, why, risk, `LoadFolders.xml` diff, conflict, references) with a content hash as plan id; read only (mod layout section 14) | `rimstudio-toolkit` | M4 | R5, WS-001 | ProjectRoot | none | as built (ADR 0046) |
| `project_layout_fix_apply` | job | carries out the selected items of a reviewed plan, refuses a stale plan id, writes an undo journal first, runs the check again | `rimstudio-toolkit` | M4 | R5, WS-001 | ProjectRoot | none | as built (ADR 0046) |
| `project_layout_fix_undo` | action | reverses an apply from its journal, or refuses and changes nothing | `rimstudio-toolkit` | M4 | R5, WS-001 | ProjectRoot | none | as built (ADR 0046) |
| `project_layout_fix_history` | query | the journals of a project with whether each can still be undone | `rimstudio-toolkit` | M4 | R5, WS-001 | ProjectRoot | none | as built (ADR 0046) |
| `project_import_legacy` | action | read an old project file and map the fields for review | `rimstudio-toolkit` | M4 | R5, WS | ProjectRoot | none | proposed by workspace |
| `project_about_read` | query | model plus diagnostics | `rimstudio-toolkit` | M4 | R5, WS | ProjectRoot | none | proposed by workspace |
| `project_about_plan_edit` | query | returns a write plan | `rimstudio-toolkit` | M4 | R5, WS | ProjectRoot | none | proposed by workspace |
| `project_load_plan` | query | effective files for a version and mod set | `rimstudio-toolkit` | M4 | R5, WS | ProjectRoot | none | proposed by workspace |
| `project_loadfolders_plan_edit` | query | returns a write plan | `rimstudio-toolkit` | M4 | R5, WS | ProjectRoot | none | proposed by workspace |
| `project_version_folders` | query | `ProjectVersionFoldersRequest`, `ProjectVersionFoldersResponse` (by convention) | `rimstudio-toolkit` | M4 | R5, WS | ProjectRoot | none | proposed by workspace |
| `project_description_convert` | query | BBCode, Unity rich text and markdown conversion; no network | `rimstudio-toolkit` | M4 | R5, WS | none | none | proposed by workspace |
| `project_dev_launch_plan` | query | launch plan; no process is started | `rimstudio-toolkit` | M4 | R5, WS | ProjectRoot | none | proposed by workspace |
| `project_dev_restore_list` | action | `ProjectDevRestoreListRequest`, `ProjectDevRestoreListResponse` (by convention) | `rimstudio-toolkit` | M4 | R5, WS | GameRoot | none | proposed by workspace |

### 4.10 Item designer

The designer always writes vanilla definitions; a Combat Extended patch is an optional, opt in toggle per item (`cePatch`, default false, D-085) and is never automatic. Release 0.1.0 builds the weapons part of these rows backend first and reaches them through the CLI ([ADR 0036](../adr/0036-release-scope-and-backend-first.md)).

| Command | Kind | DTOs and result | Owning crate | Milestone | Requirements | Root class | Aliases | Status |
|---|---|---|---|---|---|---|---|---|
| `designer_reference_list` | query | reference items of a kind and class with index, tier, role, stats | `rimstudio-toolkit` (engine `rimstudio-design`) | M5 | R7, IT | none | none | proposed by items toolkit |
| `designer_preview` | query | exact readouts, suggestions and bands; at most 1 ms Rust work | `rimstudio-toolkit` (engine `rimstudio-design`) | M5 | R7, IT | none | none | proposed by items toolkit |
| `designer_fit` | query | fit meter data: bands, ranks, typicality | `rimstudio-toolkit` | M5 | R7, IT | none | none | proposed by items toolkit |
| `designer_ce_suggest` | query | suggestions for the optional CE block of a draft: value, source, band, rating, asks; works with the toggle off and never turns it on | `rimstudio-toolkit` (engine `rimstudio-design`) | M5 | R7, IT, CE | none | none | proposed by items toolkit and CE patching |
| `designer_ce_ammo_catalog` | query | every ammo set of the installed Combat Extended with ammo types, projectile numbers, caliber group, generic and similar to relations, weapon counts and a suggested flag for a draft; searchable, filterable, paged; works with the toggle off | `rimstudio-toolkit` (engine `rimstudio-design`) | M5 | R7, IT, CE | none | none | owner request 2026-10-05 |
| `designer_ce_ammo_suggest` | query | defaults for a new custom ammo type from the nearest of the user's own ammunition (source and rating per field), or a copy of an existing ammo type | `rimstudio-toolkit` (engine `rimstudio-design`) | M5 | R7, IT, CE | none | none | owner request 2026-10-05 |
| `designer_quiz_next` | query | next question for a draft | `rimstudio-toolkit` | M5 | R7, IT | none | none | proposed by items toolkit |
| `designer_quiz_answer` | action | apply an answer, return the updated estimate | `rimstudio-toolkit` | M5 | R7, IT | none | none | proposed by items toolkit |
| `designer_quiz_back` | action | drop the last answer of the draft's quiz and return the draft and the new step (`DesignerQuizBackRequest { draft }` returns `DesignerQuizAnswerResponse`); `designer.quiz-wrong-answer` when no answer of the user is left to take back | `rimstudio-toolkit` | M5 | R7, IT | none | none | proposed by items toolkit |
| `designer_material_matrix` | query (job if the bench exceeds 1 ms) | stuff by quality grid, about 330 evaluations | `rimstudio-toolkit` | M5 | R7, IT | none | none | proposed by items toolkit |
| `designer_calibrate` | job | build pools and run the leave-one-out harness; cached | `rimstudio-toolkit` (engine `rimstudio-design`) | M5 | R7, IT | none | none | proposed by items toolkit and item balance math |
| `designer_convert_scan` | job | list convertible defs of a project with status | `rimstudio-toolkit` | M5 | R7, IT, CE | ProjectRoot | none | proposed by items toolkit and CE patching |
| `designer_export_plan` | query | write plan and rendered preview; pure; vanilla files always, CE patch files only when the draft's `cePatch` toggle is on | `rimstudio-toolkit` | M5 | R7, IT, CE | ProjectRoot | none | proposed by items toolkit and CE patching |
| `designer_apply_plan` | job | write, back up, re-read, dry apply | `rimstudio-toolkit` | M5 | R7, IT, CE | ProjectRoot | none | proposed by items toolkit and CE patching |
| `designer_draft_save` | action | drafts in the `designer-drafts` collection of the JSON document store (D-083) | `rimstudio-toolkit` | M5 | R7, IT | ProjectRoot | none | proposed by items toolkit |
| `designer_draft_list` | query | `DesignerDraftListRequest`, `DesignerDraftListResponse` (by convention) | `rimstudio-toolkit` | M5 | R7, IT | ProjectRoot | none | proposed by items toolkit |
| `designer_draft_delete` | action | `DesignerDraftDeleteRequest`, `DesignerDraftDeleteResponse` (by convention) | `rimstudio-toolkit` | M5 | R7, IT | ProjectRoot | none | proposed by items toolkit |
| `designer_clone` | action | flow C: `DesignerCloneRequest { projectId, source, defName, label?, modPrefix? }` returns `DesignerCloneResponse { entry, notes }`; copies every modelled field of a loaded weapon into a new stored draft, source as first anchor and `clonedFrom`, Combat Extended toggle off | `rimstudio-toolkit` | M5 | R7, IT | ProjectRoot | none | proposed by items toolkit |
| `designer_clone_diff` | query | `DesignerCloneDiffRequest { draft }` returns `DesignerCloneDiffResponse { source, sourceLabel, changes, readouts, notes }`: the changed fields of a clone against its source with the effect on the exact readouts | `rimstudio-toolkit` | M5 | R7, IT | none | none | proposed by items toolkit |
| `designer_structure_defaults` | query | `DesignerStructureDefaultsRequest { draft }` returns `DesignerStructureDefaultsResponse { draft, reference?, filled, notes }`: parent, projectile, cost list and stuff of a new weapon from the nearest reference weapon, as suggestions | `rimstudio-toolkit` | M5 | R7, IT | none | none | proposed by items toolkit |
| `designer_projectile_own` | query | `DesignerProjectileOwnRequest { draft, own }` returns `DesignerProjectileOwnResponse { draft, notes }`: gives a gun a projectile of its own copied from the one it fires (new name derived from the weapon), or points it back at the shared one; the answer is not stored | `rimstudio-toolkit` | M5 | R7, IT | none | none | proposed by items toolkit |
| `designer_asset_info` | query | `DesignerAssetInfoRequest { path, projectId? }` returns `DesignerAssetInfoResponse { path, status, kind?, bytes?, sha256?, width?, height?, channels?, sampleRate?, durationMs?, preview?, diagnostics }`: the facts of a texture or sound clip file offered for import, read under the size limits and never decoded; a thumbnail data URL for a PNG up to 256 KiB (items toolkit section 8.6) | `rimstudio-toolkit` | M5 | R7, IT-093 | none | none | as built (ADR 0047) |

### 4.11 Workshop publishing

| Command | Kind | DTOs and result | Owning crate | Milestone | Requirements | Root class | Aliases | Status |
|---|---|---|---|---|---|---|---|---|
| `publish_projects` | query | candidate projects with status chips | `rimstudio-publish` | M6 | R9, WP | ProjectRoot | none | proposed by publishing |
| `publish_state_get` | query | state, history summary, recovery banner | `rimstudio-publish` | M6 | R9, WP | ProjectRoot | none | proposed by publishing |
| `publish_plan` | job | build the plan, run local preflight; result `PublishPlan` with `planId` | `rimstudio-publish` | M6 | R9, WP | ProjectRoot | none | proposed by publishing and workspace |
| `publish_ignore_edit` | action | CST edit of `uploadIgnore` | `rimstudio-publish` | M6 | R9, WP | ProjectRoot | none | proposed by publishing |
| `publish_metadata_save` | action | metadata and drafts into `state.json` | `rimstudio-publish` | M6 | R9, WP | ProjectRoot | none | proposed by publishing |
| `publish_dry_run` | job | stage, verify, write the report | `rimstudio-publish` | M6 | R9, WP, WS | ProjectRoot | `publish_stage` (workspace spec) | proposed by publishing |
| `publish_probe` | job | helper probe or identity, then Steam dependent checks | `rimstudio-publish` | M6 | R9, WP | ProjectRoot | none | proposed by publishing |
| `publish_start` | job | stage, connect, create or update, submit; streams progress as `JobEvent` | `rimstudio-publish` | M6 | R9, WP | ProjectRoot | none | proposed by publishing |
| `publish_cancel` | action | same as `cancel_job` for a publish job; kills the helper | `rimstudio-publish` | M6 | R9, WP | none | none | proposed by publishing (a thin alias of `cancel_job` that also stops the helper) |
| `publish_history` | query | history entries and manifest file lists | `rimstudio-publish` | M6 | R9, WP | ProjectRoot | none | proposed by publishing |
| `publish_resume` | job | resume after a crash | `rimstudio-publish` | M6 | R9, WP | ProjectRoot | none | proposed by publishing |
| `publish_vdf_export` | action | write the SteamCMD VDF and instructions (WP-036) | `rimstudio-publish` | M6 | R9, WP | UserPick | none | proposed by publishing |

Notes on the tables.

1. `rimstudio-app::dispatch_to_file` is a function, not a command. It takes the same arguments as `rimstudio_app::dispatch` (context, command name, JSON request) plus a destination chosen by the user, runs the handler, and writes the JSON result through `rimstudio-io::atomic_write` so the CLI can write its `--out` files without a clippy exception (OD-03, D-075). The destination is a `UserPick` path validated by `RootGuard`; the function has no webview meaning and is therefore not in the registry table.
2. `settings_set_window_state` is called by a window listener in `shared/platform` after a debounce; it merges geometry into the `window` key of `settings.jsonc` through the CST writer of the settings use case (D-076).
3. `app_webview_info` is the only command that the shell allows before the feature probe passes, so the fallback screen can tell the person what is missing. Every other command waits for the probe ([cross-platform](cross-platform.md) section 5).
4. `rimstudio-library::deploy` is an engine inside `rimstudio-library`; the handlers for `deploy_*` sit in the manager `api` module because the manager owns the save and launch flow and the registry may name feature crates only inside `rimstudio-app`.
5. `launch_start` is used by the manager (MM-029) and by the workspace dev launcher (`project_dev_launch_plan` builds the plan, `launch_start` runs it); the root class is `GameRoot` because it reads the game folder and launches the executable.

### 4.12 Handlers built for 0.1.0

The backend of release 0.1.0 implements the handlers below as plain functions; `rimstudio-app` (not built yet) owns the registry rows and the mapping from engine types to DTOs. Where a name or shape differs from the rows above, this table is the truth for the Rust side and the rows keep the wire name. The six rows added here (the five extra designer rows and `job_status`) are counted in section 8 but have no row in the tables above; they join those tables when their milestone starts.

| Wire command | Rust function | Request and result as built | Notes |
|---|---|---|---|
| `settings_get`, `settings_update` | `rimstudio_manager::settings::{get, update}` | `SettingsGetRequest { sections }` and `SettingsUpdateRequest { patches }` return a `SettingsView` (settings, effective values, problems, origin, read only flag, rev); the app maps it to `SettingsDto::from_core` | custom folders and path overrides live in `workspace.jsonc`; the DTO is a flattened view without window geometry, launch, deploy and datasets |
| `detect_run`, `detect_get_report`, `detect_set_override` | `rimstudio_manager::detect::{run, get_report, set_override}` | `DetectRunRequest`, `DetectGetReportRequest`, `DetectSetOverrideRequest { field, path, pinned }` return `DetectRunResponse` or `DetectGetReportResponse` carrying the engine `DetectionReport` | a `None` path clears an override; `How` is written `library-folders-vdf` in the DTO |
| `sources_list`, `sources_add_folder`, `sources_update`, `sources_remove`, `sources_probe_folder` | `rimstudio_manager::sources::*` | `Sources*Request` and `Sources*Response` structs of the manager | built in sources accept only `enabled` |
| `library_scan` | `rimstudio_manager::scan::library_scan(ctx, LibraryScanRequest, &dyn ProgressSink, &CancelToken)` | `LibraryScanResult` (not serialisable: the caller keeps the `LibraryIndex`; counts, timings, diagnostics and sources are) | job body; a cancelled scan does not save the manifest |
| `defs_search`, `defs_get_resolved` | `rimstudio_toolkit::defs::{search, resolve}` | `DefSearchRequest` returns `DefPage`; `DefsGetResolvedRequest` returns `ResolvedDefDto` | take a `&WorkspaceSession`; `defs_open_session` and `defs_close_session` are session handling of the app |
| `project_tree`, `project_layout_check` | `rimstudio_toolkit::project::{tree, layout_check}` | `ProjectTreeRequest { projectId, maxNodes? }` returns `ProjectTreeDto` (annotated tree, counts, profile, Combat Extended folder state, issues); `ProjectLayoutCheckRequest` returns `ProjectLayoutCheckDto` (issues, counts per severity, `autoFixable`) | read only; the scan never follows a link; issue codes `layout.*` ([mod layout](../features/mod-layout.md) section 8) |
| `project_scaffold_missing`, `project_read_file` | `rimstudio_toolkit::project::{scaffold_missing::scaffold_missing, read::read_file}` | `ProjectScaffoldMissingRequest { projectId, dryRun }` returns `ProjectScaffoldMissingDto { folders, files, skipped }`; `ProjectReadFileRequest { projectId, path, maxBytes? }` returns `ProjectFileDto { path, role, bytes, text, truncated, binary }` | both go through the guarded writer; folders only, never a file; reads are size limited (262144 default, 1048576 at most) |
| `project_layout_fix_plan`, `project_layout_fix_apply`, `project_layout_fix_undo`, `project_layout_fix_history` | `rimstudio_toolkit::project::{fix::plan, fix_apply::apply, undo::undo, history::history}` | `ProjectLayoutFixPlanRequest { projectId, fixes? }` returns `ProjectLayoutFixPlanDto { planId, items, safe, needsReview, conflicts }`; `ProjectLayoutFixApplyRequest { projectId, planId, items: [{ id, renameOnConflict }] }` returns `ProjectLayoutFixApplyDto { applyId, done, edited, skipped, cancelled, check }`; `ProjectLayoutFixUndoRequest { projectId, applyId }` returns `ProjectLayoutFixUndoDto { movedBack, restored, removedFolders, check }`; `ProjectLayoutFixHistoryRequest { projectId }` returns `ProjectLayoutFixHistoryDto { journals }` | apply is a job (progress per item, cancellation between items); errors `designer.plan-stale`, `project.fix-not-found`, `project.fix-journal-damaged`, `project.fix-undo-refused`, `project.path-outside-root` |
| `project_open`, `project_close` | `rimstudio_toolkit::project::{open, close}` | `ProjectOpenRequest` returns `ProjectSummaryDto`; `ProjectCloseRequest` returns `ProjectCloseResponse` | close reports whether the id is registered and keeps the record |
| `project_scaffold_plan`, `project_create` | `rimstudio_toolkit::project::{create_plan, create}` | `ScaffoldSpec` returns a plan; `create(&ProjectEnv, &ScaffoldSpec, protected)` returns `CreateReport` | `ProjectRoot` and protected paths are checked by the guarded writer |
| `designer_reference_list` | `designer::reference_list` | `DesignerReferenceListRequest` returns `ReferenceListDto` | extra row `designer_reference_list_ce` (`reference_list_ce`) for the CE pools |
| `designer_preview` | `designer::preview` | `DesignerPreviewRequest` returns `PreviewDto` (readouts, suggestions with source label, band and lock, estimate summary, anchor cards) | extra row `designer_suggest_fill` (`suggest_fill`: writes suggestions into a draft) |
| `designer_fit` | `designer::fit` | `DesignerFitRequest` returns `FitReportDto` | |
| `designer_ce_suggest` | `designer::ce_suggest` | `DesignerCeSuggestRequest` returns `CeSuggestionDto`; the export request gains the optional `acceptSuggestions { fields }` | pure; without CE data the answer is `available: false` with a plain reason |
| `designer_ce_ammo_catalog` | `designer::ce_ammo_catalog` | `DesignerCeAmmoCatalogRequest { draft?, query?, caliber?, class?, page?, pageSize? }` returns `CeAmmoCatalogDto { available, reason?, total, matching, page, pageSize, entries, calibers, classes }` | read only; the draft only ranks the sets; without CE data `available: false` with a plain reason |
| `designer_ce_ammo_suggest` | `designer::ce_ammo_suggest` | `DesignerCeAmmoSuggestRequest { class, hints, copyFrom? }` returns `CeAmmoSuggestionDto { available, reason?, class, classLabel, copiedFrom?, nearest, fields, ammoType, notes }` | read only; the type is returned, not stored; a typed value of a draft is never touched because the draft is not part of the request |
| `designer_clone` | `designer::clone_draft` | `DesignerCloneRequest` returns `DesignerCloneResponse { entry: DraftEntryDto, notes }` | the draft is stored by the call; `clonedFrom` and the first anchor name the source; inherited values stay inherited (`parent.inheritedStats`); a source with a Combat Extended conversion is refused; the source def is never edited |
| `designer_clone_diff` | `designer::clone_diff` | `DesignerCloneDiffRequest` returns `DesignerCloneDiffResponse` | read only; the changed fields as old and new JSON values and the readouts of both (cycle time, DPS, strength, price for ranged; panel and in fight DPS for melee) with their delta |
| `designer_structure_defaults` | `designer::structure_defaults` | `DesignerStructureDefaultsRequest` returns `DesignerStructureDefaultsResponse` | read only; an extra row beyond the two clone rows, used by `designer new --strength` of the CLI |
| `designer_projectile_own` | `designer::projectile_own` | `DesignerProjectileOwnRequest` returns `DesignerProjectileOwnResponse` | read only; the draft in the answer is not stored, the caller saves it; an own projectile is named `<prefix>_Bullet_<rest>` after the weapon; the shared projectile it was copied from is kept in `copiedFrom` |
| `designer_asset_info` | `designer::asset_info` | `DesignerAssetInfoRequest` returns `DesignerAssetInfoResponse` | read only; a link, a folder or an unreadable file is `status` `refused`, a missing one `missing`, one above 8 MiB (20 MiB for audio) `too-large`; the `diagnostics` are those of importing the file; a relative path needs `projectId` |
| `designer_quiz_next`, `designer_quiz_answer`, `designer_quiz_back` | `designer::{quiz_next, quiz_answer, quiz_back}` | `DesignerQuizNextRequest` returns `QuizStepDto`; `DesignerQuizAnswerRequest` and `DesignerQuizBackRequest` return `DesignerQuizAnswerResponse` | back removes the last answer the user gave (the setup answers taken from the spec stay); the values earlier answers wrote into the spec stay, so the returned step and estimate are the truth; the terminal quiz uses it for `b` instead of keeping earlier drafts |
| `designer_calibrate` | `designer::calibrate(ctx, req, &dyn ProgressSink, &CancelToken)` | `DesignerCalibrateRequest` returns `CalibrateResultDto` | job body; results cached in the cache root collection `designer-calibration` |
| `designer_draft_save`, `designer_draft_list`, `designer_draft_delete` | `designer::{draft_save, draft_list, draft_delete}` | `DesignerDraftSaveRequest`, `DesignerDraftListRequest`, `DesignerDraftDeleteRequest` and their responses | extra row `designer_draft_load` (`draft_load` returns a `DraftEntryDto`) |
| `designer_convert_scan` | `designer::convert_scan(ctx, req, &dyn ProgressSink, &CancelToken)` | `DesignerConvertScanRequest` returns `ConvertScanDto` (candidates with status and asks) | job body; needs CE data |
| `designer_export_plan` | `designer::export_plan` | `DesignerExportPlanRequest` (with an optional `convert` member that switches to the conversion flow, `ConvertRequestDto`) returns `WritePlanDto { planId, files, diagnostics, hasErrors }` | pure; `WritePlanDto` has rendered text, diff and byte size, not node trees; open conversion questions become error diagnostics `designer.convert-needs-answer` |
| `designer_apply_plan` | `designer::apply_plan(ctx, DesignerApplyPlanRequest, &dyn ProgressSink, &CancelToken)` | the request re-sends the export inputs plus `planId`; returns `ApplyReportDto` with `AppliedFileDto`s | job body; `designer.plan-stale`, `designer.apply-failed`, `designer.apply-cancelled`; backup path is absolute |
| `designer_material_matrix` | not built | `DesignerMaterialMatrixRequest` and `MaterialMatrixDto` exist in `rimstudio-ipc-types` | apparel and matrix are outside 0.1.0 |

Extra rows that no table above carries (a list, not table rows, so that the registry parity test reads only the real catalog rows). All are `proposed by items toolkit` except `job_status`.

- `designer_reference_list_ce`: query, `designer::reference_list_ce`, root class none; the reference pools of the Combat Extended load.
- `designer_suggest_fill`: action, `designer::suggest_fill`, none; writes suggestions into a draft and never replaces a typed value.
- `designer_draft_load`: query, `designer::draft_load`, ProjectRoot; returns a `DraftEntryDto`.
- `designer_convert_plan`: query, `designer::convert_plan`, ProjectRoot; the write plan of one conversion, with the open questions as diagnostics.
- `job_status`: query, handler in `rimstudio-app` (M1), none; reports the state of a job id.

Notes. The job events as built have no `chunk` variant yet (`started`, `progress` and the terminal `finished`, `failed`, `cancelled`), because no 0.1.0 job streams partial rows. The `designer.*`, `project.*`, `defs.*`, `store.*`, `xpath.*`, `xml.*`, `scan.*`, `steam.*`, `ce.*` and `manager.*` codes of the crates are mapped to the DTO error registry by `rimstudio-app`, which carries a test that every crate error code is registered.

## 5. Broadcast events

Broadcast events are Tauri events named `<area>:<noun>`. All are low rate and carry facts that no single caller owns.

| Event | Payload | Emitted when | Owning crate | Milestone | Requirements | Aliases | Status |
|---|---|---|---|---|---|---|---|
| `settings:changed` | `{ rev, sections: string[] }` | any settings write | `rimstudio-manager` | M1 | R10 | none | registered (IPC and state section 3.1) |
| `game:running-changed` | `{ state: "running" or "stopped" or "unknown" }` | process or Steam flag change (D-041); `unknown` inside sandboxes | `rimstudio-manager` | M2 | R5, MM-029 | `game.state` | registered |
| `sources:changed` | `{ sourceIds: string[] }` | the file watcher saw a change in a registered root (D-028) | `rimstudio-manager` | M2 | R4, MM-041 | none | registered |
| `app:second-instance` | `{ args: string[] }` | the single instance plugin forwarded a second launch | `rimstudio-shell` through `rimstudio-app` | M0 | R1 | none | registered |
| `game:list-changed` | `{ diffSummary }` | an external or post-exit `ModsConfig.xml` change is detected | `rimstudio-manager` | M2 | R5, MM-015 | `game.list-changed` | proposed by the mod manager spec |

OS file and folder drops are not app events. The platform adapter listens to the Tauri drag-drop event and passes the paths to the feature, which sends them to `sources_probe_folder`.

## 6. Channels and message unions

A channel is never named on the wire. It is an argument of a `stream` or `job` command, so the command name identifies it. Every stream message union starts with `hello`, carries domain messages and ends with `closed`, as in [IPC and state section 6](ipc-and-state.md#6-snapshot-plus-delta).

| Channel owner (command) | Message type | Message kinds | Replaces |
|---|---|---|---|
| `mods_subscribe` | `ModsStreamMsg` | hello, upserts, removes, order, diagnostics (changed rows only, `{rev, byMod}`), closed | events `library.delta`, `library.diagnostics` |
| `datasets_subscribe` | `DatasetsStreamMsg` | hello, `{ rev, upserts: DatasetStatusDto[] }`, closed | event `datasets.status` |
| `launch_state` | `LaunchStreamMsg` | hello, `{ running, pid? }`, closed | event `game.state` |
| `logs_follow` | `LogsStreamMsg` | hello, chunks of 250 to 1000 records, closed | event `log.append` |
| `applog_follow` | `AppLogStreamMsg` | hello, log records, closed | none |
| every `job` command | `JobEvent<R>` | progress `{jobId, done, total, message}` at most 20 per second, exactly one terminal event | events `job.progress`, `job.finished` |

Jobs in the catalog and their result types: `detect_run` (`DetectionReportDto`), `library_scan` (`LibraryScanResult`), `library_folder_size`, `deploy_apply`, `launch_check`, `datasets_refresh`, `datasets_changelog`, `rules_import_rimsort`, `rules_classify`, `rules_export_patch`, `settings_import_rimsort_apply`, `logs_open`, `defs_open_session`, `defs_build_snapshot`, `designer_calibrate`, `designer_convert_scan`, `designer_apply_plan`, `publish_plan` (`PublishPlan`), `publish_dry_run`, `publish_probe`, `publish_start` and `publish_resume`. A command that is promotable to a job (section 2, rule 4) uses the same `JobEvent<R>` shape after promotion.

## 7. Root classes

The registry declares, per command, which request fields are paths and which root class each may fall in. A test (`xtask check-docs`) fails when a registry command has a path field with no declared class ([security and privacy section 5](security-and-privacy.md#5-path-root-validation), step 7). The catalog column is the planned declaration. The assignment follows these rules.

1. **LibraryRoot**: configured custom folders, Mods and Workshop. Used by the scanner and by deployment planning. These roots are read-only for the manager.
2. **ProjectRoot**: a mod project opened in the workspace. Writable only for the project tools (project, designer and publish rows).
3. **DataRoot**: config, data, cache and logs. Writable only through the stores; rows such as `settings_backups_list`, `datasets_refresh` and `applog_query` use it.
4. **GameRoot**: the install and game config folders, read-only except for the two fenced operations (`list_save`, `deploy_apply`) and the operations that restore or remove what those two wrote (`deploy_unlink_all`, `list_backups_restore`, `project_dev_restore_list`).
5. **UserPick**: a path just chosen in a dialog, valid for that one command, then registered when the command adds it as a root (`sources_add_folder`, `project_create` and `project_open`). Export and import commands that read or write a file the person picked also use it.
6. **none**: the request has no path field and every file the handler touches is resolved from ids (`SourceId`, `ModId`, `ProfileId`, plan tokens) inside an already registered root.

A row can name more than one class when the request carries more than one path; the catalog shows the class of the path field that carries the risk. Names (project names, designer file names, mod folder names) are sanitised to a single path component by step 3 of the same section, whatever the root class.

## 8. Counts

By kind (the first word of the kind column, so families and promotable queries count under their base kind):

| Kind | Rows |
|---|---|
| action | 52 |
| family | 4 |
| job | 22 |
| query | 75 |
| stream | 5 |

By milestone:

| Milestone | Rows |
|---|---|
| M0 | 5 |
| M1 | 13 |
| M2 | 54 |
| M3 | 23 |
| M4 | 27 |
| M5 | 23 |
| M6 | 12 |
| M7 | 1 |

By owning crate (the crate whose `api` module holds the handler):

| Owning crate | Rows |
|---|---|
| rimstudio-app | 10 |
| rimstudio-manager | 83 |
| rimstudio-publish | 12 |
| rimstudio-toolkit | 53 |

By root class:

| Root class | Rows |
|---|---|
| LibraryRoot | 3 |
| ProjectRoot | 28 |
| DataRoot | 16 |
| GameRoot | 12 |
| UserPick | 14 |
| none | 85 |

The milestone M2 core subset, rows 1 to 40 of [IPC and state section 11](ipc-and-state.md#11-the-first-40-commands), keeps its own table there because it is the list that the milestone M2 exit criteria and the CLI parity test refer to; the milestone entries of those rows include the M0, M1 and M4 rows that the table lists for context.

## 9. Adding or changing a command

1. Add or edit the row in this catalog first, with kind, DTO stems, owning crate, milestone, requirement ids, root class and aliases. A new area needs a line in the registry module docs and, when it belongs to a tool, an entry in `ToolDescriptor` ([IPC and state section 3](ipc-and-state.md#3-naming)).
2. Add the row to `for_each_command!` and the DTOs to `rimstudio-ipc-types`. The shell and the CLI need no edit: the wrapper, the permission (`allow-<command>`), the CLI route and the TypeScript binding are generated.
3. Declare the root class of every path field. A path field with no class fails the registry check.
4. Run `cargo xtask check` (registry parity: names, permissions, bindings and CLI routes must agree) and `cargo xtask bindings --check`.
5. When a spec names a command that is not in this catalog, add the row here and record a decision in the [register](decision-register.md) only when the name or the kind differs from what the spec wrote.
6. Changing a kind (for example promoting a query to a job) edits the row kind and nothing else; the TypeScript wrapper changes shape and the compiler finds every call site.
7. Retiring a name keeps it in the alias column for at least one milestone and removes it from the wire at once.

## 10. Open points

1. **DTO shapes of proposed rows.** Only the registered rows and a few proposed rows (`SortPreviewResponse`, `DiagnosticsForMod`, `PublishPlan`) have shapes in the documents. The conventional stems in the table are placeholders; the shapes are written with each milestone.
2. **Defs snapshot and patch tester.** OD-10 stays open: `defs_build_snapshot`, `patch_test_run`, `patch_test_explain` and the conflict view need request and response shapes before M4 starts.
3. **Milestone of the logging commands.** `log_event`, `applog_query` and `applog_follow` have no milestone in the source documents; M2 is assumed because the first manager screens report webview errors and show the app log. Confirm when the frontend plan is written.
4. **`app_check_update`.** The updater relaunch is specified as a Rust command so that no `process` permission reaches the webview (OD-09). Which crate holds the handler is open until OD-09 is checked; the catalog lists `rimstudio-app` as the registry owner.
5. **Families.** `history_*`, `groups_*`, `usermeta_*` and `bisect_*` need their concrete names before M2 (usermeta) and M3 (the others). Bisect placement is an owner decision (roadmap section 12).
6. **Alias mapping in the task description.** Research and early draft aliases are mapped to the registered `mods_*` names; the alias spellings `library_snapshot`, `library_subscribe` and `library_mod_detail` are kept in the alias column only.
7. **Root class of mixed rows.** Rows that read the game folder and write a library or data root (for example `deploy_plan`) show one class; the registry attribute may need a list when it is written.

Related documents: [IPC and state](ipc-and-state.md), [crate catalog](crate-catalog.md), [decision register](decision-register.md), [workspace layout](workspace-layout.md), [security and privacy](security-and-privacy.md), [roadmap](../roadmap.md), [mod manager](../features/mod-manager.md), [load order and validation](../features/load-order-and-validation.md), [community datasets](../features/community-datasets.md), [settings](../features/settings.md), [game and mod discovery](../features/game-and-mod-discovery.md), [modding workspace](../features/modding-workspace.md), [items toolkit](../features/items-toolkit.md), [Workshop publishing](../features/workshop-publishing.md), [Combat Extended patching](../features/combat-extended-patching.md).
