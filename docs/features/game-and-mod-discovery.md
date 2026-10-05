# Game and mod discovery: functional specification

This document specifies how RimStudio finds the RimWorld installation, Steam libraries and Workshop content, how it models mod sources including any number of user-added custom mod folders, how it scans them, how it makes custom folders visible to the game, and how it behaves when things are missing or go wrong (requirements R3 and R4). It defines the `DetectionReport` and its screens, Workshop item status, scan levels, duplicate handling, the link farm, running-game detection and change detection, with acceptance criteria and tests for each requirement. The detection algorithms themselves are in the [Steam and game detection research](../research/steam-and-game-detection.md) and are summarised here only as far as the requirements need. Settings storage and the Settings screen are in [settings](settings.md); the mod list built on top of this is in [mod manager](mod-manager.md).

Status: draft | Last updated: 2026-10-04


Milestone numbering follows the [roadmap, section 1.1](../roadmap.md#11-mapping-to-the-milestone-names-in-the-register) (older mentions of M3 to M6 use the decision register's numbering).
## Contents

1. [Scope and principles](#1-scope-and-principles)
2. [Conventions](#2-conventions)
3. [Detection](#3-detection)
4. [Instances and libraries](#4-instances-and-libraries)
5. [Workshop item status](#5-workshop-item-status)
6. [Mod sources and visibility to the game](#6-mod-sources-and-visibility-to-the-game)
7. [Custom mod folders](#7-custom-mod-folders)
8. [Scanning](#8-scanning)
9. [Duplicates](#9-duplicates)
10. [The link farm](#10-the-link-farm)
11. [Running game, change detection](#11-running-game-and-change-detection)
12. [Failure and empty states](#12-failure-and-empty-states)
13. [Commands, events and diagnostics](#13-commands-events-and-diagnostics)
14. [Test plan](#14-test-plan)
15. [Owner decisions and open points](#15-owner-decisions-and-open-points)
16. [As built: the facts of a scan](#16-as-built-the-facts-of-a-scan)

## 1. Scope and principles

In scope: locating the game, its user data folder and the Steam content on all three operating systems; presenting what was found and letting the user correct it; every place a mod can live; keeping the mod index current; and the managed bridge that lets the game see mods that live outside its own folders.

Out of scope: sorting and validation of the list (mod manager), the Settings screen layout (settings), Workshop publishing (later milestone), and the contents of the def index (toolkit).

Principles that every requirement below follows:

1. Detection proposes, the user decides. A user override always wins, is persisted, and is never silently deleted ([Steam detection](../research/steam-and-game-detection.md) section 3.1 and 7.3).
2. Report all candidates, not the first hit, each with how it was found and a confidence level.
3. Content beats names: an install is a folder that contains `Data/Core/About/About.xml`, not a folder called RimWorld.
4. The game builds its mod list from only three places: the install `Data` folder, the install `Mods` folder and Steam-subscribed items. It silently deactivates active mods it cannot find. Therefore anything outside those places reaches the game only through a managed link or copy, and a launch that would lose active ids is stopped first ([Steam detection](../research/steam-and-game-detection.md) section 2 and 8.6).
5. The only writes under the install or config folder are owned link-farm entries and `ModsConfig.xml` after a timestamped backup and a running-game check (write fence, D-040). Mod folders of the user are never modified by the manager.
6. Nothing about drives, Steam or the filesystem may block the UI: every probe has a deadline, every scan is a job, and content problems are diagnostics, never failures of the whole operation (I-10, I-13).
7. Everything the detector touches goes through `DetectEnv` ports (home, environment, registry, filesystem with deadline), so Linux CI runs every OS layout without Steam installed (D-009, D-037).

## 2. Conventions

Requirement ids are `GD-nnn`. Each requirement has a milestone tag (M1 library, M2 manager, M5 publisher, per the roadmap), a statement, acceptance criteria (AC) and the test layer that proves it. Fixtures `L1` to `P1` are the synthetic Steam trees defined in the [detection research](../research/steam-and-game-detection.md) section 7.4; new fixtures introduced here are prefixed `D` (discovery), `F` (farm) and `S` (scan). All fixtures are built by `rimstudio-testing` with fictional content and injected ports; no fixture contains vanilla or Combat Extended data (R11, I-07). Golden outputs are JSON. Diagnostic codes follow `<area>.<kebab-name>` (D-046); codes marked "proposed" are added to the code registry in `rimstudio-validate` when the requirement is implemented.

## 3. Detection

### GD-001 Detection run (M1)

Statement: `detect_run` (a job) produces a `DetectionReport` containing every Steam root, library, game install and user data folder candidate, with `how`, `confidence`, validity checks and warnings. The result is cached in memory and on disk (cache root) so the next start can show the last report instantly while a fresh run executes in the background.

Algorithm summary (details in the research): collect Steam root candidates per OS in a fixed order, keep all that exist and deduplicate by canonical path; enumerate libraries from the union of `config/libraryfolders.vdf` and `steamapps/libraryfolders.vdf` plus the root itself; for each library match `appmanifest_294100.acf` by exact name, ignoring `.tmp` and zero-byte files; accept an install only when the content test passes; read the game version from `Version.txt` (never from `ModsConfig.xml`); probe all user data folder candidates and rank by the newest `ModsConfig.xml`.

| OS | Steam root candidates (after the user override) | User data folder candidates | Notes |
|---|---|---|---|
| Linux | `$XDG_DATA_HOME/Steam`, `~/.steam/steam`, `~/.steam/root`, `~/.steam/debian-installation`, Flatpak, Snap | `~/.config/unity3d/Ludeon Studios/RimWorld by Ludeon Studios`, the Proton prefix `pfx/.../LocalLow/...`, both Flatpak variants | Symlinked roots deduplicate by canonical path ([research 3.2](../research/steam-and-game-detection.md)) |
| Windows | `HKCU\Software\Valve\Steam` `SteamPath`, `HKLM\SOFTWARE\WOW6432Node\Valve\Steam` `InstallPath`, `HKLM\SOFTWARE\Valve\Steam`, then `C:\Program Files (x86)\Steam` and `C:\Program Files\Steam` | `LocalLow\Ludeon Studios\RimWorld by Ludeon Studios` from the known-folder API | Escaped backslashes in VDF, case-insensitive drive letters, UNC installs ([research 3.3](../research/steam-and-game-detection.md)) |
| macOS | `~/Library/Application Support/Steam` | `~/Library/Application Support/RimWorld` with `Config/` | The game is an app bundle and `Mods` lies inside it ([research 3.4](../research/steam-and-game-detection.md)) |

Non-Steam installs (GOG, manual copies) are probed in the roots listed in [research 3.9](../research/steam-and-game-detection.md), accepted by the same content test, labelled `gog` or `manual`, ranked below manifest-confirmed Steam installs, and have no Workshop features. Itch.io and Epic layouts are not researched and are not promised for v1; the manual "choose folder" path covers them.

Privacy rule: detection never opens `loginusers.vdf` or `steam.token`, and opens `config.vdf` only to find a Proton compatibility tool mapping ([research 5.5](../research/steam-and-game-detection.md)).

AC:
1. On fixtures L1 to L6, W1 to W3, M1 and N1 to N3 the report matches the golden JSON for each, including `how`, `confidence`, `proton` and `userDirs`.
2. On fixture P1 (zero-byte manifest, BOM in `libraryfolders.vdf`, missing `installdir`, dangling Steam symlink, offline library, Steam plus GOG) the run completes, returns every valid candidate, emits one documented warning per defect, and never panics.
3. On the research machine (two libraries, 690 workshop folders) a run completes in under 500 ms warm; the budget is enforced by a criterion bench against `xtask/budgets.jsonc`.
4. Every filesystem probe respects its deadline: a fake `FsProbe` that blocks forever on one library produces `steam.library-timeout` for that library and the other candidates still arrive.
5. Golden output is identical at 1 and 8 threads.

Tests: unit and golden tests on `rimstudio-steam` with `DetectEnv` fakes (L, W, M, N fixtures); property test that VDF parsing round-trips structure and agrees with the dev-only reference parser on real samples (the 28 of 28 differential result in the research); `#[ignore]` real-install test gated by an environment variable.

### GD-002 DetectionReport model (M1)

Statement: the report is a versioned JSON document (`schema` 1, camelCase on IPC, kebab-case enum strings) whose shape is the one in [research 7.1](../research/steam-and-game-detection.md): `steamRoots`, `libraries`, `installs`, `userDirs`, `selected`, `warnings`. `how` is a closed string set so the UI can say "found because ..." (as built: `override`, `registry-hkcu`, `registry-hklm`, `xdg-data-home`, `symlink-steam`, `flatpak`, `snap`, `libraryfolders.vdf`, `appmanifest`, `directory-probe`; there is no `gog-registry`, because no GOG registry layout is known; the IPC DTO writes `libraryfolders.vdf` as `library-folders-vdf`); `confidence` is `high` (manifest plus content test), `medium` (content test only) or `low` (name based or stale). Install `health` is `installed`, `update-pending` or `needs-verify`, derived from `StateFlags` and `TargetBuildID` ([research 3.6](../research/steam-and-game-detection.md)). Identity of an install is `kind:canonical library path`, stable across runs and used as the key for selection, overrides and the link ownership manifest.

As built (`rimstudio-steam::report`): `generatedAt` is `generated_at_ms`, a number from the clock port, and a user folder's `ModsConfig.xml` info carries `mtime_ms` (not nanoseconds); every path in the report uses `/` separators on every operating system (backslashes and the `\\?\` prefix are normalised) so goldens are host independent; an install is `kind` `steam`, `gog`, `manual` or `override` and a user folder is `native`, `proton`, `flatpak`, `snap`, `macos`, `windows` or `override`; the report carries only the fields the probes can read (the writable check of the research is dropped because the probe port is read only; the `Mods` writability mark comes from the manager's marker file probe, never inside Steam folders). The game version of a user folder's `ModsConfig.xml` is read through the function pointer `DetectOptions::mods_config_version` that the application supplies (it wraps `rimstudio-xml`), because the steam crate parses no XML; without it `steam.userdir-version-mismatch` is never raised. Selection ranks an override first, then a manifest confirmed Steam install, other Steam installs and non Steam installs; the research idea of ranking an install by its user folder's modification time is not used, because user folders are not tied to installs (only the default user folder is the one with the newest `ModsConfig.xml`).

AC: the DTO is generated into TypeScript (no hand-written copy); the golden JSON files validate against the committed schema; an unknown `how` value from a newer build is displayed as its raw string, not as an error.

### GD-003 Detection screens and the report UI (M1)

Statement: the report is shown in two places that render the same DTO: the first-run wizard (MM-001 in the [mod manager](mod-manager.md)) and Settings, Game and Steam (see [settings](settings.md) section 5). Both show cards, never a raw tree.

Cards, one each for: RimWorld install, Steam libraries, Workshop content, user data folder (with `ModsConfig.xml`), game version.

1. Each card shows the selected path, a "found because" line from `how` (for example "Steam library file", "Windows registry", "default path"), a confidence chip (high, medium, low), a health chip where relevant, and an Open folder action.
2. When several candidates exist the card shows a segmented list of all of them, best preselected, with a one-click switch. Switching writes an override (GD-005) with `pinned: false` unless the user also ticks Keep this choice.
3. Warnings appear on the card they concern, with a plain sentence, the affected paths and one suggested action. Codes are stable identifiers translated through the JSON message catalogue ([data and persistence](../architecture/data-and-persistence.md) section 3.1, D-056).
4. A card with no candidate says "Not found", offers Choose folder, accepts a dropped folder, and explains what the app will still do without it (for example "mod list works from custom folders; launching is disabled").
5. A Run detection again button re-runs GD-001 and shows a diff against the previous report ("1 new candidate, 1 candidate gone") instead of silently replacing the selection. A changed selection is applied only when it was not pinned or the user confirms.
6. A path chip shows the validity checks (exists, readable, writable) as three small marks with tooltips; the writable mark for `Mods` is from a marker-file probe and is never run inside Steam folders.

Warnings the UI must be able to show (from [research 7.1](../research/steam-and-game-detection.md)), each with a stable code under `steam.*` or `deploy.*`: game version differs from `ModsConfig.xml` version (informational), more than one install, library offline, `Mods` not writable, Proton and native user folders both present, unreadable ACF, unparseable `Version.txt`, install and Workshop in different libraries, install is a symlink, Steam client appears to be running. Codes as built (`rimstudio-steam::report::codes`): `steam.root-dangling-link`, `steam.library-offline`, `steam.library-timeout`, `steam.probe-timeout`, `steam.libraryfolders-unreadable`, `steam.acf-unreadable`, `steam.manifest-leftover`, `steam.apps-table-stale`, `steam.install-dir-missing`, `steam.install-content-invalid`, `steam.install-update-pending`, `steam.install-needs-verify`, `steam.install-is-symlink`, `steam.version-missing`, `steam.version-unparseable`, `steam.multiple-installs`, `steam.userdir-version-mismatch`, `steam.proton-and-native-userdirs`, `steam.workshop-different-library`, and `deploy.override-invalid` for an invalid override. `steam.client-running` is not implemented (the detection environment has no process probe); `deploy.mods-not-writable` belongs to the manager's deploy checks.

AC: every warning code in the registry has a catalogue entry (checked by the `check-docs` and catalogue completeness tests); with two installs the card lists both and switching persists across restart; a screenshot test in the gallery covers each card state (found, ambiguous, not found, override invalid) in both themes; the screen is fully keyboard operable.

Tests: component tests with mocked IPC (Vitest), gallery screenshots, Playwright flow "detect, switch candidate, restart, selection kept".

### GD-004 Game version and user data folder (M1)

Statement: the installed game version is read from the selected install's `Version.txt` and parsed into major, minor, build and revision with the raw text kept; `ModsConfig.xml`'s `<version>` is never used as the installed version. The user data folder default is the candidate whose `Config/ModsConfig.xml` is newest; when native and Proton folders both exist the UI shows both, marks the newest and explains that the game reads the one that matches how it is launched. A `-savedatafolder` launch override is supported and is written into the report as an `override` user folder.

AC: fixture L5 selects the Proton folder when its `ModsConfig.xml` is newer and the native one when it is older; an unparseable `Version.txt` yields `steam.version-unparseable` and the raw text is still displayed; the version shown in the footer changes after the user picks another install.

Tests: golden on L1, L5, L6; unit tests of the version parser (with fictional versions).

### GD-005 User override and re-detect (M1)

Statement: each of install, user data folder, Steam root, extra Workshop folders and ignored installs can be overridden. Overrides live in the `paths` section of `workspace.jsonc` ([settings](settings.md) section 4.2). Rules:

1. An override that passes validation is used instead of detection for that field.
2. An override that fails validation (folder gone, content test fails) is kept, reported as `deploy.override-invalid`, and detection is used for that run. The UI shows the stale override on the card with Fix, Re-point and Forget actions. Nothing deletes it automatically.
3. `pinned: true` stops auto-switching when a better candidate appears (for example after the user moves the game to another library); `pinned: false` lets the report suggest a switch and ask once.
4. Detection runs at start (in the background, GD-001), on Run detection again, and when a watched root changes (GD-071).
5. Paths are stored as text exactly as chosen plus a canonical form computed on use; a path that is not valid UTF-8 is kept as a lossy display string with a flag and reported as `scan.non-utf8-path`.

AC: set an override, delete the folder, restart: the override is still in the file, the card shows it as invalid, and detection supplies a working fallback; fix the folder, restart: the override is valid again with no user action; ignoring an install by id hides it from candidates until Forget.

Tests: unit on override resolution; golden on the invalid override case; Playwright restart flow.

## 4. Instances and libraries

### GD-010 Multiple installs and the active one (M1)

Statement: RimStudio does not model RimSort's "instances" as separate configuration universes. It models installs (what detection found plus manual choices) and one active install, selected by install id. Per-install state is limited to what must differ: the ownership manifest and deployed links (keyed by install id, `deploy/<farmId>/ownership.json`), the last `ModsConfig.xml` backup set, and launch arguments. Mod lists are profiles that can be used with any install; switching install re-resolves each profile against the new install and reports ids that do not resolve. This follows the mapping chosen in the [mod manager](mod-manager.md) (F-060 is mapped to profiles and sources, not to separate installs).

AC: with two installs (Steam and GOG) switching the active install changes the version footer, the visible sources and the deploy plan within one second and does not alter any profile; deploying to one install never writes into the other; the ownership manifests are separate files.

Tests: integration test with fixture P1 (two installs) asserting two manifests and no cross writes through the recording filesystem.

### GD-011 Multiple Steam libraries (M1)

Statement: every library from `libraryfolders.vdf` is a first-class entry. The report records, per library, `root` (the path), `label`, `online`, `timedOut`, `hasApp` and `stale` (a `writable` field is not recorded). The `apps` table is a hint; `appmanifest_*.acf` is the truth. The game lives in one library, and its Workshop content normally lives in the same library; all libraries are still scanned for `workshop/content/294100` and `appworkshop_294100.acf` and merged, because a user who moved the game can have content in the old library ([research 3.7](../research/steam-and-game-detection.md)). One Workshop source per library that holds content is created automatically (GD-020).

An offline library (path missing, removable drive) stays in the report as `offline`, its Workshop mods stay in the index as "unavailable" from the cached scan, and nothing is removed from `ModsConfig.xml` by RimStudio.

AC: on fixture L2 the install is found through the VDF path of library 1, library 0 has `hasApp` false, the Workshop folder is in library 1, and Workshop sources are created only for libraries with content; on P1 the offline library is listed as offline and its cached mods appear greyed.

Tests: golden on L2 and P1; library scan test with a library that disappears between runs.

### GD-012 Non-Steam installs degrade gracefully (M1)

Statement: for `gog` and `manual` installs the Workshop source does not exist, `workshop_*` commands report "not available for this install", the Workshop status column is hidden, and the rest of the product (library, sorting, custom folders, launch via executable) works.

AC: fixtures N1 to N3 yield a working library with Mods and custom folders and no Workshop UI; launch uses the executable path.

## 5. Workshop item status

### GD-020 Workshop sources and item status (M1)

Statement: each Workshop item folder `workshop/content/294100/<id>` is a mod root (or a root with version subfolders and `LoadFolders.xml`). Its status is computed from `workshop/appworkshop_294100.acf` and the folder, with the rules of [research 5.4](../research/steam-and-game-detection.md). As built the precedence is downloading, then not-downloaded, then update-available, then current, and orphan is a separate state for folders the ACF does not list; the table below gives the rules:

| Status | Rule | Display |
|---|---|---|
| `downloading` | `BytesDownloaded < BytesToDownload` in the item details, or top-level `NeedsUpdate` or `NeedsDownload` is 1 and the on disk item is stale (the global flags are not applied to every item, otherwise two absent items on the research machine would read as downloading) | Spinner badge, mod is usable only if the folder reads cleanly; scan marks it provisional |
| `update-available` | `latest_timeupdated > timeupdated` or `latest_manifest != manifest` | "Update available" badge with date; opens the Steam page; v1 extends to an update check column (F-054) |
| `not-downloaded` | Item in `WorkshopItemsInstalled` but its folder is missing | "Subscribed, not downloaded" row with an Open in Steam action; cannot be activated |
| `orphan` | Folder present but absent from the ACF (for example after unsubscribe) | "Not in Steam's list" badge; usable but flagged, because the game will not see it as subscribed |
| `current` | none of the above | no badge |

As built (`rimstudio-steam::workshop`): `item_status(&AppWorkshop, content_dir: &Utf8Path, &dyn Listing) -> Vec<ItemStatus>` takes the content folder as well, sorted by id, with `ItemStatus { id, state: ItemState, on_disk, installed_time_updated, latest_time_updated, size_hint }`; because the `Listing` trait has no children call, folders are modelled by the `WorkshopFolders` trait (`item_status_with`), and a `Listing` derived view cannot see an empty orphan folder; `status_from_env(env, content_dir, acf_path, deadline)` reads the real children. Items that appear only in the item details table (not in the installed table) are known items, not orphans.

Rules: `size` in the ACF is a change hint only and is never presented as a folder size (3 of 5 sampled items differed from disk); the Steam client usually is not running while a manager is open, so the ACF may be stale: when `steam.pid` indicates the client is running the report would carry `steam.client-running` and the status column would show "may change" (not implemented in 0.1.0; see the code list in section 3). Status is computed from the ACF only (no network call), so it works offline.

Important consequence for the game: the game asks the Steam client API for subscribed items, not the folder. An orphan folder is not loaded by the game, and a not-downloaded item is not loaded either. The pre-launch check (GD-066) uses this table to decide what resolves.

AC: on the third ACF fixture of the research (item 111 current, item 222 with newer `latest_*`, folder 333 without an ACF entry) the statuses are `current`, `update-available`, `orphan`; adding an installed entry without folder yields `not-downloaded`; the real-install test on the research machine reports 692 entries, 690 folders and two `not-downloaded` items; no fixture uses `size`.

Tests: unit on the status function (table-driven, all five states plus precedence), golden on the ACF fixtures, real-install `#[ignore]` test.

## 6. Mod sources and visibility to the game

### GD-030 Source kinds and visibility (M1)

Statement: a source is an ordered, labelled place where mod folders live. The unified library is keyed by `(packageId, rootPath)`, and each mod carries its source id. Source kinds and what the game does with them:

| Kind | Location | Created by | Visible to the game | Editable by RimStudio tools |
|---|---|---|---|---|
| `game-data` (official) | `<install>/Data/<Core, Royalty, ...>` | detection | yes, always loaded first by the game | never (read only) |
| `game-mods` | `<install>/Mods` | detection | yes | only through the project tools on folders the user registered; the manager never edits |
| `workshop` | `<library>/steamapps/workshop/content/294100`, one per library with content | detection, GD-011 | yes if the item is subscribed, downloaded and in the ACF (GD-020) | no (read only) |
| `custom` | any user path | the user (R4) | no, until made visible (section 10) | yes if not `readOnly` |

The order of sources is the default tie-break for duplicates (GD-051) and the order in the source filter. The game loads sources in the order official, install Mods, Workshop ([research 2](../research/steam-and-game-detection.md)); that order is fixed and is shown for information, while list order inside a profile is the user's.

Each source card (Settings and the library source filter) shows path, mod count, last scan time, reachability, and a visibility chip: "Visible to the game" or "Needs a link" for custom sources. Linked entries inside `<install>/Mods` that RimStudio owns are not a second source; they are recognised via the ownership manifest and attributed to their target (GD-063, and GD-031 for the same-folder rule).

AC: the library source filter lists exactly the enabled sources; an owned link in `Mods` does not produce a duplicate row; the visibility chip for a custom source turns to "Visible to the game" only after a successful deploy that the pre-launch check confirms.

Tests: scan integration test with `Data`, `Mods`, a Workshop folder, a custom folder and two owned links; snapshot test of the source DTOs.

### GD-031 Same folder reached by two sources (M1)

Statement: a folder that is reachable through two sources (a custom folder inside the Workshop path, a custom folder that is a link into `Mods`) is detected by canonical path and by `(device, inode)` or Windows file id and listed once, attributed to the first source in order, with a note naming the other.

As built: the library scanner lists the folder once under the first source with `also_in` and the diagnostic `scan.same-folder`, comparing canonical paths; a custom source mod whose folder is also reached through game `Mods` or Workshop counts as loadable. The `(device, inode)` or file id identity check is not implemented in 0.1.0; the manager's `probe_folder` also refuses overlaps lexically on canonical paths (nested, duplicate, enclosing, inside the game `Mods` folder, or a symlink to a known source).

AC: fixture D3 (custom folder equals a subfolder of Workshop content) lists each mod once.

## 7. Custom mod folders

### GD-040 The custom folder model (M1, R4)

Statement: the user can add any number of custom mod folders. Each is an entry in `customModFolders` of `workspace.jsonc` with the fields below ([research 8.1](../research/steam-and-game-detection.md), [data and persistence](../architecture/data-and-persistence.md) section 3.1):

| Field | Meaning | Default |
|---|---|---|
| `id` | `cf_` plus hex, generated once; it is the `<sourceId>` inside `ModId`, so moving a folder never changes mod identity | generated |
| `path` | absolute path as typed | required |
| `label` | display name | folder name |
| `enabled` | include in scans and lists | true |
| `layout` | `modsRoot` (children are mods), `singleMod` (the path is a mod), `auto` (classify at scan time) | `auto` |
| `scanDepth` | levels below the root to look for mods, 1 to 4 | 1 |
| `watch` | use a watcher for this folder (advisory) | true on local volumes |
| `priority` | lower wins duplicate ties; defaults to list order | list order |
| `readOnly` | never write, never create anything inside, hide edit tools | false |
| `link` | `auto`, `links`, `copy`, `none`: how the game gets these mods | `auto` |
| `volumeHint` | mount, label, optional volume UUID for re-finding the drive | filled at add time |
| `lastSeen` | last time the path was reachable | set by scans |

A path is a mod when `About/About.xml` exists, with `About` matched case-insensitively; scanning never descends below a mod root (version folders such as `1.6` are not separate mods). A folder that holds both mod children and its own `About` is classified as a mod and a warning `scan.ambiguous-layout` (proposed) is shown. The owner's own folder, scanned at depth 2, yields 22 mod roots and its template repository is correctly not a mod ([research 8.1](../research/steam-and-game-detection.md)).

AC: add a folder via picker or drop; within one second of the scan the card shows the mod count; removing the entry removes only its derived cache and rows and deletes no file (active mods from it become missing mods, MM-026); changing `scanDepth` rescans that folder only; entries survive restart, and unknown keys in the entry survive a CST edit by another version.

Tests: scan fixtures S1 (modsRoot depth 1), S2 (grouping folders at depth 2), S3 (singleMod), S4 (auto classification, template repository that is not a mod); settings round trip tests.

### GD-041 Validation when adding or editing a folder (M1)

Statement: `sources_probe_folder` runs before the entry is saved and returns "looks like a mods folder", a mod count estimate, whether the volume is removable or network, and warnings. Hard errors (cannot save): relative path, path not found, not a directory, path equals or contains the install `Mods` folder, or is inside it, overlaps a Workshop content folder, or equals, contains or lies inside another custom folder (the same mods would be counted twice). Soft warnings (can save): no mods found, path longer than 240 characters on Windows without long-path support, removable or network volume, a path inside a cloud-sync folder (unverified detection by name), a path that is a symlink (shows its target).

Overlap is judged on canonical paths, case-folded only on Windows and macOS after a case-sensitivity probe ([research 8.5](../research/steam-and-game-detection.md)). `~` and environment variables are expanded at input time only.

AC: each hard error has a distinct code and message (`deploy.source-overlap`, `deploy.source-inside-mods`, `scan.path-not-directory`, all proposed) and a one-line fix suggestion ("Remove the other folder or use a deeper path"); adding a folder inside Workshop content is refused; on a case-insensitive volume `D:\Mods` and `d:\mods` are treated as equal.

Tests: table-driven unit tests over canonical path pairs on Linux fixtures and simulated Windows semantics.

### GD-042 Offline and removable drives (M1)

Statement: reachability is checked with a deadline (a stalled network mount can block `stat` indefinitely) and a stored `volumeHint`. An unreachable custom source stays enabled, shows `offline`, keeps its mods in the library from the cached scan as "unavailable", and cannot have its mods activated (the message names the drive). RimStudio never edits `ModsConfig.xml` because a drive is missing. When the path is gone but the volume is found at a new mount, the app proposes "same volume found at X" once and never rewrites the path itself. A focus or mount event triggers a re-check (GD-071).

AC: with the folder renamed during a test, the source card shows offline within the deadline, the cached rows remain, activating one is blocked with `deploy.source-offline`, restoring the folder brings rows back without restart, and a missing-volume proposal appears when a fixture changes the mount point but keeps the volume hint.

Tests: fake `FsProbe` with a delayed stat; integration test that renames and restores a temp folder.

### GD-043 Priorities and ordering (M1)

Statement: custom folders have a user-visible order that doubles as default priority. Reorder by drag or keyboard. Priority affects only duplicate resolution (GD-051) and the order of the source filter; it never affects load order. The effective priority of each folder is visible on its card.

AC: reordering two folders that contain the same packageId flips which copy is effective, and the Duplicates panel (GD-052) shows the new reason within one second.

### GD-044 Scan depth and layouts in practice (M1)

Statement: `scanDepth` 1 treats direct children as candidates; 2 allows one level of grouping folders such as `Weapons/` or `Archive/`; the cap is 4. The scanner never follows symlinks below a root unless the target is inside a configured root, tracks `(device, inode)` or file id to stop loops, and skips `.git`, `node_modules` and other configured ignored names at any depth. Copies named like "Mod - Copy" are real folders and are scanned normally (the owner's folder contains several, [research 8.3](../research/steam-and-game-detection.md)); they surface through the Duplicates panel.

AC: a self-referencing link and a dangling link in a fixture produce no mod and one `scan.link-loop` or `scan.dangling-link` info diagnostic (proposed); a folder nested five levels deep is not scanned at depth 4.

Tests: fixture S5 (loops, dangling links, ignored names); scan golden.

### GD-045 Watch or poll (M1)

See GD-071 for the mechanism; per folder, `watch: false` forces poll and manual refresh only.

## 8. Scanning

### GD-050 Scan levels and what is shown (M1, M3)

Statement: the library scan is two-phase and purpose-built (D-021). The levels, with measured sizes from the [corpus note](../research/rimworld-mod-format-and-corpus.md) section 7.4 and budgets from the [scan performance spike](../research/scan-performance-spike.md):

| Level | Work | Cost on the research library (about 700 mods) | Shown to the user while it runs | Used by |
|---|---|---|---|---|
| 0 metadata | list each mod root, read `About.xml`, `LoadFolders.xml`, `PublishedFileId.txt`, stat preview and icon | 1,753 files, 1.6 MB; warm budget 100 ms to show the list | The mod list appears from the cache immediately; rows arrive as deltas; footer chip "Scanning n of m" after 200 ms; cold start shows rows progressively with a progress bar | Mod manager, sorting, validation |
| 1 def and patch index | level 0 plus every `Defs` and `Patches` file in the folders the game would load | 43,339 files, 358 MB; cold SSD budget 600 ms, never blocks the list | Library is already usable; a "Indexing definitions" job in the task centre with cancel; toolkit screens show "Index building n percent" | Def explorer, patch tester, designer, conflict view |
| 2 plus languages | adds translation XML | 64,354 files | Only on demand by the language tools; job in the task centre | Translation tools (later) |
| 3 full walk | stat of every entry, no reads | 94 ms for 306k files | Only for size accounting or an asset browser; lazy, never at start | Storage summary |

Rules: level 0 never waits for level 1; textures and audio are never read except on demand; the cache is a compact JSON manifest keyed per file by `(relative path, size, mtime ns, file id)` compared for equality, with a blake3 fallback on FAT-family volumes ([data and persistence](../architecture/data-and-persistence.md) section 11, D-023); the cache lives in the cache root, is always deletable, and a corrupt or newer cache is discarded and rebuilt without error; scan workers are capped at 8 and configurable ([settings](settings.md) section 8). Per-mod problems (unreadable folder, malformed `About.xml`, non-UTF-8 name) are diagnostics with counts in the footer and never abort the scan.

AC: with a warm cache the list is interactive within the 150 ms budget on the research library; a refresh after one changed `About.xml` updates exactly one row within 50 ms; deleting the cache directory costs one slow start and no data loss; scan output is identical at 1 and 8 threads; cancelling a level 1 job leaves the list intact and keeps already indexed files cached.

Tests: criterion benches against budgets (2x regression fails), property tests on the stat key logic (including FAT rounding), goldens on fixtures S1 to S5, cancellation test.

## 9. Duplicates

### GD-051 Duplicate packageId policy (M1)

Statement: duplicates are a normal state for a modder (7 duplicated ids on the research machine, 3 across roots including a Workshop copy of the user's own mod, 4 inside one folder; [research 8.3](../research/steam-and-game-detection.md)). The policy, from [research 8.4](../research/steam-and-game-detection.md):

1. The comparison key is the lower-cased packageId; the display and stored value keep the original casing, because `ModsConfig.xml` must keep it.
2. All scan results are grouped by key; a group of one is normal.
3. The effective entry of a group is chosen by: explicit user pin, then source rank (custom folders by priority, then game `Mods`, then Workshop; configurable), then the best `supportedVersions` match for the current game version (as built, a LOWER priority value wins, following the custom folder default of list position, although the doc comment of the core `ModSource::priority` says the opposite; the default class order is custom, game `Mods`, Workshop, game `Data`; `ChoiceReason` is one of available, pinned, source priority, version match, newer, path order), then the newest `About.xml` mtime, then lexicographic path as the final tie-break. The choice is deterministic and does not depend on directory enumeration order.
4. What the game does: when exactly one of two same-id mods is a Workshop item, the game appends `_steam` to the Workshop id and keeps both; otherwise it rejects the later one with an error and "later" depends on unsorted filesystem enumeration. RimStudio shows this consequence in the panel: "the game will treat the Workshop copy as `<id>_steam`", and it never hides the Workshop copy from the game.
4a. As built (`rimstudio-library::duplicates`): `resolve(&LibraryIndex, &DuplicatePolicy) -> DuplicateReport` groups by lowercase id (groups sorted by key) and `game_treatment` replays the game's `TryAddMod` per member (`Loaded { effective_id }`, `Rejected` or `NotVisible`), including the `_steam` rule above.
5. When linking (section 10), two entries with the same id are never linked into `Mods` unless the user asks; the effective entry is the one linked.

### GD-052 The Duplicates panel (M1)

Statement: a panel (also reachable from the footer count and the source filter) lists every group with the effective entry marked, each member's source, path, version match, mtime and the reason the effective entry won ("pinned", "higher source priority", "matches game 1.6", "newer"). Per group: Pin this copy, Unpin, Open folder, Ignore this group (stored in the ignore file by identity). Counting: groups, not entries. The panel can be filtered to groups that involve the active list only.

The panel never deletes or renames anything. A suggested "Remove the older copy" is not offered for user folders in v1.

AC: the owner's folder fixture yields 7 groups with the right reasons; pinning a copy persists across restart and changes the effective entry; same-folder copies (`... - Copy`) are listed with their folder names; `_steam` explanation appears only for groups where exactly one member is Workshop; activating the non-effective copy is not possible without pinning it first.

Tests: fixture D1 (the 7 cases with fictional ids and the same shape: 3 cross-root, 4 same-root), table-driven tests of the choice ladder, panel component tests.

## 10. The link farm

### GD-060 Why and what (M2, gated by spike S-03)

Statement: because the game sees only `Data`, `Mods` and subscribed items, an active custom mod must appear inside `<install>/Mods` before launch. RimStudio does this with a managed link farm: for each active mod whose source is not visible to the game it creates an entry in `<install>/Mods` pointing at the real folder. Copying and moving alternatives were rejected: moving destroys the user's organisation, replacing the whole `Mods` folder with a link surrenders the install folder, and copying is only the fallback ([research 8.6](../research/steam-and-game-detection.md), decisions D-039 and D-040). Placement: the logic is `rimstudio-library::deploy` (planner, ownership manifest, `ModsConfig.xml` write, pre-launch check) over the `LinkBackend` port implemented in `rimstudio-platform`.

Pitfall learned from the reference tool RimCrow: link deployment as a hard launch dependency with batch scripts and filesystem pre-checks produced launch failures in practice ([RimCrow analysis](../research/rimcrow-analysis.md) section 3.7 and section 7.2 pitfall 5). RimStudio therefore keeps the farm optional per folder (`link: none`), reversible in one action, listed in one manifest, and a failure to create a link never prevents launching a list that does not need it; the only launch block is the one in GD-066, which prevents silent loss of the user's list.

### GD-061 Modes (M2)

Statement: three modes, chosen per custom folder through `link` and by the platform default under `auto`.

| Mode | Mechanism | Default for | Needs | Limits |
|---|---|---|---|---|
| `symlink` | directory symbolic link, absolute target | Linux, macOS | write access to `Mods` | Windows needs Developer Mode or a privilege, so it is not the Windows default |
| `junction` | NTFS junction, absolute local target | Windows | write access to `Mods`, local volume | cannot target a network share (UNC), cannot be relative |
| `copy` | copy of the mod into `Mods` with a `.rimstudio.json` marker inside the copy | fallback everywhere | disk space | stale copies, game and mods may write into the copy; disposable unless the user opts in |
| `none` | nothing is deployed | user choice | none | mods from this folder cannot be activated for launch |

`auto` resolves per mod at plan time: platform default first; if it fails or is impossible (UNC target on Windows, no privilege, Flatpak grant missing) the plan says "copy" for that mod and why. `links` forces links and fails the mod instead of copying; `copy` forces copying. Copy ignores `.git`, `Source`, `.vs` and build outputs by a configurable ignore list and warns above a size threshold.

Link naming: a sanitised folder name plus a short stable suffix from the source id (for example `MyMod__rs3a7f`), so two sources with the same folder name cannot collide and owned links are recognisable. Folder names need not match packageIds.

AC: on each OS the default mode is the one in the table; a UNC target on Windows is planned as copy with the reason shown; names with spaces, `&` and brackets work; two sources with the same folder name get different link names.

Tests: fixtures F1 (symlink farm, Linux and macOS CI), F2 (junction farm, Windows CI), F3 (copy fallback with a fake `LinkBackend` that fails), F4 (name collision), property test on the naming function (valid, unique, stable).

### GD-062 Plan and dry run (M2)

Statement: `deploy_plan` (a query) takes the intended active list and returns a plan: links to create, links to remove, copies to create or resync, entries to leave, blockers and warnings. The plan carries a token (hash of the plan and the revision it was computed from); `deploy_apply` accepts only a current token. The UI shows the plan in the Save preview ("will create 14 links, remove 2, copy 1"), expandable to the full list with the reason for each line, and offers Apply and Cancel; the dry run is the default view and nothing is written by viewing it. The CLI renders the same plan as JSON (`rimstudio-cli deploy --dry-run`).

AC: a stale token is refused with `list.revision-conflict`; the plan for an unchanged state is empty; the plan lists every entry with source and target; viewing the plan never touches the filesystem (asserted with the recording filesystem).

Tests: golden plans on F1 to F4 and a plan after a mod is deactivated; recording filesystem test.

### GD-063 Ownership manifest and unlink-only cleanup (M2)

Statement: every link or copy created is recorded in `deploy/<farmId>/ownership.json` (path, target, targetId, createdAt, kind), keyed by install id. An entry in `Mods` is "ours" only if it is in the manifest and either is a link whose target equals the recorded target, or is a copy carrying the marker. Rules:

1. Never overwrite a non-owned folder: choose another name or report `deploy.name-conflict` (proposed).
2. Cleanup removes owned entries whose mod is inactive, whose source is gone, or that dangle. It runs on Apply, on deactivation and at start, is idempotent and journaled.
3. Removal uses the platform's unlink primitive: `remove_file` for a Unix symlink, `remove_dir` for a junction. Recursive deletion is never applied to anything that is a link. A copy is removed recursively only if the marker is present and matches the manifest.
4. "Remove all links created by RimStudio" is one action that removes owned entries only, reports counts, and leaves non-owned entries untouched.
5. A missing or unreadable manifest puts the farm in read-only audit: RimStudio lists what looks like its own entries (by naming pattern and target) and offers Adopt, never auto-removes.
6. No marker is ever written into the user's source folder; copies carry their marker inside the copy.
7. The write fence (`GameWriteFence`) enforces all of this: it allows only manifest-listed entries under `Mods` and `ModsConfig.xml`, and a unit test with a recording filesystem asserts that nothing else is written ([data and persistence](../architecture/data-and-persistence.md) section 10.3).

AC: after deploy and unlink-all the `Mods` folder equals its original listing byte for byte (names, contents of non-owned entries); a non-owned folder with a conflicting name is never modified; a link whose target was retargeted by hand is not removed (target mismatch) and is reported; on Windows removing a junction does not delete the target's contents (verified in the in-game and CI tests); crash between creating a link and writing the manifest leaves an entry that the audit finds on the next start.

Tests: fixtures F1 to F4 plus F5 (pre-existing non-owned folder with the same name), F6 (hand-retargeted link), F7 (missing manifest); fault injection test that kills between link creation and manifest write; recording filesystem assertion that no write falls outside the two allow lists.

### GD-064 Safety rules (M2)

Statement: apply refuses when `Mods` is not writable (explains per OS, GD-068); refuses a target that is the `Mods` folder, an ancestor of it, or inside it; never links above depth one; does not run while the game is running (GD-070); each mod entry is applied atomically and the whole apply rolls back on error; links are re-checked at every launch, because a Steam integrity check or a game update could remove entries that RimStudio does not own and could in principle remove ours too (not tested: unverified, open question 6 of the research), and the manifest makes restoration automatic.

AC: with `Mods` read-only the plan shows a single blocker with a fix hint, no partial state is created; killing the process mid-apply leaves a consistent manifest on restart (journal replay).

### GD-065 ModsConfig.xml protocol (M2)

Statement: the full protocol is in [data and persistence](../architecture/data-and-persistence.md) section 8.2 (D-040); the discovery-relevant points: the file is found through the selected user data folder; a timestamped byte-exact backup is taken and verified before each RimStudio write; the file is edited by byte-span splice through `rimstudio-xml`; after the game exits the file is re-read and diffed against what was written, and ids the game dropped are reported as `deploy.game-changed-list` with the ids and the likely cause (offline source, orphaned Workshop item, duplicate rejection). Retention: the last 20 backups plus the first of each day for 30 days, never the most recent one that precedes a RimStudio write.

AC: a backup exists and verifies before the write, in a test that fails the write; the diff report names a dropped id in a simulated session where the fixture "game" removes one id; restoring a backup is one click and is itself backed up.

Tests: unit tests on splice and backup with fixtures that include unknown elements and BOM; integration test with a fake process that edits the file.

### GD-066 Pre-launch resolution check (M2)

Statement: before launch (and before saving the list to `ModsConfig.xml`), for every id in the intended active list RimStudio computes where the game will find it, using the table below. If any id has no resolution, the launch is blocked and the ids are listed with the reason, because the game would silently remove them from `ModsConfig.xml` and rewrite the file, losing order irreversibly ([research 8.6](../research/steam-and-game-detection.md) step 6).

| Resolution | Counts as resolved when |
|---|---|
| Official | the id is in `Data` and the folder exists |
| Install Mods | a real folder or an owned link in `Mods` exists, its target is reachable right now, and `About.xml` reads |
| Workshop | the item is in the ACF as installed, the folder exists on disk and status is not `downloading` or `not-downloaded` (GD-020) |
| `_steam` binding | the id in the list is `<id>_steam` and a Workshop copy exists, or the plain id exists as a non-Workshop copy (section GD-051 item 4) |

Block reasons: custom mod not deployed, link target offline (`deploy.source-offline`), link broken or retargeted, Workshop item not downloaded, duplicate id rejected by the game, `Mods` not writable. The block dialog offers, in order: Fix automatically (apply the deploy plan, one click), Deactivate unresolved mods and launch (writes the list without them, after the backup), and Launch anyway (a typed confirmation, with a statement that the game will rewrite the list; the backup is still taken). Launching a list that needs no deployment is never blocked by link failures.

Also verified: every link target reachable now, the `_steam` rule, the Workshop copy fully downloaded.

AC: unplugging the drive after linking turns the check red with the id named; fixing the folder turns it green without restart; a list with only official and Workshop mods launches with zero deploy work; the block dialog never appears when all ids resolve; the check on 700 mods completes in under 50 ms.

Tests: table-driven unit tests over the resolution table; F-fixtures with an offline link; Playwright flow for the three dialog choices.

### GD-067 Launch (M2)

Statement: after the check passes the Launcher starts the game. With `launch.method = auto` (the default) a detected Steam install is launched through `steam://rungameid/294100` via the platform opener, which is required on Linux with Proton and in Flatpak ([research 3.8](../research/steam-and-game-detection.md)) and is the same default as MM-029 and settings ST-050; a non-Steam install, or `launch.method = direct`, executes the detected executable with the profile arguments and the `-savedatafolder` choice. Detailed launch settings are in [settings](settings.md) section 9.

### GD-068 Failure modes and messages per OS (M2)

Statement: each failure has a code, a plain sentence and one suggested action. The table is the minimum; the messages live in the JSON catalogue.

| OS | Failure | Code (proposed) | Message intent | Handling |
|---|---|---|---|---|
| Windows | symlink needs a privilege | `deploy.link-privilege` | "Windows needs a different link type" | use junction; never ask for administrator rights |
| Windows | junction target on a network share | `deploy.link-unc` | "Network folders cannot be linked" | plan copy for that mod |
| Windows | drive letter changed | `deploy.link-broken` | "The folder moved (was D:\, found as E:\)" | show broken, offer re-point using `volumeHint`, never retarget silently |
| Windows | install under Program Files, not writable | `deploy.mods-not-writable` | "RimStudio cannot write to the game's Mods folder" | explain permissions or move the install; offer no admin elevation |
| macOS | `Mods` inside `RimWorldMac.app`, writes blocked | `deploy.mods-not-writable` | "macOS is protecting the game bundle" | show the permission instruction (App Management or Full Disk Access); a copy elsewhere cannot help |
| macOS | app translocated, read-only | `deploy.app-translocated` | "Move RimStudio to Applications and reopen it" | detect read-only, instruct |
| macOS | a game or Steam update replaces the bundle and removes entries | `deploy.link-missing` | "Links were removed by an update" | the manifest restores them at the next apply or launch check |
| Linux | `Mods` on a read-only mount or root-owned | `deploy.mods-not-writable` | "The Mods folder is read-only" | explain, offer nothing destructive |
| Linux | Steam Flatpak cannot see the target | `deploy.flatpak-grant` | "Steam's sandbox cannot see this folder" | check the target prefix against the known grants (`/mnt`, `/media`, `/run/media` per the Flathub manifest), otherwise plan copy or show the `flatpak override --user --filesystem=<path> com.valvesoftware.Steam` instruction |
| Linux | AppImage or Flatpak build of RimStudio cannot write to the install | `deploy.mods-not-writable` | same as above plus sandbox grant hint | show the grant instruction |
| all | duplicate ids in the farm | `deploy.duplicate-link` | "Two mods share this id" | the effective entry is linked only |
| all | partial apply | `deploy.partial` | "Applied 12 of 14, rolled back" | rollback, list failures |

AC: for each row a test exercises the code and the message key exists in `en.json`; the macOS and Linux rows are covered with fake backends and a read-only temp directory; Windows rows run on the Windows CI leg.

### GD-069 In-game verification requirement (gate)

Statement (the per-OS test procedure is in [cross-platform](../architecture/cross-platform.md) section 9): the Mono directory test on the research machine shows symlinked mod folders are listed and `About.xml` reads through them, but it used system Mono and did not load assets through the game's own runtime ([research 8.6](../research/steam-and-game-detection.md) and open question 1). Therefore the link farm does not ship until spike S-03 passes on each OS: a real game session with a symlinked mod on Linux and macOS and a junctioned mod on Windows, verifying that the mod loads (defs, textures, assemblies), that `ModsConfig.xml` keeps the id, that Steam "verify integrity" behaviour is recorded (removes, keeps or repairs the entries), and, for Flatpak, that granted and non-granted targets behave as described. Until S-03 passes, `link: auto` resolves to `copy` on any OS without a passing record, and the UI labels the farm "experimental". The S-03 report is committed as a note under `docs/research/` with the game build and date; results can lower defaults (for example macOS copy only).

AC: the release checklist item "S-03 recorded for the target OS" blocks the packaging task for the link farm; the default-mode table reads from a data file that S-03 results edit.

## 11. Running game and change detection

### GD-070 Running game detection (M2)

Statement: before writing `ModsConfig.xml`, creating or removing links, or launching, RimStudio asks whether the game is running ([D-041](../architecture/decision-register.md)): the process name through `ProcessProbe` and Steam's running flag in the app manifest `StateFlags` (the value 64 for "app running" is from the open question in the research and is unverified), with `steam.pid` consulted only to say whether the client is up. Three answers: running (block with "Close RimWorld first" and a Check again button), not running, unknown. Unknown happens in sandboxes where the host process is invisible (Flatpak, AppImage under restrictions); in that case the action asks for a confirmation sentence instead of blocking. The check is repeated immediately before the write, not only when the dialog opens.

AC: a fake process list containing the game blocks apply and save; unknown asks once per action and offers Remember for this session; the UI never shows "not running" when the answer was unknown.

Tests: unit on the combination table (process, flag, sandbox) and the answer; Flatpak fixture with a hidden process list.

### GD-071 Change detection: watch versus poll (M1)

Statement: change detection uses the cheapest mechanism that is correct, per root ([data and persistence](../architecture/data-and-persistence.md) D-028):

1. Watch only roots and metadata files, never the file tree of a 690-item Workshop (306k files): the watcher observes the `Mods` folder, each custom folder's root level (and grouping folders at `scanDepth` 2 and above), the Workshop content folder's directory level, `appworkshop_294100.acf`, `ModsConfig.xml` and `Version.txt`. A recursive watch is used only for the active project (toolkit).
2. Events are debounced (at least 500 ms) and coalesced into a targeted rescan of the affected mod root; an event inside a linked entry in `Mods` is ignored because the real target is watched (this prevents double events and storms).
3. Poll fallback: network and removable drives that deliver no events, inotify limit errors (ENOSPC) and `watch: false` folders use the periodic rescan (directory mtimes, interval from [settings](settings.md) section 8), a Refresh command, and a rescan on window focus.
4. `ModsConfig.xml` changes while the app is open (the game wrote it) raise an inline notice "The game changed the active list" with Reload and Compare, and never overwrite unsaved edits.
5. Detection re-runs when a Steam root or library file changes.

AC: touching an `About.xml` in a custom folder updates one row within a second on a local volume; on a poll-only folder the update appears on window focus; a fixture that reports ENOSPC falls back to polling with `scan.watch-fallback` (proposed) shown once; no events cause the list order or unsaved edits to change.

Tests: integration tests with a real watcher on temp directories (Linux and Windows legs), simulated ENOSPC through the watcher port, debounce unit tests.

## 12. Failure and empty states

### GD-080 Empty and error states (M1)

Statement: every state has designed copy and exactly one primary action. They apply to the wizard, Settings and the library.

| State | What the user sees | Primary action |
|---|---|---|
| No install found | "RimWorld was not found" with the list of places checked (collapsible) | Choose folder |
| Install found, no mods | "No mods yet" with the three source kinds explained | Add a mod folder |
| Install found, Workshop folder missing | "Steam Workshop content not found; you can still use your own folders" | Choose Workshop folder or continue |
| Steam client or helper absent | no block; features that need it explain themselves later | none |
| Custom folder offline | card chip offline, rows greyed | Find the drive (proposal from `volumeHint`) |
| Custom folder empty | "No mods found here" with the layout and depth hint | Change scan depth |
| Everything unreadable | one banner with the first error and Details | Run detection again |
| Read-only data roots | banner "Settings will not be saved", in-memory settings | Open data folder |
| Scan failed for one mod | per-mod diagnostic; row stays | none |
| Game running | "RimWorld is running" on the deploy and save actions | Check again |

AC: each state exists in the gallery in both themes and is reachable in Playwright with mocked IPC; no state shows a raw error code without a sentence; no state blocks navigation to the rest of the app.

## 13. Commands, events and diagnostics

Commands (from the registry table in [IPC and state](../architecture/ipc-and-state.md)): `detect_run` (job), `detect_get_report`, `detect_set_override`, `sources_list`, `sources_add_folder`, `sources_update`, `sources_remove`, `sources_probe_folder`, `library_scan` (job), `library_get_status`, `deploy_plan`, `deploy_apply` (job), `launch_start`. The mod manager spec additionally names `deploy_unlink_all` and `library_scan`; the registry table is authoritative and must gain `deploy_unlink_all` (action) and `deploy_audit` (query) before M2 (see section 15). Events: `library.delta`, `job.progress`, `datasets.*` unaffected.

Diagnostic codes used here: existing `deploy.override-invalid`, `deploy.source-offline`, `deploy.game-changed-list`, `scan.non-utf8-path`, `list.revision-conflict`; existing in code `steam.library-timeout`, `steam.version-unparseable`, `deploy.source-overlap`, `deploy.source-inside-mods`, `scan.path-not-directory`, `scan.dangling-link`, `scan.same-folder`; proposed `steam.client-running`, `deploy.source-inside-mods`, `deploy.name-conflict`, `deploy.link-privilege`, `deploy.link-unc`, `deploy.link-broken`, `deploy.link-missing`, `deploy.mods-not-writable`, `deploy.app-translocated`, `deploy.flatpak-grant`, `deploy.duplicate-link`, `deploy.partial`, `scan.ambiguous-layout`, `scan.link-loop`, `scan.dangling-link`, `scan.path-not-directory`, `scan.watch-fallback`.

## 14. Test plan

| Layer | What | Where |
|---|---|---|
| Unit | VDF reader, status function, override resolution, choice ladder, link naming, plan builder, resolution table, canonical path overlap | each crate, nextest, rstest, proptest |
| Golden JSON | DetectionReport per fixture (L1 to P1), deploy plans (F1 to F7), scan results (S1 to S5), duplicate groups (D1), Workshop statuses; `UPDATE_GOLDEN=1` regenerates; 1 and 8 threads identical | `tests/golden`, `rimstudio-testing` helpers |
| Fixtures | L1 to P1 from the research; D1 duplicates (7 cases, fictional ids), D3 nested sources; S1 to S5 scans; F1 to F7 farm; generated by builders, no real content | `rimstudio-testing` |
| Integration | Real filesystem in temp dirs: link and unlink, junction on Windows, copy fallback, offline rename, watcher events and fallback, ModsConfig backup and diff with a fake game process | per-OS CI legs |
| Bench | Detection under 500 ms, level 0 warm 150 ms, refresh 50 ms, pre-launch check under 300 ms (MM-030) | criterion against `xtask/budgets.jsonc` |
| Frontend | Component tests with mocked IPC, gallery screenshots for every card and empty state in both themes, Playwright flows (detect and switch, add folder, deploy and launch check, duplicate pin) | Vitest, Playwright mockIPC |
| Real install | Detection and Workshop status on a real machine | `#[ignore]` with an environment variable |
| In-game | S-03 per OS, a manual checklist with recorded results | release gate |

## 15. Owner decisions and open points

1. The in-game link test (GD-069, spike S-03) is a gate; until it passes `auto` resolves to copy. The owner or a tester on each OS must run it.
2. Whether Launch anyway (GD-066) should exist at all, or whether the block should offer only Fix and Deactivate. Assumption: it exists, behind a typed confirmation, because the user always has the backup.
3. The registry table must gain `deploy_unlink_all` and `deploy_audit` (section 13); the mod manager spec already lists the former.
4. macOS default: this document assumes symlink farm with a copy fallback, but the [packaging research](../research/cross-platform-packaging-research.md) section 2.3 judges link farms inside the app bundle fragile, since updates can wipe them. If S-03 confirms that, the macOS default should become copy with a re-check at every launch.
5. Itch.io and Epic layouts are not supported beyond manual selection (open question 9 of the research).
6. Open research questions that affect this document: whether the Steam integrity check removes unknown entries in `Mods` (not tested), the exact macOS user data path produced by the game, which Flatpak config path RimWorld writes, whether `HKCU` `SteamPath` exists on current Windows, and the value of the app-running flag (all unverified; each has a fallback above).

## 16. As built: the facts of a scan

The result of the `library_scan` job carries the numbers that the Setup page shows next to the statistics. They are computed once per scan by `rimstudio_manager::facts::gather` from the index the scan just built and from the duplicate report of that same index. Nothing is guessed from a folder name or a Workshop item id, and nothing is read again from disk except one `About.xml` (see `ceInLibrary`).

| Field of `LibraryScanResult` | Where the number comes from |
|---|---|
| `counts.mods`, `counts.defs` | the number of mods and of indexed definitions of the index |
| `counts.loadable` | mods whose loadability flag is `loadable`: the game sees the folder (install `Data`, install `Mods`, a Workshop folder, or a custom folder mod that is linked into `Mods`) |
| `counts.customOnly` | mods whose flag is `needs-link`: the mod exists only in a custom folder, so the game cannot see it until it is linked or copied into `Mods`. `loadable + customOnly` is always `mods` |
| `counts.unavailable`, `unparsedAbout`, `syntheticIds` | rows restored from the cache of an offline source, `About.xml` files that could not be parsed, and mods with a made up package id |
| `counts.duplicateGroups` | the number of package ids that appear more than once, compared ignoring case |
| `sources[].mods`, `loadable`, `customOnly` | the same flags counted per source of the mod. A source with no mods reports zeros; the sums over all sources equal the totals |
| `duplicates.total`, `skippedTotal` | every duplicate group and every skipped copy of the scan |
| `duplicates.groups` | the first 50 groups, sorted by package id (the cap keeps the result small; compare with `total`). Each group names the package id as the kept copy spells it, whether all copies share one source, the rung of the choice ladder that decided (`available`, `pinned`, `source-priority`, `version-match`, `newer` or `path-order`), the kept copy and the skipped copies |
| `duplicates.groups[].kept`, `skipped[]` | the folder, display name, source id and kind of the copy, what the game does with it (`loaded`, `rejected` or `not-visible`) and, for a skipped copy, an English reason built from the deciding rung |
| `ceInLibrary` | the index entry with the package id `ceteam.combatextended`, compared ignoring case; with several copies the kept copy of the duplicate group. `present` is false when there is none. `version` is the `modVersion` of that copy's `About.xml`, read when the copy's source is online; it is absent when the file has none |

The library keeps the copy of a duplicate group that ranks first in its ladder (a custom folder ranks first by default), which need not be the copy the game loads: a custom folder copy that is not linked is `not-visible` to the game, and the copy the game loads is then one of the skipped ones. Both entries say what the game does, so the page can show both facts.

On the owner's machine (read only run of `real_install_detects_adds_the_custom_folder_and_scans`): 763 mods, 744 loadable, 19 custom only, 6 duplicate groups with 6 skipped copies, Combat Extended 16.7.3.0 in the Workshop folder, loadable.

## 17. As built: linking one project for testing

Requirement ids GD-090 to GD-093. The link farm of section 10 stays deferred; this is its smallest useful part ([ADR 0050](../adr/0050-link-one-project-into-the-game.md), D-170, D-171).

### GD-090 The entry and its states

`project_link_status {projectId}` reads `<install>/Mods/<project folder name>` and answers one of: `not-linked` (free), `linked` (a link RimStudio made, pointing at the project), `linked-by-hand` (a link RimStudio did not make that points at the project, never removed), `stale` (a link RimStudio made that points elsewhere or nowhere), `foreign-link`, `foreign-folder`, `copy` (a marked copy RimStudio made), `in-mods` (the project folder is inside `Mods`) and `unavailable` (no game, no `Mods` folder or no project folder). It also gives the game folder, the entry path, where a link points, whether the game is running (`running`, `not-running`, `unknown`), what the link backend can create, `canCreate`, `canRemove`, the manual command and diagnostics.

### GD-091 Create and remove

`project_link_create {projectId, mode?, confirmGameRunning?}` creates the entry through the write fence (a symbolic link by default; `junction` where the backend offers one; `copy` only on request). It never replaces anything. A refused call changes nothing and answers `done: false` with a `refusal` (`deploy.name-taken`, `deploy.already-linked`, `deploy.game-running`, `deploy.mods-missing`, `deploy.mods-read-only`, `deploy.mode-unsupported`, `deploy.target-refused`, `deploy.game-not-found`, `deploy.manifest`, `deploy.io`) and the status after the call. `project_link_remove {projectId}` removes only what the manifest says RimStudio made, and is not blocked by a running game.

### GD-092 Manual command and the active list

The status carries the command that makes the same link by hand: `ln -s '<project>' '<entry>'` on Linux and macOS, `mklink /J "<entry>" "<project>"` on Windows. The page shows it, with a copy button, when the automatic way is refused or impossible. The status reads `Config/ModsConfig.xml` (read only, through the codec of `rimstudio-xml`) and says whether the project's package id, and Combat Extended's when the project has a gated Combat Extended folder, are in the active list (`active`, `inactive`, `unknown`).

### GD-093 The page card

"Test in RimWorld" on the Project page: the state and its sentence, Link into the game (with a confirmation that names the entry and the target, the mode, and the running game check), Remove link, the manual command, the active list hints, and the short steps to find the mod in the game, with the Combat Extended step and its load order note when the project has a Combat Extended patch. The CLI has `project link status|create|remove` (create changes nothing without `--yes`).

Checked on the owner's machine with a temporary game folder (the real `Data` linked in, a temporary `Mods`): status, create, status, remove, a refusal because a folder of that name existed, and a missing `Mods` folder, all through the bridge; the owner's real `Mods` folder was never used.
