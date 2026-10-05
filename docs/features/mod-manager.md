# Mod manager: functional specification

This document specifies the RimStudio mod manager (requirements R5 and, at the UX level, R6): who it is for, what it does, how every screen and flow behaves, which backend commands and events each requirement uses, what the performance targets are and how it is tested. It is a product specification, not a design brief: pixel styling belongs to the later design prompt (R8). It refers to the research notes for evidence and to the architecture documents for crate and command naming. RimSort and RimCrow are described in our own words as concept references only (R11).

Status: draft | Last updated: 2026-10-04


Milestone numbering follows the [roadmap, section 1.1](../roadmap.md#11-mapping-to-the-milestone-names-in-the-register) (older mentions of M3 to M6 use the decision register's numbering).
## Contents

1. [Purpose and users](#1-purpose-and-users)
2. [Guiding principles](#2-guiding-principles)
3. [Scope by milestone](#3-scope-by-milestone)
4. [Conventions for requirements](#4-conventions-for-requirements)
5. [Functional requirements](#5-functional-requirements)
6. [Keyboard shortcuts](#6-keyboard-shortcuts)
7. [Screen states: empty, loading, error, offline](#7-screen-states)
8. [Accessibility](#8-accessibility)
9. [User flows and step counts](#9-user-flows-and-step-counts)
10. [Backend commands and events](#10-backend-commands-and-events)
11. [Performance targets](#11-performance-targets)
12. [Test plan](#12-test-plan)
13. [Owner decisions and open points](#13-owner-decisions-and-open-points)

## 1. Purpose and users

The mod manager is the part of RimStudio that decides which RimWorld mods are active, in which order, and launches the game with that list. It must be as capable as RimSort (see [feature and UX inventory](../research/rimsort-feature-and-ux-inventory.md)) and noticeably faster and easier, and it must treat the owner's multi-source setup (Steam workshop, install folder, folders of own mods on other drives) as the normal case rather than an advanced one (R3, R4).

| User | Typical library | What they need from the manager |
|---|---|---|
| Player with a large list | 300 to 700 workshop mods, one or two Steam libraries | Fast startup, a sort they can trust, readable warnings, safe launch, undo |
| Player who shares lists | Same, plus forum or Discord support | Import and export in common formats with a diff, share codes |
| Mod author | 5 to 50 own mods in a folder outside the game, plus the workshop | Own folders as first-class sources, test lists, bisect, log viewer, handoff to the toolkit |
| Troubleshooter | A list that crashes | Log analysis tied to mods, bisect, history to compare "last good" with now |

Out of scope for this document: the toolkit (defs, patches, designer) and the publisher; they have their own specifications. The manager only provides entry points to them (open a mod in the project workspace, open a log line's file in the def explorer).

## 2. Guiding principles

1. **Performance.** The list appears from cache before any disk work finishes; no interaction waits on the disk, the network or a dataset. Budgets are in [section 11](#11-performance-targets) and are enforced by benchmarks ([scan performance spike](../research/scan-performance-spike.md), [webview and IPC performance](../research/webview-and-ipc-performance.md)).
2. **Intuitive.** The common loop (find a mod, enable it, see whether it conflicts, sort, launch) needs fewer steps than RimSort ([section 9](#9-user-flows-and-step-counts)). Features are discoverable through visible controls, the command palette and searchable settings; nothing important lives only in a context menu.
3. **Keyboard first.** Every action has a command id, a palette entry and a rebindable shortcut. Lists are fully operable without a pointer, with the same results as drag and drop.
4. **Non-blocking.** Long work runs as a job with progress and cancel in the task centre; modals are reserved for destructive or irreversible confirmations. The UI never shows a frozen window.
5. **Undoable.** Every list edit is a command on one stack with undo and redo. Destructive disk actions go to the OS trash where possible and announce an undo toast.
6. **Honest about the game.** The game silently deactivates active mods it cannot find, and rewrites its own list when the stored version differs in major.minor ([mod format and corpus](../research/rimworld-mod-format-and-corpus.md), sections 4.2 and 4.3). The manager therefore never writes a list it knows the game would damage, and tells the user why.
7. **User data is separate from derived data.** Notes, tags, colours, groups, profiles, rules and history are user-owned JSON (machine-written) or JSONC (hand-editable) and survive a rescan; scan results and dataset copies are caches and can be deleted (I-17). The manager never modifies a user's mod folders (D-040).

## 3. Scope by milestone

Milestones use the decision register names (M1 library, M2 manager, M3 toolkit core, M4 designer, M5 publisher, M6 hardening; see [decision register](../architecture/decision-register.md)). The [roadmap](../roadmap.md) renumbers them M0 to M7 (section 1.1 there: register M2 is roadmap M2 and M3, register M3 to M6 are roadmap M4 to M7); all feature specs in this folder use the register names. "MVP" is the first usable release at the end of M2; "v1" is feature parity and polish before M6; "later" is post-v1. Every RimSort feature id from the inventory is mapped to a requirement below or dropped with a reason.

### 3.1 Requirement summary by milestone

| Milestone | Requirements |
|---|---|
| MVP | MM-001 to MM-008, MM-009, MM-011 to MM-018, MM-020 (tags, colour, notes), MM-023 (ModsConfig, RimSort JSON, RimPy, text), MM-024 (export list, clipboard report), MM-025, MM-027, MM-029 to MM-031, MM-033, MM-036 to MM-038, MM-040, MM-041, MM-042 |
| v1 | MM-010, MM-019, MM-021, MM-022, MM-023 (share code, save file, Workshop collection, ids), MM-026, MM-028, MM-032, MM-034, MM-035, MM-039, MM-043 to MM-046 |
| Later | MM-047 and the "later" rows below |

### 3.2 Mapping of every RimSort feature

| F id | Feature | Decision | Requirement |
|---|---|---|---|
| F-001 | Refresh | MVP | MM-003, MM-041 |
| F-002 | Clear | MVP, as an undoable command | MM-014 |
| F-003 | Restore | Replaced by undo, redo and diff against saved | MM-014, MM-015 |
| F-004 | Sort | MVP, with preview | MM-017, MM-018 |
| F-005 | Save | MVP | MM-031 |
| F-006 | Run | MVP | MM-029, MM-030 |
| F-007 | Game version label | MVP | MM-004 |
| F-008 | Status bar | MVP, as task centre footer | MM-033 |
| F-009 | Single instance, updater switches | v1: single instance guard in the shell; updater is a shell concern | MM-046 |
| F-010 | Window launch state | v1: geometry remembered automatically | MM-046 |
| F-011 | Dialog placement option | Dropped: the windowing layer handles it | none |
| F-020 | Open mod list | MVP | MM-023 |
| F-021 | Append mod list | v1, as import mode "merge" | MM-023 |
| F-022 | Save mod list as | MVP | MM-024 |
| F-023, F-027 | Rentry import and upload | Later: fragile third-party paste site; share code and text report cover the use case | MM-024 |
| F-024 | Workshop collection import | v1 | MM-023 |
| F-025 | Import from save | v1 | MM-023 |
| F-026 | Export to clipboard | MVP | MM-024 |
| F-028 | Mod list history | v1 | MM-022 |
| F-029 | Upload logs | Later: needs a paste service and consent design | MM-035 |
| F-030 | Open folder shortcuts | MVP, one "Open" menu fed from detected sources | MM-040 |
| F-031 | Settings | MVP | MM-002, MM-046 |
| F-032 | Exit | MVP, shell menu | none |
| F-040 | Cut, copy, paste | Dropped: text fields only | none |
| F-041 | Rule editor | v1 | MM-045 |
| F-042, F-043 | Ignore list editor, reset warnings | v1 | MM-016, MM-045 |
| F-044 | Reset all colours | v1 | MM-020 |
| F-045, F-046 | Translation auto-add and status | Later | MM-047 |
| F-050 | Download RimWorld version | Dropped: out of scope | none |
| F-051, F-055 | Git mods, GitHub mods | Later | none (listed in MM-044 as extension point) |
| F-052 | Add zip mod | v1 | MM-044 |
| F-053 | Browse Workshop | v1: opens Steam pages and resolves ids; no embedded browser in MVP | MM-044 |
| F-054, F-216 | Workshop update check | v1 | MM-028 |
| F-056 | Verify game files | Later: opens the Steam page for the game | MM-040 |
| F-060 | Switch instance | v1, mapped to profiles and sources, not to separate installs | MM-021 |
| F-061 | Instance backup and clone | Later | none |
| F-062, F-063 | Texture optimisation | Later: a tool wrapper, not the manager | none |
| F-064 | Self update | v1, in the shell | MM-046 |
| F-065 | Help links | MVP | MM-034 |
| F-100 | Two lists | MVP | MM-004 |
| F-101 | Search with field selector | MVP, extended to query language | MM-009 |
| F-102, F-103 | Filter panel, hide versus highlight | MVP search and MM-010 for panel | MM-009, MM-010 |
| F-104 | Counters as filters | MVP for diagnostics, v1 for the rest | MM-010 |
| F-105 | Inactive sort selector | MVP | MM-009 |
| F-106 | Folder size | v1, background job | MM-007 |
| F-107 | Dividers | v1, as groups | MM-019 |
| F-108, F-109 | Keyboard moves, double click | MVP | MM-012, MM-013 |
| F-110 | Multi-select drag | MVP | MM-011, MM-012 |
| F-111 | Lazy row widgets | Replaced by true virtualisation | MM-004, MM-042 |
| F-120, F-121 | Source icon, content icon | MVP | MM-006 |
| F-122 | Warning and error icons | MVP | MM-006, MM-016 |
| F-123 | Updated and new badges | v1 | MM-028 |
| F-124, F-125 | Colour, tags | MVP | MM-020 |
| F-126 | Startup impact | Later | none |
| F-127 | Tooltip | MVP | MM-006 |
| F-130 | Open folder, open in editor | MVP | MM-040 |
| F-131 | Tag and colour actions | MVP | MM-020 |
| F-132 | URL actions | MVP | MM-040 |
| F-133 | Source conversion | Dropped: custom folders and link farm replace it | MM-002 |
| F-134 | Re-download, re-subscribe | v1, via Steam helper when available | MM-044 |
| F-135 | SteamDB blacklist | Dropped: needs write access to a community repository | none |
| F-136 | Copy id, edit rules, translations | MVP copy id, v1 rest | MM-040, MM-045 |
| F-137 | Deletion menu | MVP, trash by default | MM-040 |
| F-138 | Duplicates panel | MVP | MM-027 |
| F-139 | Missing mods prompt | v1 | MM-026 |
| F-140 | Missing dependencies dialog | MVP | MM-025 |
| F-141 | Missing properties panel | v1, folded into diagnostics | MM-016 |
| F-142 | Use This Instead panel | v1 | MM-039 |
| F-200 | Player.log viewer | v1 | MM-035 |
| F-201 | File search | Dropped from the manager: subsumed by the def explorer and workspace search (toolkit) | none |
| F-202 | ACF log reader | Dropped: SteamCMD internals | none |
| F-203, F-205 | Game recovery, Steam utilities | Later, with undoable backups | none |
| F-204 | Orphan cleanup, reset to vanilla | v1 as "clear to vanilla" command and orphan report | MM-014, MM-027 |
| F-210 | Dataset sources | v1: automatic fetch, status panel | MM-039 |
| F-211, F-212 | DB builder and CLI | Dropped: datasets are consumed, not built | none |
| F-213 | Metadata store | MVP, as JSON user data | MM-020 |
| F-214 | Watchdog | MVP (roots and metadata only, D-028) | MM-041 |
| F-215 | Startup dataset refresh | v1 | MM-039 |
| F-217 | Backups | MVP for ModsConfig backups; saves backups later | MM-031 |
| F-218 | Metadata scan | MVP | MM-003 |
| F-219 | Steamworks integration | v1, optional, sidecar | MM-044 |
| F-220 | SteamCMD integration | Dropped: not required; revisit on owner request | none |
| F-221 | Steam folder detection | MVP, superseded by the detection report | MM-001, MM-002 |
| F-222 | Runner panel | v1, as a task console | MM-033 |
| F-223 | Modal progress | Dropped: task centre replaces it | MM-033 |
| F-300 | Tiered sort | MVP | MM-017 |
| F-301 | Alphabetical sort | Dropped: deprecated upstream | none |
| F-302 | Dependency and incompatibility checks | MVP | MM-016 |
| F-303 | Duplicate id warning | MVP | MM-016, MM-027 |
| F-304 | Rule sources | MVP for About and user, v1 for community | MM-016, MM-039 |
| F-305 | Version mismatch | MVP | MM-016 |

## 4. Conventions for requirements

- Each requirement has an id (MM-nnn), a milestone, a statement and acceptance criteria (AC). Criteria are testable; the test plan in [section 12](#12-test-plan) refers to them by id.
- Backend names are the registry names proposed here. The architecture's [IPC and state](../architecture/ipc-and-state.md) document is authoritative for command names; names there are used here as written, and any name marked (proposed) is not yet in its registry table and must be added when its milestone starts. Commands follow the registry convention `<area>_<verb>` with kinds query, action, stream and job (D-005, D-046); DTOs are `<Command>Request` and `<Command>Response` with camelCase fields.
- Mods are addressed on the wire by `ModId` strings and in hot paths by session `ModIdx` handles (D-029). List rows carry list columns only; descriptions and images are fetched on demand (D-043, D-044).
- Diagnostics use the code areas of D-046 (`list.*`, `sort.*`, `scan.*`, `deploy.*`, `dataset.*`, `log.*`). Content problems are diagnostics, never errors.
- Words such as "the game list" mean the active list as stored in `ModsConfig.xml`; "the working list" is the manager's in-memory list including unsaved edits.

## 5. Functional requirements

### 5.1 Setup and sources

#### MM-001 First-run setup wizard (MVP)

On first launch (no `settings.jsonc`) a full-window wizard runs detection and shows the result, instead of an empty app. It has at most three screens: Detected (always), Sources (only if something is missing or ambiguous), Ready.

1. The Detected screen shows one card each for the RimWorld install, the Steam libraries, the workshop content folder, the user config folder (with `ModsConfig.xml`) and the game version read from `Version.txt`. Each card shows the path, how it was found (for example "Steam library file", "registry", "default path") and a confidence chip, taken from the `DetectionReport` ([steam and game detection](../research/steam-and-game-detection.md)).
2. When several candidates exist (two libraries, native and Proton config folders, Flatpak), all are listed with the best preselected; the user changes the choice with one click. Overrides are persisted and never dropped on the next detection run (D-038).
3. A card with no candidate shows "Not found" and a Choose folder button plus a drop target for a folder from the file manager.
4. A "Mod folders" row offers Add folder for custom sources (MM-002) and shows an explanation of what a custom source is in one sentence.
5. Ready shows counts (mods found per source, active mods in the game list) and a single primary button, Open library. Datasets start fetching in the background on this screen if the user left the default "fetch automatically" on.
6. Steam helper or Steam client absence never blocks the wizard; features that need it show their own explanation later.

AC: with a standard Steam install on each of the three platforms the wizard needs one click (Open library) after it appears; a missing workshop folder produces the Sources screen; the wizard can be reopened from Settings, Sources, "Run detection again" and never overwrites overrides without confirmation; the detection result is produced by `detect_run` in under 500 ms on the research machine; the wizard is fully keyboard operable. Commands: `detect_run` (query), `detect_set_override` (action), `sources_add_folder` (action). Events: none (the screen is request and response).

#### MM-002 Sources and custom mod folders (MVP; R3, R4)

Sources are the places mods come from. The manager shows them in Settings, Sources and as a source filter in the library. Kinds: official (install `Data`), install Mods, Steam workshop (one per library), and custom (any number, user added, each with a label, optional colour and a recursive or flat scan mode). A custom folder can be on any drive, including removable or network drives.

1. Add folder opens the native folder picker or accepts a drop; the new source is scanned immediately and its mod count appears on its card.
2. Each source card shows path, mod count, last scan time, whether the folder is currently reachable, and a visibility state for the game: "visible to the game" (official, install Mods, workshop) or "needs a link" (custom). Custom sources never modify the folder itself.
3. A custom mod can be listed and edited in lists at all times, but enabling it requires a managed link or copy into the game's Mods folder (the game reads only three places; [mod format and corpus](../research/rimworld-mod-format-and-corpus.md), section 4). The manager shows the row state "not visible to the game" and, on enable, offers Make visible (default method per OS as in GD-061: junction on Windows, symlink on Linux and macOS, copy fallback; until spike S-03 passes on an OS, `auto` resolves to copy there, GD-069). The deploy plan is shown in the Save preview (MM-031), recorded in an ownership manifest, and removable in one action ("Remove all links created by RimStudio", unlink only) (D-039, D-040).
4. An unreachable source (unplugged drive) keeps its mods in the cache as "offline" rows with a distinct badge; active offline mods raise a blocking diagnostic `deploy.source-offline` before save and launch rather than silently dropping them.
5. Removing a source never deletes files; it removes the source entry and its derived cache, and active mods from it become missing mods (MM-026).
6. The same mod folder reached through two sources (for example a custom folder inside the workshop path) is detected by canonical path and listed once.

AC: adding a second external folder with 40 mods lists them within 1 s of the scan completing; enabling a custom mod and saving results in a link, a manifest entry and an active id that the pre-launch check resolves; unplugging the drive after linking turns the check red with the id named; "Remove all links" leaves zero owned entries and never touches non-owned entries; a simulated filesystem without link support falls back to copy with a warning and launching still works. Commands: `sources_list`, `sources_add_folder`, `sources_update`, `sources_remove`, `library_scan` (job), `deploy_plan` (query), `deploy_apply` (job), `deploy_unlink_all` (action). Events: `library.delta`, `job.progress`.

### 5.2 Workspace

#### MM-003 Startup, scan and refresh (MVP)

1. The shell paints with no data dependency. The library view renders the cached list (snapshot) immediately, marked with a thin "refreshing" indicator in the footer while a background rescan checks stat keys (path, size, mtime, file id) per About file and re-parses only changed ones (D-023).
2. Refresh (shortcut `F5`) triggers the same rescan and, if the user wants, a dataset freshness check. It never rebuilds or reorders the working list; changes arrive as deltas and unsaved edits are preserved (the list order is a user draft, not a view of the disk).
3. Scan problems (unreadable folders, malformed About files) are per-mod diagnostics with counts in the footer, never an abort (scan spike, implication 11).
4. A cold start with no cache shows rows progressively as level-0 metadata arrives, with a progress bar and an estimated count.

AC: with a warm cache and no changes the list is interactive within the budget in section 11; a rescan with one changed About file updates exactly one row; a deleted mod folder produces a `library.delta` removal and, if it was active, a missing-mod diagnostic; unsaved edits survive Refresh in a test. Commands: `mods_snapshot` (query), `mods_subscribe` (stream), `library_scan` (job). Events: `library.delta {rev, upserts, removes, order}`, `job.progress`.

#### MM-004 Workspace layout (MVP)

The library screen is a four-column workspace, following the owner's preferred layout from the Parallax mockup brief (mod detail, available, active order, actions rail) at about 34 px row height with virtualised lists:

| Column | Content | Default width | Behaviour |
|---|---|---|---|
| Detail | Selected mod detail (MM-007); with nothing selected, a library summary (counts, dataset freshness, game version) | 300 px, resizable 240 to 480 | Collapsible with `Ctrl+B`; remembered |
| Inactive | Header (title, count, search, filter, sort), virtual list of mods not in the game list | flexible, equal share | Header counters are clickable filters (MM-010) |
| Active | Header (title, count, search, filter), virtual list in load order with position numbers, group headers, diagnostics counters | flexible, equal share | Shows the dirty marker when the working list differs from the saved list |
| Actions rail | Vertical stack: Save, Run, Sort, Undo, Redo, Refresh, Clear, Profiles, Import and export, Palette | 56 px icon mode, 200 px labelled mode | Icon mode by default under 1440 px width; tooltips show label and shortcut |

A top bar holds the profile switcher, the dirty marker with a diff button (MM-015), the game version label, the Steam and datasets status chips and the palette button. A footer holds the task centre summary, scan state and diagnostics totals. The game version label shows the full version string and revision.

AC: all lists are virtualised and hold at most 3000 DOM nodes (webview budget 5); both lists scroll at 60 fps with 5000 rows; column widths and collapse state persist; the layout has no horizontal page scroll at 1280 x 800. Commands: none beyond MM-003.

#### MM-005 Responsive behaviour down to 1280 x 800 (MVP)

The supported minimum window is 1280 x 800. Behaviour by width:

| Width | Layout |
|---|---|
| 1920 and above | Four columns, rail labelled, density "comfortable" default |
| 1440 to 1919 | Four columns, rail icons, detail 300 px |
| 1280 to 1439 | Detail 280 px, rail icons (56 px), lists share the remaining 944 px, about 472 px each; row shows icons and diagnostics at fixed right-aligned slots; name truncates by CSS ellipsis |
| 1100 to 1279 (unsupported but usable) | Detail becomes an overlay drawer opened by selection or `Ctrl+B`; two list columns plus rail |
| Below 1100 | The two lists become tabs (Inactive, Active); a mod moves between them by button or keyboard; detail is a drawer |

Heights down to 800 px keep the top bar and footer fixed and the lists flexible; below 700 px the footer collapses into the top bar. Row icon slots have fixed positions so names and badges do not jump (a RimSort weakness, P15).

AC: Playwright screenshots at 1280 x 800, 1440 x 900 and 1920 x 1080 in both themes show no clipped controls, no overlapping badges and no horizontal scroll; a resize while dragging does not drop the drag.

#### MM-006 List row anatomy (MVP)

A row is 34 px (comfortable), 28 px (compact) or 40 px (roomy), chosen in settings. Slots left to right, with fixed positions:

| Slot | Element | Meaning | Trigger and tooltip |
|---|---|---|---|
| 1 | Group colour bar (3 px) | Row belongs to a group, in the group's colour | Group name on hover |
| 2 | Position number (active list only) | Load index, 1 based | Click selects; tabular digits |
| 3 | Source icon | Official, install Mods, workshop, custom (with the source's own colour dot) | Tooltip: source label and path |
| 4 | Content icon | "C#" when the mod has assemblies, "XML" when content only | Tooltip explains: code mods can conflict at runtime |
| 5 | Name | Display name, then optional tags (chips) when density allows, in the mod's custom colour if set | Long tooltip after 600 ms: name, authors, package id, version, supported versions, size if cached, path, last modified |
| 6 | Version chip | Mod version when it differs between duplicates, or always if enabled | Hover shows supported game versions |
| 7 | State badges | "Not visible to the game" (custom without link), "Offline" (source unreachable), "New" (not in the latest save when the option is on), "Updated" (changed within N days, default off, N default 3), "Pinned" | Each badge has its own tooltip; "Updated" opens the Workshop changelog |
| 8 | Diagnostics icon | Red circle with count for errors, amber triangle for warnings, blue for info (for example a replacement is available) | Click opens the diagnostics popover for that mod (MM-016); hover shows the first three messages; the popover has Mute and Fix actions |
| 9 | Overflow button (appears on hover or focus) | Row context menu | Same as right click |

Rows use `role="option"` inside a `listbox` with `aria-selected`, and diagnostics carry text alternatives (MM-038). Colour is never the only carrier of meaning: every badge has an icon or letter. Selected, hovered, focused, dragging and drop-target states are separate visual states. Pinned means the user has pinned the row so that sort does not move it (MM-017).

AC: a screenshot test of every state in the gallery in both themes; icon slots are at identical x positions across 100 rows with different name lengths; tooltips never appear while dragging; the row height is within 1 px of the setting.

#### MM-007 Mod detail panel (MVP; folder size v1)

Shown in the detail column for the last selected mod (or the selected row count and a bulk summary when several are selected).

- Header: preview image from the `rsimg` scheme (256 px thumbnail), name, source and status badges.
- Fields: package id (copy button), authors, mod version, supported versions (the current game version highlighted or flagged), tags, group, colour, notes (editable, saved as user data), path (click opens the folder), Steam page and homepage links, last touched, file system modified, workshop created and updated times when known, folder size (computed in a background job, "Calculating" until done, cached), dependencies and incompatibilities with satisfied or missing state and a jump link to each mod, and the rules that apply with their source layer (About, community, user) and the explanation of "why is this mod above or below that one" (MM-018).
- Description rendered from Steam BBCode or plain text through the rich text renderer, loaded lazily on selection (descriptions are never in list payloads).
- Tabs: Overview, Relations (dependency and order graph for this mod, v1), Diagnostics, Files (open in the toolkit, v1).

AC: selecting a row shows the detail within 50 ms from cache (description may follow); the description is sanitised and cannot run script or load remote resources; copy and open actions work for custom and workshop mods; with a multi-selection the panel shows a count and bulk actions (tag, colour, group, enable, disable). Commands: `mods_get_detail` (query), `library_folder_size` (job), `usermeta_set_notes` (action). Events: `job.progress`.

#### MM-008 Actions rail (MVP)

The rail is the persistent home of the global actions. Order and behaviour:

| Action | Behaviour | Default key |
|---|---|---|
| Save | Opens the save preview (MM-031) unless "save without preview" is on; disabled when clean; shows a dot when dirty | `Ctrl+S` |
| Save and Run | Primary, below Save; saves if dirty and launches (MM-029) | `Ctrl+Enter` |
| Sort | Starts the sort preview (MM-017) | `Ctrl+Shift+T` |
| Undo, Redo | Command stack (MM-014) | `Ctrl+Z`, `Ctrl+Shift+Z` |
| Refresh | Rescan (MM-003) | `F5` |
| Clear | Moves all non-mandatory mods to inactive (undoable); official content stays unless the user chooses otherwise | none, palette |
| Profiles | Switcher popover (MM-021) | `Ctrl+P` |
| Import and export | Menu with import, export, share code (MM-023, MM-024) | `Ctrl+O`, `Ctrl+Shift+E` |
| Tools | Open the toolkit and logs (MM-035) | none |

AC: every action is reachable from the palette; disabled actions explain why in their tooltip; no action requires a modal to start.

### 5.3 Finding and selecting

#### MM-009 Search, filters and inactive sorting (MVP; query language MVP-lite)

Each list has a search box (`Ctrl+F` focuses the box of the focused list, `Ctrl+Shift+F` searches both lists). Plain text matches name, package id and authors, case and diacritics insensitive, with substring and word-start matching.

- **Fields:** `name:`, `id:`, `author:`, `workshop:`, `version:`, `tag:`, `group:`, `source:`, `type:` (code or content), `note:`, `desc:` (description, v1), `has:` (`error`, `warning`, `note`, `rules`, `link`), `is:` (`active`, `inactive`, `new`, `updated`, `pinned`, `duplicate`, `offline`, `official`).
- **Operators:** implicit AND between terms, `OR`, `-` for negation, quotes for phrases, parentheses (v1), `:` contains, `=` equals, `>` and `<` for versions and dates (`updated>7d`, `size>500mb`), `~` for fuzzy.
- **Mode:** hide non-matching rows (default) or keep and highlight, toggled per list (`Alt+H`).
- **Inactive sorting:** by name, modified time, author, folder size, package id, version, colour, tags, updated, source; ascending or descending; persisted optionally. The active list is never re-sorted by column; its order is the load order.
- Invalid queries show the parse error inline with a position and keep the previous result.

AC: a 5000 mod list returns results for every keystroke within 50 ms (index built in Rust, frontend only receives the matching `ModIdx` array); the query `is:inactive tag:qol -author:ludeon` is covered by a unit test; fuzzy and field matching work with non-ASCII names. Commands: `library_query` (query, returns a `ModIdx` array and match ranges). Events: none.

#### MM-010 Counters and saved filters (v1)

Header counters (errors, warnings, new, updated, offline, duplicates) are clickable toggles that add the corresponding `has:` or `is:` term. A filter panel offers source, type and tag checkboxes with Select all, None and Clear. Any query can be saved with a name (Save filter button); saved filters appear in a dropdown and in the palette, stored in user data JSONC. Filters are combinable with the free-text query.

AC: clicking the error counter and then the warning counter shows rows with errors or warnings (the two terms combine with OR within the counter group); saved filters survive restart and rescan; deleting a source does not break a saved filter (it simply matches nothing).

#### MM-011 Selection (MVP)

Click selects; `Ctrl` or `Cmd` click toggles; `Shift` click selects a range in the current list order; `Ctrl+A` selects all visible (filtered) rows in the focused list; `Esc` clears. Rubber-band selection is not provided. Selection is per list, survives filtering, and is restored after undo of a move. The selected count is shown in the list header and the detail panel.

AC: a range select across 2000 rows takes under 50 ms; selection does not change when a delta arrives for an unrelated mod; a selection of 500 rows can be moved by one key press in under 50 ms.

#### MM-012 Drag and drop (MVP)

Pointer based drag and drop through the shared facade, with a keyboard equivalent for every drop (D-054, gated by spike S-05).

1. Dragging a selected row drags the whole selection, preserving its relative order; a ghost shows the first row and a count.
2. A drop within the active list reorders; a drop from inactive to active enables at the drop position; a drop from active to inactive disables. A drop on a group header moves into that group (MM-019).
3. A live insertion line shows where rows will land. Hard order rules the move would break show the line in red with a one-line reason; the move is allowed but raises diagnostics, except hard rules involving official content (Core first, expansion order) which are refused with a message (the game refuses the same).
4. Auto-scroll at list edges; `Esc` cancels the drag.
5. Each completed drop is exactly one undoable command, however many rows moved.
6. External drops: a folder dropped on the window offers to add it as a custom source; a mod list file dropped on the window opens the import preview (MM-023).

AC: spike S-05 passes (two virtualised lists, 5 selected rows, 16 ms frames); a drop dispatches exactly one `list_move` intent and never produces duplicated rows (a RimSort weakness, P9); keyboard parity test performs the same edits without a pointer and produces an identical list.

#### MM-013 Keyboard moves (MVP)

With a list focused: Up and Down move focus; `Home` and `End` jump; `PageUp` and `PageDown` page; `Space` or `Return` toggles the selection to the other list (appended at the end of the active list or placed after the last selected active row when the user has "insert after selection" on); `Left` and `Right` move focus to the other list keeping the nearest row; `Alt+Up` and `Alt+Down` move selected rows one position within the active list; `Alt+Shift+Up` and `Alt+Shift+Down` move to the next group boundary; `Alt+Home` and `Alt+End` move to top (after mandatory official content) or bottom; `Delete` opens the remove menu (MM-040). Holding a key repeats without flicker (one command per repeat, coalesced into one undo entry within 400 ms).

AC: every pointer operation in MM-012 has a keyboard path; moving a row 20 positions with held `Alt+Down` produces one undo entry; key handling for 3000 rows stays under 50 ms per press.

### 5.4 Editing model

#### MM-014 Command stack, undo and redo (MVP)

All edits of the working list go through one command stack in `LibrarySession` (rimstudio-manager). Commands: enable, disable, move, sort apply, clear, import apply, profile switch, group edits, pin, and tag, colour and note edits. A command has a label ("Move 3 mods", "Sort by tiers"), a forward and a backward patch, and is coalesced when repeated within 400 ms by the same actor.

1. `Ctrl+Z` and `Ctrl+Shift+Z` (and `Ctrl+Y`) undo and redo; the Undo button tooltip names the command.
2. The stack holds at least 200 commands per session and is cleared when the user starts a bisect restore or switches profile (the profile switch itself is undoable, in the same session).
3. Undo of an import restores the exact previous order and sets.
4. Disk effects (links, ModsConfig writes, trash) are not on the stack; they are separate confirmed actions with their own undo (toast with Undo for trash, "Remove links" for deploy).
5. A history popover (v1) lists the stack so the user can jump to any point.

AC: property test: any random sequence of commands followed by the same number of undos returns the original list byte for byte; 1000 commands on 610 mods run under 1 ms each; undo and redo update both lists in one delta. Commands: `list_toggle`, `list_set_active`, `list_move`, `list_clear`, `list_undo`, `list_redo` (actions returning `{rev, delta}`) (proposed). Events: `library.delta`.

#### MM-015 Dirty marker and diff against the saved list (MVP)

The top bar shows a "Modified" marker whenever the working list differs from the saved list (the last list written to `ModsConfig.xml` by RimStudio, or loaded from it at start). Clicking the marker opens a diff panel: added mods, removed mods, moved mods (with before and after positions, collapsed when a block moved together), and changed groups, each row clickable to select the mod. The panel has Discard changes (revert to saved, undoable), Save, and Copy diff.

If `ModsConfig.xml` changed on disk since the last load (for example the player used the in-game Mods screen), the watcher raises a banner: "The game list changed outside RimStudio" with Show diff, Reload and Keep mine.

AC: after moving three mods and enabling one, the diff shows exactly those changes and no others; a block move of 40 mods is shown as one entry; an external edit is detected within 3 s while the app is focused; Discard returns a clean state. Commands: `list_diff` (query), `list_reload_game_list` (action). Events: `game.list-changed`.

### 5.5 Diagnostics and sorting

#### MM-016 Live diagnostics (MVP)

The diagnostic catalogue (codes, severities, triggers, message templates, suggested actions and ignorability) is owned by LO-019 in [load order and validation](load-order-and-validation.md#lo-019-the-catalogue-mvp-unless-noted); this requirement keeps its id as a pointer. The Rust core computes diagnostics for the working list as pure functions and ships only changed rows (P2); mute and fix behaviour are specified with LO-021 and LO-027 in the same document. The codes `deploy.not-visible` and `deploy.source-offline` (errors that block save) belong to the deploy producers of [game and mod discovery](game-and-mod-discovery.md).

AC: golden diagnostics on the fixtures of section 12 are byte identical at 1 and 8 threads; one recalculation of 610 mods with 1500 constraints takes under 1 ms (target, benchmarked); muting a diagnostic changes counts without a recomputation of unrelated rows; every code has a translated message with the mod names in it. Commands: `diagnostics_list` (query, full), delivered incrementally as `library.diagnostics` deltas; `diagnostics_mute` (action). Events: `library.diagnostics {rev, byMod}`.

#### MM-017 Sort with preview and explanation (MVP)

Sort is a two-step flow: preview, then apply.

1. Sort opens a preview panel over the active column (not a modal). The panel shows a diff: mods that move, with before and after positions, grouped into blocks; counts of moved mods; any cycles found, any hard-rule conflicts that cannot be satisfied, and mods that could not be placed (never dropped).
2. Algorithm choice: "Tiers" (canonical: core and official, frameworks, normal, load-bottom, deterministic order inside levels) is the default; "Game style" (stable depth-first from the current order, minimal movement) is the alternative and is recommended when "keep my order as much as possible" is chosen (core domain, implication 8).
3. Options (remembered): include community rules, include user rules, dependencies imply order, treat pinned mods as fixed, ask about missing dependencies first (MM-025).
4. Apply is one undoable command. Cancel discards the preview. "Sort and apply" in the palette skips the panel for users who trust it; a setting turns this into the default behaviour.
5. Pinned mods and group boundaries: a pinned mod keeps its index when the rules allow; groups stay contiguous unless a hard rule requires otherwise, in which case the preview shows the group as split with the reason.
6. Sorting is deterministic: the same input gives identical output across runs and thread counts, and shuffled input gives the same output.

AC: on the owner's 610 mod fixture the tier sort yields zero violations and the game-style sort moves far fewer mods than 600 of 610; the preview is shown in under 100 ms for 610 mods; cycles are reported by mod names with the rule source of each edge (the lowest priority edge dropped is listed); a test applies preview, undo, redo, and compares lists. Commands: `sort_preview` (job, result is a list of `ModIdx` plus a move list and explanation), `sort_apply` (action) (proposed). Events: `job.progress`.

#### MM-018 Explain why (v1, simple form MVP)

For any two mods, or one mod and its position, "Why is A above B?" opens an explanation: the chain of rules that forces the order, each with its source layer (About, community, user, derived), or "no rule; the position comes from the tie-break". In the MVP, the detail panel lists the rules touching the mod with their sources; in v1 the Relations tab shows the order graph of that mod and the explanation path.

AC: for a fixture pair with a three-edge chain the explanation lists all three edges in order with their layers; unknown cases say "no rule" rather than guessing. Commands: `sort_explain` (query).

### 5.6 Organising

#### MM-019 Groups (v1)

Groups are named, coloured, collapsible ranges of the active list, replacing RimSort's plain dividers (F-107). A group header row (same height as a mod row) shows a chevron, colour dot, name, mod count and diagnostics totals. Create from a selection ("Group selected"), from the context menu or by dragging a header. Rename inline, recolour, collapse, delete (mods stay), move as a unit by dragging the header or `Alt+Up` on it. Collapsed groups hide their rows from the virtual list but search still finds them and expands the group. Groups are user data, stored by mod id so they survive path changes (core domain, implication 14), and exported with profiles and history. Sort keeps groups contiguous (MM-017).

AC: a group of 30 moves with one command and one undo entry; collapsing a group in a 600 row list updates the list within 50 ms; groups survive a rescan and a profile round trip; a mod removed from the library disappears from the group without error.

#### MM-020 Tags, colours and notes (MVP; reset and bulk v1)

Tags are free text (case preserving, matched case insensitively), shown as chips. Colours are chosen from a palette of theme-aware swatches plus a custom picker; a colour tints the name text (or the row background, a setting) and is never the only signal. Notes are plain text up to 4000 characters. All three are stored in user data JSON keyed by mod id (workshop id for workshop mods, `<source>:<packageId>` otherwise; D-029), apply to a multi-selection through a bulk bar in the detail panel and the context menu, and are searchable (`tag:`, `note:`). "Reset all colours" and "Rename or delete tag everywhere" are in Settings, Library data.

AC: applying a tag to 100 selected mods is one undoable command and completes in under 50 ms; tags, notes and colours survive deleting the cache folder; a rename of a tag is atomic across all mods; notes edits are debounced and saved without a prompt.

#### MM-021 Profiles (v1)

A profile is a named saved list: active ids in order, groups, pins, and a launch preset (arguments, launch method). Profiles live in user data; the current working list is the "live" profile. The switcher in the top bar shows the current profile, a modified marker and a menu: Switch, New from current, Duplicate, Rename, Delete, Compare with current, Set as default. Switching a profile replaces the working list in one undoable command, keeps the inactive list as is, and does not clear unsaved work without asking (a three way choice only if the working list is dirty: Save to the profile first, Discard, Cancel). Unlike RimSort instances, profiles do not require separate folders or restarts (F-060); all profiles share the sources.

AC: switching between two 600 mod profiles updates both lists within 150 ms; a profile that references a missing mod loads and raises missing mod diagnostics; "Compare with current" opens the diff panel of MM-015 with the profile as the baseline; deleting a profile asks for confirmation and offers undo for 10 seconds. Commands: `profiles_list`, `profiles_save`, `profiles_load` (action returning delta), `profiles_delete` (proposed).

#### MM-022 History snapshots (v1)

Every successful save writes a snapshot (ordered ids, groups, game version, time, source profile, optional note) to the data root, named by timestamp and a short hash of the ordered id list; duplicates of the previous snapshot are not written. Retention defaults to 100 snapshots, configurable, with the daily thinning of the backup policy applied to ModsConfig backups (keep all of today, the last of each earlier day, always keep one, verify readability after writing; [rimcrow analysis](../research/rimcrow-analysis.md), implication 6). The History panel (opened from the profile menu or palette) is a list with time, note, counts and mod delta against the previous snapshot. Selecting one shows the diff against the live list; two selected snapshots are compared with each other. Actions: Restore (as an undoable command), Export, Edit note, Open folder, Delete.

AC: restoring a snapshot that contains uninstalled mods marks them missing and does not drop them silently; a diff between two snapshots lists added, removed, moved, newly installed and no longer installed mods; retention prunes at the configured count with a clock injected in tests. Commands: `history_list`, `history_diff`, `history_restore`, `history_note` (proposed).

### 5.7 Import and export

#### MM-023 Import with diff preview (MVP; some formats v1)

Import is a single flow: choose or drop a file, paste text, paste a code or a Workshop collection link. The format is detected, never asked, and the result is shown as a preview before anything changes.

| Format | Detection | Milestone |
|---|---|---|
| `ModsConfig.xml` and RimWorld mod list files | Root element `ModsConfigData` or the saved mod list root | MVP |
| RimSort JSON list | Text begins with `{` and has an active mods list | MVP |
| RimPy XML list | Root recognised by its mod list element | MVP |
| Plain text (one id or name per line, mixed encodings) | Fallback; names are matched by package id first, then exact name | MVP |
| Workshop id lists (numbers separated by space, comma or line) | All tokens numeric | v1 |
| Share code | Version prefix and checksum ([rimcrow analysis](../research/rimcrow-analysis.md), implication 7) | v1 |
| RimWorld save (`.rws`) | Root `savegame`; read with a streaming reader that stops after the meta block | v1 |
| Workshop collection URL | Steam collection link; resolved through the Steam web page or helper | v1 |
| History snapshot, RimStudio profile | Own JSON markers | v1 |

Preview shows: how many entries matched, which mods are missing (with Workshop links or "from a custom source that is offline"), which already active, which would be added, removed or moved relative to the working list, and unmatched lines. Modes: Replace (default), Merge (append mods not yet active, keeping existing order; F-021), and Add as new profile. A warning appears when the file's game version differs in major.minor from the installed game. Apply is one undoable command; missing mods feed MM-026.

AC: each format has a golden fixture and a round trip test; a file with an unknown encoding is decoded with fallbacks and reports it; the preview for a 610 mod list appears within 200 ms; an unreadable file never changes the working list and shows an error card with the reason; a save file of 82 MB is read in under 100 ms. Commands: `list_import_detect` (query), `list_import_preview` (job), `list_import_apply` (action) (proposed). Events: `job.progress`.

#### MM-024 Export and share (MVP; share code v1)

Export formats: `ModsConfig.xml` style list (for the game and RimPy), RimStudio JSON list, plain text report (names, ids, links, optionally Markdown), Workshop id list, and a share code. Copy to clipboard is one key (`Ctrl+Shift+C`: share code in v1, text report in MVP); Save as file uses a native dialog. The export dialog is a side panel with a live preview, an option set (include inactive count, include links, include groups), and the size of the output; Markdown reports warn above 200,000 characters and offer truncation. Third party paste uploads are not provided in the MVP (F-023, F-027).

AC: an exported list imports to an identical list; a share code survives a round trip through the clipboard on all three platforms; exports never contain local paths or user names unless the user ticks "include paths".

### 5.8 Missing things and duplicates

#### MM-025 Missing dependencies flow (MVP)

Triggered by Sort (when the option is on), by the diagnostics popover or by the "Fix dependencies" button when `list.missing-dependency` errors exist. A non-modal panel summarises required mods in three groups: satisfied, available locally (inactive in any source, with source icon), and not installed (with Workshop link). Each row has a checkbox. Actions: Add selected (enables the local ones, in the right place when sorting follows), Add selected and sort, Sort without adding, Open Workshop pages for the missing ones, Ignore (mutes). `Enter` triggers the primary action, `Esc` closes.

AC: a fixture with one unmet dependency shows exactly one row and offers its inactive provider when present; adding is one undo entry; alternatives and replacements are listed as options for the same requirement rather than as separate missing rows.

#### MM-026 Missing mods flow (v1)

Missing mods are ids that the working list or an import references but no source provides. They appear in a "Missing" section above the active list (collapsible), not as ghost rows mixed into the load order, each with the name when known (from imports or the SteamDB slim index), its Workshop id and status. Actions: Open Workshop page, Subscribe (with the Steam helper, MM-044), Search in custom sources, Remove from list. Missing active ids are blocking for Save and Run unless the user chooses Remove from list or Keep and accept the game dropping them (an explicit checkbox that names the DeactivateNotInstalledMods consequence).

AC: a list with 4 missing ids lists them in the section with names where known; Save is blocked until each is resolved or accepted; subscribing and rescanning removes the entry from the section without user action.

#### MM-027 Duplicates resolver (MVP)

The panel never deletes or renames anything, as in [discovery](game-and-mod-discovery.md) GD-051 and GD-052; removing a copy is the separate, explicit Delete action of MM-040. Duplicates are mods that share a package id (case insensitive, `_steam` postfix handled as in the game's rules). The Duplicates panel (opened from the footer counter, a diagnostic, or the palette) groups them by id and shows for each copy: source, path, version, last modified, folder size and whether it is active. The resolver offers: Keep this one (marks others "shadowed" without deleting), Use the workshop copy, Use my copy, and a global policy setting (prefer workshop, prefer custom, ask). The policy decides which copy the working list references; the `_steam` marker is written only when a workshop and a non-workshop copy coexist (core domain, implication 6). Orphan entries (ids in the game list matching no folder) are shown in the same panel.

AC: for each source mix (workshop plus install, workshop plus custom, two custom) a fixture shows the right default and writes the right ids; resolving a duplicate changes the list by one command and touches no file on disk.

### 5.9 Updates

#### MM-028 Update and new badges (v1)

"Updated" marks a mod whose files changed within N days (default 3, off by default), using the file times and, when the dataset or Steam helper provides it, the Workshop update time. "New" marks a mod that was not in the most recent save file's mod list (on by default; the save is read through the streaming reader). The Updates panel lists workshop mods whose Workshop time is newer than the local time (a Steam web check, optional and offline tolerant), with Select all and Open on Steam. When the Steam helper is available, Update subscribed asks Steam to redownload; without it the panel only opens pages. Badges have a settings toggle and a default window.

AC: with the option off no network request is made; a fixture save file marks exactly the mods missing from it as New; a failed network check shows a retry card and never blocks the list.

### 5.10 Launching and saving

#### MM-029 Launching the game (MVP)

Run is `Save and Run` (`Ctrl+Enter`). Launch methods:

| Method | Use | Notes |
|---|---|---|
| Steam URL (`steam://rungameid/294100`) | Default when the install is a Steam install and the Steam client is detected | Reliable on Proton and Flatpak; custom command line arguments are not guaranteed to pass (unverified), so the UI says so next to the arguments field |
| Executable | Default for non-Steam installs; optional elsewhere | Supports arguments (for example save data folder, dev mode flags) and an environment override table; on Linux with Proton the Windows executable is not launched directly |

The launch preset (method, arguments) is per profile with a global default. The flow:

1. Pre-launch checks (MM-030) run as a job with a visible checklist.
2. If all pass, the list is saved if dirty, the launch method runs and a toast shows "Launching" with a Cancel (for the Steam URL case only a hint, since the process is not owned).
3. The manager watches the game process (process name plus Steam running flag; unknown inside sandboxes, D-041) and shows a "Game running" state in the top bar; Save, Sort and Import stay allowed but the write of `ModsConfig.xml` is blocked while the game is running, with the reason in the tooltip.
4. When the game exits, the manager diffs `ModsConfig.xml` against what it wrote and, if different (the game rewrites on version mismatch, new DLC or an in-game change), shows the "The game changed the list" banner with Show diff, Adopt and Keep mine.
5. Optionally (setting), the Player.log is opened in the log viewer on exit if it contains errors.

AC: each launch method works against a fake launcher in tests (`Launcher` port); the running state clears within 5 s of process exit; a launch with a blocking pre-launch failure never starts the game; the arguments field is validated for quoting errors. Commands: `launch_check` (job), `launch_start` (action), `launch_state` (stream) (proposed). Events: `game.state {running, pid?}`, `game.list-changed`.

#### MM-030 Pre-launch checks (MVP)

Checks run in order and are shown as a checklist with Pass, Warn or Block:

1. Game path valid and executable present (Block).
2. Game not running (Block for Save and Run when the write is needed).
3. Every active id resolves to a folder the game can reach: scans the game's three sources and the owned link set; custom mods without a valid link, broken links, offline sources and unresolvable workshop ids block with the DeactivateNotInstalledMods explanation (Block).
4. Link farm integrity: each link in the ownership manifest exists and points to the recorded target; stale ones are offered for repair (Warn, Block if used).
5. `ModsConfig.xml` is writable, `version` matches the installed `Version.txt` major.minor (Block if a mismatch would cause the game to discard the list; the manager stamps the full version string itself).
6. Diagnostics: any `list.*` error is a Warn by default (setting "block launch on errors"), because many players deliberately run lists with warnings.
7. Backup of `ModsConfig.xml` written and verified (Pass, Block on failure).

A failed check offers its fix when one exists (Make visible, Repair links, Remove from list). "Launch anyway" is available for Warn but not Block.

AC: each check has a unit test and a golden; simulated unmounted drive blocks with the source named; a stale link is repaired in one click; the checklist finishes in under 300 ms for 610 active mods on the research machine.

#### MM-031 Saving ModsConfig.xml safely (MVP)

Save writes the working list to the game's `ModsConfig.xml` under the write fence (D-040):

1. The Save preview (a non-modal panel unless disabled) shows the diff against the saved list, the deploy plan (links to create or remove) and the pre-launch checks that apply to saving (3 to 5 above).
2. A timestamped backup of the current file is written and read back.
3. Link changes in the plan are applied through `deploy_apply` (only owned entries).
4. The file is rewritten by byte-span edit of the existing document (preserving unknown elements), ids lower case with the `_steam` postfix only where a workshop copy coexists, `version` set from `Version.txt`, `knownExpansions` kept and extended only for installed DLC, then replaced atomically (temp file then rename).
5. A history snapshot (MM-022) is written; the dirty marker clears.
6. If the game is running, saving is blocked. If the write fails, the backup is untouched, the error card names the file, and the working list stays dirty.

Auto-save is off by default; a setting offers "save on every change" for players who want the game list to follow the manager (a snapshot is still written at most once per minute).

AC: crash injection between each step leaves either the old or the new file, never a partial one; the output file is byte identical to a golden for fixture lists; unknown elements in the input file survive; the backup policy of MM-022 is enforced. Commands: `list_save_preview` (query), `list_save` (job), `list_backups_list`, `list_backups_restore` (proposed). Events: `job.progress`, `game.list-changed`.

### 5.11 Troubleshooting

#### MM-032 Bisect flow (v1)

Bisect finds the mod that causes a problem with the fewest launches.

1. The user opens Bisect from the palette or the Tools menu, chooses what to keep fixed (default: official content, frameworks flagged by the tier rules, and the closure of dependencies of any kept mod), and describes the problem in one line (optional, stored with the session).
2. The manager saves the current list as a snapshot named "Before bisect" and creates a bisect session (stored in user data so it survives a restart).
3. Each round: the manager builds a candidate list that enables half of the remaining suspects plus the fixed set and their dependency closure, sorts it, runs the pre-launch checks, saves it and launches the game. The panel shows the round number, suspects remaining and the maximum rounds left (about 10 for 600 suspects).
4. When the game closes, the user answers "Problem still happens" or "Problem is gone" (or "Skip this one", which marks the half as inconclusive). The suspect set narrows accordingly.
5. At the end the manager names the suspect mod or the minimal pair, offers Disable, Open its page, Find in logs, and Restore the original list. Exit at any time restores the original list in one click; restoring is also offered at next start if a session is found open.

Guards: the original list is never lost (snapshot plus an explicit session record); dependency closures keep the candidate list free of `list.missing-dependency` errors; incompatible pairs are never enabled together; interaction between two mods is handled by the "minimal pair" end state (a second pass tests the culprit with the other half).

AC: a simulated problem with one culprit among 600 is found in at most 10 rounds; a simulated pair interaction is reported as a pair; killing the app mid-session and restarting offers resume or restore; the original list returns byte for byte on restore. Commands: `bisect_start`, `bisect_answer`, `bisect_abort` (actions), `bisect_state` (query) (proposed).

### 5.12 Shell services

#### MM-033 Task centre and toasts (MVP)

Every job (scan, sort preview, import, deploy, dataset fetch, folder sizes, bisect rounds, log parsing) appears in the task centre: a footer summary (running count and the newest task's progress) that expands into a drawer. Each task shows name, progress (determinate when known), elapsed time, Cancel, and a Details view with the log lines and result diagnostics; finished tasks stay for the session with their outcome and a Retry for failures. Jobs use caller-minted ids, at most 20 progress messages per second and cancellation on request or channel drop (D-045). Toasts (bottom right, stacked, at most 3 visible, 5 s default, persistent for errors) report short outcomes and carry one action (Undo, Show, Retry). Modal dialogs are limited to destructive confirmations (permanent delete, reset to vanilla, remove all links, discard unsaved changes on quit).

AC: cancelling a scan stops it within 250 ms and leaves the previous cache intact; no operation in this document blocks input; toasts are announced to screen readers with `role="status"` (errors `role="alert"`); the task list survives a webview reload during a job because jobs live in the Rust registry. Commands: `cancel_job`, `job_list` (query). Events: `job.progress`, `job.finished`.

#### MM-034 Command palette (v1; MVP basic)

`Ctrl+K` opens a palette listing every command by name and id, with fuzzy search, recent items first, keyboard shortcut hints, and parameterised commands (go to mod, switch profile, apply saved filter, add folder). It also searches mods (select and reveal in the right list) and settings. Commands are registered with the shortcut registry (single source for shortcuts, menus, palette and the keyboard map). The Help entries (wiki, issue tracker, shortcut map) are palette commands.

AC: every action in this document appears in the palette with an id; typing "sort" lists the sort commands in under 30 ms; a command that is currently unavailable is shown disabled with the reason.

#### MM-035 Log viewer (v1)

A Logs screen reads the game's `Player.log` (path from detection; other files can be opened or dropped), streaming and classifying blocks without blocking the UI ([rimcrow analysis](../research/rimcrow-analysis.md), implication 10; the game log has no severity token, so classification is by pattern, [mod format and corpus](../research/rimworld-mod-format-and-corpus.md), section 6).

1. Counters: errors, exceptions, warnings, mod issues, info; each is a filter toggle.
2. Grouping: identical or near identical blocks are folded with a count; blocks are attributed to a mod from the bracketed mod prefix and stack frames when possible, shown as a mod link (select it in the library, MM-007).
3. Filters: text, severity, mod, time range; Previous and Next navigation (`F3`, `Shift+F3`) over the filtered set.
4. Live mode follows the file while the game runs; scrolling up pauses following.
5. Row actions: Copy, Copy block, Open mod folder, Disable mod (undoable command), Bisect from this mod, Open in the def explorer when the block names a def (toolkit, later).
6. A 100,000 line log opens without blocking and the first screen is visible in under 300 ms; the list is virtualised.

AC: the log fixtures (clean, with exceptions, with patch errors) produce golden classifications; folding is stable; live mode appends without moving the scroll position when paused. Commands: `logs_open` (job), `logs_page` (query, paged), `logs_follow` (stream) (proposed). Events: `log.append`.

#### MM-036 Screen states (MVP)

Every screen has designed empty, loading, error and offline states; the catalogue is in [section 7](#7-screen-states). Acceptance: each state exists in the component gallery and in the screenshot suite in both themes.

#### MM-037 Keyboard shortcuts (MVP)

Defaults are in [section 6](#6-keyboard-shortcuts). They are rebindable in Settings, Shortcuts (single registry, conflict detection on assignment, reset per binding and all), exported as part of settings JSONC. Chords are limited to two steps. Shortcuts never fire while a text field is focused unless the shortcut carries `Ctrl` or `Cmd`.

#### MM-038 Accessibility (MVP)

Requirements are in [section 8](#8-accessibility).

#### MM-042 Performance budget (MVP)

The targets in [section 11](#11-performance-targets) are requirements: each row has a benchmark or lab check, runs in CI as a warning first and fails on a 2x regression. Lists are always virtualised, and the main thread never does work proportional to library size outside rendering visible rows.

### 5.13 Rules, datasets and migration

#### MM-039 Datasets and rules status (v1; R6 at the UX level)

The manager uses the community sorting rules and the related datasets (SteamDB, Use This Instead, version lists) without per-dataset setup: the app fetches them at runtime, conditionally, with the last good copy kept ([rules fetch and merge design](../research/rules-fetch-and-merge-design.md)). The datasets are unlicensed community data and are never bundled (R11).

1. A status chip in the top bar (Fresh, Updating, Stale, Offline, Error) opens the Data sources panel in Settings: one row per dataset with purpose, active source, status chip, entries, version, last checked, last changed, and the row actions Refresh now, Changelog (added, removed, changed counts), Change source, Enable, Revert to previous, Open cache folder, plus an attribution line stating each dataset's source and licence status.
2. Policies: offline mode, auto refresh (on start, daily, manual), apply or notify only, skip large downloads on metered connections, proxy and TLS roots. Defaults: on start, apply.
3. Errors are plain sentences with the next action ("Rejected: entries dropped from 631 to 12; kept the last good copy. Inspect.").
4. Use This Instead suggestions appear as `list.replacement-available` info diagnostics and in a Replacements panel (F-142) grouped by replaced mod, with Replace (enable the new one, disable the old, one undoable command) and Ignore.
5. Rule priority is user-orderable in the sort options (user, About force, About soft, community, derived); a user rule can add or suppress a community rule without editing shared data.

AC: with no network the app starts, shows cached or no data and the chip reads Offline without error toasts; a corrupted download is quarantined and the last good copy stays in use; refreshing never blocks the UI and a large dataset arrives as a job with progress; the replacement panel applies a replacement in one undo entry. Commands: `datasets_status` (query), `datasets_refresh` (job), `datasets_set_source`, `datasets_revert` (actions). Events: `datasets.status`.

#### MM-043 Migration from RimSort (v1; R6 import)

An Import from RimSort wizard (Settings, Migration) finds a RimSort settings folder, shows what it can bring (user rules, ignore list, mod lists and history snapshots, instance paths as detected-path hints) and imports into RimStudio's own JSON stores with a preview. User rules are read losslessly (unknown keys kept) and become the user rule layer. Notes, tags and colours stored in RimSort's SQLite database need the read-only SQLite dependency that the owner accepted (D-031, D-083, feature `aux-db` in `rimstudio-datasets`, off by default until that crate exists); while the feature is off the wizard shows that item as unavailable with a one-line explanation. Nothing in the RimSort folder is modified.

AC: a RimSort fixture folder imports its user rules and ignore list into JSON with a preview; re-running the import is idempotent; the wizard works with the aux-db feature compiled out.

#### MM-045 Rule editor and ignore list (v1)

A table editor for user rules: per mod, rows for load after, load before, incompatible with, load first and load last, each with provenance and a switch to show community and About rules read-only for context. Changes write to the user rules layer (`userdata/rules/user-rules.json`, lossless JSON in the community format with unknown keys and order kept; see [community datasets](community-datasets.md) CD-013 and the data and persistence document) and are undoable inside the editor session; "Export for community" produces a patch in the community format (concept only; the manager does not upload). The ignore list editor shows muted diagnostics with mod, code and date and a Remove action.

AC: adding a rule re-sorts the preview and changes diagnostics of the affected rows only; the file keeps unknown keys and key order after an edit (JSON carries no comments); a rule that creates a cycle is rejected with the cycle shown.

### 5.14 Mod actions, watching and settings

#### MM-040 Mod actions and context menu (MVP)

Right click, `Shift+F10` or the overflow button opens the menu; the same actions are palette commands and, for the selection, work in bulk where meaningful.

| Group | Actions |
|---|---|
| List | Enable or disable, Move to top or bottom, Pin, Move to group |
| Annotate | Tags, colour, notes |
| Open | Open folder, open in the default editor, open About file, open Steam page (via URL or the Steam client), open homepage, copy package id, copy Workshop id, copy path |
| Rules | Edit rules (v1), mute diagnostics, Why here (MM-018) |
| Remove | Remove from list; Delete mod (moves the folder to the OS trash with undo toast; refused for workshop mods because Steam owns them, replaced by Unsubscribe when the Steam helper is present, or a link to the Steam page); Delete and keep optimised textures is not offered |
| Toolkit | Open in project workspace (later, toolkit) |

Deletions never apply to official content and never to a path outside a known source. Trash failures (for example network drives without a trash) fall back to a confirmation for permanent deletion with the path shown.

AC: delete of a custom mod goes to the trash and an Undo toast restores it; the delete action is unavailable on official mods; every open action works with paths containing spaces and non-ASCII characters on all three platforms.

#### MM-041 Watching and live refresh (MVP)

The manager watches source roots, each mod's About folder timestamps through a debounced stat walk, and `ModsConfig.xml` (D-028). Changes arrive as deltas; the list never reorders by itself. A polling fallback every few seconds applies when inotify limits are hit. Steam downloads that add or remove workshop folders appear as new or removed rows within 5 s while the window is focused and are quiet when it is not.

AC: touching one About file produces one upsert; unmounting a source marks its mods Offline within the poll interval; the watcher uses no more than 1 percent CPU idle on the 743 folder library (target).

#### MM-044 Adding mods (v1)

Add mod opens a menu: From zip file (choose or drop, extract with progress into a chosen custom source, handle existing folder with Replace, Keep both or Cancel), From Workshop id or link (opens the Steam page, or subscribes through the Steam helper when available), and Browse Workshop (opens the Steam Workshop in the system browser; an embedded browser is not provided). Git and GitHub sources are an extension point only (later). All installs go to a custom source the user picks, never into the workshop folder.

AC: a zip with a nested folder extracts to one mod folder; a path traversal entry in a zip is rejected with an explanation; Subscribe is hidden when the Steam helper is absent and replaced by Open page.

#### MM-046 Manager settings and settings search (MVP; some v1)

Settings has a search box (matching labels and descriptions) and sections: Sources (MM-001, MM-002), Library (density, row options, badge windows, save and sort behaviour), Sorting (algorithm default, rule priority, dependencies imply order), Launch (method, arguments, defaults), Data sources (MM-039), Appearance (theme light, dark, system, user JSONC themes), Shortcuts (MM-037), Performance (scan workers up to 8 by default, watch policy), Privacy (no telemetry; network requests list), About and diagnostics. Window geometry and column widths are remembered automatically; the shell prevents a second instance and focuses the first. Every setting shows its default and a reset button; secrets live in the OS credential store.

AC: typing a word in settings search shows matching items across sections within 30 ms; every setting is stored in `settings.jsonc` with comments preserved on edit; changing a setting applies without restart unless marked.

#### MM-047 Language packs and translation status (later)

Per-row translation icon and "add language mods" action for the selected game language, driven by the SteamDB slim index. Not scheduled; listed to keep the diagnostics code `list.missing-language-pack` reserved.

## 6. Keyboard shortcuts

Defaults use `Ctrl`; on macOS `Cmd` replaces `Ctrl` except where stated. All are rebindable (MM-037).

| Shortcut | Context | Action |
|---|---|---|
| `Ctrl+K` | Global | Command palette |
| `Ctrl+,` | Global | Settings |
| `Ctrl+S` | Global | Save (preview) |
| `Ctrl+Enter` | Global | Save and Run |
| `Ctrl+Z`, `Ctrl+Shift+Z`, `Ctrl+Y` | Library | Undo, redo, redo |
| `Ctrl+Shift+T` | Library | Sort preview |
| `F5` | Library | Refresh |
| `Ctrl+O` | Library | Import |
| `Ctrl+Shift+E` | Library | Export panel |
| `Ctrl+Shift+C` | Library | Copy share code or text report |
| `Ctrl+P` | Library | Profile switcher |
| `Ctrl+B` | Library | Toggle detail column |
| `Ctrl+F`, `Ctrl+Shift+F` | Library | Search focused list, search both |
| `Alt+H` | Library | Hide versus highlight non-matching |
| `Ctrl+A` | List | Select all visible |
| `Esc` | List, drag, panels | Clear selection, cancel drag, close panel |
| `Up`, `Down`, `Home`, `End`, `PageUp`, `PageDown` | List | Move focus |
| `Shift+Up`, `Shift+Down` | List | Extend selection |
| `Space`, `Return` | List | Move selection to the other list |
| `Left`, `Right` | List | Focus the other list |
| `Alt+Up`, `Alt+Down` | Active list | Move selection one position |
| `Alt+Shift+Up`, `Alt+Shift+Down` | Active list | Move to the next group boundary |
| `Alt+Home`, `Alt+End` | Active list | Move to top or bottom |
| `Delete` | List | Remove menu |
| `Shift+F10`, `Menu` | List | Context menu |
| `F6`, `Shift+F6` | Global | Cycle focus between panes |
| `F3`, `Shift+F3` | Logs | Next, previous match or block |
| `Ctrl+G` | Active list | Group selected |
| `Ctrl+1`, `Ctrl+2`, `Ctrl+3` | Global | Library, Logs, Settings |
| `?` | Global | Shortcut map |

## 7. Screen states

| Screen | Empty | Loading | Error | Offline |
|---|---|---|---|---|
| Wizard | Not applicable | Detection skeleton cards | Detection failed card with Retry and Choose folder | Not applicable: works offline |
| Library, no game found | Illustration, "RimWorld not found", Choose install, Run detection | Skeleton rows | Path invalid card with Open settings | Same |
| Library, no mods | "No mods found in the sources", Add folder, open Workshop | Cached rows then progress | Scan failed with counts per source and Retry | Offline sources marked on their rows |
| Inactive list, filter without result | "No mods match", the query, Clear filter | Rows appear progressively | Query error inline | Same |
| Active list empty | "Only official content is active", Import list, drag hint | Skeleton rows | `ModsConfig.xml` unreadable card with backups list and Restore | Same |
| Detail panel | Library summary | Skeleton, description loads last | "Could not read this mod's details" with path and Copy diagnostics | Preview image placeholder |
| Sort preview | "Already sorted" with a tick | Progress bar with Cancel | Cycle or conflict report (a result, not an error) | Not affected |
| Profiles, History | "No profiles yet", Create from current | List skeleton | "History file unreadable", the file is kept, Open folder | Same |
| Import | Drop zone with accepted formats | Detect and preview progress | Unreadable file card with reason and encoding tried | Collection links need network: explained inline |
| Data sources | "Datasets not fetched yet", Fetch now | Per-row progress | Per-row plain error with last good copy shown | Offline banner, cached copy and age shown |
| Logs | "Game log not found", expected path, Choose file | Streaming progress, first page early | Parse error with raw view fallback | Not affected |
| Updates panel | "All workshop mods are current" | Checking | Retry card | "Needs network", last checked time |
| Bisect | "Start a session", explanation | Round setup progress | Launch failed with check list | Not affected |

Error cards always show a short cause, a primary fix button, Copy diagnostics (redacted, with an error id), and never a raw stack trace. Offline states never use error styling when the feature has a cached copy.

## 8. Accessibility

1. All functionality is reachable by keyboard with a visible focus ring (at least 3:1 contrast against both adjacent colours); there are no keyboard traps; panels return focus to their opener on close.
2. Lists expose `listbox` or `grid` semantics with `aria-activedescendant`, `aria-setsize` and `aria-posinset` from the virtualiser so the full count is announced despite windowing; a live region announces moves ("Moved 3 mods to position 12").
3. Text contrast at least 4.5:1 and non-text at least 3:1 in light and dark and in user themes (checked in the gallery by an automated contrast test); colour never carries meaning alone.
4. Respect `prefers-reduced-motion` (no drag ghost animation, no list transitions), `prefers-contrast` and the operating system text scale; layout holds at 200 percent zoom at 1280 x 800 by falling back to the drawer layout.
5. Drag and drop always has the keyboard equivalent of MM-013; touch targets in the rail are at least 32 px.
6. Dialogs are `role="dialog"` with labelled titles and focus trapping; toasts use polite live regions, errors assertive.
7. Every icon button has an accessible name, every badge a text alternative, tooltips are reachable by focus.
8. Language: all strings come from catalogues (no hard coded text), with plural and gender forms through the ICU layer; right to left layout is not a v1 goal but logical CSS properties are used.

AC: axe checks run in the Playwright suite on each screen state; a manual screen reader pass (NVDA, VoiceOver, Orca) is recorded per release.

## 9. User flows and step counts

Counts are user actions (clicks and key presses) excluding waiting, compared with the RimSort figures inferred in [the feature inventory, section 2.9](../research/rimsort-feature-and-ux-inventory.md).

| # | Flow | RimSort | RimStudio | Steps in RimStudio |
|---|---|---|---|---|
| 1 | First run | 9 to 14 | 1 to 3 | 1 Wizard appears with detection done; 2 Confirm or fix a card (only if needed); 3 Open library |
| 2 | Enable a mod | 2, plus Save | 2, Save later | 1 Select (or `Ctrl+F` and type); 2 `Space`, double click or drag; `Ctrl+S` when ready |
| 3 | Sort | 1 to 3, no preview | 2 | 1 Sort; 2 Apply (the diff is already visible); or 1 with "Sort and apply" |
| 4 | Fix a warning | 3 to 6 | 2 | 1 Click the row's diagnostics icon; 2 Choose the fix (Enable dependency, Move after, Mute) |
| 5 | Save and launch | 2, plus a 3 way dialog if dirty | 1 | 1 `Ctrl+Enter`; checks run, the list saves, the game starts; a dialog appears only for a blocking problem |
| 6 | Share a list | 3 to 4 | 1 to 2 | 1 `Ctrl+Shift+C` copies a share code; or 1 Export, 2 choose format |
| 7 | Add a workshop mod | 8 to 12 | 4 to 6 | 1 Add mod, 2 Workshop link (opens Steam), 3 Subscribe in Steam, 4 Row appears (watcher), 5 `Space`; with the helper, subscribe is inside the app |
| 8 | Update mods | 4 to 7 | 2 to 3 | 1 Open Updates from the counter; 2 Select all; 3 Open or Update |
| 9 | Remove a mod | 4 | 2 | 1 Select; 2 `Delete`, Remove or Delete (trash, Undo toast, no confirmation dialog) |
| 10 | Troubleshoot a crash | 5 to 10 | 3 to 4 | 1 Open Logs (log loads automatically); 2 Errors are grouped by mod; 3 Click the mod link; 4 Disable (or Bisect) |
| 11 | Undo a bad sort | not possible | 1 | `Ctrl+Z` |
| 12 | Add own folder on a second drive | up to 6 | 3 | 1 Settings, Sources, Add folder; 2 Pick folder; 3 Enable and Save (link created) |
| 13 | Compare today with last good list | not available | 2 | 1 History; 2 Select snapshot (diff against live is shown) |

Numbered procedure for flow 12 (the owner's own mods on an external drive), the central R4 case:

1. Open Settings, Sources and choose Add folder (or drop the folder on the window).
2. The folder is scanned; its mods appear in Inactive with the "not visible to the game" badge.
3. Enable a mod. The row shows the badge "needs link"; the diagnostics popover offers Make visible, which only plans the link.
4. Press `Ctrl+S`. The Save preview shows the link to create and the list diff; Confirm.
5. The link is created, the ownership manifest updated, the backup written, `ModsConfig.xml` replaced; the badge turns to "visible".

## 10. Backend commands and events

Names that exist in the registry of [IPC and state](../architecture/ipc-and-state.md) are used as written (mods_snapshot, mods_subscribe, mods_get_detail, profiles_load); the rest are proposed; the registry kinds are query (Q), action (A), stream (S) and job (J). Research used the aliases `list_mods_snapshot`, `subscribe_mods` and `get_mod_detail` for the first three entries of the table ([webview and IPC performance](../research/webview-and-ipc-performance.md), implication 2).

| Command | Kind | Requirements | Notes |
|---|---|---|---|
| `detect_run`, `detect_set_override` | Q, A | MM-001 | DetectionReport and overrides |
| `sources_list`, `sources_add_folder`, `sources_update`, `sources_remove` | Q, A | MM-002 | |
| `library_scan` | J | MM-002, MM-003 | |
| `mods_snapshot` | Q | MM-003 | List columns only, `{rev, rows, order}` |
| `mods_subscribe` | S | MM-003, MM-041 | `{rev, upserts, removes, order}`; a gap triggers a new snapshot |
| `mods_get_detail`, `library_folder_size` | Q, J | MM-007 | Description and size on demand |
| `library_query` | Q | MM-009, MM-010 | Returns `ModIdx` array and ranges |
| `list_toggle`, `list_set_active`, `list_move`, `list_clear`, `list_undo`, `list_redo` | A | MM-012 to MM-014 | Return `{rev, delta}`; sort results are `ModIdx` arrays |
| `list_diff`, `list_reload_game_list` | Q, A | MM-015 | |
| `diagnostics_list`, `diagnostics_mute` | Q, A | MM-016 | Diagnostics also arrive as `library.diagnostics` deltas |
| `sort_preview`, `sort_apply`, `sort_explain` | Q (promotable to a job above 1 ms, see issue 3), A, Q (proposed) | MM-017, MM-018 | |
| `groups_*`, `usermeta_*` | A | MM-019, MM-020 | User data writes |
| `profiles_*`, `history_*` | Q, A | MM-021, MM-022 | |
| `list_import_detect`, `list_import_preview`, `list_import_apply`, `list_export_render` | Q, J, A, Q | MM-023, MM-024 | |
| `duplicates_list`, `duplicates_resolve` | Q, A | MM-027 | |
| `launch_check`, `launch_start`, `launch_state` | J, A, S | MM-029, MM-030 | Uses the Launcher and ProcessProbe ports |
| `list_save_preview`, `list_save`, `list_backups_list`, `list_backups_restore` | Q, J, Q, A | MM-031 | Write fence and byte-span edit |
| `deploy_plan`, `deploy_apply`, `deploy_unlink_all` | Q, J, A | MM-002, MM-030, MM-031 | library::deploy |
| `bisect_*` | A, Q | MM-032 | |
| `datasets_status`, `datasets_refresh`, `datasets_set_source`, `datasets_revert` | Q, J, A | MM-039 | |
| `logs_open`, `logs_page`, `logs_follow` | J, Q, S | MM-035 | Paged only for logs |
| `cancel_job` | A | MM-033 | Shared with the CLI |

| Event | Payload | Emitted for |
|---|---|---|
| `library.delta` | `{rev, upserts, removes, order}` | Scan, watcher, edits |
| `library.diagnostics` | `{rev, byMod}` | Changed rows only |
| `job.progress`, `job.finished` | `{jobId, done, total, message}` | All jobs, at most 20 per second |
| `game.state` | `{running, pid?}` | Launch and exit |
| `game.list-changed` | `{diffSummary}` | External or post-exit `ModsConfig.xml` change |
| `datasets.status` | Per dataset status | Fetch and merge |
| `log.append` | Chunks of 250 to 1000 records | Live log |

Rules for all of them: payloads are JSON (R10), 64-bit ids are strings, messages are below 8 KiB or rare and large, no per-row events, preview images only through the `rsimg` scheme.

## 11. Performance targets

Targets for release candidates on a mid range machine; the research machine values are the measured evidence. Numbers marked (target) are not yet measured and are enforced as warning-only benchmarks first.

| Measure | Target | Evidence |
|---|---|---|
| Cold start to painted shell | under 1.5 s | webview budget 1 |
| Cached list visible after shell | under 100 ms for 3000 rows (snapshot 8 ms transport, 4 ms parse in the lab) | webview budget 2 |
| Level-0 scan, empty cache, 690 mods | under 100 ms on SSD (measured 3.7 ms) | scan spike |
| Warm start with manifest, verified index fully usable | under 150 ms (measured 46 to 100 ms); a nothing-changed rescan alone about 25 ms | scan spike |
| One changed mod rescan | under 50 ms | scan spike |
| Full def index warm (toolkit, background) | under 300 ms (measured 122 ms) | scan spike |
| Input to paint for select, toggle, drag step | under 50 ms with 3000 rows; frames under 16 ms | webview budget 4 |
| Search result per keystroke, 5000 mods | under 50 ms (target) | MM-009 |
| Diagnostics recalculation, 610 mods, 1500 constraints | under 1 ms (target) | core domain |
| Sort of 610 mods | under 10 ms of Rust work (replica 3 ms) | core domain |
| Sort preview shown | under 100 ms | MM-017 |
| Import preview, 610 mods | under 200 ms (target) | MM-023 |
| Save (backup, write, snapshot) | under 300 ms (target) | MM-031 |
| Pre-launch checks, 610 active | under 300 ms (target) | MM-030 |
| DOM nodes | at most 3000 per list, 10,000 per page | webview budget 5 |
| Memory | webview under 400 MB with 3000 mods; app under 600 MB | webview budget 7 |
| Initial JavaScript | under 300 KB gzip; Logs, graph and charts are lazy chunks | webview budget 8 |
| List payload | snapshot under 2 MB; no descriptions | webview budget 6 |
| Idle CPU with watcher | under 1 percent (target) | MM-041 |

## 12. Test plan

### 12.1 Fixtures

The owner's library (743 mod folders: 690 workshop, 47 local, 6 official; 610 active mods) is private and carries unlicensed community data, so it is never committed (R11). Three fixture levels are used.

| Level | Content | Where it runs |
|---|---|---|
| Synthetic small | 20 to 40 mods, fictional ids, each rule and failure case once (missing dependency, mutual incompatibility, hard and soft order violation, version mismatch, duplicate id in each source mix, custom source without link, offline source, cycle, 979 entry `loadAfter`, differently spelled references, lowercase `about.xml`, BOM, missing About) | CI, every commit |
| Synthetic large | A generator that builds a 610 active and 743 folder library with the measured shape: tier sizes about 8, 15, 560, 35, about 3,200 About ordering pairs, one 2-cycle, 7 hard violations in game-style rules | CI benchmarks, fixtures built by `xtask fixtures` |
| Real library | The owner's own library through an environment variable; tests are `#[ignore]` | Owner machine, release gate |

Recorded expectations on the real library (from the research replicas, asserted only under the environment variable): 610 active ids all resolved, tier sort with 0 violations, game-style rules with 1,426 pairs and about 7 violations on the current order, one unmet dependency, no active incompatibility, one About-only 2-cycle reported, sort of 610 mods in a few milliseconds.

### 12.2 Test layers

| Layer | What | Tooling |
|---|---|---|
| Unit and property | Command stack (random sequences and undo), query parser, diagnostics, sort determinism at 1 and 8 threads, importers, share code checksum, backup retention with an injected clock, ModsConfig writer | nextest, proptest, rstest |
| Golden | Diagnostics JSON per fixture (codes, severities, mod ids), sort results, preview diffs, ModsConfig output bytes, import and export per format, log classifications | goldenfile with JSON goldens |
| Fault injection | Crash between save steps, unmounted source, failing trash, no link support, corrupted dataset download, truncated file | `RecordingFs`, fake ports in rimstudio-testing |
| Contract | Command and event shapes, snapshot plus delta with rev gaps, jobs cancel on channel drop | Bindings drift check, in-process registry |
| Frontend | Store actions, windowing hook (rendered rows never above visible plus overscan), search box, shortcut registry, drag facade | Vitest |
| End to end | Flows 1 to 13 of section 9 against mocked IPC; screenshots of every state at 1280 x 800, 1440 x 900 and 1920 x 1080 in both themes; axe checks | Playwright with mockIPC |
| Smoke on real shell | Launch, detect, open library, enable, save, launch with a fake game executable on each OS | WebdriverIO nightly |
| Benchmarks | Section 11 rows against `xtask/budgets.jsonc`, 2x regression fails | criterion, lab script |
| Manual | Link farm with the real game per OS (spike S-03), drag spike S-05, screen reader pass, WebView2 and WKWebView numbers (spike S-10) | Release checklist |

### 12.3 Acceptance traceability

Each AC line above is tagged in the test name with its requirement id (for example `mm_017_sort_preview_is_deterministic`). A requirement is done when its AC tests pass on the three platforms (Linux first in CI; Windows and macOS legs from M1) and, for MVP rows, the golden diagnostics are reviewed against the fixtures by the owner.

## 13. Owner decisions and open points

| # | Item | Assumption made here |
|---|---|---|
| 1 | Link method and copy fallback for custom folders is gated by the in-game test per OS (S-03); until then the Make visible action is labelled experimental | Junction on Windows, symlink elsewhere, copy as fallback |
| 2 | Read-only SQLite import of RimSort notes and colours: accepted (D-031, D-083) | The migration wizard works without the feature; item shown as unavailable |
| 3 | Steam helper (D-062, D-086: the user's installed Steam, nothing shipped): subscribe, unsubscribe and update-through-Steam depend on it and on spike S-04 | The manager works without it and opens Steam pages instead |
| 4 | Default of the "Updated" badge window and the "New" badge (the inventory leaves the right default open) | "New" on, "Updated" off, three days |
| 5 | Whether Steam URL launch can pass arguments is unverified | Arguments are documented as executable launch only |
| 6 | Whether blocking `list.*` errors should block launch by default | Warn by default, setting to block |
| 7 | The IPC and state document is not yet written; command names here are proposals | Reconcile before M2 starts |
| 8 | Rentry and other paste services are dropped from MVP; the owner may want them later | Share code and text report cover sharing |
| 9 | Windows and macOS timings are missing for scan and IPC budgets | Targets stay warning-only until measured |
| 10 | Whether to offer SteamCMD downloads (F-220) was dropped | Revisit on owner request |

Related documents: [mod format and corpus](../research/rimworld-mod-format-and-corpus.md), [steam and game detection](../research/steam-and-game-detection.md), [rimsort core domain](../research/rimsort-core-domain.md), [feature and UX inventory](../research/rimsort-feature-and-ux-inventory.md), [rimcrow analysis](../research/rimcrow-analysis.md), [rules fetch and merge design](../research/rules-fetch-and-merge-design.md), [scan performance spike](../research/scan-performance-spike.md), [webview and IPC performance](../research/webview-and-ipc-performance.md), [architecture overview](../architecture/overview.md), [decision register](../architecture/decision-register.md).

## Open spec issues

Found by the cross-spec review of 2026-10-04. Each item lists the proposed resolution; repairs already made are noted as fixed.

| # | Issue | Proposed resolution |
|---|---|---|
| 1 | Command names in this spec differed from the registry in [IPC and state](../architecture/ipc-and-state.md) (library_snapshot, setup_detect, list_enable, job_cancel and others). Fixed for names the registry has. Rows still proposed: `diagnostics_list`, `diagnostics_mute`, `sort_explain`, `list_diff`, `list_reload_game_list`, `list_clear`, `library_query`, `library_folder_size`, `groups_*`, `usermeta_*`, `history_*`, `import_*`, `list_export_render`, `duplicates_*`, `launch_check`, `launch_state`, `save_*`, `backups_*`, `deploy_unlink_all`, `bisect_*`, `datasets_set_source`, `datasets_revert`, `logs_*`; also `deploy_audit`, `settings_reset`, `settings_backups_list`, `settings_backup_restore`, `settings_import_rimsort_scan`, `settings_import_rimsort_apply`, `diagnostics_export`, `settings_storage_summary` from settings and discovery | Add these rows to the registry table when their milestone starts; the registry macro then generates the shell wrappers and CLI routes. `job_list` was dropped here because the registry has none |
| 2 | Milestone names: feature specs use the decision register names (M1 to M6); the roadmap uses M0 to M7. Mapping is in section 3 and roadmap section 1.1 | Keep register names in specs; add a footer line to each spec pointing at the mapping |
| 3 | `sort_preview` is a query in the registry but the convention says Rust work over 1 ms is a job; 610 mods sort in about 3 ms | Resolved by D-073: a query that must be promotable to a job; see the [command catalog](../architecture/command-catalog.md) |
| 4 | Link method default on macOS: discovery GD-061 names symlink, while the packaging research (section on macOS Mods inside the app bundle, App Management protection) calls link farms in the bundle fragile, and copy also writes into the bundle | Until spike S-03 records macOS, `auto` resolves to copy (GD-069); decide the macOS default after S-03 and test the App Management prompt |
| 5 | `launch.jsonc` is a fourth config file beside the three of D-030 and the data and persistence document | Resolved by D-077: kept as a fourth tiny config file, recorded in data-and-persistence section 3.1 (needed before the webview starts for safe graphics) |
| 6 | Rule layer ranking: the research note on rules merge puts about-force below user rules and tie-breaks by current order; the specs rank hard layers first with an order-independent tie-break | Update the research note (rules-fetch-and-merge-design section 7.2) to the spec behaviour; the spec is testable for shuffle invariance |
| 7 | Dataset policy location and suppression sidecar name differ between the research (`rule-overrides.jsonc`, one `datasets` block) and the architecture (`userdata/rules/suppressions.json`, `datasets.items.<id>` in `workspace.jsonc`) | Specs follow the architecture; correct the research note |
| 8 | Steam URL launch cannot pass custom arguments (unverified); discovery GD-067 now matches MM-029 and ST-050 (auto is the Steam URL for a detected Steam install) | Verify argument passing on Windows, Linux and macOS during M2; keep the warning next to the arguments field |
| 9 | MM-043 and CD-017 depend on the `aux-db` feature accepted by D-031 and D-083 | Both degrade without it; keep as written |
| 10 | The MM-016 table lists 11 diagnostic codes; the catalogue LO-019 adds `list.core-inactive`, `list.unresolved-id`, `deploy.modsconfig-version` and preview-only `sort.*` | Treat LO-019 as authoritative and shorten MM-016 to a pointer |
| 11 | The `density` enum (standard, compact, touch) is inferred, not confirmed by the UI design | Confirm in the design prompt stage (R8) |
| 12 | Requirement coverage: R7 is specified outside this folder (item specs, reviewed separately) and R8 is a later deliverable | Re-run the coverage check when the item specs and the design prompt exist |
