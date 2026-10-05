# Settings: model and screen specification

This document specifies RimStudio's settings: which files hold them, every setting with its type, default, scope and place in the UI, the anatomy of the Settings screen (search, mod sources and custom folders editor, datasets entry point, appearance, performance, launch, diagnostics and data actions), importing from RimSort, validation and error display, and migration between versions (requirements R3, R4, R6 and R10). It re-homes and drops RimSort's 98 global and 13 per-instance settings as the [settings catalogue](../research/rimsort-settings-catalog.md) recommends, and stores all of it as JSONC and JSON. Path detection and the custom folder behaviour behind the editor are specified in [game and mod discovery](game-and-mod-discovery.md); persistence mechanics are in [data and persistence](../architecture/data-and-persistence.md).

Status: draft, section 17 records the 0.1.0 backend as built | Last updated: 2026-10-05

## Contents

1. [Principles](#1-principles)
2. [Conventions](#2-conventions)
3. [Files and roots](#3-files-and-roots)
4. [Settings tables](#4-settings-tables)
5. [Screen anatomy](#5-screen-anatomy)
6. [Mod sources and custom folders editor](#6-mod-sources-and-custom-folders-editor)
7. [Datasets entry point](#7-datasets-entry-point)
8. [Appearance and performance](#8-appearance-and-performance)
9. [Launch settings and safe graphics](#9-launch-settings-and-safe-graphics)
10. [Diagnostics and data folder actions](#10-diagnostics-and-data-folder-actions)
11. [Import from RimSort](#11-import-from-rimsort)
12. [Validation and error display](#12-validation-and-error-display)
13. [Migration between versions](#13-migration-between-versions)
14. [RimSort disposition table](#14-rimsort-disposition-table)
15. [Commands and tests](#15-commands-and-tests)
16. [Owner decisions and open points](#16-owner-decisions-and-open-points)
17. [As built in the 0.1.0 backend](#17-as-built-in-the-010-backend)

## 1. Principles

1. A setting exists only if a user will plausibly change it. Anything the game or the data decides is not a setting. The last column of the tables says how often a user needs each one (everyday, occasional, rare) so the screen can order them; nothing is hidden, because JSONC users can edit any key and the search finds it.
2. Defaults are never written to disk. Only keys the user changed exist in the files. This keeps diffs small, lets defaults improve between versions, and makes a fresh install produce an empty (or absent) settings file ([data and persistence](../architecture/data-and-persistence.md) section 3.1).
3. Files are JSONC where a person may edit them (comments survive because the app edits through CST operations, D-025) and JSON where only the app writes. No XML, TOML, YAML or SQLite for settings (R10, I-03).
4. Setters never write. A store flushes debounced (at most one write per second per file), on blur and on exit, atomically, with retention backups ([data and persistence](../architecture/data-and-persistence.md) section 7 and 8.3).
5. Changes emit fine-grained events by key path, and views recompute only what the key affects. A settings change never triggers a full list rebuild unless the key is one of the few that change the library (paths, sources, scan depth).
6. Secrets never live in a settings file, a log, an export or an IPC response; DTOs carry an "is set" flag only (I-17 and the [secrets section](../architecture/data-and-persistence.md) 13).
7. Every setting has a search entry (title, description, aliases, key path), so there is no tab hunting (RimSort has 98 keys over 9 tabs and no search).
8. The screen always shows the effective value and its source (default, `settings.jsonc`, `workspace.jsonc`, environment or command line) and a link to open the file, so JSONC editing and the UI feel like one thing.

## 2. Conventions

Requirement ids are `ST-nnn`. Keys use camelCase dotted paths (`library.scanThreads`); on disk they appear as nested objects. Types: `bool`, `int`, `number`, `string`, `enum(a|b)`, `path` (absolute text path, validated), `list<T>`, `object`. Scope: `app` is `settings.jsonc`; `workspace` is `workspace.jsonc`; `secret` is the credential store; `profile` is stored in a profile document and the workspace holds only the default; `state` is remembered automatically and has no UI. "Needed" is how often a user changes it: E (everyday or first-run), O (occasional), R (rare, for support or experts).

The enums are kebab-case strings. A value that fails validation is kept in the file and reported (section 12); the app uses the default for that run.

## 3. Files and roots

Three user-facing documents and one bootstrap file, resolved through `rimstudio-io::DataRoots` (config, data, cache, logs; portable mode when a `rimstudio.portable` marker exists, which moves everything under `./data`; D-024):

| File | Root | Format | Content | `schemaVersion` | Written by |
|---|---|---|---|---|---|
| `settings.jsonc` | config | JSONC | App behaviour: language, theme, density, shortcuts, performance, library display, diagnostics, updates, designer mode | 1 | settings use case via CST edits |
| `workspace.jsonc` | config | JSONC | The person's setup: path overrides, mod sources, custom folders, launch defaults, datasets, active profile and install. This is the file a portable install carries to another machine | 1 | same |
| credential store (fallback `secrets.json`, mode 0600) | OS store, config root fallback | OS store, JSON | Secrets only; v1 stores none ([secrets section](../architecture/data-and-persistence.md) 13, D-030) | 1 (fallback file) | `CredentialStore` port |
| `launch.jsonc` | config | JSONC | Fourth config file (D-077). Bootstrap only: graphics environment variables and the safe-graphics choice, read before the webview exists (section 9.2) | 1 | the person, the crash dialog |
| `themes/<name>.jsonc` | config | JSONC | User themes (token overrides); the app reads and offers export | 1 | the person |

The Steam detection research proposed a separate `paths.jsonc`; D-030 folds it into the `paths` section of `workspace.jsonc`, with the shape unchanged ([data and persistence](../architecture/data-and-persistence.md) section 3.1). View state (sort keys, scroll positions, panel sizes, window geometry, dividers, custom colours) is not a setting: geometry is remembered automatically by the shell and view state is a per-viewer convenience in browser storage that the app works without; durable list structure (groups, dividers, colours) lives in `userdata/groups.json` and `userdata/mod-meta.json`.

Every JSONC file starts with `"$schema"` (a relative path to the committed generated schema, D-027) and `"schemaVersion"`. Unknown keys are preserved on every edit, so a file touched by a newer build and opened by an older one loses nothing.

### 3.1 Example `settings.jsonc`

```jsonc
{
  // Generated schema: gives editors completion and validation.
  "$schema": "./schemas/settings.schema.json",
  "schemaVersion": 1,

  // Only keys that differ from the defaults are written.
  "language": "en",
  "appearance": {
    "theme": "dark",            // light | dark | system | the name of a file in themes/
    "accent": "amber",          // a preset name or a #rrggbb colour
    "density": "compact",       // standard | compact | touch
    "fontScale": 1.1,
    "reduceMotion": "system"    // system | on | off
  },
  "library": {
    "scanThreads": 0,           // 0 = automatic (at most 8)
    "watch": { "mode": "auto", "pollIntervalSeconds": 60 },
    "recentlyUpdated": { "enabled": true, "days": 3 }
  },
  "sorting": {
    "dependenciesAsLoadAfter": true
  },
  "shortcuts": {
    // command id to chord; absent = default
    "list.save": "Mod+S",
    "library.refresh": "F5"
  },
  "tools": {
    "textEditor": { "command": "code", "folderArgs": ["{path}"], "fileArgs": ["--goto", "{path}:{line}"] }
  },
  "designer": { "mode": "calibrated" },
  "updates": { "channel": "stable", "checkOnStart": true },
  "logging": { "level": "info" }
}
```

### 3.2 Example `workspace.jsonc`

```jsonc
{
  "$schema": "./schemas/workspace.schema.json",
  "schemaVersion": 1,

  // Overrides win over detection. An override that fails validation is kept
  // and reported; detection fills in for that run only. Nothing deletes it.
  "paths": {
    "gameInstall": { "path": "/games/RimWorld", "pinned": true },
    "userDir": null,                 // null = detected
    "steamRoot": null,
    "extraWorkshopDirs": [],
    "ignoredInstalls": ["gog:/home/u/GOG Games/RimWorld/game"]
  },

  // The built-in sources are listed only to allow reordering or disabling.
  "modSources": [
    { "id": "game-data",  "kind": "game-data",  "enabled": true },
    { "id": "game-mods",  "kind": "game-mods",  "enabled": true },
    { "id": "workshop-0", "kind": "workshop",   "enabled": true }
  ],

  // Any number of custom folders (R4). Order is the default priority.
  "customModFolders": [
    {
      "id": "cf_7f3a",
      "path": "/run/media/me/projects/RimWorld Mods",
      "label": "My mods",
      "enabled": true,
      "layout": "auto",
      "scanDepth": 2,
      "watch": true,
      "readOnly": false,
      "link": "auto",
      "volumeHint": { "mount": "/run/media/me/projects", "label": "projects" }
    }
  ],

  "deploy": { "copyIgnore": [".git", "Source", ".vs", "obj", "bin"], "copyWarnMegabytes": 500 },

  // Global launch defaults; a profile may override method and arguments.
  "launch": { "method": "auto", "arguments": "", "saveDataFolder": null },

  "datasets": {
    "autoUpdate": true,
    "items": {
      "communityRules": { "enabled": true, "refreshHours": 24 },
      "useThisInstead": { "enabled": true, "url": null }
    }
  },

  "activeInstall": "steam:/home/u/.local/share/Steam",
  "activeProfile": "pr_default"
}
```

`lastSeen` and similar state fields in custom folder entries are written by scans into the cache root, not into this file, so the file only changes when the user changes it.

## 4. Settings tables

### ST-001 The complete settings catalogue (M1, M2)

Statement: the tables below are the complete set of RimStudio settings for v1 and the milestones noted. Section 14 maps every RimSort key to a row here or records why it is dropped.

### 4.1 App settings (`settings.jsonc`)

| Key | Type | Default | UI location | Description | Needed |
|---|---|---|---|---|---|
| `language` | enum (locale code from the shipped catalogues) | system locale if shipped, else `en` | Appearance, Language | UI language; catalogues are flat JSON and optional languages load lazily; applies live | E |
| `appearance.theme` | string: `light`, `dark`, `system`, or a user theme name | `system` | Appearance, Theme | Colour scheme; user themes are JSONC token overrides in `themes/` | E |
| `appearance.accent` | string (preset name or `#rrggbb`) | `amber` (placeholder, final value from the UI design) | Appearance, Accent | The single accent variable; contrast against both themes is checked and a failing value is refused with a message | O |
| `appearance.density` | enum(standard\|compact\|touch) | `standard` | Appearance, Density | Row height and spacing; `touch` keeps controls at least 40 CSS pixels and no density goes below 24 ([cross-platform](../architecture/cross-platform.md) section 8) | E |
| `appearance.fontScale` | number 0.8 to 1.6 | 1.0 | Appearance, Text size | Scales the root font size; replaces RimSort's font family and size pair (fonts are self-hosted, D-055) | O |
| `appearance.reduceMotion` | enum(system\|on\|off) | `system` | Appearance, Motion | Disables transitions; follows the OS setting by default | O |
| `appearance.colourMode` | enum(background\|text) | `background` | Appearance, Mod colours | Mod colour fills the row background or colours the name (RimSort `color_background_instead_of_text_toggle`) | O |
| `appearance.richText` | bool | true | Appearance, Descriptions | Render game rich text and BBCode in descriptions; off shows plain text | R |
| `shortcuts` | object: command id to chord | `{}` (defaults come from the shortcut registry) | Shortcuts | Per-command overrides; conflicts are detected live; absent key means default | O |
| `library.scanThreads` | int 0 to 8 | 0 (automatic) | Performance | Scan workers; automatic is `min(8, cores)`, capped at 8 by D-021 and D-069 | R |
| `library.watch.mode` | enum(auto\|poll\|off) | `auto` | Performance, Change detection | `auto` watches roots and metadata and polls where events do not arrive; `poll` never watches; `off` refreshes only on request and on focus ([discovery](game-and-mod-discovery.md) GD-071) | R |
| `library.watch.pollIntervalSeconds` | int 10 to 3600 | 60 | Performance, Change detection | Poll interval for poll-only roots | R |
| `library.watch.debounceMs` | int 100 to 5000 | 500 | Performance, Change detection | Event coalescing window | R |
| `library.checkWorkshopUpdates` | bool | true | Library | Compute Workshop update status from the Steam ACF on refresh; no network call (RimSort `steam_mods_update_check`, there default off because it queried the network) | O |
| `library.recentlyUpdated.enabled` | bool | false | Library | Show a "recently updated" badge on Workshop mods | O |
| `library.recentlyUpdated.days` | int 1 to 90 | 3 | Library | Threshold for the badge | O |
| `library.showSaveComparison` | bool | true | Library | Mark mods that are in or not in the latest save (v1 feature F-100 family in the [mod manager](mod-manager.md)) | O |
| `library.autoLoadPlayerLog` | bool | false | Library, Logs | Open the log viewer on game exit when the log has errors (mod manager MM-029) | O |
| `sorting.dependenciesAsLoadAfter` | bool | false | Sorting | Treat declared dependencies as load-after rules when sorting (RimSort `use_moddependencies_as_loadTheseBefore`) | O |
| `sorting.alternativeIdsSatisfyDependencies` | bool | true | Sorting | An alternative package id satisfies a dependency | R |
| `sorting.checkDependenciesOnSort` | bool | true | Sorting | Offer to add missing dependencies when sorting | O |
| `history.enabled` | bool | true | Library, History | Write an automatic list snapshot on deploy, sort apply and launch | R |
| `history.keep` | int or null | 50 | Library, History | Snapshots kept per profile before thinning; null keeps all; the retention rule is in [data and persistence](../architecture/data-and-persistence.md) section 8.3 | R |
| `tools.textEditor.command` | string | empty (use the OS opener) | Tools | External editor executable for "Edit in editor" | O |
| `tools.textEditor.folderArgs` | list<string> with `{path}` | `["{path}"]` | Tools | Arguments when opening a folder | R |
| `tools.textEditor.fileArgs` | list<string> with `{path}`, `{line}` | `["{path}"]` | Tools | Arguments when opening a file | R |
| `designer.mode` | enum(simple\|calibrated) | `simple` | Tools, Item designer | Whether the item designer uses the generic baseline or the quiz-calibrated one (R7) | E |
| `updates.channel` | enum(stable\|beta) | `stable` | About and updates | Update feed; hidden for system packages and Flatpak where the package manager owns updates | O |
| `updates.checkOnStart` | bool | true | About and updates | Check for a new release at start (RimSort `check_for_update_startup`); never installs without a click | O |
| `logging.level` | enum(error\|warn\|info\|debug\|trace) | `info` | Diagnostics | Log verbosity; replaces RimSort's marker file; takes effect immediately; debug is reverted to info with a notice after 24 hours unless pinned (assumption) | R |
| `window` | object (geometry, maximised, monitor) | absent | none | Remembered automatically: the webview window listener sends a debounced `settings_set_window_state` action and the settings use case writes it (D-076); never shown | state |
| `onboarding.completed` | bool | absent (wizard runs when `settings.jsonc` is missing) | Setup wizard | Set when the wizard finishes; "Run setup again" in Sources clears nothing | state |

### 4.2 Workspace settings (`workspace.jsonc`)

| Key | Type | Default | UI location | Description | Needed |
|---|---|---|---|---|---|
| `paths.gameInstall` | object `{path, pinned}` or null | null (detected) | Game and Steam | Override of the install folder; `pinned` stops auto-switching | O |
| `paths.userDir` | object or null | null | Game and Steam | Override of the user data folder (the one that holds `Config/ModsConfig.xml`) | O |
| `paths.steamRoot` | object or null | null | Game and Steam | Override of the Steam root | R |
| `paths.extraWorkshopDirs` | list<path> | `[]` | Game and Steam | Additional Workshop content folders (for example an old library) | R |
| `paths.ignoredInstalls` | list<string install id> | `[]` | Game and Steam | Detected installs the user does not want listed | R |
| `activeInstall` | string install id | detected best | Game and Steam | The install in use; stored by id, never by position | E |
| `modSources` | list of `{id, kind, enabled}` | built-in three (workshop one per library) | Mod sources | Order and enabled flag of built-in sources; read only otherwise | O |
| `customModFolders` | list of folder objects | `[]` | Mod sources | Any number of user folders (R4); per-folder fields below | E |
| `customModFolders[].id` | string `cf_` plus hex | generated | not editable | Stable identity; it is the source id inside every `ModId` | n/a |
| `customModFolders[].path` | path | required | Mod sources | Folder location | E |
| `customModFolders[].label` | string | folder name | Mod sources | Display name | E |
| `customModFolders[].enabled` | bool | true | Mod sources | Include in scans and lists | E |
| `customModFolders[].layout` | enum(auto\|modsRoot\|singleMod) | `auto` | Mod sources, Advanced | How to treat the path | O |
| `customModFolders[].scanDepth` | int 1 to 4 | 1 | Mod sources, Advanced | Levels below the root to search | O |
| `customModFolders[].watch` | bool | true on local volumes | Mod sources, Advanced | Use a watcher; advisory | R |
| `customModFolders[].priority` | int | list order | Mod sources (drag order) | Duplicate tie-break; lower wins | O |
| `customModFolders[].readOnly` | bool | false | Mod sources | Never write inside; hides edit tools; no link created inside | O |
| `customModFolders[].link` | enum(auto\|links\|copy\|none) | `auto` | Mod sources, Advanced | How the game gets these mods ([discovery](game-and-mod-discovery.md) GD-061) | O |
| `customModFolders[].volumeHint` | object `{mount, label, uuid?}` | filled at add | not editable | Re-finds a moved drive | n/a |
| `customModFolders[].colour` | string or null | null | Mod sources | Optional colour tag shown on rows from this source (mod manager MM-002) | R |
| `deploy.copyIgnore` | list<string> | `.git`, `Source`, `.vs`, `obj`, `bin` | Mod sources, Advanced | Names skipped by copy mode | R |
| `deploy.copyWarnMegabytes` | int | 500 | Mod sources, Advanced | Warn before copying a larger mod | R |
| `launch.method` | enum(auto\|steam\|direct) | `auto` | Launch | `auto` is the Steam URL for a detected Steam install and the executable otherwise (mod manager MM-029); replaces RimSort `launch_via_steam_protocol` | O |
| `launch.arguments` | string | empty | Launch | Command-line arguments; validated for quoting; not guaranteed to pass through the Steam URL (the UI says so) | O |
| `launch.saveDataFolder` | path or null | null | Launch | Passes `-savedatafolder`; refuses a path containing `=` | R |
| `launch.environment` | object name to value | `{}` | Launch, Advanced | Environment variables for direct launch | R |
| `launch.devMode` | bool | false | Launch, Developer | Toggles the game's developer mode in its own preferences when the game is not running | O |
| `launch.quickTest` | bool | false | Launch, Developer | Adds the verified quick-test flag for the dev launcher | R |
| `launch.popupWindow` | bool | false | Launch, Developer | Adds the borderless-window flag | R |
| `datasets.autoUpdate` | bool | true | Datasets | Refresh datasets at start and on the interval (replaces RimSort `update_databases_on_startup`) | E |
| `datasets.items.<id>.enabled` | bool | true | Datasets | Use this dataset; ids `steamDb`, `communityRules`, `noVersionWarning`, `useThisInstead`, `rimworldVersions` | O |
| `datasets.items.<id>.url` | string or null | null (built-in descriptor) | Datasets, Advanced | Source override; a mirror or a fork | R |
| `datasets.items.<id>.localFile` | path or null | null | Datasets, Advanced | Use a local file instead of fetching (RimSort "configured file path" mode) | R |
| `datasets.items.<id>.refreshHours` | int 1 to 720 | per dataset (24 for rules) | Datasets | How stale a copy may be before refetching (replaces `database_expiry`) | O |
| `activeProfile` | string profile id | `pr_default` | not a screen (profile switcher) | Last used list | state |

### 4.3 Secrets (credential store)

| Key | Stored where | Used by | v1 |
|---|---|---|---|
| none | OS credential store through `CredentialStore`, fallback `secrets.json` opt-in per secret | reserved for a future Steam web token, a GitHub token or a paste service code | The v1 publisher uses the signed-in Steam client through the helper and stores nothing ([data and persistence](../architecture/data-and-persistence.md) section 13). The Settings screen shows a Secrets section only once a feature stores one, with Set, Replace, Remove and "stored in the OS keychain" (or "stored in a file readable only by you") |

### ST-002 No settings the game decides (M1)

Statement: the following RimSort toggles are not settings in RimStudio because the game's own behaviour is fixed or the engine handles it: preferring versioned About tags over base tags, case-insensitive `About.xml` lookup (always probed, case-insensitively), the alphabetical sort method (the sort is deterministic canonical or game-style, chosen per sort action), the "try to download missing mods" prompt (a choice in the import dialog), the mod type filter (part of the search language), hide-invalid-while-filtering (a filter option), save and inactive sort state (view state), clear moving official content (Clear is undoable and keeps official content active by default), instance folder override (portable marker and roots). AC: none of these keys exist in the schema; a RimSort import never produces them.

### ST-003 Settings that depend on the OS or install source (M1)

Statement: some controls are hidden or replaced by an explanation depending on the platform: the updater controls are hidden for packages owned by a package manager (deb, rpm, AUR, Flatpak, Snap) and for portable mode on Windows, where "download new zip" replaces them ([packaging research](../research/cross-platform-packaging-research.md) section on install source); `launch.method` offers `steam` only when the Steam client is detected; link mode `junction` is Windows only and `symlink` is Unix only (the UI shows the platform's real mode and never lets a user choose one that cannot work). AC: on each CI leg the Settings screen shows exactly the controls valid for that OS (component tests with a fake `InstallSourceProbe`).

## 5. Screen anatomy

### ST-010 Layout (M1)

Statement: Settings is a full page with a left navigation, a content column and a search box at the top. It is one screen for both "settings" and "sources", so users find paths where they expect them. Sections, in order:

| Section | Contains | Keys |
|---|---|---|
| Game and Steam | the DetectionReport cards ([discovery](game-and-mod-discovery.md) GD-003), overrides, active install, Run detection again, Run setup again | `paths.*`, `activeInstall` |
| Mod sources | the sources list and custom folders editor (section 6) | `modSources`, `customModFolders`, `deploy.*` |
| Datasets | the datasets entry point (section 7) | `datasets.*` |
| Library and sorting | scan, watch, sort and history options | `library.*`, `sorting.*`, `history.*` |
| Appearance | theme, accent, density, text size, language, motion | `language`, `appearance.*` |
| Shortcuts | the shortcut table with search and conflict marks | `shortcuts` |
| Launch | method, arguments, environment, developer switches, safe graphics | `launch.*`, `launch.jsonc` |
| Tools | external editor, item designer mode | `tools.*`, `designer.*` |
| Performance | scan threads, watch policy | `library.scanThreads`, `library.watch.*` |
| Diagnostics and data | log level, data folders, backups, reports (section 10) | `logging.*` |
| About and updates | version, channel, update check, licences | `updates.*` |
| Import | Import from RimSort (section 11) | none |

Rules:
1. The top search box filters live across all sections by title, description, aliases and key path (including the dotted path, so a person who read the JSONC file finds the control). Results show the section and highlight the match; Enter opens it with the control focused.
2. Every row shows: a label, the control, one sentence of description, a Reset to default link visible only when the value differs, and a small source tag when the effective value does not come from the file the section normally edits (for example "set by command line").
3. Changes apply immediately (no OK and Cancel pair, no restart for anything except what the row says). The screen shows a quiet "Saved" mark after the debounced write and an inline error when the write fails (read-only roots, GD-080).
4. Reset all settings in an Advanced menu takes a backup first and removes only `settings.jsonc` keys, never `workspace.jsonc`, never user data, with a typed confirmation; it is a single undoable action through the backup restore list.
5. The page is keyboard operable, uses the same components as the rest of the app, and renders every row state in the gallery in both themes.

AC: searching "thread" finds `library.scanThreads`; searching "library.watch" finds the three watch rows; Reset to default removes exactly the key from the file and leaves comments intact; no row requires an application restart except `language` for any untranslated native menu strings (stated on the row).

Tests: Vitest component tests for the search index and row states; Playwright flow "change theme, restart, kept"; CST edit tests on files with comments and unknown keys.

## 6. Mod sources and custom folders editor

### ST-020 The sources list (M1, R4)

Statement: Settings, Mod sources shows one ordered list of sources, built-ins first (greyed handles), then custom folders in priority order. Each row is a card: label, path, kind chip, mod count, last scan time, reachability (online or offline), visibility to the game ("Visible to the game" or "Needs a link"), and a menu. Built-in rows allow enabling, disabling and Open folder only; Workshop rows are per library with the library label. Custom rows allow everything below.

Actions on a custom folder:

| Action | Behaviour |
|---|---|
| Add folder | Picker or drag and drop; runs `sources_probe_folder` first and shows the result (looks like a mods folder, estimate of mods, removable or network, warnings) in a confirm panel before saving; hard errors listed in [discovery](game-and-mod-discovery.md) GD-041 prevent saving and name the fix |
| Remove | Removes the entry and its derived cache only; never deletes files; confirms with the count of active mods that would become missing |
| Reorder | Drag handle or Alt plus arrow keys; the new order is the new priority; a toast shows what changed ("2 duplicate ids now prefer My mods") |
| Enable or disable | Toggle; disabled folders are not scanned and their mods leave the library (cached rows are kept for re-enable) |
| Label | Inline edit |
| Colour | Optional tag colour |
| Open | Open in the OS file manager |
| Test | Runs the probe again and shows a result panel: reachable, mod count by layout, depth reached, mods that failed to parse, overlap check, link plan preview (how each mod would be made visible and which would use copy) and the time taken; it never changes anything |
| Rescan | Rescans this folder only (a job) |
| Advanced (expand) | Layout (auto, modsRoot, singleMod), scan depth 1 to 4, watch toggle, read-only toggle, link mode (auto, links, copy, none) with the platform's real mode named, copy ignore list link, volume hint (read only) |

Defaults on add: layout auto, depth 1 (the probe suggests 2 when it sees grouping folders, with one click to accept), watch on for local volumes and off for network volumes, link auto, readOnly false. The priority of a new folder is last.

AC: adding a folder and seeing its mod count takes one picker action and one confirmation; the Test button's result for the owner's folder fixture lists 22 mods at depth 2 and flags the template repository as "not a mod"; reordering updates the file with one CST move, keeping comments; removing the entry leaves the folder's files untouched (checked by a recording filesystem); read-only folders show no edit tools anywhere; two folders cannot be saved when one contains the other.

Tests: component tests with mocked `sources_*` commands; fixtures S1 to S5 and D1 from [discovery](game-and-mod-discovery.md); CST tests on reorder; Playwright add, test, reorder, remove.

### ST-021 Offline and moved folders in the editor (M1)

Statement: an offline folder keeps its row with an offline chip and a Find the drive action when a volume with the stored hint appears under a different path; accepting rewrites only `path`, never `id` ([data and persistence](../architecture/data-and-persistence.md) section 5.3). The Test button on an offline folder says which part is unreachable.

AC: simulating a changed mount point produces the proposal; accepting it keeps every `ModId` valid; declining keeps the entry offline.

### ST-022 Visibility and link controls in the editor (M2)

Statement: the editor shows, per custom folder, what happens at launch: the chip "Needs a link" becomes "Linked (12 mods)" after a deploy, with a "Remove links created by RimStudio" action for the whole farm in the section footer (unlink only; [discovery](game-and-mod-discovery.md) GD-063) and a link audit action that lists owned and unowned entries in `Mods`. While spike S-03 is open the section carries a visible "Experimental" tag ([discovery](game-and-mod-discovery.md) GD-069).

AC: unlink-all leaves zero owned entries; the audit never offers to delete a non-owned entry.

## 7. Datasets entry point

### ST-030 Datasets section (M2, R6)

Statement: Settings, Datasets is the entry point to the dataset system; the dataset pipeline itself is specified with the mod manager and the [rules fetch and merge design](../research/rules-fetch-and-merge-design.md). The section shows one card per dataset (`steamDb`, `communityRules`, `noVersionWarning`, `useThisInstead`, `rimworldVersions`) with: name, what it provides in one sentence, enabled toggle, status (current, stale, failed with the reason, never fetched), last fetched time, size, and a Refresh button; a global "Update automatically" toggle and a Refresh all button. A card expands to Advanced: URL override, local file override, refresh interval, and a "Revert to built-in source" action. There is no git repository field and no per-dataset source-mode radio group (RimSort's four radios times five datasets collapse into `enabled`, `url` and `localFile`, catalogue implication 4).

Rules: datasets are fetched at runtime and never bundled or mirrored (R11, I-06); the card for each unlicensed community dataset says "downloaded from the community repository on this computer; not redistributed by RimStudio"; no network call happens before the user finished the wizard or turned the toggle on (first-run default is on, the wizard says so in one sentence); a failed fetch keeps the last good copy and shows the reason; an unsupported format quarantines the download and is reported, not applied ([data and persistence](../architecture/data-and-persistence.md) section 6).

AC: with network disabled the cards show "using last copy from <date>" and no error toast storm; entering a URL override is validated (https only, reachable test button) before it is saved; Revert removes the override key; the card statuses update live from `datasets_subscribe`.

Tests: component tests with mocked streams; integration tests with a fake `RemoteDataset` transport.

## 8. Appearance and performance

### ST-040 Appearance (M1, R8)

Statement: the Appearance section previews changes live in a small sample panel (a mod row, a diagnostic chip, a button, a card) and in the whole app. Controls: theme (light, dark, system, user themes with Open themes folder and Export current), accent (presets plus a hex field with a contrast check against both themes), density (three previews), text size (slider, 80 to 160 percent), mod colour mode, language, reduced motion. The first paint reads the theme from a bootstrap value so there is no flash of the wrong theme. User themes are JSONC files of token overrides applied in the last cascade layer; an invalid token is ignored with a warning shown next to the theme name, never breaking the app. There is no window size setting, no per-window launch state and no font family picker; geometry is automatic, fonts are the bundled ones (catalogue implication 7).

AC: changing the theme repaints in under 100 ms without reload; a user theme with an unknown token loads with a warning; the accent field refuses a colour that fails the contrast check with the numbers; language switch reloads the catalogue lazily and falls back to English for missing keys.

### ST-041 Performance (M1)

Statement: the Performance section has three controls, none of them required: scan workers (automatic by default; the maximum is 8 because more workers did not improve the measured scan, [scan performance spike](../research/scan-performance-spike.md) section S6), change detection mode with poll interval and debounce, and a "Rebuild caches" action that deletes the cache root contents after confirmation (the next start is one slow start, no data loss). Next to the controls the screen shows the last scan timings by level from the footer data (level 0 and level 1 durations, file counts) so a user can see whether a change helped.

AC: setting `library.scanThreads` to 1 changes the next scan only; output is identical at 1 and 8 workers; Rebuild caches never touches `userdata`, config or backups.

## 9. Launch settings and safe graphics

### ST-050 Launch settings (M2)

Statement: the Launch section edits the global defaults in `launch.*` (a profile may override method and arguments; the mod manager owns that UI). Controls: launch method (Automatic, Through Steam, Direct), arguments with a live parse that shows the resulting argument list and flags quoting errors, a save data folder picker (refuses a path containing `=`), environment variables table (direct launch only), and the developer switches (developer mode, quick test, borderless window) that are the four verified switches of the dev launcher ([ecosystem survey](../research/ecosystem-survey.md) section 3). Rules: the arguments field carries a note next to it when the method is Steam, because arguments are not guaranteed to pass through the Steam URL (unverified); developer mode edits the game's preference file only when the game is not running and only after a backup, through the same write fence rules as `ModsConfig.xml`; "Open Player.log after exit when it has errors" is `library.autoLoadPlayerLog`.

AC: invalid quoting produces an inline error and Save stays possible for other fields; a direct launch fixture shows the exact argument vector; developer mode toggling while the fake game is running is refused with the reason.

Tests: launcher port tests with fake `Launcher`; unit tests for the argument parser (quotes, empty, `=` rule).

### ST-051 Safe graphics (M0)

Statement: RimStudio's own window can fail on some Linux WebKitGTK stacks (blank or garbled window). The recovery policy is in [cross-platform](../architecture/cross-platform.md) section 5 and is reflected here. Safe graphics is a bootstrap concern, not a normal setting, because it must be read before the webview is created:

1. `launch.jsonc` in the config root holds graphics environment variables and the safe-graphics flag; it is created on demand, read before webview creation, and edited through CST so comments survive.
2. The command line flag `--safe-graphics` applies the software fallback for one run.
3. A crash marker is written before the webview is created and removed after the first painted frame; if it exists at the next start, a small native dialog (no webview) offers "Start in safe graphics mode" and remembers the choice in `launch.jsonc`.
4. Settings, Launch, Safe graphics shows the current state, the engine version, session type (Wayland or X11) and the variables in effect, with Turn on, Turn off and Open file; changes apply on the next start and say so. The variable names come from the WebKitGTK documentation at implementation time and are not listed here (unverified).

Example `launch.jsonc`:

```jsonc
{
  "$schema": "./schemas/launch.schema.json",
  "schemaVersion": 1,
  // Set by the crash dialog or by hand; read before the window exists.
  "safeGraphics": true,
  "environment": {
    // Names of graphics switches are taken from the engine documentation
    // at implementation time; this is a placeholder.
    "EXAMPLE_GRAPHICS_SWITCH": "1"
  }
}
```

AC: with a marker present the native dialog appears before any webview, and choosing safe mode persists; on Windows and macOS the section shows only the explanation that no action is needed; the file keeps its comments after the dialog edits it.

## 10. Diagnostics and data folder actions

### ST-060 Diagnostics and data (M1)

Statement: the section groups support and housekeeping actions, each with one sentence and a confirmation where destructive.

| Action | Behaviour |
|---|---|
| Log level | `logging.level`; shows the log folder path and the rolling file count |
| Open data folder, config folder, cache folder, logs folder | OS file manager; the four roots and the mode (installed or portable) are shown with sizes |
| Storage summary | Size of cache, backups, history, datasets; per-row Clear (cache and datasets) and "Clear old backups" that never removes the newest backup of any document |
| Restore from backup | Lists `settings.jsonc` and `workspace.jsonc` backups by time with a diff preview; restoring first backs up the current file |
| Export diagnostics report | One JSON file: versions, OS, webview engine, detection report with home paths redacted, settings keys that differ from defaults (values redacted for paths), last errors; no credentials, no mod content ([security and privacy](../architecture/security-and-privacy.md)) |
| Export or import preferences | Copies `settings.jsonc` and `workspace.jsonc` as they are, excluding secrets; import shows a diff first and takes a backup |
| Open settings file | Opens `settings.jsonc` or `workspace.jsonc` in the configured editor or the OS opener |
| Reset | Resets `settings.jsonc` keys only (section 5 rule 4) |
| Rebuild caches | Section 8 |
| Run detection again, Run setup again | Re-runs the detection and the wizard without overwriting overrides unless confirmed |

No telemetry exists, so there is no analytics toggle; the About section states this in one line (D-060).

AC: the diagnostics report passes a redaction test (no path under the home directory, no token-like string); restoring a backup produces the earlier values and keeps unknown keys; Clear old backups keeps the newest of each document; the roots shown match `rimstudio-cli detect`.

## 11. Import from RimSort

### ST-070 What is read (M2, R6)

Statement: Settings, Import offers "Import from RimSort" as a read-only, idempotent operation that never writes into RimSort's folders. The procedure is in [data and persistence](../architecture/data-and-persistence.md) section 12.1 and the [rules design](../research/rules-fetch-and-merge-design.md). Steps:

1. Detect RimSort data folders per OS (Linux `~/.local/share/RimSort` is verified; macOS and Windows locations are unverified) plus Choose folder.
2. Show a checklist with counts: user rules, saved lists, notes, tags and colours, ignore list, dataset source choices, instance paths.
3. Import the checked items and write an import report JSON (`userdata/imports/<timestamp>-rimsort.json`) listing imported, skipped and unmatched rows.

What is read and where it goes:

| RimSort item | Read from | RimStudio destination | Notes |
|---|---|---|---|
| User sorting rules | RimSort user rules file | `userdata/rules/user-rules.json`, lossless | unknown keys kept; the community format is preserved |
| Saved mod lists | exported list files (RimWorld-shaped XML) | `userdata/lists/<id>.json` profiles | parsed through `rimstudio-xml`; the game version recorded |
| Notes, colours, tags, ignore flags | `aux_metadata.db` (SQLite) | `userdata/mod-meta.json`, `userdata/ignore.json` | needs the read-only SQLite dependency behind the `aux-db` feature (accepted, D-031 and D-083); while the feature is off this row is greyed with the reason; mapped by path or Workshop id; unmatched rows kept in the report |
| Instance paths | `settings.json`: `game_folder`, `config_folder`, `local_folder`, `workshop_folder` of each instance | suggestions only: offered as path overrides and as custom folders (the `local_folder` becomes one custom folder entry) | each is validated and shown before it is accepted; never applied without confirmation |
| Run arguments | instance `run_args` | `launch.arguments` | offered, not forced |
| Dataset source choices | the `external_*` mode and file path keys | `datasets.items.<id>` overrides | git repository and archive URL keys are ignored; a "configured file path" becomes `localFile` |
| Tool paths | `text_editor_*` | `tools.textEditor` | offered |
| Theme, language, sizes, other UI keys | none | none | RimSort themes and window geometry are not migrated |

What is never migrated: the Steam web API key, the paste-service code, the GitHub username and token (all plain text in RimSort's settings; secrets are never imported, never copied into a settings file and never shown), SteamCMD and todds settings and paths (features not in v1), the aux database retention keys, window and dialog sizes, the divider and custom colour state keys (dividers and colours come from the aux database import where they exist), and per-instance first-run flags. The import screen states this list, so a user is not surprised.

Idempotence: running the import again updates rows keyed by identity instead of duplicating them. A rules file from a newer format version is read tolerantly and unknown keys are kept; a file outside the supported range is rejected with the reason.

AC: importing the real user rules file from the research machine's RimSort data produces a `user-rules.json` that round-trips byte-equivalent in structure (keys and order) in a test using a fictional copy; the secrets keys in a fixture `settings.json` never appear in any output, log or report (grep test); running the import twice produces the same state; with the SQLite feature off the notes row is disabled with the reason and the rest works.

Tests: fixtures `R1` (RimSort data folder with rules, lists, settings with secrets), `R2` (aux database with notes and colours, fictional), golden import reports, redaction grep test.

## 12. Validation and error display

### ST-080 Validation (M1)

Statement: every setting has a validator in `rimstudio-core` (types and ranges) and, for paths, one in the manager (existence, content, overlap) so the same rules run in the UI, the CLI and file loading. Three severities:

| Severity | Meaning | Behaviour |
|---|---|---|
| Error | The value cannot be used (wrong type, out of range, path fails a hard check) | Not saved from the UI (the control shows the message); a bad value read from the file is kept in the file, reported, and the default is used for that run |
| Warning | The value works but is suspect (a network volume, a path over 240 characters on Windows, a long poll interval) | Saved, message stays visible |
| Info | Explanation (a value is overridden by the command line) | Source tag |

Rules: a file that does not parse as JSONC is not rewritten; the app starts with defaults, shows a banner with the line and column, offers Open file, Restore from backup and Start fresh (which first copies the broken file to a `.broken-<time>` backup); keys of an unexpected type are reported individually and do not discard the rest of the file; a file with a `schemaVersion` newer than the app opens read only with a banner and is never rewritten; read-only roots put the app in a banner state "Settings will not be saved" with in-memory settings ([data and persistence](../architecture/data-and-persistence.md) section 2). Settings problems are `settings.*` diagnostics (proposed: `settings.invalid-value`, `settings.unknown-key`, `settings.file-unreadable`, `settings.newer-version`, `settings.not-writable`) and use the same error envelope as every command; the UI never shows a stack trace or a raw code without a sentence.

Display: inline under the control (message and, if useful, the accepted range), a count badge on the section in the navigation, and a summary list at the top of the screen when any problem exists, each item linking to the control or, for unknown keys in the file, to Open file at that line.

AC: setting `library.scanThreads` to 99 in the UI is refused with "Use 0 to 8"; the same value in the file is reported, the default is used, and the other keys still apply; a broken JSONC file produces the banner and the three actions and is not overwritten; a newer-version file stays untouched after the app exits.

Tests: validator unit tests with proptest on ranges; file corruption fixtures (truncated, wrong type, unknown keys, newer version); component tests of the problem summary.

## 13. Migration between versions

### ST-090 Versioning and migration (M1)

Statement: rules from [data and persistence](../architecture/data-and-persistence.md) section 4 apply:

1. Documents carry `schemaVersion`; migrations are pure forward functions (`settings_vN`, `workspace_vN`) in `rimstudio-io::migrate`, applied in order in memory and then written atomically.
2. New optional keys need no bump (`serde` default); removing, renaming or changing the meaning of a key bumps the version and adds a migration.
3. Before a migration the pre-migration copy is kept as `<name>.bak-vN` until the next successful migration, and ordinary retention backups continue (last 10).
4. Unknown keys are preserved by migrations and by CST edits.
5. A historical fixture per version lives in `tests/fixtures/settings/vN.jsonc` and `tests/fixtures/workspace/vN.jsonc`; one test loads all of them and compares to the expected struct; adding a version without a fixture fails CI.
6. Moves between keys inside the file (for example `paths` split or merged) are migration steps and are described in the change log shown after an update ("Settings updated to version 2: nothing to do").
7. Application identifier changes would move the config root and orphan settings; the owner chooses the identifier before the first release (D-049); until then the placeholder `app.rimstudio.desktop` fixes the folder names, and a migration note is required if it changes.
8. Portable mode moves with the executable; the portable `workspace.jsonc` paths are re-resolved through `volumeHint` when the drive letter or mount differs.

Planned first migration shape (illustrative, not a promise): a version 2 that adds `datasets.items.<id>.pinnedVersion` would need no migration; a rename of `launch.method` values would. These are examples to show what bumps the version and what does not.

AC: loading every historical fixture yields the current struct; a file one version newer is read only; a migration failure leaves the original file untouched and starts with defaults plus a banner; the pre-migration backup exists after a successful migration.

## 14. RimSort disposition table

Every key of the [RimSort settings catalogue](../research/rimsort-settings-catalog.md) is accounted for here by group (numbers are the catalogue row numbers). Counts: 98 global attributes and 13 per-instance fields.

| Catalogue rows | RimSort keys | Disposition | RimStudio home |
|---|---|---|---|
| 1 | `check_for_update_startup` | Keep, renamed | `updates.checkOnStart` |
| 2 to 9, 11 to 22 (20 keys) | the four keys of each of five datasets: source mode, local path, repository, archive URL | Collapsed into one table | `datasets.items.<id>` with `enabled`, `url`, `localFile`, `refreshHours`; repository keys dropped (fetch at runtime, R6, R11) |
| 10 | `aux_db_time_limit` | Drop | not applicable; notes are not purged by age |
| 23 | `sorting_algorithm` | Drop | deterministic canonical or game-style sort chosen per action |
| 24 to 26 | dependency sort options | Keep, renamed | `sorting.*` |
| 27 | `prefer_versioned_about_tags` | Drop | game rule, not a preference |
| 28 | `render_unity_rich_text` | Keep, renamed | `appearance.richText` |
| 29 | `color_background_instead_of_text_toggle` | Keep, renamed | `appearance.colourMode` |
| 30 | `case_insensitive_about_xml_lookup` | Drop | always case-insensitive probe |
| 31 | `try_download_missing_mods` | Drop | choice in the import dialog |
| 32 | `duplicate_mods_warning` | Drop | Duplicates panel and ignore list; no global switch |
| 33, 34 | updated indicator and threshold | Keep, renamed | `library.recentlyUpdated.*` |
| 35 | `mod_list_startup_impact` | Later | revisit with the Loading Progress integration |
| 36 | `mod_type_filter` | Drop | no control in RimSort; part of the search language |
| 37 | `hide_invalid_mods_when_filtering` | Drop | filter option |
| 38 to 40 | inactive sort state keys | Drop | view state, per-viewer |
| 41 to 45 | DB builder keys including `steam_apikey` | Drop | the Steam database is fetched, not built; no API key stored |
| 46 to 54 | SteamCMD and todds options | Not in v1 | separate later tools; the keys are reserved under `tools.*` when built |
| 55 to 57 | text editor command and arguments | Keep, reshaped | `tools.textEditor.*` |
| 58, 59 | `enable_themes`, `theme_name` | Replace | `appearance.theme` (light, dark, system, user themes) |
| 60, 61 | font family and size | Replace | `appearance.fontScale`; bundled fonts |
| 62 | `language` | Keep | `language` |
| 63 | `constrain_dialogues_to_main_window_monitor` | Drop | the shell positions dialogs |
| 64 to 71 | window launch state and nine size keys | Drop | geometry remembered automatically (`window`, state) |
| 72 | `debug_logging_enabled` | Replace | `logging.level` |
| 73 | `watchdog_toggle` | Replace | `library.watch.mode` |
| 74 to 77 | save backup keys, last backup date | Later | a save-backup tool, if built, owns its keys; the date is state |
| 78 | `steam_mods_update_check` | Keep, renamed | `library.checkWorkshopUpdates` |
| 79 | `update_databases_on_startup` | Keep, renamed | `datasets.autoUpdate` |
| 80 | `include_mod_notes_in_mod_name_filter` | Drop | search query fields |
| 81 | `show_save_comparison_indicators` | Keep, renamed | `library.showSaveComparison` |
| 82, 83 | history enabled and retention | Keep, renamed | `history.enabled`, `history.keep` |
| 84 | `clear_moves_dlc` | Drop | Clear is undoable and keeps official content |
| 85, 86 | app backup before update, max backups | Drop | the updater and package managers own this |
| 87 | `rentry_auth_code` | Secret, not in v1 | credential store slot if the paste feature is built |
| 88, 89 | GitHub user and token | Secret, not in v1 | credential store slot if the GitHub feature is built; never imported |
| 90, 91 | GitHub update check keys | Not in v1 | later with the GitHub tool |
| 92 | `enable_aux_db_behavior_editing` | Drop | notes and flags are always editable in RimStudio's own data |
| 93 | `auto_load_player_log_on_startup` | Keep, reshaped | `library.autoLoadPlayerLog` (on game exit) |
| 94 to 96 | current instance, path, instances map | Replace | `activeInstall` and the detected installs; per-install state in the ownership manifest |
| 97, 98 | custom colours, dividers | State | `userdata/mod-meta.json`, `userdata/groups.json` |
| per-instance `game_folder` | install path | Replace | `paths.gameInstall` |
| per-instance `config_folder` | user data folder | Replace | `paths.userDir` |
| per-instance `local_folder` | the single local mods folder | Replace | `customModFolders[]` (any number, R4) |
| per-instance `workshop_folder` | Workshop folder | Replace | detected per library; `paths.extraWorkshopDirs` for extras |
| per-instance `run_args` | launch arguments | Keep | `launch.arguments` |
| per-instance `launch_via_steam_protocol` | Steam URL toggle | Replace | `launch.method` |
| per-instance `steam_client_integration` | Steamworks on or off | Drop | automatic: the helper is used only where a feature needs it and its absence never blocks |
| per-instance SteamCMD fields (4) | SteamCMD paths and flags | Not in v1 | later tool |
| per-instance `instance_folder_override` | data folder | Drop | portable marker and data roots |
| per-instance `initial_setup` | first-run flag | Replace | wizard runs when `settings.jsonc` is absent; `onboarding.completed` |

Result: of the 111 RimSort keys, 20 dataset keys collapse into one table, about 30 are dropped as game rules, view state, window geometry or obsolete tools, about 14 are deferred with their features, 4 are secrets that RimStudio will not import, and the rest map to the keys in section 4.

## 15. Commands and tests

Commands from the registry ([IPC and state](../architecture/ipc-and-state.md)): `settings_get` (sections optional; the DTO never includes secrets and carries effective values with source tags), `settings_update` (section patches, returns the new DTO and a revision; applied through CST edits, debounced flush), `detect_run`, `detect_get_report`, `detect_set_override`, `sources_list`, `sources_add_folder`, `sources_update`, `sources_remove`, `sources_probe_folder`, `datasets_status`, `datasets_subscribe`, `datasets_refresh`, `rules_import_rimsort`. Needed additions to the registry (to reconcile in the architecture documents, section 16): `settings_set_window_state` (action, debounced window geometry, D-076), `settings_reset` (action, with key paths), `settings_backups_list` and `settings_backup_restore`, `settings_import_rimsort_scan` (query: candidates and counts) and `settings_import_rimsort_apply` (job) alongside the rules-only command, `diagnostics_export`, and `settings_storage_summary`.

Events: `settings.changed {rev, keyPaths}` so views subscribe by key path; `settings.problem` for file-level problems.

Test plan:

| Layer | What |
|---|---|
| Unit | Validators, effective value resolution with source tags, schema defaults, migration chain, argument parser |
| CST | Edit, reset, reorder, insert on files with comments, trailing commas, unknown keys and BOM; comments and order intact (spike S-02 gates the library choice) |
| Golden | Generated JSON schemas for both files (drift-checked by xtask), import reports, diagnostics report |
| Fixtures | `tests/fixtures/settings/vN.jsonc`, `workspace/vN.jsonc`; broken and newer-version files; R1 and R2 RimSort data (fictional) |
| Integration | Debounced writes (one per second), atomic replace with a crash between temp and rename, backup retention, portable mode root resolution, read-only roots |
| Frontend | Search index tests, row state gallery in both themes, Playwright flows (change theme, add folder, reorder, test button, reset, import) with mocked IPC |
| Security | Grep tests that secret-like keys from the RimSort fixture appear nowhere in output; `RootGuard` tests for every path the UI can submit |

## 16. Owner decisions and open points

1. Keychain use for secrets (D-030): v1 stores none, so the decision can wait until a feature needs one.
2. The accent default and the exact theme set belong to the design stage (R8); `amber` is a placeholder.
3. `launch.jsonc` is a fourth config file beside the three of D-030; it exists only because safe graphics must be read before the webview. Resolved by D-077: the architecture documents now list it next to `settings.jsonc` and `workspace.jsonc`.
4. The registry additions in section 15 and the `settings.*` diagnostic codes need to be added to the architecture's command table and code registry.
5. Density names (`standard`, `compact`, `touch`) are an assumption derived from the cross-platform document's mention of a compact touch density; confirm with the UI design.
6. `history.keep` defaults to 50 following the retention rule in the persistence document, not RimSort's 100; confirm.
7. Application identifier (D-049) fixes where these files live; it must be settled before the first public release.
8. Open questions inherited from research: which RimSort data folder locations exist on macOS and Windows (unverified), and whether the Steam URL passes launch arguments (unverified; the UI says so).

## 17. As built in the 0.1.0 backend

`rimstudio-manager` implements settings, detection, sources and the library scan as plain functions over a narrow context (`Ctx`); the application supplies the real file system probe for scans (`scan_fs`), the platform file id function, the clock and, for the `ModsConfig.xml` version warning, a function that reads that version through `rimstudio-xml`. Details of the shapes are in the [crate catalog](../architecture/crate-catalog.md) (rimstudio-manager).

1. First run writes a commented `settings.jsonc` (`DEFAULT_FILE_TEXT`); defaults are never written afterwards. `settings::get(SettingsGetRequest { sections })` returns a `SettingsView` with the stored settings, an `effective` map of full section values (an invalid stored section reads as its default there, while the file keeps the value and the problem is listed), the origin, a read only flag (a file newer than the app or with a syntax error), the schema version found, and a `rev` fingerprint. A failure to write the defaults file inside `get` is logged, not fatal; `update` propagates write failures.
2. `settings::update(SettingsUpdateRequest { patches })` takes section patches (merge patch semantics, `null` resets a key to its default). The patchable sections are listed in `settings::SECTIONS`. Each changed leaf is written as a minimal comment preserving edit through `Store::edit`, so the bytes outside the edit are identical (tested); unknown sections or keys, invalid values, and syntax-error or newer-version files are refused without writing. The `window` section and free form maps are replaced wholesale (no per leaf patch). Settings migration uses the `Versioned::migrations` hook, which has no steps yet because the schema is at version 1.
3. Custom folders, path overrides and sources are stored in `workspace.jsonc`, as section 3 says, and not in `settings.jsonc`. `detect::set_override` validates and stores an override for the game install, user folder, Steam root or Workshop directory, then reruns detection; a `None` path clears it. A Workshop directory override is added to `paths.extraWorkshopDirs` and clearing it removes all of them; `sources::effective_sources` appends those directories as `workshop-N` sources because an override install gets no Workshop from detection. Detection is not rerun automatically when a custom folder or the Steam environment changes; the cached report (`detection-report.json`, see open design issue OD-18) is reused until `detect_run`, `set_override` or a full scan.
4. Sources: `list`, `add_folder`, `update`, `remove` and `probe_folder`. Overlaps are refused as nested, duplicate, enclosing, inside the game `Mods` folder, or a symlink to a known source. Removing a folder only edits the document. Built in sources accept only `enabled`. Custom folder ids come from `custom_from_seed(path, clock ms, counter)` and are unique among current sources. The mod count estimate uses `classify_folder` at its suggested depth, not the configured depth. The removable drive hint is path based (`/run/media`, `/media`, `/mnt`, `/Volumes`, a non C drive letter) plus the volume class (FAT or exFAT); it is not a device check.
5. `library_scan` builds the source set from the selected install, Workshop directories and custom folders and runs the library scanner with the manifest cache; a mod found only in a custom folder is flagged `NeedsLink` until it is linked into `Mods`. A cancelled scan does not save the manifest. The result is not serialisable; the app keeps the index and sends counts, timings and diagnostics.
6. Known gaps: the macOS app bundle override check is implemented but untested; GD-031 identity (device and inode) is not used; no per leaf file level patch exists for the `window` section.
