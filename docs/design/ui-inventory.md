# RimStudio UI inventory

Scope: the screen, state and component inventory extracted from the feature specifications, the architecture documents and the research notes. It feeds two consumers: the written design prompt for the visual design tool (requirement R8) and the frontend implementation (Preact, Tailwind, the component gallery). It lists what must exist and in which states; it does not fix pixel values beyond those the specifications already state. Every item is traced to requirement ids (MM-, GD-, ST-, LO-, CD-, WS-, IT-, WP-).

Status: draft | Last updated: 2026-10-04

## Contents

1. [Shell anatomy](#1-shell-anatomy)
2. [Screen inventory](#2-screen-inventory)
3. [Component inventory](#3-component-inventory)
4. [Interaction catalogue](#4-interaction-catalogue)
5. [Data realism guide](#5-data-realism-guide)
6. [Microcopy guide](#6-microcopy-guide)
7. [Accessibility and performance constraints for designers](#7-accessibility-and-performance-constraints-for-designers)
8. [Traceability table](#8-traceability-table)
9. [Open questions for the design stage](#9-open-questions-for-the-design-stage)

## 1. Shell anatomy

The shell is static HTML plus tokens and paints before any data arrives (budget: visible shell under 1.5 s; initial JavaScript under 300 KB gzip; frontend architecture 7.1, MM-003). Every region below is a component slot, so the visual design can change without touching features. Sources: MM-004, MM-005, MM-033, MM-034, MM-038, ST-051, GD-080, frontend architecture sections 7, 9, 12.

### 1.1 Regions

| Id | Region | Content | Behaviour and states |
|---|---|---|---|
| SH-01 | Window frame | Custom title bar where the platform adapter allows it, otherwise the OS bar; window controls; product name RimStudio in Barlow Condensed | Frameless in Steam Deck game mode is not assumed: native dialogs and `--fullscreen` must work with no custom title bar (cross-platform 8, item 4). A thin blueprint corner-tick frame is allowed on panels, never on the window edge |
| SH-02 | Tool rail (left) | One icon plus label per tool: Mod manager, Workspace (def tools), Project, Designer, Workshop, Logs, Settings; generated from the tool registry (WS 3.1) | Icon mode 56 px, labelled mode about 200 px. A tool whose capability is missing (`game-install`, `open-project`, `ce-installed`, `steam-helper`, `log-file`) is shown disabled with the reason as tooltip and a link to the fix, never hidden (WS 3.2). Active tool has an accent bar; pages stay mounted so scroll and selection survive switching |
| SH-03 | Top bar | Installation picker (active RimWorld install, with game version label showing the full version string and revision), profile switcher with Modified marker and diff button (MM-015, MM-021), search and command palette entry (`Ctrl+K`), Steam chip, datasets chip (Fresh, Updating, Stale, Offline, Error; MM-039), Game running chip, Launch (Save and Run, `Ctrl+Enter`) | Launch is the single primary button of the bar. Launch disabled states explain themselves: game running, blocking diagnostic, no install. Pickers are popovers, not modals |
| SH-04 | Tab strip | Per-tool document tabs (several projects, several Def Explorer queries, patch tester beside the explorer) with dirty marker | Only for tools that need tabs; closable; dirty state per tab (WS 6.4) |
| SH-05 | Workspace area | The active tool's page. Manager: four columns. Toolkit tools: left list or tree, main pane, right detail pane, bottom diagnostics strip (WS 6.4) | Fills remaining space; no horizontal page scroll at 1280 wide |
| SH-06 | Status strip (bottom) | Game version and detection state, active mod count, scan state ("refreshing" thin indicator), diagnostics totals (errors, warnings), duplicates counter (opens SCR-09), dirty indicator with undo depth, dataset freshness, task centre summary (running count and the newest task's progress) | Fixed height, one line. Below 700 px window height it folds into the top bar (MM-005). Every counter is a button |
| SH-07 | Task centre drawer | Bottom or right drawer: each job with name, phase, determinate or indeterminate bar, elapsed time, Cancel, Details; finished tasks stay for the session with outcome and Retry for failures (MM-033) | Opened from the status strip; never blocks input; survives a webview reload because jobs live in the Rust registry |
| SH-08 | Toasts | Bottom right stack with one action each (Undo, Show, Retry) | At most 3 visible (MM-033; frontend architecture says 4, see open questions); default 5 s, warnings 8 s, errors persist; `role="status"`, errors `role="alert"` |
| SH-09 | Command palette | Centered overlay, fuzzy search over commands, mods, settings entries; recent first; shortcut hints; parameterised commands (go to mod, switch profile, apply saved filter, add folder); unavailable commands shown disabled with the reason (MM-034) | Zag combobox. Typing "sort" lists the sort commands in under 30 ms. Result groups labelled |
| SH-10 | Dialog layer | Native `<dialog>` wrapped as Dialog | See the modal policy below |
| SH-11 | Banner slot | Under the top bar: info, warning, danger banners with one action each (for example "The game list changed outside RimStudio", "Settings will not be saved") | One banner at a time per severity; stack collapses to a counter after three |

### 1.2 Modal policy

Modal dialogs are limited to destructive confirmations: permanent delete when the trash is unavailable, reset to vanilla, remove all links, discard unsaved changes on quit, delete a profile, reset all settings (typed confirmation), restore a backup, "Launch anyway" after a block (typed confirmation, GD-066), and the publish confirmation (WP 4.1 step 6, one explicit click, no countdown). Everything else is a non-modal panel over the relevant column or a drawer: sort preview, save preview, missing dependencies, duplicates, import and export, diagnostics popover, quiz stepper (a stepper over the form, not a gate; IT 5.5). Progress is inline or in the task centre, never a blocking window (RimSort pain points P4, P5, P6). Dialogs trap focus and return it to the opener (MM-038).

### 1.3 Minimum window and density modes

The minimum supported window is 1280 x 800 at 100 percent scale (MM-005). Density is a root attribute (`data-density`) with three modes plus one derived mode.

| Mode | Row height | Intended use | Notes |
|---|---|---|---|
| Compact | 28 px | 5,000-row libraries on large monitors | Tags hidden, version chip only on duplicates |
| Comfortable | 34 px | Default from 1920 wide; the owner's preferred row height | Tags shown when width allows |
| Roomy | 40 px | Touch and accessibility | Primary controls at least 40 px; the Steam Deck "compact touch" acceptance asks for 40 px primary controls and nothing below 24 px in any density (cross-platform 8, item 2) |

Row height is within 1 px of the setting and never changes with content (MM-006). Rail buttons are at least 32 px (MM-038).

### 1.4 Responsive behaviour

| Width | Layout | Design notes |
|---|---|---|
| 1280 to 1439 (the floor) | Detail 280 px, rail icons 56 px, two lists share the remaining 944 px (about 472 px each). Row icons and diagnostics sit in fixed right-aligned slots; name truncates with CSS ellipsis | Design this width first. No clipped controls, no overlapping badges, no horizontal scroll; screenshots at 1280 x 800 in both themes |
| 1440 to 1919 | Four columns, rail icons, detail 300 px (resizable 240 to 480) | Rail labelled mode available by setting |
| 1920 to 2559 | Four columns, rail labelled (about 200 px), Comfortable default | Tags and version chips fit on most rows |
| 2560 and above | Same four columns; extra width goes to the two lists, never to the detail column beyond 480 px; optionally a fifth region: issues panel docked right | Cap line length of text in detail at about 80 characters; the lists may show a second metadata line only in Roomy mode |
| 1100 to 1279 (unsupported, usable) | Detail becomes an overlay drawer (selection or `Ctrl+B`); two lists plus rail | Same row slots |
| Below 1100 | The lists become tabs (Inactive, Active); move by button or keyboard; detail is a drawer | A drag between lists is replaced by the Space and Return keyboard move |
| 200 percent zoom at 1280 x 800 | Falls back to the drawer layout (MM-038 item 4) | Test in the gallery |

Heights down to 800 px keep top bar and status strip fixed and the lists flexible.

### 1.5 Steam Deck

Native Linux at 1280 x 800. Requirements: the default layout works with a collapsible side panel; Roomy density gives 40 px primary targets; every drag has a keyboard equivalent (also serves a controller mapped to keyboard); in game mode the app starts fullscreen, uses native dialogs and no custom title bar; safe graphics must be reachable without a keyboard through the native crash dialog; an offline SD card shows the offline state, not errors.

### 1.6 Shell-level states

Each state is a designed screen or banner reachable in the gallery with mocked IPC, in both themes (MM-036, GD-080).

| Id | State | Trigger | Presentation | Primary action |
|---|---|---|---|---|
| SS-01 | First run | No `settings.jsonc` | Full-window wizard (SCR-01) replaces the workspace; rail and top bar are dimmed and inert | Open library |
| SS-02 | Datasets offline | No network or offline mode (CD-011) | Datasets chip reads Offline in neutral styling (never error styling when a cached copy exists, MM section 7); status strip shows "using last copy from <date>"; no toast storm | Refresh now |
| SS-03 | Game running | `game.state {running}` (MM-029, GD-070) | Game running chip in the top bar; Save, deploy and launch disabled with the reason tooltip "RimWorld is running"; list editing still works | Check again |
| SS-04 | Safe graphics | `launch.jsonc` flag or `--safe-graphics` (ST-051) | Small persistent chip in the status strip on Linux only; the crash-marker choice itself is a native dialog drawn before any webview, so it is not designed in the web UI but its copy is in section 6 | Open Settings, Launch |
| SS-05 | Outdated webview page | Feature probe fails (`@layer`, `@property`, `color-mix()`, container queries, `:has()`) | Static full-page notice naming the minimum engine and what to update; no shell | None (informational) |
| SS-06 | Crash notice | Crash marker from the previous run, or unhandled error with an `errorId` | Banner after restart: "RimStudio closed unexpectedly last time" with Copy diagnostics and Open logs; render failures in one feature show an ErrorCard in that region only | Copy diagnostics |
| SS-07 | Update available | Updater check (M7, `updates.*` settings) | Quiet chip in the top bar and a line in About; an update dialog (SCR-37) only on click; never interrupts a running task or an unsaved list | View update |
| SS-08 | Backend unavailable | `app_ping` transport error | Full-screen banner: the backend stopped, Restart | Restart |
| SS-09 | Read-only data roots | Settings write fails (GD-080) | Banner "Settings will not be saved"; settings live in memory | Open data folder |
| SS-10 | Modified, unsaved | Working list differs from saved list (MM-015) | Modified marker and dirty dot on Save in the rail and status strip | Save |

## 2. Screen inventory

Each row lists: the screen and its purpose; tool and roadmap milestone (M0 to M7: M2 manager MVP, M3 manager v1, M4 toolkit foundation, M5 designer, M6 publisher, M7 release); requirement ids; key regions with the primary data (DTO or command names as written in the specifications); primary actions; the required states; notable interactions. The universal state set is: empty, loading (skeleton, never a blocking spinner), partial (some data present, some pending or failed), populated, error (card with short cause, one fix button, Copy diagnostics with an error id, never a raw stack trace), offline (neutral styling when a cached copy exists) and extreme data (5,000 rows; names up to the longest real name of 51 characters and beyond; mixed Latin and CJK scripts; 41-character package ids; 20 or more authors). A state listed as "n/a" is not applicable. The catalogue of empty, loading, error and offline states in mod-manager.md section 7 is authoritative for the manager screens.

### 2.1 Manager and its panels

| Id | Screen and purpose | Tool, M | Req | Regions and data | Actions | Required states | Notable interactions |
|---|---|---|---|---|---|---|---|
| SCR-01 | First-run wizard: show detection already done; at most three screens (Detected, Sources, Ready) | Manager, M2 (cards from M1) | MM-001, GD-003, GD-004, GD-005, GD-010, GD-011, GD-012, CD-007 | Five cards (install, Steam libraries, Workshop content, user data folder with `ModsConfig.xml`, game version) from the `DetectionReport` via `detect_run`; each with path, "found because" line, confidence chip (high, medium, low), validity marks (exists, readable, writable), candidate segmented list; Mod folders row; dataset opt-in sentence and toggle; Ready counts | Confirm or switch candidate, Choose folder (or drop a folder), Add folder, Keep this choice, Run detection again, Open library | Empty: n/a. Loading: skeleton cards. Partial: some cards Not found with the consequence stated ("launching is disabled"). Populated: all found, one click. Error: detection failed card with Retry and Choose folder. Offline: works offline, dataset sentence says it will fetch later. Extreme: two Steam libraries, native and Proton user folders, Flatpak and a non-ASCII path | Switching a candidate writes an override with `pinned: false`; re-run shows a diff against the previous report ("1 new candidate, 1 candidate gone"); fully keyboard operable; Steam helper absence never blocks |
| SCR-02 | Manager workspace: the main screen; four columns Detail, Inactive, Active, Actions rail | Manager, M2 | MM-003 to MM-015, MM-019, MM-020, MM-040, MM-041, MM-042 | Detail 300 px; Inactive list with header (title, count, search, filter, sort, counters); Active list with position numbers, group headers, diagnostics counters, dirty marker; Actions rail (Save, Save and Run, Sort, Undo, Redo, Refresh, Clear, Profiles, Import and export, Tools, Palette). Data: `mods_snapshot` (`ModRowDto[]`, `order`, `counts`), `mods_subscribe` (`library.delta`), `library_query` (`ModIdx[]` plus match ranges), `library.diagnostics` | Move, enable, disable, search, filter, group, pin, tag, colour, note, save, run, undo | Empty, no game: "RimWorld not found", Choose install, Run detection. Empty, no mods: "No mods found in the sources", Add folder. Empty, active list: "Only official content is active", Import list, drag hint. Empty filter: "No mods match", query, Clear filter. Loading: cached rows at once, thin "refreshing" indicator; cold start shows rows progressively with a bar and estimated count. Partial: scan problems as per-mod diagnostics and footer counts. Populated. Error: scan failed with counts per source and Retry; `ModsConfig.xml` unreadable card with backups and Restore. Offline: offline sources marked on their rows. Extreme: 5,000 rows at 60 fps, at most 3,000 DOM nodes per list, CJK names, 51-character names, 41-character ids | Drag and drop between lists with insertion line; keyboard parity (section 4); selection per list survives filtering; Esc cancels drag; rows never reorder by themselves on a delta |
| SCR-03 | Mod detail panel: everything about the selected mod, or a library summary when nothing is selected, or a bulk bar for a multi-selection | Manager, M2 (folder size and Relations M3) | MM-007, MM-018, MM-020, LO-024 | Header with 256 px preview via `rsimg`, name, source and status badges; fields (package id with copy, authors, version, supported versions with the current game version highlighted, tags, group, colour, notes, path, links, times, folder size "Calculating", dependencies and incompatibilities with satisfied or missing state and jump links, applying rules with source layer); lazy description (BBCode or plain text); tabs Overview, Relations, Diagnostics, Files. Data: `mods_get_detail`, `library_folder_size` (job), `usermeta_set_notes` | Copy id, open folder, edit notes (debounced autosave), tag, colour, group, enable, disable | Empty: library summary (counts, dataset freshness, game version). Loading: skeleton, description last. Partial: folder size calculating, preview placeholder. Error: "Could not read this mod's details" with path and Copy diagnostics. Offline: preview placeholder. Extreme: 20 authors, 4,000-character notes, a description with many images | Detail within 50 ms from cache; the description is sanitised and loads no remote resources; multi-selection shows count and bulk actions |
| SCR-04 | Why-is-it-here drawer: explain a position, or why A is above B | Manager, M2 simple, M3 full | MM-018, LO-023, LO-024 | Right drawer over the detail column: tier and reason; binding predecessor and successor with edge sources; slack range; violated constraints; chain steps `{earlier, later, kind, sources}` each with layer chip (About, community, user, derived), rule reference, comment and dataset version; kinds chain, reverse, tier, dropped, none. Data: `sort_explain` (query) with rules revision | Open source (About line, community rule read-only, user rule editable), Ignore this rule, Restore suppressed edge, Refresh when stale | Empty: "No rule; position is user choice". Loading: skeleton. Partial: some sources unresolved. Populated: three-edge chain. Error: explain failed card. Offline: n/a. Extreme: chain of 12 steps with long names | Each source is a link; stale panel detects a changed revision and offers refresh; the engine never guesses a reason |
| SCR-05 | Sort preview: two-step sort, preview then apply, as a panel over the active column | Manager, M2 | MM-017, LO-012, LO-013, LO-015, LO-025 | Diff of moves grouped into blocks with before and after positions and a short reason each; counts (positions changed, mods moved, tier sizes); algorithm switch Tiers or Game style; options (community rules, user rules, dependencies imply order, pinned fixed, ask about missing dependencies); lists of dropped edges, cycles by mod name with rule source, tier conflicts, unmapped mods, remaining diagnostics. Data: `sort_preview` (job; `order`, `moves`, `counts`, `token`, `stats`), `sort_apply` | Apply (one undoable command), Cancel, Sort and apply (palette), change algorithm or options (debounced recompute) | Empty: "Already sorted" with a tick. Loading: progress bar with Cancel. Partial: cycles and conflicts shown as a result, not an error. Populated: "Sorted 14 mods" example. Error: engine failure card. Offline: not affected. Extreme: 600 of 610 moves, 40-mod block move shown as one entry | Never silently reorders; preview under 100 ms for 610 mods; groups shown as split with the reason when a hard rule forces it |
| SCR-06 | Save preview: what will be written, non-modal | Manager, M2 | MM-031, MM-030, GD-062, GD-065 | Diff against the saved list; deploy plan (links to create or remove); applicable pre-launch checks; backup status. Data: `save_preview`, `save_apply` (job) | Confirm, Cancel, Copy diff, "Save without preview" setting | Empty: clean, nothing to save. Loading: progress. Partial: some checks warn. Error: write failed, backup untouched, list stays dirty. Game running: blocked with reason. Extreme: 40-mod block move, 12 links in the plan | Disabled when clean; a dot on Save when dirty |
| SCR-07 | Issues panel and diagnostics popover: all diagnostics for the working list, per row and global | Manager, M2 | MM-016, LO-018, LO-019, LO-021, LO-022 | Row popover (first three messages on hover; Mute and Fix); full panel grouped by severity then code with counts; filters `has:error`, `is:muted`; each item shows code, message with mod names, fix buttons. Data: `validate_list`, `library.diagnostics {rev, byMod}`, `validate_mute` | Fix (Enable dependency, Disable incompatible mod, Move after X, Use replacement, Link now, Reconnect and rescan), Mute, Open About.xml | Empty: "No issues" with a tick. Loading: skeleton. Partial: some codes muted (hidden from counts). Populated. Error: engine failure card. Offline: dataset-dependent codes noted. Extreme: 400 diagnostics grouped, a mod with 9 messages | Fixes are undoable commands; blocking codes (`deploy.not-visible`, `deploy.source-offline`, `list.unresolved-id`, `list.core-inactive`) are visually distinct from advisory ones; red only for hard conflicts |
| SCR-08 | Missing dependencies flow | Manager, M2 | MM-025, LO-019 | Non-modal panel with three groups: satisfied, available locally (inactive in any source, with source icon), not installed (Workshop link); checkboxes; alternatives and replacements as options of one requirement, not separate rows | Add selected, Add selected and sort, Sort without adding, Open Workshop pages, Ignore (mutes); Enter primary, Esc closes | Empty: nothing missing. Loading: skeleton. Populated: one unmet dependency with its inactive provider. Error: n/a. Offline: Workshop links explain they need network. Extreme: 30 missing across 12 mods | Adding is one undo entry; reached from Sort, a popover or a Fix dependencies button |
| SCR-09 | Duplicates resolver | Manager, M2 | MM-027, GD-051, GD-052, GD-031 | Groups by package id; per copy: source, path, version, modified, size, active; the effective entry marked with the reason it won ("pinned", "higher source priority", "matches game 1.6", "newer"); orphan ids. Data: duplicate groups from the library | Pin this copy, Unpin, Keep this one, Use the workshop copy, Use my copy, Open folder, Ignore this group; policy setting prefer workshop, prefer custom, ask | Empty: "No duplicates". Loading: skeleton. Populated: the owner's 7 groups (3 cross-root, 4 same-root). Error: n/a. Extreme: copies named "... - Copy" shown with their folder names | Never deletes or renames; counting is by group; the non-effective copy cannot be activated without pinning; `_steam` explanation only where exactly one member is Workshop |
| SCR-10 | Missing mods section: ids referenced but provided by no source | Manager, M3 | MM-026, LO-019 (`list.unresolved-id`) | Collapsible section above the active list, not ghost rows inside the order; name when known, Workshop id, status | Open Workshop page, Subscribe (helper), Search in custom sources, Remove from list, Keep and accept (checkbox naming the DeactivateNotInstalledMods consequence) | Empty: section absent. Populated: 4 ids. Error: n/a. Offline: names from the slim index only. Extreme: 60 missing ids after importing an old list | Save and Run blocked until each is resolved or accepted; a subscribe plus rescan removes the entry unprompted |
| SCR-11 | Profiles and history with diff | Manager, M3 | MM-021, MM-022, MM-015 | Top-bar switcher popover (Switch, New from current, Duplicate, Rename, Delete, Compare with current, Set as default); History panel: list with time, note, counts, delta against previous; diff of selected snapshot against the live list or of two snapshots (added, removed, moved, newly installed, no longer installed). Data: `profiles_list`, `profiles_load`, `history_list`, `history_diff`, `history_restore` | Switch, Restore (undoable), Export, Edit note, Open folder, Delete (confirm plus 10 s undo) | Empty: "No profiles yet", Create from current. Loading: list skeleton. Error: "History file unreadable", file kept, Open folder. Offline: same. Extreme: 100 snapshots, a profile referencing missing mods | Dirty profile switch asks a three-way choice (Save to the profile first, Discard, Cancel); restoring uninstalled mods marks them missing |
| SCR-12 | Import dialog with diff preview | Manager, M2 (some formats M3) | MM-023, MM-043 | Drop zone and paste box; format detected, never asked; preview: matched count, missing mods, already active, to add, remove, move, unmatched lines; mode Replace, Merge, Add as new profile; game version warning. Data: `import_detect`, `import_preview` (job), `import_apply` | Apply (one undoable command), Cancel, switch mode | Empty: drop zone with accepted formats. Loading: detect and preview progress. Error: unreadable file card with reason and encoding tried; the list is never changed. Offline: collection links explain they need network. Extreme: 610-mod list, 82 MB save file, mixed encodings | A folder dropped on the window offers Add as custom source; a list file opens this preview |
| SCR-13 | Export panel | Manager, M2 (share code M3) | MM-024 | Side panel with live preview, option set (inactive count, links, groups, paths), output size; formats ModsConfig style, RimStudio JSON, plain or Markdown report, Workshop id list, share code | Copy (`Ctrl+Shift+C`), Save as file | Empty: nothing active. Loading: preview progress. Extreme: Markdown above 200,000 characters warns and offers truncation | Exports contain no local paths unless "include paths" is ticked |
| SCR-14 | Pre-launch checks and block dialog | Manager, M2 | MM-029, MM-030, GD-066, GD-068, GD-070 | Checklist with Pass, Warn, Block per check (path valid, game not running, every active id resolves, link integrity, `ModsConfig.xml` writable and version, diagnostics, backup); block dialog listing unresolved ids with reasons; "The game changed the list" banner after exit. Data: `launch_check` (job), `launch_start`, `launch_state`, `game.list-changed` | Fix automatically, Deactivate unresolved mods and launch, Launch anyway (typed confirmation), Show diff, Adopt, Keep mine | Empty: all pass, no dialog shown (never appears when all ids resolve). Loading: visible checklist under 300 ms. Error: launch failed with check list. Extreme: 8 unresolved ids across two drives | Launching a list that needs no deployment is never blocked by link failures; Warn allows Launch anyway, Block does not |
| SCR-15 | Link farm and game visibility panel | Manager and Settings, M2 | GD-060 to GD-069, ST-022, MM-002 | Per custom folder: chip "Needs a link" or "Linked (12 mods)"; deploy plan with dry run; link audit listing owned and unowned entries in `Mods`; "Remove links created by RimStudio" (unlink only); Experimental tag while spike S-03 is open. Data: `deploy_plan`, `deploy_apply` (job), `deploy_unlink_all`, `deploy_audit` | Make visible, Repair links, Re-point drive (from `volumeHint`), Remove all links, Audit | Empty: no custom mods need links. Loading: plan progress. Partial: "Applied 12 of 14, rolled back" (`deploy.partial`). Error: per-OS messages with codes (GD-068). Offline: drive offline, row greyed, Find the drive. Extreme: 22 mods across 3 folders | Never retargets silently; never offers to delete a non-owned entry; no administrator elevation is ever requested |
| SCR-16 | Datasets panel (Data sources) | Settings and top-bar popover, M2 to M3 | MM-039, CD-001 to CD-012, CD-020, ST-030 | One row or card per dataset (`steamDb`, `communityRules`, `noVersionWarning`, `useThisInstead`, `rimworldVersions`): purpose, status chip (never-fetched, ready, stale, updating, rejected, offline, failed, disabled), entries, source and version, last checked, last changed, licence note; global switches; request list. Data: `datasets_status`, `datasets_subscribe` (`DatasetStatusDto`), `datasets_refresh` (job) | Refresh now, View changelog (added, removed, changed), Change source (https only, Test button), Enable, Revert to previous, Open cache folder, Inspect, Force accept | Empty: "Datasets not fetched yet", Fetch now. Loading: per-row progress with bytes. Partial: "3 records skipped". Error: per-row plain sentence with the last good copy shown. Offline: banner, cached copy and age. Extreme: rejected download after an entry count collapse | Every status renders a plain sentence and the next action; a status never hides the last good copy; panel works with every dataset disabled |
| SCR-17 | User rule editor and ignore list | Manager, M3 | MM-045, CD-013 to CD-016, LO-007, LO-021 | Table per mod with rows Load after, Load before, Incompatible with, Load first, Load last; layer and comment columns; toggle to show community and About rules read-only; suppressed rules; ignore list of muted diagnostics (mod, code, date). Data: `userdata/rules/user-rules.json` through rule commands | Add edge (pick installed mod or package id), Suppress, Restore, Export for community, Remove ignore entry, in-session undo | Empty: "No rules of your own". Error: cycle rejected with the cycle shown (edges and layers). Extreme: a mod with 40 community rules | Validation before accept: no self edge, no duplicate, no hard-edge conflict, no cycle; adding a rule re-sorts the preview and changes only affected rows |
| SCR-18 | Replacements panel | Manager, M3 | MM-039 item 4, LO-019 (`list.replacement-available`) | Grouped by replaced mod; replacement and the match basis (Workshop id, or package id as a hint only) | Replace (enable new, disable old, keep position, one undo entry), Ignore | Empty: "No replacements suggested". Offline: cached suggestions with age | Info severity only; never red |
| SCR-19 | Bisect flow | Manager, M3 | MM-032 | Setup (what stays fixed, one-line problem note); round view (round number, suspects remaining, rounds left, about 10 for 600); answer buttons; end state naming the suspect or minimal pair | Problem still happens, Problem is gone, Skip this one, Disable, Open its page, Find in logs, Restore the original list | Empty: "Start a session" with the explanation. Loading: round setup progress. Error: launch failed with the check list. Resume after crash: offer resume or restore | The original list is never lost; exit restores in one click |
| SCR-20 | Updates panel | Manager, M3 | MM-028 | List of Workshop mods with update and new badges; counter in the status strip | Open Workshop changelog, Update, Select all | Empty: "All workshop mods are current". Loading: Checking. Error: Retry card. Offline: "Needs network", last checked time. Extreme: 300 updates | Reached from the counter; Select all then Open or Update in 2 to 3 steps |

### 2.2 Settings and logs

| Id | Screen and purpose | Tool, M | Req | Regions and data | Actions | Required states | Notable interactions |
|---|---|---|---|---|---|---|---|
| SCR-21 | Settings shell and search: one full page for settings and sources | Settings, M1 | ST-010, ST-001, ST-080, MM-046 | Left navigation (Game and Steam, Mod sources, Datasets, Library and sorting, Appearance, Shortcuts, Launch, Tools, Performance, Diagnostics and data, About and updates, Import); content column; top search box matching title, description, aliases and dotted key path. Row anatomy: label, control, one-sentence description, Reset to default (only when changed), source tag ("set by command line") | Edit (applies immediately), Reset to default, Reset all (typed confirmation, backup first) | Empty search: "No setting matches" with the query. Loading: skeleton rows. Error: inline write error; read-only roots banner "Settings will not be saved". Extreme: search "thread" finds `library.scanThreads`; "library.watch" finds three rows | A quiet "Saved" mark after the debounced write; Enter on a result opens it with the control focused; no OK and Cancel pair |
| SCR-22 | Mod sources and custom folders editor | Settings, M1 (links M2) | ST-020, ST-021, ST-022, GD-040 to GD-045, MM-002 | Ordered source cards: built-ins first (greyed handles), then custom folders in priority order; card shows label, path, kind chip, mod count, last scan, reachability, visibility; Add folder confirm panel with probe result; Test result panel (reachable, mod count by layout, depth, parse failures, overlap check, link plan preview, duration); Advanced expander (layout, scan depth 1 to 4, watch, read-only, link mode, copy ignore list, volume hint). Data: `sources_list`, `sources_probe_folder`, `sources_add_folder`, `sources_update`, `sources_remove` | Add folder (picker or drop), Remove (never deletes files; confirms the count of active mods that would become missing), Reorder (drag handle or Alt plus arrows; toast "2 duplicate ids now prefer My mods"), Enable, Label, Colour, Open, Test, Rescan, Find the drive | Empty: "No mods yet" with the three source kinds explained, Add a mod folder. Loading: card scan progress. Partial: folder with unparsable mods. Error: hard errors with distinct codes and a one-line fix (`deploy.source-overlap`, `deploy.source-inside-mods`, `scan.path-not-directory`). Offline: chip and greyed rows, Find the drive. Empty folder: "No mods found here", Change scan depth. Extreme: 8 folders, a 240-plus character Windows path, an owner folder yielding 22 mods at depth 2 | Probe may suggest depth 2 with one click; the template repository is flagged "not a mod"; read-only folders show no edit tools anywhere |
| SCR-23 | Appearance | Settings, M1 | ST-040, ST-041, MM-046 | Controls for theme (light, dark, system, user themes with Open themes folder and Export current), accent (presets plus hex with contrast check against both themes), density (three previews), text size 80 to 160 percent, mod colour mode, language, reduced motion; live sample panel (a mod row, a diagnostic chip, a button, a card) | Change, Reset | Error: invalid user theme shows which key failed; unknown token loads with a warning. Extreme: accent that fails contrast is refused with the numbers | Repaint under 100 ms without reload; first paint reads the bootstrap theme, no flash |
| SCR-24 | Shortcuts | Settings, M2 | MM-037, ST-010 | Searchable table of commands with chord recorder, conflict marks and reset per binding and all | Record, Reset, Export (part of settings JSONC) | Error: conflict on assignment names the clashing command. Extreme: a chord of two steps | Single-key chords never fire in text fields; `Ctrl` or `Cmd` chords may |
| SCR-25 | Advanced: Launch, safe graphics, performance, diagnostics and data, migration | Settings, M2 (migration M3) | ST-050, ST-051, ST-041, ST-060, MM-043 | Launch method (Automatic, Through Steam, Direct), arguments with live parse and quoting errors, save data folder picker, environment table, four developer switches; safe graphics state (engine version, session type, variables in effect; Turn on, Turn off, Open file); scan workers and watch policy with last scan timings; storage summary, Clear caches, restore from backup with diff preview; Import from RimSort wizard (unavailable items explained, D-031) | Save, Turn on safe graphics, Rebuild caches (confirm), Restore backup | Empty: n/a. Error: invalid quoting inline, Save stays possible for other fields. Windows and macOS: safe graphics shows only that no action is needed. Extreme: arguments with quotes and `=` | Rebuild caches never touches user data, config or backups; changes needing a restart say so on the row |
| SCR-26 | Log viewer | Logs, M3 | MM-035, WS-007 | Counters as filter toggles (errors, exceptions, warnings, mod issues, info); folded identical blocks with counts; mod attribution as link; filters (text, severity, mod, time); Previous and Next; live follow toggle; raw block with stack on the right. Data: `logs_open` (job), `logs_page`, `logs_follow` (`log.append`) | Copy, Copy block, Open mod folder, Disable mod (undoable), Bisect from this mod, Open in Def Explorer | Empty: "Game log not found", expected path, Choose file. Loading: first page under 300 ms, streaming progress. Error: parse error with raw view fallback. Offline: n/a. Extreme: 100,000 lines, 2,000 identical blocks folded | Scrolling up pauses follow; F3 and Shift+F3 navigate; `log-file` capability gates the tool |

### 2.3 Toolkit

| Id | Screen and purpose | Tool, M | Req | Regions and data | Actions | Required states | Notable interactions |
|---|---|---|---|---|---|---|---|
| SCR-27 | Def Explorer: search any def, see parents, provenance and the resolved XML | Defs, M4 | WS-004, WS-005 (session) | Search bar with filters (type, mod, load folder, abstract, patched, overridden); virtualised results; four tabs Tree (parent chain up, children computed), Provenance (defining file plus every patch operation in order with file and operation index), Resolved (final XML with a layer slider "as defined", "after patches", "after inheritance" and changed-field highlights), References (def and texture edges in and out). Data: `defs_search` (`DefRowDto`, `queryId`), `defs_get_resolved`, `defs_find_references`, `defs_get_provenance`, `defs_tree`, `defs_compare` | Copy as XML, Open file at line, Copy as patch starter, Compare with | Empty: "Open a project or choose a reference set". Loading: index progress, snapshot "stale" badge. Partial: types not known show "unknown type"; custom operations labelled "not simulated". Error: session failed card. Extreme: 13,212 vanilla defs, 600-mod session, 12 CE overrides shown as "overridden" data and not errors | New text or filter makes a new `queryId` and stale pages are dropped; search under 20 ms |
| SCR-28 | Patch tester and simulator | Patch tester, M4 | WS-005 | Left operation editor with XPath highlighting and inline lints; right top match list; right bottom diff of the first match with selector; gutter badges pass, fail, "not simulated" | Run, Run in load order here, Copy as file, "assume CE active" simulation | Empty: no operation. Loading: running. Partial: some operations not simulated. Error: invalid XPath inline. Extreme: 500 matches paged | Same status the game log would give |
| SCR-29 | Validator | Validator, M4 | WS-006 | Left list grouped by severity and check with counts per code; centre rows with path and message; right detail with offending XML line highlighted and the game-behaviour explanation; scope switch (project, project plus conflicts, whole reference set) | Fix action, Suppress per project path (kept in reports as suppressed), Copy report | Empty: "No findings". Loading: progress. Error: n/a. Extreme: 800 findings across 12 codes | Codes follow `<area>.<kebab-name>` (`author.*`, `defs.*`, `xpath.*`, `ce.*`) |
| SCR-30 | Project scaffolder | Project, M4 | WS-001 | Three-step dialog (what, where, options) with live tree preview and About.xml text preview; end checklist | Create (opens the project), open About editor, create first def | Empty: template list. Error: folder exists. Extreme: a path on an external drive | Template files pass WS-002 and WS-003 with zero findings |
| SCR-31 | About editor | Project, M4 | WS-002, WS-011 | Form sections (identity, versions, dependencies, ordering, incompatibilities, description) left; raw XML right with edited span highlighted; issues strip; dependency rows with "pick from installed mods"; description length counter and BBCode preview | Save (byte-span write plan with diff), pick dependency | Error: lint findings with fixes. Extreme: 20 dependencies | Edits are write plans with a diff, stale plan reported as `project.plan-stale` |
| SCR-32 | Version folders and LoadFolders manager | Project, M4 | WS-003 | Tree with version badges; list of LoadFolders blocks with condition chips; effective files pane with active-mod simulation | Drag a folder into a block, toggle condition, add version (write plan with file count and size) | Empty: banner explaining the fallback when no LoadFolders file exists. Error: `author.loadfolders-case`, `author.loadfolders-missing-folder`, `author.loadfolders-ce-gate` | Case problems found by listing spelling, not by opening paths |
| SCR-33 | Dev launcher | Project, M4 | WS-009 | Compact panel: list source dropdown, four switches (dev mode shows locked state), isolated save folder path with inline `=` check, big Launch that becomes "Running" then "Restore list and show log" | Launch, Restore | Game running: switches refused with reason. Error: bad save folder path | Gated by `game-stopped` for writes |
| SCR-34 | Save inspector | Saves, M4 | WS-008 | Table of saves (name, game version, mod count, size, date); selected save shows its mod list with chips (active and matching, different position, installed but inactive, missing) | Activate exactly these mods (undoable manager edit), Copy list, Save as profile | Empty: no saves found. Extreme: an 82 MB save | Read through a streaming reader |

### 2.4 Item designer

| Id | Screen and purpose | Tool, M | Req | Regions and data | Actions | Required states | Notable interactions |
|---|---|---|---|---|---|---|---|
| SCR-35 | Item list and reference browser | Designer, M5 | IT-020 | Table of drafts and project defs (label, defName, kind, calibration mode, CE patch off, on or already CE, status draft, written, out of date; fit summary; last changed); reference browser tab with strength index, tier, role sortable by any stat | New, Clone, Convert, Delete draft, Open; use as anchor or compare | Empty: explains the three starting points (new weapon, new apparel, clone or convert). Error: n/a. CE absent: the CE patch toggle and Convert disabled with the reason. Extreme: 200 drafts, 19 vanilla ranged references | Flows A to D from items-toolkit section 4 |
| SCR-36 | Design form with live readouts, fit meter and charts | Designer, M5 | IT-021 to IT-025, IT-022, IT-023 | Left: grouped fields with unit, source chip (typed, anchor, class median, answer), band and pool p10, median, p90; ruler-tick sliders; right: live readouts (cycle time, nominal and hit-adjusted DPS, implied AP, P4, armor-adjusted survival; melee; apparel coverage, API, price breakdown; CE sustained DPS, mass, Bulk, meets-armor, shown only when the item's CE patch toggle is on); fit meter at top, beside it the off by default toggle "Add a Combat Extended patch (optional)" with the gated folder explanation (the vanilla definition is never changed, nothing is automatic even when CE is installed); reference comparison charts (scatter, distribution strips, DPS against distance, rank readout) | Edit field, Set to suggestion, Recalibrate, Start quiz, Toggle the optional CE patch | Empty: pick a kind and parent base. Loading: calibration job progress. Partial: "bands are rough" below about 15 items, "bands optimistic: many near twins". Error: formula domain error inline. Extreme: 47 stuffs, a field with an "ask" flag showing no estimate | Preview calls coalesced to one per animation frame while dragging; charts lazy chunk |
| SCR-37 | Material and quality matrix | Designer, M5 | IT-027 | Rows allowed stuffs (up to 47), columns seven quality levels; switchable cell metric; cap, threshold and band highlights; ranged weapons show the quality ladder only | Switch metric, sort | Empty: unstuffed ranged weapon shows the ladder. Extreme: 47 x 7 = 329 cells | Highlights never rely on colour alone |
| SCR-38 | Quiz stepper | Designer, M5 | IT-026 | Modal-like stepper over the form: anchor card (name, picture, index components with real numbers), three large answers or options, typed value, "not sure"; header "question n of about m" with cost up front; side panel with live estimate | Back, Skip to result, Use what I have | Empty: n/a. Loading: n/a. Extreme: cap of 9 questions | Answers saved in the draft; not a gate |
| SCR-39 | Output preview and write to project | Designer, M5 | IT-028, IT-029, IT-014 | Tabs Definition, CE patch and LoadFolders (only with the toggle on), About changes, Files; diff against project files; static validation codes above; confirm step lists files with status (create, update region, unchanged), size, location | Write (job, cancel between files), Cancel | Empty: nothing to write. Error: blocking diagnostics disable Write; dry-apply failure lists operations. Result links to the Def Explorer entry | No file changes before confirmation |
| SCR-40 | CE update and lint | Designer and Validator, M5 | combat-extended-patching 4.x, WS-006 | Lists convertible defs with status ("already converted" never converted again); lint findings (`ce.*`); update mode shows regions to change | Convert, Check my CE patch | CE not installed: "not checked: CE not installed" for rules needing CE data. Extreme: a mod with 40 convertible defs | Reviewable diff before any write |

### 2.5 Publisher

| Id | Screen and purpose | Tool, M | Req | Regions and data | Actions | Required states | Notable interactions |
|---|---|---|---|---|---|---|---|
| SCR-41 | Project picker | Workshop, M6 | WP-001, WP-031, WP 5.1 | Searchable list: name, packageId, source badge, Workshop id or "not published", last published, status chip (Ready, Has blocking findings, Needs recovery, Not uploadable) | Open plan, drop a folder to register | Empty: "No projects yet". Extreme: duplicate Workshop ids flagged | Header stepper: Project, Plan, Metadata, Review, Publish, Result |
| SCR-42 | Plan and ignore editor | Workshop, M6 | WP 5.2, WS-010 | Tri-state file tree with sizes and excluding rule chips; lock on required paths; totals; changes since last upload; ignore chips (VCS, IDE and build, Source folders, Raw art, Junk, Archives, Docs suggestion); gitignore text area with live matching. Data: `publish_plan` (job, `PublishPlan`) | Check, uncheck, edit patterns, Dry run, export file list | Loading: plan progress. Extreme: 3,000 files, unmatched pattern flagged | Edits invalidate the plan and rebuild it incrementally |
| SCR-43 | Preflight results | Workshop, M6 | WP 5.3 | Blocking, Warnings, Information groups; code, message, file or field, fix action; banner whether publishing is allowed | Fix, filter by code, copy report | Empty: all clear. Extreme: 40 findings | Live update as the user edits |
| SCR-44 | Metadata and description editor | Workshop, M6 | WP 5.4, 5.5, WS-011 | Form (title with 128 counter, description source switch with 8000 counter, tags with 255 and 1024 limits, visibility, preview image info, read-only dependencies) with live Steam BBCode preview; change note with "Fill from changes" | Convert Markdown to BBCode, downscale image (staging copy only) | Error: wrong image format, over 1 MB, not 16:9 | BBCode rendered through the sanitiser |
| SCR-45 | Review and confirm | Workshop, M6 | WP 5.6, 4.1 | Summary of target, account, title, tags, visibility, files and bytes, diff counts, warnings, terms link; confirmation dialog | Dry run, Publish | Blocking findings disable Publish. Recovery banner for created-not-submitted | Dialog has no countdown |
| SCR-46 | Progress | Workshop, M6 | WP 5.7 | Stage labels (Staging, Connecting to Steam, Creating item, Preparing configuration, Preparing content, Uploading content, Uploading preview, Committing changes); byte bar or indeterminate; log drawer | Cancel (always enabled, says what it does) | Mirrored in the task centre | May leave the screen |
| SCR-47 | Result | Workshop, M6 | WP 5.8, 10.1 | Done, NeedsAgreement, Failed, Cancelled variants | Open Workshop page, "I accepted it", Retry, Create a new item | Failed shows code, message, remedy, EResult, log hint | NeedsAgreement is a first-class state, not an error |
| SCR-48 | Publish history | Workshop, M6 | WP 5.9 | Runs: time, outcome code, item id, files, bytes, note excerpt, manifest; manifest file list and diff against current plan | Select run | Empty: "Nothing published yet" | History is never edited |

### 2.6 Shell screens

| Id | Screen and purpose | Tool, M | Req | Regions and data | Actions | Required states | Notable interactions |
|---|---|---|---|---|---|---|---|
| SCR-49 | Task centre | Shell, M2 | MM-033 | Drawer listing jobs: name, phase, bar, elapsed, Cancel, Details with log lines; finished stay for the session. Data: `job_list`, `job.progress`, `job.finished`, `cancel_job` | Cancel, Retry, Details, dismiss | Empty: "Nothing is running". Extreme: 40 finished tasks, 20 progress messages per second coalesced | Cancelling a scan stops within 250 ms |
| SCR-50 | Command palette | Shell, M2 basic, M3 full | MM-034 | Input, grouped results (commands, mods, settings), shortcut hints, parameter steps | Run, go to mod | Empty: recent items. No result: "No command matches". Disabled commands show the reason | Fuzzy match under 30 ms |
| SCR-51 | Diagnostics and About | Settings, M1 and M7 | ST-060, ST-010 | Version, channel, licences, data roots with mode (installed or portable) and sizes, log folder, Copy diagnostics (redacted, with error id) | Open folders, Copy | Error: write failure | No telemetry statement and the network request list |
| SCR-52 | Update dialog | Shell, M7 | ST-010 (`updates.*`) | Version, notes, size, Download and Install, Later | Install, Skip | Offline: "Needs network". Error: signature failure message | Not shown while a task runs or the list is dirty |
| SCR-53 | Shortcut map overlay | Shell, M2 | MM-037, MM-034 | Grouped table of shortcuts by context, built from the registry | Close with Esc | n/a | Opened with `?` |

## 3. Component inventory

The shared library is `rimstudio-ui` (frontend architecture section 13): 46 primitives and wrappers listed there, plus the composites below that features assemble from them. Components are stateless or hold only widget-local state, receive translated text as props, are documented in typed story lists and screenshot-tested in both themes. Every component must render the universal state vocabulary where it applies: default, hover, active (pressed), focus-visible, disabled, loading, error, selected, dragging, drop-target. A state that does not apply is simply absent from its story list.

### 3.1 Token vocabulary used below

Colour roles: `--rs-bg`, `--rs-surface`, `--rs-surface-raised`, `--rs-border`, `--rs-text`, `--rs-text-muted`, `--rs-accent`, `--rs-accent-contrast`, `--rs-danger`, `--rs-warning`, `--rs-success`, `--rs-info`, plus diagnostic severity colours and a grid line colour for the blueprint background. Scales: spacing, radius, type (Barlow for text, Barlow Condensed for headings and labels, Azeret Mono for ids, versions, paths, numbers and rule names), line heights, z layers, durations. Density is `data-density`. Row states use `data-*` variants (`data-selected`, `data-invalid`). The owner's earlier brief asked for a narrow palette: a blue-black slate ground with rust and amber as the only accents, red reserved exclusively for hard conflicts. In token terms `--rs-danger` is only used by `list.incompatible`, `list.order-hard`, `list.duplicate-id`, blocking deploy codes and destructive confirmations; warnings use `--rs-warning`, info uses `--rs-info`, and the accent is never a status colour.

### 3.2 Primitives (summary of the 46)

Variants and states for primitives are fixed in frontend architecture 13.2 and are not repeated. Notes for designers:

| Group | Components | Design rules |
|---|---|---|
| Actions | Button (primary, secondary, ghost, danger; sm, md), IconButton, ToggleButton, SegmentedControl | One primary per surface; danger only for destructive confirmations; IconButton always has a tooltip showing label and shortcut (Kbd); minimum 32 px target, 40 px in Roomy |
| Inputs | TextField, NumberField (unit, stepper, range hint), Slider (ticks), SearchField (filter chips, parse error inline with position), Select, Combobox, TagInput, ColorPicker (theme-aware swatches plus custom), ShortcutRecorder | Invalid state shows text plus icon, never colour only; NumberField shows unit in Azeret Mono |
| Containers | Panel (plain, framed with corner ticks, titled), Card, Tabs, TabStrip, Splitter (keyboard resize), Drawer, Dialog, Popover, Menu, ContextMenu, FormField, FormSection | Corner-tick frames are the blueprint signature and are drawn with borders or a CSS gradient, no filters |
| Feedback | Badge, StatusDot (colour plus shape), Chip, ProgressBar, Spinner, Skeleton, Tooltip, Toast, Banner, EmptyState, ErrorCard | Every badge has an icon or a letter; ErrorCard has cause, one fix button and Copy diagnostics |
| Data | Table, TreeView, PropertyGrid, Meter (target band), DiffView, ModPreview | Tables over 100 rows use the windowing hook |

### 3.3 Composite components

| Component | Variants and sizes | States | Tokens | Accessibility | Used in |
|---|---|---|---|---|---|
| ModRow | Compact 28, Comfortable 34, Roomy 40 px; active (with position number) and inactive; slots in fixed order: group bar 3 px, position, source icon, content icon (C# or XML), name with tags, version chip, state badges (Not visible to the game, Offline, New, Updated, Pinned), diagnostics icon, overflow button on hover or focus | default, hover, focus-visible, selected, multi-selected, dragging (ghost shows first row plus count), drop-target (insertion line), disabled (offline, greyed), loading (skeleton), error (diagnostics icon) | `--rs-surface`, `--rs-border`, `--rs-accent`, severity colours, group and mod colours (theme-aware), Azeret Mono for position | `role="option"` in a `listbox`, `aria-selected`, `aria-setsize`, `aria-posinset`, `aria-activedescendant`; diagnostics have text alternatives; colour never the only carrier; tooltip after 600 ms never while dragging | SCR-02, SCR-09, SCR-10, SCR-19 |
| GroupHeader | Same height as a row; chevron, colour dot, name (inline rename), count, diagnostics totals; collapsed and expanded | default, hover, focus-visible, selected, dragging (moves as a unit), drop-target (move into group), collapsed | group colour, `--rs-surface-raised` | Button with `aria-expanded`; Alt+Up moves the whole group | SCR-02 |
| DiagnosticsBadge and DiagnosticItem | Badge: red circle with count (errors), amber triangle (warnings), blue (info); Item: code in Azeret Mono, message with mod names, fix buttons, Mute | default, hover (first three messages), focus-visible, muted, blocking | severity colours, `--rs-danger` only for hard conflicts | Text alternative "3 errors"; popover reachable by focus; fixes are buttons with names | SCR-02, SCR-03, SCR-07, SCR-29, SCR-43 |
| RuleChip | Layer chip: About, community, user, derived; kinds load after, load before, incompatible, force top, force bottom; suppressed (struck through plus icon) | default, hover, focus-visible, selected, disabled (read-only layers greyed), dropped (dashed) | `--rs-info`, `--rs-accent`, muted | Layer written as text, not just colour; chip is a link when it opens a source | SCR-03, SCR-04, SCR-05, SCR-17 |
| ProvenanceTrail | Vertical or horizontal steps: defining file, patch operations in order with file and operation index, inheritance; "not simulated" marker | default, step hover, step selected, loading, error (operation failed) | `--rs-border`, Azeret Mono for paths | Ordered list semantics; each step has a text label with index | SCR-04 (explain chain), SCR-27, SCR-28 |
| DiffBlock | Inline and side by side; line diff with added, removed, moved; collapsed unchanged; block-move entry ("moved 40 mods"); counts header | default, empty ("unchanged"), loading, collapsed, expanded | added and removed tints with sign characters `+` and `-` | Signs carried as text; not colour only; copy button | SCR-05, SCR-06, SCR-11, SCR-12, SCR-31, SCR-32, SCR-39, SCR-45 |
| DatasetCard | Card or table row; status chip (never-fetched, ready, stale, updating, rejected, offline, failed, disabled); progress with bytes; licence note | default, hover, focus-visible, updating (progress), rejected (badge beside last good copy), offline (neutral), failed, disabled | `--rs-surface`, `--rs-info`, `--rs-warning` | Status sentence in text; Refresh has name including the dataset | SCR-01, SCR-16, SCR-21 |
| TaskItem | Name, phase, ProgressBar (determinate or indeterminate), elapsed, Cancel, Details; finished with outcome | running, paused, finished, failed (Retry), cancelled | `--rs-accent`, `--rs-danger` for failure | Progress has `role="progressbar"` and values; completion announced politely | SCR-49, SCR-46 |
| FitMeter | Panel head (share of stats inside P80, reference item count, typicality 0 to 100, calibration date); per-stat bar with prediction marker, P50 and P80 bands, item value marker; notices "bands are rough", "bands optimistic: many near twins" | within P50 (green, "typical"), within P80 (blue, "plausible"), outside P80 (amber, "unusual"), unknown (no estimate), calibrating | success, info, warning, band tints (red is never used: it is reserved for error-severity problems) | Each stat has text "typical", "plausible", "unusual" and an icon shape per state, plus the nearest reference value for unusual stats | SCR-36 |
| RulerSlider | Text input plus slider whose track is a ruler: ticks at pool minimum, p10, median, p90, maximum; labelled marks for the three nearest reference items; shaded P50 and P80 bands; distinct current and suggested markers; snapping to natural step | default, hover, focus-visible, dragging, disabled, invalid, "ask" (no estimate, pool range shown) | `--rs-border`, `--rs-accent`, band tints, Azeret Mono labels | `role="slider"` with value text including unit and band; Arrow keys step, PageUp and PageDown larger steps; effect on the headline readout announced politely, throttled | SCR-36 |
| QuizStep | Anchor card (name, picture where available, index components with real numbers) plus three large answers (weaker, about the same, stronger) or option set, typed value, "not sure"; header with "question n of about m" | default, answer hover, focus-visible, selected, answered, loading (recomputing) | `--rs-surface-raised`, accent | Answers are a radio group; Back and Skip to result are reachable by keyboard; live estimate in a polite region | SCR-38 |
| ReferenceChart | Scatter (two stats, reference items labelled, new item highlighted, band), distribution strips, DPS against distance, survival against penetration, swing damage against cooldown; rank readout beside each | loading, empty, populated, hover point (tooltip), error | chart palette from tokens, item highlight in accent | Each chart has a text summary and a data table toggle; points reachable by keyboard; under 200 points | SCR-36 |
| MaterialMatrix | Rows stuffs, columns seven qualities; metric switch; highlighted cells for cap, threshold and band violations | default, cell hover, focus-visible, selected cell, violation | `--rs-warning`, `--rs-danger` for cap violations | `role="grid"` with row and column headers; violation has icon plus text | SCR-37 |
| VirtualList | Fixed row height windowing with overscan; sticky group headers | loading (skeleton rows), empty, populated, scrolling, dragging with auto-scroll | none own | Provides `aria-setsize`, `aria-posinset`, live region for moves ("Moved 3 mods to position 12") | SCR-02, SCR-26, SCR-27, SCR-35 |
| DropIndicator | Insertion line between rows; red variant with one-line reason when a hard rule would break; refused variant for official content | allowed, breaks-hard-rule, refused | `--rs-accent`, `--rs-danger` | Reason is text in a live region while dragging | SCR-02 |
| ListHeader | Title, count, SearchField, filter button, sort menu, clickable counters (errors, warnings, new, updated, offline, duplicates), selected count | default, filtered, has query error, mode hide or highlight | severity colours | Counters are toggle buttons with names and counts | SCR-02 |
| ActionsRail | Icon mode 56 px or labelled mode 200 px; Save (dot when dirty), Save and Run (primary), Sort, Undo, Redo, Refresh, Clear, Profiles, Import and export, Tools, Palette | default, hover, disabled with reason tooltip, dirty, busy | accent for primary | Each button has name, tooltip with shortcut; 32 px minimum targets | SCR-02 |
| SourceBadge | Official, install Mods, workshop, custom (with the source's own colour dot) | default | source colours | Text in tooltip and `aria-label` | ModRow, SCR-09, SCR-22 |
| SourceCard | Label, path, kind chip, mod count, last scan, reachability, visibility, menu, expander | default, selected, offline, disabled, dragging (reorder), probe result shown | `--rs-surface`, `--rs-warning` for offline | Drag handle has keyboard alternative Alt plus arrows | SCR-01, SCR-22 |
| DetectionCard | Path, "found because", confidence chip, validity marks, candidate segmented list, Open folder | found, ambiguous, not found, override invalid | confidence tints plus text | Validity marks have tooltips and text | SCR-01, SCR-21 |
| StatusChip | Datasets (Fresh, Updating, Stale, Offline, Error), Steam, Game running, Safe graphics, Update available | each status | info, warning, neutral | Text carries status | SH-03, SH-06 |
| QueryBar | Free text plus fields (`name:`, `id:`, `tag:`, `has:`, `is:` and so on), parse error inline with position, saved filter dropdown | empty, typing, results, no results, error | `--rs-danger` for parse error with icon | Announces result count politely | SCR-02 |
| FileTree | Tri-state checkboxes, size column, excluding-rule chip, lock on required paths | collapsed, expanded, partial, excluded, required | `--rs-border` | Tree semantics, Space toggles | SCR-42, SCR-32 |
| LogBlock | Severity icon, folded count, mod link, raw block with stack | default, folded, expanded, selected | severity colours | Navigable with F3 and Shift+F3 | SCR-26 |
| StepperHeader | Project, Plan, Metadata, Review, Publish, Result with state | current, done, blocked | accent | `aria-current="step"` | SCR-41 to SCR-47 |
| PreflightChecklist | Pass, Warn, Block lines with fix buttons | pending, running, pass, warn, block | success, warning, danger | Text labels Pass, Warn, Block | SCR-14, SCR-43 |
| ProfileSwitcher | Current profile, Modified marker, menu | default, dirty, open | accent | Menu semantics | SH-03 |
| BBCodePreview | Sanitised rendering of Steam BBCode, banner "Steam renders BBCode, so Markdown shows literally" | empty, populated, over limit | `--rs-surface-raised` | No remote resources, no script | SCR-03, SCR-31, SCR-44 |

## 4. Interaction catalogue

### 4.1 Selection model (MM-011)

Click selects; `Ctrl` or `Cmd` click toggles; `Shift` click selects a range in the current list order; `Ctrl+A` selects all visible (filtered) rows in the focused list; `Esc` clears. No rubber band. Selection is per list, survives filtering, is restored after undo of a move, and is unaffected by a delta for an unrelated mod. The selected count shows in the list header and the detail panel. Design a visible difference between selected, focused (keyboard cursor) and hovered, because all three can be on different rows at once.

### 4.2 Drag and drop (MM-012, frontend architecture 8.2)

| Aspect | Specification | Visual need |
|---|---|---|
| Single and multi | Dragging a selected row drags the whole selection in relative order; the ghost shows the first row and a count badge | Ghost with count; source rows dim; no ghost animation under reduced motion |
| Between lists | Inactive to Active enables at the drop position; Active to Inactive disables; one drop is one undoable command | Target list shows an insertion line; list header may show "Enable 3 mods here" |
| To groups | A drop on a GroupHeader moves into that group | GroupHeader drop-target highlight with outline |
| Insertion indicator | Live line between rows; red with a one-line reason when a hard rule would break (move allowed, raises diagnostics); refused indicator for official content (Core first, expansion order) with a message | Three DropIndicator variants: allowed, breaks hard rule, refused |
| Auto-scroll | At list edges, speed rising with proximity | Edge zones 40 px, no visible chrome |
| Cancel | `Esc` cancels; a resize while dragging does not drop the drag | Ghost vanishes immediately |
| External drops | A folder dropped on the window offers Add as custom source; a list file opens the import preview | Full-window drop overlay with two targets, never a blocking dialog |
| Tooltips | Never appear while dragging | Suppress hover cards |
| Keyboard parity | Every drop has a keyboard path (below) | Same insertion line appears while moving by key |

### 4.3 Keyboard map (mod-manager.md section 6, reused verbatim; all rebindable, MM-037)

On macOS `Cmd` replaces `Ctrl` except where stated. Single-key chords never fire while a text field is focused; `Ctrl` or `Cmd` chords may.

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

Holding a repeat key moves without flicker: one command per repeat, coalesced into one undo entry within 400 ms. Designers show the keyboard hint (Kbd chord) in every tooltip and menu item that has one.

### 4.4 Context menus per surface (MM-040 and the registry)

| Surface | Items |
|---|---|
| Mod row (either list) | List (Enable or Disable, Move to top or bottom, Pin, Move to group); Annotate (Tags, Colour, Notes); Open (folder, editor, About file, Steam page, homepage, copy package id, copy Workshop id, copy path); Rules (Edit rules, Mute diagnostics, Why here); Remove (Remove from list; Delete mod to the trash with an Undo toast, refused for workshop mods and replaced by Unsubscribe when the helper exists; never for official content); Toolkit (Open in project workspace, later) |
| Group header | Rename, Recolour, Collapse, Delete group (mods stay), Move up, Move down |
| Empty list area | Import list, Add folder, Refresh |
| Source card | Open, Test, Rescan, Enable or Disable, Label, Colour, Remove |
| Snapshot or profile row | Restore, Compare with current, Export, Edit note, Open folder, Delete |
| Diagnostics item | Fix, Mute, Open About.xml, Copy message |
| Log block | Copy, Copy block, Open mod folder, Disable mod, Bisect from this mod, Open in Def Explorer |
| Def row | Copy as XML, Copy defName, Open file at line, Find references |
| File tree node (publisher) | Include, Exclude, Add pattern, Copy path |

### 4.5 Tooltips

Long tooltip after 600 ms on a mod name: name, authors, package id, version, supported versions, size if cached, path, last modified. Badge tooltips explain the badge ("Not visible to the game: this mod is in a custom folder and needs a link"). Disabled actions explain why in their tooltip ("RimWorld is running"). Tooltips are reachable by keyboard focus and by touch long press, hold no interactive content, and are suppressed during drag.

### 4.6 Undo and redo visuals (MM-014, MM-015)

| Element | Behaviour |
|---|---|
| Undo and Redo buttons | Tooltip names the command ("Undo: Move 3 mods"); disabled when the stack is empty with the reason |
| Dirty marker | "Modified" marker in the top bar and a dot on Save whenever the working list differs from the saved list; click opens the diff panel (SCR-06 style) |
| Toast on bulk actions | "Sorted 14 mods" with an Undo action |
| Changed rows | Rows changed by the last command get a brief non-animated outline that fades by opacity only; none under reduced motion |
| History popover (M3) | Lists the stack so the user can jump to any point |
| Trash deletion | Toast with Undo for 10 s; profile deletion offers 10 s undo |

### 4.7 Progress patterns

| Pattern | When | Examples |
|---|---|---|
| Inline in place | Short, tied to one region | Thin "refreshing" bar in the status strip during rescan; per-row dataset progress; "Calculating" for folder size; skeleton rows |
| Panel progress with Cancel | A computation the user is waiting on | Sort preview bar; import preview; pre-launch checklist under 300 ms |
| Task centre | Anything that may outlast attention | Scan, deploy, dataset fetch, log parsing, bisect rounds, publish upload (also shown on its own screen) |
| Toast on completion | Outcome with one action | Show diff, Open log, Undo |
| Never | A blocking progress window | RimSort pain point P4 |

### 4.8 Confirmation patterns

Confirm only destructive actions (modal policy, section 1.2). Everything else uses preview, undo or a toast with Undo. Examples that confirm: permanent delete when no trash exists (shows the path), delete profile, remove source (states how many active mods would become missing), reset all settings (typed confirmation), "Launch anyway" (typed confirmation, states that the game will rewrite the list), publish (one explicit click naming the target). Examples that do not confirm: Remove from list, Clear (undoable), Sort apply, Restore snapshot (undoable), switching profile when clean, Delete mod to trash (Undo toast).

### 4.9 Empty-state pattern

Every empty state is a designed card with an illustration slot, one sentence of what is missing, one sentence of consequence or help, exactly one primary action, and at most one secondary link (GD-080, frontend architecture 12.2). Examples in the catalogue: "RimWorld not found" (Choose install, Run detection), "No mods found in the sources" (Add folder), "Only official content is active" (Import list, drag hint), "No mods match" (Clear filter), "No profiles yet" (Create from current), "Datasets not fetched yet" (Fetch now), "Game log not found" (Choose file), "All workshop mods are current". An offline state with a cached copy never uses error styling.

## 5. Data realism guide

Designs must look like the owner's real library, not like lorem ipsum. The numbers and examples below come from the specifications and the owner's list (610 active mods, 573 of them found on the Workshop; the owner's mod folder at depth 2 holds 22 mod roots; a library folder count of 743 is the watcher target in MM-041). Invented items are marked "fictional".

### 5.1 Names, ids and scripts

| Case | Real example from the sample rows | Design implication |
|---|---|---|
| Short | `Harmony`, id `brrainz.harmony`, 1.2 to 1.6 | Rows must not look empty; right slots stay aligned |
| Long Latin | `Expanded Prosthetics and Organ Engineering - Forked` (51 characters, `vat.epoeforked`) and `Vanilla Fishing Expanded - Fishing Treasures AddOn` (50, `vanillaexpanded.vcefaddon`) | Ellipsis by CSS; full name in the tooltip and the detail header, which wraps to two lines |
| Bracket prefixes | `[ARY] Butchering Drops Skeleton [1.5 - 1.6]` | Do not parse or strip prefixes |
| Mixed CJK and Latin | `[TW1.6]堂丸贴图重置~制成品 Tang's_Retexture_Manufactured` (id `tw.tangs.retexture.manufactured`, author TangW); `[JM]Disable Pseudo Translate 禁用泰南语` (id `jm.disstynan`, author `術滅(Jutsumetsu)`) | Font stack needs a CJK fallback after Barlow; line height tolerates tall glyphs inside 28 px rows; truncation never splits a surrogate pair; sorting and search work with non-ASCII (MM-009) |
| Id shapes | up to 41 characters (`tw.tangs.retexture.manufactured`), lower case, dotted, four or more segments (`oskarpotocki.vanillafactionsexpanded.core`, `arylice.rimworld.butcheringdropsskeleton`); official ids `ludeon.rimworld`, `ludeon.rimworld.royalty`, `.ideology`, `.biotech`, `.anomaly`, `.odyssey` | Ids always in Azeret Mono; copy button; no wrap in lists |
| Many authors | `Oskar Potocki, XeoNovaDan, Orion, Kikohi, Taranchuk, Sarg Bjornson, Erdelf` (7), and rows with an empty author | Authors truncate in rows, wrap in detail; handle empty |
| Version lists | `1.0,1.1,1.2,1.3,1.4,1.5,1.6` (HugsLib) down to `1.5,1.6` | Chip shows current version highlight; tooltip lists all |
| Counts | Dependencies 0 to 6 per mod, loadAfter 0 to 6 (HugsLib has loadAfter 6) | Relations tab must handle 0, 1 and many |
| Realistic head of list | 1 Prepatcher (`zetrith.prepatcher`), 2 Harmony (`brrainz.harmony`), 3 to 8 the six official packages, 9 Adaptive Storage Framework, 13 HugsLib (`unlimitedhugs.hugslib`), 14 Vanilla Expanded Framework | Use this order for the Active list mock |

### 5.2 Cardinality cases

Every list and panel is designed for 0, 1 and many, and for the stress case. The stress case is 5,000 rows: both lists virtualised, at most 3,000 DOM nodes each, scroll position indicator, counts in headers ("4,312 of 5,000"), search results within 50 ms per keystroke. Also design: 0 diagnostics ("No issues"), 1 diagnostic (single card, no grouping), 400 diagnostics (grouped by severity then code, counters), 7 duplicate groups, 4 missing ids, 22 mods in one custom folder, 12 links in a deploy plan, 100 snapshots, 40-mod block move, 13,212 defs in the explorer, a 100,000-line log, a 3,000-file publish plan.

### 5.3 Realistic diagnostics (codes from the LO-019 catalogue)

Message templates are the catalogue's. Mod names come from the sample list, but the pairings, directions and counts in the examples are illustrative and must not be read as claims about those mods.

| Code | Severity | Example message | Suggested actions |
|---|---|---|---|
| `list.missing-dependency` | Error | "Vanilla Backgrounds Expanded requires Vanilla Expanded Framework, which is not active" (state `inactive-available`) | Enable dependency; Mute |
| `list.incompatible` | Error | "{mod} is incompatible with {other} (declared by community rules)" (fictional pair) | Disable one of them; Mute |
| `list.order-hard` | Error | "Harmony must load before HugsLib (force rule from About)" (illustrative direction) | Move after or before X; Re-sort |
| `list.order-soft` | Warning | "Cherry Picker should load after XML Extensions (community: example comment)" | Move; Sort; Suppress the rule |
| `list.version-mismatch` | Warning | "A fictional mod lists 1.4, 1.5, not 1.6" | Look for update; Mute |
| `list.duplicate-id` | Error | "Two copies share the package id {id}" | Keep one (opens Duplicates) |
| `list.replacement-available` | Info | "{mod} has a suggested replacement: {replacement}" | Use replacement; Mute |
| `list.missing-properties` | Warning | "{mod} has no usable package id" or "declares no supported versions" | Open About.xml; Mute |
| `list.core-inactive` | Error | "Core is not active" | Enable Core |
| `list.unresolved-id` | Error, blocks save | "{id} is in the list but no installed mod has it" | Remove from list; Locate; Subscribe |
| `list.pinned-conflict` | Warning | "{mod} is pinned at 14, but {other} must load before it" | Release pin; Move |
| `deploy.not-visible` | Error, blocks save and launch | "{mod} is in My mods, which the game cannot see until it is linked" | Link now; Disable |
| `deploy.source-offline` | Error, blocks | "{mod}'s folder {source} is not reachable" | Reconnect and rescan; Disable |
| `deploy.modsconfig-version` | Warning reading, Error writing | "ModsConfig.xml was written for 1.5; the game runs 1.6" | Re-stamp on save |
| `sort.cycle` | Warning (preview) | "Rules {members} contradict each other; {dropped} was ignored" | Open rule editor; Suppress |
| `sort.tier-conflict` | Warning (preview) | "{mod} is flagged {flag} but must load {direction} {other}" | Change flag; Suppress |
| `sort.unmapped` | Info (preview) | "{mod} could not be ordered by rules and was placed by name" | Fix About.xml |
| `dataset.rejected` | Warning | "Update rejected: entries dropped from 631 to 12. Kept the last good copy." | Inspect; Force accept |

Show a mod with several messages at once (an error, a warning and an info) so the badge shows the highest severity with a count.

### 5.4 Realistic item designer content

The acceptance mod is the owner's Gewehr 41 project (package id `oh.weapons.gewehr41`, folder name `[OH] Gewehr 41`), which the roadmap converts to a CE patch in M5. The specifications do not record its stat values, so every number of the new rifle below must be marked "illustrative" in the design and replaced by the values read from the mod's defs in the implementation. Reference values are real, from the vanilla analysis:

| Reference | Damage x burst | AP | Cycle (s) | Nominal DPS | Hit factor at 3, 12, 25, 40 tiles |
|---|---|---|---|---|---|
| Revolver | 12 x 1 | 0.18 | 1.9 (0.3 warmup, 1.6 cooldown) | 6.32 | 0.80, 0.75, 0.55, 0.40 |
| Assault rifle | 11 x 3 | 0.165 | 3.033 (1.0 warmup, 1.7 cooldown, 2 gaps of 10/60 s) | 10.88 | 0.60, 0.70, 0.65, 0.55 |

The revolver costs 135 silver. Design the form for a rifle placed between the two anchors: tier Industrial, role rifle, parent base chosen from the install's weapon bases. Fields use the ranged vanilla table of IT 6.1 (projectile damage, AP implied as 0.015 x damage when unset, burst count, ticks between shots, warmup, cooldown, range, four accuracy values, mass, work to make flagged "ask", cost list, market value computed). The optional CE patch (IT 6.2, shown only when its toggle is on) adds Bulk (flagged with the ask marker), magazine size always flagged as a question, ammo set picker, and a read-only damage and penetration lookup with the meets-armor table.

Fit meter content: P50 and P80 factors from the harness examples (vanilla ranged range x1.10 and x1.15; mass x1.17 and x2.68; CE ranged range x1.14 and x1.33, Bulk x1.25 and x1.46, magazine x1.18 and x2.0). A value inside P50 is green ("typical"), inside P80 blue ("plausible"), outside amber ("unusual"), never red, with the pool size beside it (19 reference vanilla weapons; "bands are rough" below about 15 items in the role). Typicality: peers 8, 9, 10, 11, 12, 14 give a score of 1.0 for a value of 13 and 0.003 for 30, with the sentence "low means unusual, not wrong". Rank readout: "heavier than 12 of 19 reference rifles".

Quiz stepper (questions from item-balance-math section 8): S1 "Which tech level is it?" (options from the pool, Neolithic to Archotech); S2 "What is it?" (role list); then "Is your item weaker, about the same, or stronger?" against an anchor card of the assault rifle with its real numbers; "How does it fire?" (vanilla: single, short burst 2 to 4, long burst 5 to 9, belt 10 or more); "How far does it shoot?" (under 20, 20 to 27, 27 to 35, over 35 tiles); "Compared with the assault rifle, is its mass lighter, similar or heavier?" (below 85 percent, similar, above 118 percent of the anchor's shown mass). Header text: "question 3 of about 6", with "about 6 questions" announced up front (measured mean 6.4 for vanilla ranged). Show Back, Skip to result and Use what I have.

### 5.5 Other realistic content

Datasets: five cards with the licence notes of CD-003 ("No licence file found upstream: fetched on your machine only" for community rules, SteamDB and game versions; "MIT, credit: Mlie" for Use This Instead and No Version Warning). Source cards: the install Mods folder, one Workshop library, and the owner's folders (a mods folder on an external drive, 22 mods at depth 2, with the template repository flagged "not a mod"). Publisher: the Lone Wolf Weapon Package with `.git` and `Raw Assets` excluded by default rules, sizes left as placeholders because none were measured. Steam helper states: running, missing.

## 6. Microcopy guide

### 6.1 Tone

Calm, exact, never blaming. State what is true, then what the app did or will do, then the one next step. Name the mod, folder or file; give numbers; no exclamation marks, no humour in errors, no "oops", no "failed to" without a cause. Say "RimStudio" for the app and "the game" for RimWorld. Use the user's words for objects (mod, list, folder). Errors carry a short cause, a fix button and Copy diagnostics. Warnings that the user may knowingly accept are phrased as facts with an option to proceed ("Launch anyway"). Offline is a state, not a failure: "using last copy from 2026-10-01". Messages come from catalogues, with plural and gender forms through the ICU layer, and slots named as in the code catalogue (`{mod}`, `{other}`, `{layer}`). Avoid the hyphen-as-dash and any dash-like punctuation in copy; use a comma, a colon or a new sentence. Sentence case for everything except product names.

### 6.2 Twenty-five example messages

| # | Moment | Message | Action label |
|---|---|---|---|
| 1 | Launch blocked: a link is broken (`deploy.link-broken`, GD-066, GD-068) | "RimWorld was not started. The folder for Gewehr 41 moved (it was D:\, now E:\). The game would remove it from your list." | Fix automatically |
| 2 | Launch blocked: drive offline (`deploy.source-offline`) | "RimWorld was not started. The drive holding My mods is not connected, so 14 active mods cannot be found." | Reconnect and rescan |
| 3 | Block dialog, third choice | "Launch anyway starts the game with this list. The game will rewrite ModsConfig.xml without the 14 mods it cannot find. A backup is saved first. Type LAUNCH to confirm." | Launch anyway |
| 4 | Collapsed dataset download quarantined (CD-011) | "Update rejected: entries dropped from 631 to 12. Kept the last good copy." | Inspect |
| 5 | Dataset newer format | "This data uses a newer format. Update RimStudio to use it. The last good copy is still in use." | Check for updates |
| 6 | Datasets offline | "Last checked 3 days ago. Sorting and checks use the last copy." | Refresh now |
| 7 | Unresolved active id (`list.unresolved-id`) | "4 mods in the list are not installed. Saving is paused until you remove them, find them, or accept that the game will drop them." | Review missing mods |
| 8 | Sort result with reasons (MM-017, LO-025) | "Sorted 14 mods. Most moved because Harmony loads before HugsLib (About), and 3 moved after Vanilla Expanded Framework (community rule)." | Undo |
| 9 | Sort already sorted | "Already sorted. Nothing would move." | Close |
| 10 | Sort cycle (`sort.cycle`) | "Two rules contradict each other, so one was ignored. The ignored rule is listed with its source." | Open rule editor |
| 11 | Preflight failure (WP 5.3) | "Publishing is paused: 2 problems need fixing. About.xml has no packageId, and the preview image is not a PNG or JPEG." | Open About editor |
| 12 | Publish: Steam not running (`publish.steam-not-running`) | "Steam is not running or not signed in. Start Steam, sign in, and try again." | Retry |
| 13 | Publish: needs agreement (WP 5.8) | "Uploaded. The item stays hidden until you accept the Workshop agreement on its page." | Open Workshop page |
| 14 | Publish: created, not submitted | "An empty Workshop item (id) was created earlier. Publishing will update it." | Continue |
| 15 | CE not installed (items toolkit, item balance math section 10) | "Combat Extended is not in your reference set, so the optional CE patch is off. Vanilla design works fully." | Choose reference set |
| 16 | CE lint without CE data | "Not checked: CE not installed." | Choose reference set |
| 17 | Hard conflict (`list.incompatible`) | "Mod A and Mod B are incompatible (declared by Mod A's About.xml). Disable one of them." | Disable Mod B |
| 18 | Hard order rule on drop (MM-012) | "Moving here breaks a rule: HugsLib must load after Harmony." | None (shown on the insertion line) |
| 19 | Official content refused (MM-012) | "Core stays first. The game enforces this order." | None |
| 20 | Game running (`game.state`) | "RimWorld is running. Saving waits until it closes. You can keep editing." | Check again |
| 21 | Game changed the list (`game.list-changed`) | "The game changed the list while it ran. Review the changes before they replace yours." | Show diff |
| 22 | Duplicate packages (MM-027) | "Two copies share the id {id}. RimStudio uses the Workshop copy. Nothing on disk was changed." | Pin the other copy |
| 23 | Custom folder rejected (`deploy.source-overlap`) | "This folder contains another source, so mods would be counted twice. Remove the other folder or choose a deeper path." | Choose another folder |
| 24 | Detection not found (GD-080) | "RimWorld was not found. Places checked are listed below. You can still use your own mod folders; launching is off until the game is set." | Choose folder |
| 25 | Settings cannot be saved (GD-080) | "Settings will not be saved. The data folder is read-only." | Open data folder |

### 6.3 Further wording rules

Button labels are verbs or verb phrases (Add folder, Save and Run, Use replacement). Destructive buttons name the object ("Delete profile"). Counts always precede nouns with plurals ("1 mod", "14 mods"). Paths and ids appear in Azeret Mono. The native safe-graphics dialog (SS-04) uses the wording "RimStudio did not finish starting last time. Start in safe graphics mode?" with the buttons "Start in safe graphics mode" and "Start normally", since it is drawn before the webview exists and must be understandable with a controller.

## 7. Accessibility and performance constraints for designers

Sources: MM-038, MM-005, frontend architecture 9.4, 11, 14, webview and IPC performance research budget, cross-platform 8.

### 7.1 Accessibility targets

| Topic | Target |
|---|---|
| Text contrast | At least 4.5:1 in light, dark and user themes; the Appearance accent field refuses an accent that fails, with the numbers |
| Non-text contrast | At least 3:1 for icons, borders of inputs, badges and chart marks |
| Focus ring | Visible on every interactive element, at least 3:1 against both adjacent colours, never removed; focus returns to the opener when a panel or dialog closes; no keyboard traps |
| Colour is never alone | Every badge has an icon or letter, every diagnostic has an icon shape and text, every diff line has a `+` or `-`, fit meter states have text |
| Target sizes | Rail buttons at least 32 px; Roomy density primary controls 40 px; nothing below 24 px in any density (Steam Deck) |
| Reduced motion | Honour `prefers-reduced-motion`: no drag ghost animation, no list transitions, no pulsing StatusDot; state changes by opacity only |
| Contrast and zoom | Honour `prefers-contrast`, OS text scale; layout holds at 200 percent zoom at 1280 x 800 by falling back to the drawer layout |
| Semantics | `listbox` with `aria-setsize` and `aria-posinset` for virtualised lists, live region for moves, `role="dialog"` with labelled titles, toasts `role="status"` and errors `role="alert"`, every icon button named |
| Logical CSS | Logical properties are used so right-to-left is possible later (not a v1 goal) |
| Testing | axe in Playwright on each screen state; manual NVDA, VoiceOver and Orca pass per release |

### 7.2 What to avoid, and what to use instead (WebKitGTK and the webview budget)

| Avoid | Reason | Use instead |
|---|---|---|
| `backdrop-filter` and heavy blur | Costly on WebKitGTK, can blank the window on some stacks (safe graphics exists for this reason) | Opaque or near-opaque surfaces; a small overlay may use a flat semi-transparent colour |
| Large or stacked box shadows | Repaint cost, especially on scroll | One-pixel borders and a flat `--rs-surface-raised` step; at most a small single shadow on popovers |
| Per-row filters, masks, `mix-blend-mode`, per-row gradients | Multiply across 3,000 rows | Flat colours; group colour as a 3 px bar; row states as background and border |
| Animating layout properties | Layout thrash | `transform` and `opacity` only |
| Layout reads in render, per-row `ResizeObserver` | Main thread cost | One `ResizeObserver` per container; CSS ellipsis for truncation |
| Large raster textures and SVG filter noise | Memory and decode cost | Cheap CSS-only blueprint texture: a grid from `linear-gradient` lines in the grid line colour, corner ticks from borders or pseudo-elements; use `background-attachment` only on large static regions, never per row |
| WebGL, WebGPU, `SharedArrayBuffer` | Not safe on the target webviews | Chart.js canvas under 200 points, Cytoscape only in the lazy Relations graph |
| Images larger than needed | Decode cost | Thumbnails at most 256 px in lists, lazy and async through `rsimg` |
| Content-dependent row heights | Breaks virtualisation | Fixed row heights 28, 34 or 40 px; wrapped text only in detail, drawers and panels |

### 7.3 Rules the design must respect for virtualisation and speed

1. Row height is fixed per density; names truncate by CSS ellipsis; icon slots are at fixed x positions so nothing jumps, which was a RimSort weakness (P15).
2. At most 3,000 DOM nodes in one list and under 10,000 on a page; design no row with more than about ten elements.
3. Input to paint under 50 ms at 3,000 rows and frame time under 16 ms while scrolling and dragging; no hover effect that needs layout.
4. Every design shows states with skeletons rather than spinners, because cached rows paint at once and the list is interactive within the budget.
5. Fonts: Barlow, Barlow Condensed and Azeret Mono, self hosted, Latin subset by default with `font-display: swap`; design with fallback metrics in mind and a CJK fallback for names; no external font requests.
6. Light and dark are both required and screenshot-tested; user themes are token overrides, so no colour literal appears outside tokens.
7. Blueprint character comes from tokens and components: grid background, corner-tick frames, condensed headings and monospaced labels. It must be removable by a token without touching features.

## 8. Traceability table

Source documents (all links resolve from this folder): [mod manager](../features/mod-manager.md), [game and mod discovery](../features/game-and-mod-discovery.md), [settings](../features/settings.md), [load order and validation](../features/load-order-and-validation.md), [community datasets](../features/community-datasets.md), [modding workspace](../features/modding-workspace.md), [items toolkit](../features/items-toolkit.md), [item balance math](../features/item-balance-math.md), [workshop publishing](../features/workshop-publishing.md), [frontend architecture](../architecture/frontend-architecture.md), [IPC and state](../architecture/ipc-and-state.md), [RimSort feature and UX inventory](../research/rimsort-feature-and-ux-inventory.md), [webview and IPC performance](../research/webview-and-ipc-performance.md), [frontend stack research](../research/frontend-stack-research.md), [roadmap](../roadmap.md).

| Requirement | Screens and shell parts |
|---|---|
| MM-001 First-run wizard | SCR-01, SS-01 |
| MM-002 Sources and custom folders | SCR-22, SCR-15, SCR-01 |
| MM-003 Startup, scan, refresh | SCR-02, SH-06 |
| MM-004, MM-005 Layout, responsive | SCR-02, section 1.4 |
| MM-006 Row anatomy | SCR-02 (ModRow) |
| MM-007 Mod detail | SCR-03 |
| MM-008 Actions rail | SCR-02 (ActionsRail) |
| MM-009, MM-010 Search, filters, counters | SCR-02 (QueryBar, ListHeader) |
| MM-011 to MM-013 Selection, drag, keyboard | SCR-02, section 4 |
| MM-014, MM-015 Undo, dirty, diff | SCR-02, SCR-06, section 4.6 |
| MM-016 Diagnostics | SCR-07, SCR-02 |
| MM-017, MM-018 Sort, explain | SCR-05, SCR-04 |
| MM-019, MM-020 Groups, tags, colours, notes | SCR-02, SCR-03 |
| MM-021, MM-022 Profiles, history | SCR-11 |
| MM-023, MM-024 Import, export | SCR-12, SCR-13 |
| MM-025 to MM-027 Dependencies, missing, duplicates | SCR-08, SCR-10, SCR-09 |
| MM-028 Updates | SCR-20 |
| MM-029 to MM-031 Launch, checks, save | SCR-14, SCR-06, SS-03 |
| MM-032 Bisect | SCR-19 |
| MM-033 to MM-035 Task centre, palette, logs | SCR-49, SCR-50, SCR-26 |
| MM-036 to MM-038 States, shortcuts, accessibility | all screens, SCR-24, SCR-53, section 7 |
| MM-039, MM-043, MM-045 Datasets, migration, rule editor | SCR-16, SCR-18, SCR-25, SCR-17 |
| MM-040, MM-041 Actions, watching | SCR-02 context menus, SH-06 |
| MM-042 Performance | section 7.3 |
| MM-044, MM-046 Adding mods, settings | SCR-02 (Add mod menu), SCR-21 |
| GD-001 to GD-005, GD-010 to GD-012 Detection | SCR-01, SCR-21 |
| GD-020, GD-030, GD-031 Sources and visibility | SCR-22, SCR-09 |
| GD-040 to GD-045 Custom folders | SCR-22 |
| GD-051, GD-052 Duplicates | SCR-09 |
| GD-060 to GD-070 Link farm and launch | SCR-15, SCR-14, SS-03 |
| GD-080 Empty and error states | SCR-01, SCR-22, shell states, section 4.9 |
| ST-010, ST-001, ST-080 Settings layout | SCR-21 |
| ST-020 to ST-022 Sources editor | SCR-22, SCR-15 |
| ST-030 Datasets | SCR-16 |
| ST-040, ST-041 Appearance, performance | SCR-23, SCR-25 |
| ST-050, ST-051 Launch, safe graphics | SCR-25, SS-04 |
| ST-060 Diagnostics and data | SCR-25, SCR-51 |
| LO-003 to LO-009 Rules and edges | SCR-04, SCR-17 |
| LO-012 to LO-016 Sort | SCR-05 |
| LO-018 to LO-022 Validation, muting, fixes | SCR-07 |
| LO-023, LO-024 Explain | SCR-04, SCR-03 |
| LO-025, LO-026 Sort preview, apply | SCR-05 |
| CD-001 to CD-012 Dataset panel and failure | SCR-16, SS-02 |
| CD-013 to CD-016 User rules, suppress | SCR-17 |
| CD-017 to CD-019 Import, export patch | SCR-17, SCR-25 |
| WS-001 Scaffolder | SCR-30 |
| WS-002, WS-011 About editor, BBCode | SCR-31, SCR-44 |
| WS-003 LoadFolders | SCR-32 |
| WS-004 Def Explorer | SCR-27 |
| WS-005 Patch tester | SCR-28 |
| WS-006 Validator | SCR-29, SCR-40 |
| WS-007 Log analyser | SCR-26 |
| WS-008 Save inspector | SCR-34 |
| WS-009 Dev launcher | SCR-33 |
| WS-010 Staging and build | SCR-42 |
| IT-020 Item list | SCR-35 |
| IT-021 to IT-025 Form, sliders, charts, fit meter | SCR-36 |
| IT-026 Quiz | SCR-38 |
| IT-027 Matrix | SCR-37 |
| IT-028, IT-029, IT-014 Output, write | SCR-39 |
| CE update and lint | SCR-40 |
| WP-001, WP-031 Eligibility, duplicate ids | SCR-41 |
| Publishing flow and state machine | SCR-42 to SCR-48, SH-07 |
| R1 Stack and cross-platform | section 1.4, 1.5, 7 |
| R2 Modular code | section 3 (shared library) |
| R3, R4 Detection, custom folders | SCR-01, SCR-22, SCR-15 |
| R5 Manager plus toolkit, performance | SCR-02 to SCR-34 |
| R6 Datasets, auto-fetch | SCR-16, SCR-17, SCR-25, SS-02 |
| R7 Item designer, CE, quiz | SCR-35 to SCR-40 |
| R8 Blueprint UI, design prompt | sections 3.1, 7 |
| R9 Workshop publishing | SCR-41 to SCR-48 |
| R10 JSON and JSONC, XML at the boundary | SCR-21 (JSONC keys), SCR-25 |
| R11 Licence hygiene | SCR-16 (licence notes), section 5.5 |
| R12 Docs conventions | this document |

## 9. Open questions for the design stage

1. Toast stack size: mod-manager MM-033 says at most 3 visible with 5 s default; frontend architecture 7.4 says at most 4 with 5 s for success and info and 8 s for warnings. Which limit applies? This document uses 3 and the longer warning duration.
2. Sort shortcut: mod-manager.md uses `Ctrl+Shift+T`; frontend architecture 7.3 proposes `Ctrl+Shift+S`. This document follows mod-manager.md (the single registry decides at implementation).
3. Milestone numbering: the toolkit specification labels the Def Explorer as M3, while the roadmap numbers the toolkit foundation as M4. This document uses roadmap numbering throughout; the specifications need a numbering pass.
4. Navigation depth: the rail lists seven tools (frontend architecture 7.1) while the registry has more ids (`defs`, `patch-tester`, `validator`, `project`, `logs`, `saves`, `designer`, `workshop`). Should Def Explorer, Patch tester and Validator share one Workspace entry with tabs, and Saves live under Project?
5. Where does the issues panel live: docked right on 2560 and wider, a drawer below that, or a popover only? Specifications define the popover and the footer counters but not a persistent panel.
6. Quiz stepper form: items-toolkit calls it a modal stepper, not a wizard gate, while the modal policy limits modals to destructive confirmations. This document treats it as a stepper over the form; confirm.
7. Status colour budget (resolved): red is reserved for error-severity problems and destructive confirmations. The fit meter therefore uses green (typical), blue (plausible) and amber (unusual); the items specifications were updated to match.
8. Texture and decoration: how strong should the blueprint grid and corner ticks be in the Active list versus panels, given the CSS-only and no-filter limits?
9. Group colours and mod colours: how many theme-aware swatches, and how do they stay distinct from severity colours in both themes?
10. Compact list second line: in Roomy and 2560 modes, may rows show a second metadata line (author, version list) without breaking the fixed-height rule?
11. CJK font choice: which fallback family is bundled or relied upon per platform, given Barlow has no CJK coverage and the owner's list contains Chinese and Japanese names?
12. Titlebar: custom title bar on all three platforms or only where the adapter reports it works, and how it interacts with game mode on Steam Deck.
13. Illustration style for empty states (the specifications require a slot but no style); one family or per-tool?
14. Update dialog and crash notice wording and placement need an owner review; the specifications give only triggers.
15. The Gewehr 41 stat values are not in the documents: who supplies the illustrative numbers for the design mock (read from the owner's mod defs)?
16. Relations graph (Cytoscape) styling and size limits for a mod with many edges; the specification gives only the lazy chunk rule.
17. Detail column behaviour with a multi-selection of thousands of rows: bulk summary content and which bulk actions to expose first.
