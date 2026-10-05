# Data and persistence

Scope: where RimStudio keeps its data, in which format, who owns each file, how files are written, versioned, migrated, backed up and invalidated, and where the boundary with RimWorld's own files lies. It covers requirements R3, R4, R6, R10 and R11 and the invariants I-03 (json-only-app-data), I-05 (game-folder-fence), I-17 (user-vs-derived) and I-18 (lossless-foreign). Performance numbers behind the cache design live in [performance strategy](performance-strategy.md); crate boundaries live in the [crate catalog](crate-catalog.md) and the [overview](overview.md).

Status: draft, store and backup details updated to the 0.1.0 implementation | Last updated: 2026-10-05

## 1. Principles

1. App-owned data is JSON; files that a person is expected to edit are JSONC. XML appears only where RimWorld itself reads or writes the file, and only through `rimstudio-xml` (R10, I-02).
2. Every file has exactly one owner class, and the class decides what may delete it:

   | Owner class | Meaning | Deletion |
   |---|---|---|
   | user data | Choices and content a person made | never automatic; backed up before every rewrite |
   | derived cache | Recomputable from disk or the network | always safe; rebuilt on demand (I-17) |
   | dataset | A downloaded community dataset and its state | safe, but costs a download; last good copy is kept |
   | secret | A credential | stored in the OS credential store, never in a plain file by default |
   | log | Diagnostic output | safe |

3. Derived data never lives beside user data, and user data never lives in the cache root. "Reset caches" therefore cannot lose a decision.
4. Foreign data is handled losslessly (I-18): unknown keys and key order in community rule files survive a round trip, and existing RimWorld files are edited by byte span so comments and layout survive.
5. Nothing under the game install or the game's config folder is written except through `rimstudio-io::GameWriteFence` (I-05, section 10).
6. No SQL, SQLite, YAML, TOML or binary format for app data, and no storage engine beyond the in house JSON document store of section 16 (D-083). The one foreign format reader is the read-only RimSort auxiliary database import, accepted by the owner (D-031, D-083, section 12).

## 2. Data roots and portable mode

`rimstudio-io::DataRoots` resolves four roots exactly once, before the first read, and hands them to `AppContext`. Core crates never discover directories (D-024); the shell passes the paths that Tauri's path resolver computes, and the CLI and tests use the `directories` crate, as recommended in the [crate research](../research/rust-crate-research.md) section 6.3.

| Root | Purpose | Linux | macOS | Windows |
|---|---|---|---|---|
| config | `settings.jsonc`, `workspace.jsonc`, themes | `$XDG_CONFIG_HOME/<id>` (default `~/.config/<id>`) | `~/Library/Application Support/<id>` | `%APPDATA%\<id>` |
| data | user rules, lists, history, projects, publish records, link manifests | `$XDG_DATA_HOME/<id>` (default `~/.local/share/<id>`) | `~/Library/Application Support/<id>` (under a `data` subfolder) | `%LOCALAPPDATA%\<id>\data` |
| cache | manifests, indexes, dataset copies, thumbnails | `$XDG_CACHE_HOME/<id>` (default `~/.cache/<id>`) | `~/Library/Caches/<id>` | `%LOCALAPPDATA%\<id>\cache` |
| logs | rolling JSON lines | `<data>/logs` | `~/Library/Logs/<id>` | `%LOCALAPPDATA%\<id>\logs` |

`<id>` is the application identifier. It is the placeholder `app.rimstudio.desktop` until the owner chooses a reverse-DNS id (D-049); changing it later moves every root, so the decision must precede the first public release. The exact per-OS folder names above follow the Tauri and XDG conventions and are unverified on macOS and Windows until milestone M2 measures them.

Portable mode:

1. A file named `rimstudio.portable` beside the executable moves all four roots under `./data/{config,data,cache,logs}` next to it. On AppImage the marker is searched beside the `.AppImage` file using the `APPIMAGE` environment variable, because the executable directory is a read-only mount ([packaging research](../research/cross-platform-packaging-research.md) section 2.4).
2. The resolver is one function; the chosen mode and the four absolute paths are shown on the diagnostics page and in `rimstudio-cli detect`.
3. Portable mode disables the Windows updater and offers "download new zip" instead.
4. Paths inside settings that point to external drives are stored as text plus a `volumeHint` (mount and label) so a portable install moved between machines can re-find the drive (section 5.3).
5. Roots are validated at start: not writable means a read-only banner and in-memory settings; the app still runs.

Flatpak and other sandboxes: the roots resolve inside the sandbox; the grants that the app needs for Steam folders and `ModsConfig.xml` are recorded in the packaging note and the detection report, not here.

## 3. File inventory

All names are lowercase kebab-case. A document carries `schemaVersion` (integer); a compact cache carries `v`. "Writer" is the crate that owns writes; the shell never writes files.

### 3.1 Config root (user data, JSONC)

| Path | Format | Owner | Schema (version) | Writer | Migration |
|---|---|---|---|---|---|
| `settings.jsonc` | JSONC | user data | `settings` (1) | `rimstudio-manager` settings use case via `io::jsonc` CST | `io::migrate::settings_vN`, backup `settings.jsonc.bak-vN`, forward only |
| `workspace.jsonc` | JSONC | user data | `workspace` (1) | same | same, `workspace_vN` |
| `themes/<name>.jsonc` | JSONC | user data | `theme` (1) | the person (hand edited); the app only reads and offers export | tolerant read; unknown keys ignored with a warning |
| `launch.jsonc` (created on demand; fourth config file, D-077) | JSONC | user data | `launch` (1) | the shell reads it before the webview exists; the person edits it, the app changes it only through CST edits (Linux graphics safe mode, [cross-platform](cross-platform.md) section 5) | tolerant read |
| `secrets.json` (fallback only) | JSON, mode 0600 or user-only ACL | secret | `secrets` (1) | `CredentialStore` fallback in `rimstudio-platform` | forward only; absent when the OS store works |

`settings.jsonc` holds app behaviour: language, theme choice, density, shortcuts, window geometry (remembered automatically through the debounced action `settings_set_window_state`, which the settings use case writes into the `window` key; the shell never writes files, D-076; no nine separate keys as in RimSort's catalogue, see [settings catalogue](../research/rimsort-settings-catalog.md) implication 7), scan thread cap, watcher policy, logging level, update channel, designer mode (simple or calibrated).

`workspace.jsonc` holds what describes the person's setup, and is the file that a portable install carries to another machine:

| Section | Content |
|---|---|
| `paths` | Overrides for game install, user data directory, Steam root, extra workshop directories, ignored installs. Each override has `pinned`; an override that fails validation is kept and reported as `deploy.override-invalid`, detection falls back for that run, and the choice is never deleted ([steam detection](../research/steam-and-game-detection.md) section 7.3) |
| `modSources` | Ordered list of sources (kind `game-data`, `game-mods`, `workshop`, `custom`) with `id`, `label`, `enabled`, `readOnly` |
| `customModFolders` | Per folder: `id` (generated once, `cf_` plus hex), `path`, `label`, `enabled`, `layout`, `scanDepth` (1 to 4), `watch`, `priority`, `readOnly`, `link` (`auto`, `links`, `copy`, `none`), `volumeHint`, `lastSeen` (R4: any number of entries) |
| `launch` | Launch arguments, `savedatafolder` choice, dev mode toggles |
| `datasets` | Per dataset id: `enabled`, source override, refresh hours (replaces RimSort's roughly 20 dataset keys, settings catalogue implication 4) |
| `activeProfile` | Profile id and last selected install id |

The Steam note proposes a separate `paths.jsonc`; D-030 folds that document into the `paths` section of `workspace.jsonc` to keep three config concerns (app, workspace, secrets); the tiny bootstrap file `launch.jsonc` is the one fourth file, kept apart because it is read before the webview exists (D-077). The shape is unchanged.

Every JSONC file starts with `"$schema": "<relative path or URL>"` (section 9) and may contain comments. A per-setting default is never written back: only keys the person changed exist, which keeps diffs small and lets defaults improve across versions.

### 3.2 Data root (user data, JSON unless noted)

| Path | Format | Schema (version) | Content and writer |
|---|---|---|---|
| `userdata/rules/user-rules.json` | JSON, lossless | `rules-user` (community format, version 1) | The user layer of the rule graph in the community `{timestamp, rules}` shape, unknown keys preserved. Written by `rimstudio-rules` export through `io`. This is the RimSort compatible file; the exporter can also write the community name `userRules.json` to a chosen folder |
| `userdata/rules/suppressions.json` | JSON | `rules-suppressions` (1) | Upstream rules the person muted, keyed by (subject, kind, target) with a reason |
| `userdata/ignore.json` | JSON | `ignore` (1) | Ignored mods and ignored diagnostic codes per mod, keyed by `ModId` identity (section 7) |
| `userdata/mod-meta.json` | JSON | `mod-meta` (1) | Tags, notes, colours, group membership and per-mod flags keyed by identity record; one file, written debounced by `manager::metadata` |
| `userdata/groups.json` | JSON | `groups` (1) | Group definitions (name, colour, order) |
| `userdata/lists/<id>.json` | JSON | `list` (1) | A profile: name, game version, ordered active `ModId` list with `packageId` fallbacks, notes. Written by `manager::profiles` |
| `userdata/history/<profileId>/<timestamp>.json` | JSON | `list` (1) | Automatic snapshots (profiles flagged `auto`), retention in section 8.3 |
| `userdata/designer/calibration.json` | JSON | `calibration-answers` (1) | The person's quiz answers and chosen mode; the tables derived from them are cache (R7) |
| `<collection>/<id>.json`, `<collection>/index.json`, `<collection>/.quarantine/`, `<collection>/.backups/<id>.json/` | JSON | envelope `{kind, v, data}` per kind | The in house JSON document store (section 16), one folder per collection directly under the data root as built: `projects` (project records, kind `project`) and `designer-drafts` (kind `designer-draft`), later profiles, notes and history |
| `userdata/imports/<timestamp>-rimsort.json` | JSON | `import-report` (1) | Report of one RimSort import: imported, skipped, unmatched rows (section 12) |
| `projects/<projectId>.json` (a collection document) | JSON envelope | `project` (1) | Project metadata kept outside the mod folder: folder path, name, packageId, created and last opened times, target game version and the last designer settings as an opaque value (D-063). `<projectId>` is `p-<8 hex>` derived from the normalised folder path. As built it is a collection document, not JSONC; the opt in `.rimstudio/project.jsonc` form and the import of the legacy `Config/rimstudio.project.json` are not built for 0.1.0 (`uploadIgnore` and publish settings arrive with the publisher) |
| `publish/<projectId>/state.json` | JSON | `publish-state` (1) | Last known Workshop id, change-note draft, last result |
| `publish/<projectId>/history.json` | JSON | `publish-history` (1) | Append-only list of publishes (time, helper protocol version, outcome code, manifest id) |
| `publish/<projectId>/manifests/<timestamp>.json` | JSON | `upload-manifest` (1) | The staged file list with sizes and blake3 hashes of what was uploaded |
| `deploy/<farmId>/ownership.json` | JSON | `link-ownership` (1) | Every link or copy the manager created under `<install>/Mods`: path, target, kind, created time. The only record that authorises unlinking (D-039, D-040) |
| `backups/modsconfig/ModsConfig-<timestamp>.xml` | verbatim foreign copy | none | Byte-exact copy of RimWorld's file taken before each write (section 8.2) |
| `<parent of the file>/backups/<file>/<file>.<yyyymmddThhmmssmmmZ>.bak` | as the original | as the original | Retention backups of a single user data file, UTC stamp with milliseconds (section 8.3); `<file>.bak-vN` is the pre migration copy |
| `project-backups/<projectId>/<folder of the file>/` | as the original | as the original | Backups of mod project files that the designer replaced, kept in the data root so that a Workshop upload never ships them (section 8.3) |

`<farmId>` is derived from the install id (`steam:<canonical library path>`), so two installs never share an ownership manifest.

### 3.3 Cache root (derived, JSON)

| Path | Format | Schema | Content, writer, invalidation |
|---|---|---|---|
| `library/<libraryId>.manifest.json` | compact JSON with string table | `scan-manifest` (v 1) | Per mod and per file records keyed by `(relative path, size, mtime ns, file id)`; `rimstudio-library` writes, debounced to at most once per second; invalidation in section 11 |
| `library/<libraryId>.snapshot.json` | compact JSON | `list-snapshot` (v 1) | The last mod list rows for cached first paint; marked stale until the verifying scan finishes |
| `defs/<libraryId>-<scope>.index.json` | compact JSON with string table | `def-index` (v 1) | Streaming def index per file, produced by `rimstudio-workspace` through the `rimstudio-xml` indexer (7.4 MB for 134k defs in the spike) |
| `types/<assemblySetHash>.type-table.json` | JSON | `def-type-table` (v 1) | Def type table generated from the user's assemblies (D-018, proposed) |
| `datasets/<id>/current.<ext>`, `previous.<ext>` | JSON or gzip of JSON | upstream format | Last good raw artifact and one rollback copy |
| `datasets/<id>/state.json` | JSON | `dataset-state` (1) | Status, ETag, content hash, counts, failures (section 6) |
| `datasets/<id>/index/<indexSchema>-<contentSha>.json` | compact JSON | `dataset-index` (v n) | Slim lookup index, rebuilt per content hash; the SteamDB slim index was 3.67 MB for 18,639 entries |
| `datasets/<id>/quarantine/<sha>.<ext>` plus `<sha>.reason.json` | as received plus JSON | `quarantine-reason` (1) | Rejected downloads, capped at 3 |
| `datasets/<id>/tmp/` | partial downloads | | Emptied at start |
| `ce/<ceVersionHash>.tables.json` | JSON | `ce-tables` (v 1) | Class statistics and conversion records read from the user's Combat Extended install; contains the user's values, so it is never shipped or committed (I-07) |
| `detection-report.json` (cache root) | JSON | `detection-report` (1) | Most recent `DetectionReport`, used to avoid re-probing on every start. It is derived data, so `rimstudio-manager` keeps it in the cache root (principle 3) |
| `library-manifest.json` | compact JSON | `scan-manifest` (v 1) | As built the scan manifest is one file in the cache root (see the first row of this table for the intended per library name); `{v, parser, strings, mods}` with tuple arrays and the full mod metadata of each mod so that offline source rows can be restored |
| `workspace-types/<hash>.json` (a collection) | JSON envelope | `workspace-type-table` | The def type table built by `rimstudio-workspace` from the game's managed assemblies and the mods' DLLs, keyed by a hash of the DLL path list and validated by the stat keys of the DLLs; one small document per distinct set of paths, not pruned |
| `designer-calibration/<id>.json` (a collection) | JSON envelope | `designer-calibration` (1) | Calibration metrics of the designer keyed by game version, reference set hash, index definitions and the harness version; written without backups or fsync (`CollectionOptions::cache()`); always safe to delete |
| `thumbnails/<hash>.jpg` | image bytes | none | Thumbnails of at most 256 pixels served through the `rsimg` scheme. Images are cache blobs, not a data format for app data |

### 3.4 Logs root

| Path | Format | Owner | Writer |
|---|---|---|---|
| `rimstudio.<yyyy-mm-dd>.jsonl` | JSON lines, one `tracing` event per line | log | `rimstudio-app` logging init; rolled daily, last 14 files and at most 50 MB kept, paths redacted (D-060; [error handling](error-handling-and-logging.md) section 6.2). No telemetry leaves the machine |
| `crash-marker.json`, `crash-<session>.json` | JSON | log | session crash marker written at start and removed on clean exit, and the panic report beside it ([error handling](error-handling-and-logging.md) section 9) |
| `webview-start.json` | JSON | log | marker written before webview creation and removed after the first painted frame, used by Linux graphics safe mode ([cross-platform](cross-platform.md) section 5) |

### 3.5 Things that are not files

| Item | Where it lives |
|---|---|
| API tokens and similar secrets | OS credential store behind the `CredentialStore` port (section 13) |
| Steam login state | Never stored by RimStudio; the Steam client owns it |
| Per-mod RimWorld settings (`Mod_<id>_<Name>.xml`) | RimWorld's config folder; read only and never edited |

## 4. Schema versioning

1. Documents carry `"schemaVersion": <integer>`; compact caches carry `"v": <integer>`. The version is part of the Rust type's constant (`SCHEMA_VERSION`) and of the generated schema.
2. Each document type has a migration table: `fn(Value) -> Result<Value, MigrateError>` from version N to N+1, pure, forward only, in `rimstudio-io::migrate`. Chains are applied in order in memory.
3. New optional fields use `#[serde(default)]` and need no version bump; removing, renaming or changing the meaning of a field bumps the version and adds a migration.
4. A historical fixture per version lives in `tests/fixtures/<document>/vN.json(c)`. One test loads every fixture and compares the migrated result to an expected struct; adding a version without a fixture fails CI (scenario S9 in the [overview](overview.md)).
5. A file whose version is newer than the app is opened read only with a banner and is never rewritten, discarded or downgraded. The app continues with in-memory defaults for anything it cannot read.
6. Caches follow the cheaper rule: a `v` mismatch means discard and rebuild, with no migration code (section 11).
7. Community and foreign formats (rules, SteamDB, UTI, versions) are not versioned by RimStudio. The reader has a `schemaSupport` range per dataset and keeps unknown keys; a format outside the range quarantines the download (section 6).
8. Unknown keys inside RimStudio's own JSONC files are preserved on CST edits, so a file edited by a newer build and opened by an older one does not lose data when that build changes one setting.

## 5. Identity keys that survive moves

User decisions (tags, notes, colours, ignore flags, profile membership) must not be lost when a folder is renamed, moved to another drive or switched from a local copy to the Workshop item. RimSort keys its rows by folder path, which is why its import needs an unmatched bucket ([rules fetch and merge](../research/rules-fetch-and-merge-design.md) section 8.3). RimStudio keys by identity.

### 5.1 The identity record

Every row in `mod-meta.json`, `ignore.json` and every entry in a list is addressed by a `ModId` string and carries enough to re-find the mod:

| Field | Role |
|---|---|
| `id` | `w<workshopId>` for Workshop items, `<sourceId>:<packageId>` (lowercase) for others |
| `packageId` | Lowercased package id; the fallback match |
| `workshopId` | String, when known (from the folder name or `PublishedFileId.txt`) |
| `pathHint` | Last seen absolute path, display and tie-break only |

Resolution order when loading a user file: exact `id`; then `workshopId`; then `packageId` within the effective source order; then `pathHint`. A row that matches nothing is kept, shown as "missing mod" with the last known name, and never deleted by a scan. 64-bit ids are strings everywhere on disk and on the wire.

### 5.2 Session handles are never persisted

`ModIdx(u32)` and `FileId(u32)` are valid for one `ModIndex` and are never written to disk or user-facing files. Interned string keys are written only inside a cache file together with its own string table.

### 5.3 Moves and drives

Custom folders carry `volumeHint`. When a custom folder's path is missing at start, the scan marks the source offline, keeps its cached rows greyed in the list, and re-checks when the volume mounts (focus or a watched parent). Dragging a folder to a new location through the Settings UI rewrites only `path`; every `ModId` stays valid because `<sourceId>` is the generated `cf_` id, not the path.

## 6. Datasets on disk

The five datasets (community rules, SteamDB, Use This Instead, No Version Warning, game versions) are fetched at runtime and never bundled or mirrored (R11, I-06). The pipeline, descriptors, limits and merge are specified in [rules fetch and merge](../research/rules-fetch-and-merge-design.md); this section fixes where the results live.

1. Each dataset has one slot `cache/datasets/<id>/` with `current`, `previous`, `state.json`, `index/`, `quarantine/` and `tmp/` as in section 3.3. Built-in descriptors are JSON compiled into the binary (they describe where to fetch, not the data); user overrides sit in `workspace.jsonc`.
2. A refresh writes to `tmp/`, validates (size caps, decompression ratio, parse, sanity floor), then rotates `current` to `previous` and renames the new file into place. A rejected download goes to `quarantine/` with a reason file; `current` is untouched, so the last good copy is always usable.
3. `state.json` is the single source of dataset status (`never-fetched`, `ready`, `stale`, `updating`, `rejected`, `offline`, `failed`, `disabled`) with ETag and encoding, content SHA-256, entry count, `lastChecked`, `nextCheckAfter`, failure count and the `previous` summary. Status never blocks use of the last good copy.
4. Raw artifacts stay verbatim. The No Version Warning list arrives as XML; it is converted once to a JSON id list through `rimstudio-xml` and the XML is not kept.
5. Derived indexes are named by index schema and content hash, so a parser change or a new download simply produces a new file and the old one is removed after the new one is loaded.
6. Datasets live in the cache root because they are fetchable. A person who needs offline reproducibility can export a dataset bundle (JSON plus a manifest of hashes) to a chosen folder; that is an explicit user action, not a mirror.
7. User rules are not a dataset: they live in the data root (section 3.2) and are layered over the datasets by `rimstudio-rules`. Clearing the cache never touches them.

## 7. Atomic writes

All writes go through `rimstudio-io` and nothing else calls `std::fs::write` on app data.

1. Write the full content to a temporary file in the same directory (so the rename stays on one volume), flush and fsync it, then rename over the target. The crate is `atomic-write-file` 0.3.1 ([crate research](../research/rust-crate-research.md) section 6.3), with `tempfile` persist plus an explicit fsync as the fallback.
2. Before replacing a user data file, take a backup (section 8.3).
3. Verify after write for user data: re-read and parse; on failure restore the backup and report `io.write-verify-failed`.
4. Network and removable drives can reject rename-over; the writer falls back to write, fsync, rename-away-then-in and records the fallback in the log.
5. Debounce: settings and metadata writes are coalesced to at most one per second per file; the manifest is rewritten at most once per second (budget in the [performance strategy](performance-strategy.md)). Setters never write; a store flushes on a timer, on blur and on exit.
6. A crash leaves either the old file or the new file, plus possibly a stray `tmp` file that is removed at start. A stale temporary file is only deleted when the file it belongs to exists; a crash inside the rename-aside fallback of item 4 leaves the target missing and is repaired at start by rolling the synced temporary file forward (or moving the aside copy back), never by deleting both. A temporary file next to a file in a user's mod folder can remain after a hard crash on every platform (section 19 of [security and privacy](security-and-privacy.md)).
7. Files are written with `\n` line endings and UTF-8 without BOM. Reads accept a BOM.

## 8. Backups and retention

### 8.1 Principles

Backups protect user data and the one foreign file RimStudio writes. Caches and datasets are not backed up (they have `previous` or are rebuildable).

### 8.2 ModsConfig.xml

`ModsConfig.xml` is RimWorld's file, and the game rewrites it at every launch and list save. The protocol (D-040):

1. The Launcher and deploy flows check that the game is not running (process probe plus Steam running flag, D-041); if the answer is unknown (sandbox) the person is asked to confirm.
2. Before any write, copy the current bytes to `backups/modsconfig/ModsConfig-<timestamp>.xml` and verify the copy by size and blake3 hash.
3. Edit by byte-span splice through `rimstudio-xml` (`activeMods` entries, `knownExpansions`, `version`), keeping the game's layout and any unknown elements; render from scratch only if the file does not exist.
4. Write atomically through `GameWriteFence`, which allows this one path.
5. Before the write, the pre-launch check verifies that every active id resolves in the effective mod set (including link farm entries), because the game silently deactivates ids it cannot find ([mod format](../research/rimworld-mod-format-and-corpus.md) section 4.2). A list that would lose ids blocks with an explanation.
6. After the game exits, diff the file against what was written and surface changes the game made (for example deactivated ids) as diagnostics `deploy.game-changed-list`.
7. Retention: keep the last 20 backups and the first backup of each calendar day for 30 days; never delete the most recent backup that precedes a RimStudio write.

### 8.3 User data

| Class | Backup rule |
|---|---|
| `settings.jsonc`, `workspace.jsonc` | Before every rewrite that changes the bytes, copy to `<parent>/backups/<file>/<file>.<yyyymmddThhmmssmmmZ>.bak` (policy `BackupPolicy::user_data()`: the last 10, everything of today and the last of each earlier day for 14 days; an identical copy is skipped; every backup is read back); before a migration also keep `<file>.bak-vN` (the pre-migration copy) indefinitely until the next successful migration |
| Collection documents (`projects`, `designer-drafts`) | `BackupPolicy::light()` (the last 3 plus today's) per document under `<collection>/.backups/<id>.json/`; derived collections such as `designer-calibration` keep no backups |
| Project files replaced by the designer | Before each replacement of an existing file the guarded writer copies it to `<data root>/project-backups/<projectId>/<folder of the file>/` by the `user_data` policy and reads it back; the backup path is reported as an absolute path |
| `user-rules.json`, `suppressions.json`, `mod-meta.json`, `ignore.json`, `groups.json` | Same, last 10, plus one daily backup for 14 days |
| `lists/*.json` | Edited lists are covered by the history snapshots below |
| `history/` | Automatic snapshot on every deploy, sort apply and launch; keep the last 50 per profile and thin older ones to one per day for 30 days; named snapshots are never thinned |
| `publish/*` | Append-only; no backup needed |
| `deploy/*/ownership.json` | Last 5 backups; the file is small and critical |

Backups are plain copies in the same format as the original so a person can restore by copying. A "Restore from backup" action lists them by time. Backup directories are counted in the storage summary of the settings screen with a "clear old backups" action that never removes the newest.

## 9. Comment-preserving JSONC edits and schemas

1. Read through `jsonc-parser` 0.34.0 with `serde` into the typed struct; edit through its `cst` feature so a settings change inserts, replaces or removes one value and touches nothing else (D-025). Comments, key order and the person's formatting stay.
2. Required operations: insert a key at a stable position, replace a value, remove a key, replace or append an array element, remove an array element by id. Spike S-02 is done: `jsonc-parser` 0.34.0 with `cst` supports every one of them (replace a value with `set_value`, insert a key at an index with `CstObject::insert` taking the siblings' indentation, remove a property together with its comma and attached comments, append, insert and remove array elements, replace an object element with `replace_with`), and layout, comments and trailing commas elsewhere stay byte identical (proved by tests in `rimstudio-io::jsonc`). Gaps that the editor works around: there is no set at path helper (`JsoncEditor` walks the path), numbers are inserted as raw text (formatted through `serde_json`), the CST nodes are `Rc` based and not `Send` (an editor lives on one thread inside one call), comments inside an object or array that is replaced as a whole are lost (so `sync_with` recurses into objects and replaces only what differs) and a key inserted before the first property lands after the leading comment block. `Store::save` syncs the typed value into the existing text through the CST with a baseline, so default valued sections that the person wrote out are neither removed nor filled in, and an unchanged save is a byte identical no op with no backup. The serde rewrite fallback with a visible warning stays available through `render_new` for hostile files and is not used by the app's edits.
3. Arrays of objects with an `id` (custom folders, sources) are edited by id, not by index, so a concurrent hand edit does not corrupt the target.
4. If the JSONC does not parse (a person typed a bad comma), the app does not overwrite it and does not move it aside (a hand edited file is never quarantined). `Store::load` returns the newest readable backup or the defaults with a `LoadProblem` carrying the code, line and column, `Store::save` and `edit` are blocked (`store.write-blocked`) until the file parses, and the UI shows the error with an "Open file" action and keeps edits in memory. A file newer than the app loads read only (`store.newer-than-app`).
5. Machine files (caches, state, history, lists) are plain JSON written by `serde_json`; they carry no comments.
6. JSON Schemas are generated from the Rust types with `schemars` through `cargo xtask schemas`, committed under `schemas/` and drift-checked in CI (D-027, I-19). Each user-edited file starts with `"$schema"` pointing to the relative schema path in portable mode and a stable URL otherwise, so editors give completion and diagnostics.
7. Schemas are documentation for people, not runtime validators. The Rust types are the validator; unknown keys are warnings except in user rules, where they are preserved.
8. The settings screen shows each effective value with its source (default, `settings.jsonc`, `workspace.jsonc`, environment or command line) and links to "Open file", so JSONC editing and the UI never feel like separate worlds.

## 10. The boundary with RimWorld's files

The product's data is JSON. RimWorld's files are XML (or plain text) and are handled in exactly one place.

### 10.1 Rules

1. All XML parsing and rendering is in `rimstudio-xml`. Other crates exchange node trees (`{tag, attrs, children}`) and typed codec results; no other manifest lists an XML crate (I-02, D-012).
2. Existing foreign files that a person or the game may have formatted are edited by byte-span splice (D-013). Files that do not exist yet, and templates, are rendered.
3. No RimStudio-internal file is XML: no settings, caches, projects, templates, datasets, IPC payloads, exports or golden outputs (R10).
4. Reading is tolerant (Tolerant mode with partial results and warnings); the Game mode mirrors the game's loader for the def pipeline.

### 10.2 What RimStudio reads and writes

| File | Location | Read | Write | How |
|---|---|---|---|---|
| `About/About.xml` | each mod | yes (all mods, lenient event reader) | project tools only, in the person's own mod, splice edit | `xml::about` |
| `LoadFolders.xml` | each mod | yes | project tools only, splice edit | `xml::load_folders` |
| `Defs/**/*.xml`, `Patches/**/*.xml` | each mod | yes (streaming indexer; node trees on demand) | designer export and scaffolder only, into the person's own project, rendered or spliced | `xml::defs_scan`, `xml::render`, `xml::edit` |
| `Languages/**` | each mod | later (translation tools) | later | same boundary |
| `ModsConfig.xml` | game config folder | yes | yes, fenced (section 8.2) | `xml::mods_config` |
| `Prefs.xml`, `Knowledge.xml`, `KeyPrefs.xml`, per-mod `Mod_*.xml` | game config folder | read selected keys only (dev mode, verbose logging flag) | never | a `prefs` codec in `rimstudio-xml`, added when the dev launcher needs it (M4) |
| Save file meta (`.rws` header) | game saves folder | optional, header only, for "which mods does this save need" | never | `xml::save_meta` |
| `Player.log` | game log folder | yes, text | never | `validate::log` |
| `Version.txt` | install | yes, text | never | `steam` |
| `PublishedFileId.txt` | mod `About` | yes | project and publish tools write it after first publish, plain text | `io` |
| RimSort `modlists/*.xml` | RimSort data | import only | no | boundary crate, converted to list JSON |
| No Version Warning `ModIdsToFix.xml` | downloaded | yes, once | no | converted to JSON id list |

### 10.3 The write fence

`GameWriteFence` has two allow lists and refuses everything else: (a) link or copy entries under `<install>/Mods` recorded in an ownership manifest, and unlinking only entries listed there; (b) `ModsConfig.xml`. Mod folders in a person's custom or workshop folders are never modified by the manager. Project tools write only inside the project folder that the person registered, and only through a `WritePlan` the person confirmed with a diff. Unit tests with a recording filesystem assert the write set of every use case.

Custom folders reach the game only through the managed link farm (junction or symlink with a copy fallback), because the game builds its list from the install `Data`, install `Mods` and Steam-subscribed items alone ([steam detection](../research/steam-and-game-detection.md) section 8.6). The ownership manifest is what makes the farm safe: the manager never removes anything it did not create, and a missing manifest means a read-only farm audit.

## 11. Cache invalidation

Caches are keyed so that a wrong cache cannot survive: a stale value must be detectable from data already on disk.

| Cache | Key | Invalidated when |
|---|---|---|
| Scan manifest | per file `(relative path, size, mtime ns, file id)` compared for equality only; mod folder by absolute path | `v` or parser version changes (discard all); file key differs (re-parse that file); file vanished (drop); game version from `Version.txt` changes or load-folder resolution rules change (rebuild affected mods); JSON parse failure (treat as absent) |
| FAT-family volumes | same key but mtime rounded to 2 s; a size match with a rounded mtime falls back to a blake3 content hash | the volume is detected as FAT, exFAT or a network share; the hash is computed lazily and stored in the manifest |
| List snapshot | manifest revision and active profile revision | verifying scan changes any row; always marked stale until the scan completes |
| Def index | per file key as above, plus the effective load-folder set | any key or load-set change re-indexes that file only |
| Def type table | blake3 of the ordered list of assembly files (path, size, mtime) of Core, DLC and active mods | any assembly changes; the old table is deleted after the new one loads |
| `DefDatabases` snapshot | not persisted in v1 (memory only) | rebuilt per job when the active list or any def file key changes |
| CE tables | hash of the Combat Extended version string plus its def file keys | CE updated, removed or reinstalled; the designer disables calibrated mode when CE is absent |
| Dataset slim index | index schema plus content SHA-256 | new download or parser change |
| Detection report | install and library mtimes and the `libraryfolders.vdf` key | any watched Steam file changes, or the person presses "Re-detect" |
| Thumbnails | hash of source image key and requested size | source key changes |

Rules that apply everywhere:

1. Compare for equality, never "newer than": mtime granularity differs per filesystem and clocks go backwards.
2. A corrupt or truncated cache file is treated as absent, never as an error, and is rebuilt.
3. A cache write failure is logged and ignored; the in-memory result is still correct.
4. Everything in the cache root can be deleted while the app is closed and the next start is only slower. This is a tested property: a CI test deletes the cache root between two runs of the CLI and compares golden outputs.
5. Paths are stored as UTF-8 text; a path that is not valid UTF-8 is kept as a lossy display string with a flag and reported as `scan.non-utf8-path`, never silently renamed.

## 12. Import and export

### 12.1 Import from RimSort

Procedure (read only, never writes into RimSort's directory, does not migrate secrets), from [rules fetch and merge](../research/rules-fetch-and-merge-design.md) section 8:

1. Detect candidate RimSort data directories per OS (Linux `~/.local/share/RimSort` verified; macOS and Windows paths unverified) plus a "choose folder" fallback.
2. Show a checklist with counts: user rules, saved lists, notes and colours, ignore list, dataset source choices.
3. User rules: read losslessly and store as `userdata/rules/user-rules.json`.
4. Saved lists (RimWorld-shaped XML): parse through the boundary crate and store as list JSON with the recorded game version.
5. Notes, colours, tags, ignore flags: mapped to identity records by folder path against RimStudio's own scan, or by `published_file_id` to workshop id. Unmatched rows are listed in the import report and kept as unmatched entries, never dropped.
6. Dataset source choices map to descriptor overrides in `workspace.jsonc`.
7. The import writes an `import-report` JSON and is idempotent: running it again updates rows instead of duplicating them.

Notes and colours live in RimSort's `aux_metadata.db`, an SQLite file. Reading it needs a read-only SQLite dependency (`rusqlite` behind feature `aux-db` in `rimstudio-datasets`, D-031). This is the single recorded dependency on a foreign format; the owner accepted it on 2026-10-04 (D-083) because it applies to reading someone else's file, not to storing app data, and the feature stays off by default until the datasets crate exists. Without the feature, the import still handles rules, lists, ignore list and dataset choices, and offers a one-off documented export step from RimSort for notes (owner decision, section 15).

### 12.2 Export formats

| Export | Format | Notes |
|---|---|---|
| Mod list | RimStudio list JSON; RimWorld `ModsConfig`-style XML and RimSort-compatible XML for interchange | XML written only through `rimstudio-xml::render` when the person chooses that format |
| User rules | Community `{timestamp, rules}` JSON, lossless, ready for a pull request to the community repository | Only entries the person asks for; redundancy detection removes rules already upstream |
| Profile bundle | One JSON file with a profile, its tags, notes and colours | For sharing a setup; never includes paths or secrets by default |
| Diagnostics report | JSON | Redacted paths, no credentials; for bug reports |
| Dataset bundle | JSON plus a hash manifest | Section 6 item 6 |
| Preferences | `settings.jsonc` and `workspace.jsonc` copied as they are | Portable between machines, secrets excluded |
| Item designer output | XML patch and def files into the project folder | Rendered or spliced by `rimstudio-xml`; the JSON node tree is the internal form |

Exports never contain Combat Extended or vanilla value tables; those are derived at run time from the person's install (R11, I-07).

## 13. Secrets

1. Anything that grants access (a future API key, a Steam web token if ever needed) is stored through the `CredentialStore` port. The implementation in `rimstudio-platform` uses the OS store (Keychain, Credential Manager, Secret Service) via `keyring` 4.2.0 (default `v1` feature, native stores per OS; D-030).
2. Where no store is available (a headless Linux session, a sandbox), a fallback file `secrets.json` in the config root is created with owner-only permissions (mode 0600; a user-only ACL on Windows) and the UI says so. The fallback is opt-in per secret, not silent.
3. Secrets never appear in `settings.jsonc`, `workspace.jsonc`, logs, diagnostics, exports, crash information or IPC responses. DTOs carry "is set" flags only. The log layer redacts known secret patterns and all paths under the home directory.
4. RimStudio's Workshop publishing uses the signed-in Steam client the user already has installed (D-086) through the sidecar and stores no Steam password or token; if S-04 shows that this holds, the credential store may stay unused in v1.
5. RimSort secrets (Steam credentials in its settings) are never imported.

## 14. Test and fixture policy

1. Every document type has golden JSON outputs and the version fixtures of section 4; tests run identical results at 1 and 8 threads.
2. A recording filesystem asserts write sets (fence tests) and that atomic writes never leave the target half written (fault injection between temp write and rename).
3. A property test round-trips the lossless rules model with arbitrary unknown keys and order.
4. Fixtures use fictional numbers and synthetic install trees from `rimstudio-testing`; real-install tests are `#[ignore]` behind an environment variable (R11).
5. `cargo xtask check-docs` verifies that every file listed in section 3 appears in the schema folder or is marked as having none.

## 15. Decisions, risks and owner decisions

| Decision | Choice | Alternative rejected | Evidence |
|---|---|---|---|
| Config formats | JSONC for user-edited, JSON for machine files | One format for both; TOML or YAML | R10; [crate research](../research/rust-crate-research.md) section 6.2 |
| Document storage | In house JSON document store, one file per document (section 16, D-083) | SQLite or another embedded database, a key value engine, one large file per collection |
| Cache format | Compact string-table JSON | bincode, rkyv, SQLite | Compact JSON loaded in 21 to 24 ms against 20 ms for bincode ([scan spike](../research/scan-performance-spike.md) S4) |
| One manifest per library | Single file at first; split per mod only if measured | 700 small files from the start | Spike recommendation; D-023 |
| Identity | `ModId` plus packageId and workshop id fallbacks | Path keys as in RimSort | Moves and drive changes must not lose notes |
| Datasets location | Cache root | Data root; repository | Unlicensed, fetchable (R11) |
| Settings split | `settings.jsonc`, `workspace.jsonc`, credential store | One file | D-030 |
| Backups | Plain copies with retention, verified | Versioned databases | Easy manual restore |

Risks:

| Risk | Impact | Mitigation |
|---|---|---|
| CST edit API cannot do ordered insert or remove | Comments lost on write | Spike S-02 first; fallback with warning and backup |
| Rename-over unsupported on a network drive | Partial writes | Fallback writer and log; data root is local by default |
| User edits JSONC while the app runs | Lost edit or overwrite | Watch config files; on external change reload and merge, ask on conflict |
| Windows antivirus slows many small files | Slow cold start | Few large files (single manifest) and measurements in M2 |
| Identity collisions (two copies of one packageId) | Wrong notes on a mod | `ModId` includes the source; duplicate resolution policy from the steam note section 8.4 |
| Application identifier changes after release | Users lose settings | Owner chooses before first release; migration note if changed |

Owner decisions needed:

1. The reverse-DNS application identifier (fixes folder names on every OS, D-049).
2. Whether `keyring` and the restricted fallback file are acceptable for secrets (D-030).
3. Whether the RimSort-compatible name `userRules.json` should also be written by default beside `user-rules.json` for tools that expect it (assumed: only on export).

Open measurements: Windows and macOS directory names and permissions (M2), write cost of the single manifest on a 5000 mod library, and behaviour of the atomic writer on exFAT and network shares.

## 16. The in house JSON document store

Decision D-083 ([ADR 0035](../adr/0035-in-house-json-database.md)). Some data is many small records that are saved, listed and migrated one by one: designer drafts, projects, calibration caches and, later, profiles, notes and history. They live in the document store of `rimstudio-io`, a directory based database made only of JSON files. It is not a query engine; it is a typed folder of documents with an index. Settings stay one JSONC file edited through the CST (section 9, `Store::open(&DataRoots, RootKind, name, clock)` then `load()`, `save(&T)` and `edit(..)`) and are not part of the store.

### 16.1 Layout and envelope

1. A collection is a directory under the root that matches its owner class (user data under the data root, derived caches under the cache root). Each document is one file `<id>.json`. Ids are sanitised single path components (no separators, no reserved names) and are case folded for comparison.
2. Every document is wrapped in the versioned envelope `{"kind": "<collection kind>", "v": <integer>, "data": { ... }}`. `kind` names the Rust type, `v` is its schema version and `data` is the payload. Key order inside `data` is stable (`preserve_order`) so the same value writes the same bytes.
3. Each collection has one derived index file `index.json` with a compact row per document (`id`, `v`, `size` and a `summary` value built by an optional per type summariser; there is no fixed title field). Listing reads only the index. The index is derived (I-17): it is compared with the folder listing (ids and sizes) and rebuilt when it is missing, unreadable or stale, and a rebuild never changes a document.
4. Layout as built: `<id>.json` (ids are 1 to 128 characters of `a-z0-9._-`, the id `index` is rejected), `index.json`, `.quarantine/` (moved documents with a `<id>.<ms>.reason.json` beside each), `.backups/<id>.json/` (verified backup copies when a policy is set), `.lock` (informational: it names the process that opened the folder, nothing enforces it) and the reserved but unused `.journal`. A collection folder sits directly under its root (`<data root>/projects`, `<cache root>/designer-calibration`).

### 16.2 Guarantees

| Guarantee | How |
|---|---|
| Atomic writes | Temp file in the same directory, then rename, through `atomic_write` (section 7); the index is written after the document |
| Verified backup | Before a rewrite the previous bytes are kept under the retention policy of section 8.3 (`CollectionOptions::user_data()`: light backups, read back verification, fsync; `CollectionOptions::cache()`: none) |
| Forward only migrations | `kind` plus `v` select the chain in `rimstudio-io::migrate`; pure functions on `serde_json::Value`, applied in memory on read and persisted on the next save |
| Newer than the app | A document with a larger `v` than the app knows is returned read only and is never rewritten, as in section 4 item 5 |
| Quarantine | A document that is not valid JSON, has the wrong kind or envelope, or fails its migration or decoding is moved to the collection's `.quarantine/` folder with a reason file and reported as `store.document-corrupt`, listed by `quarantined()` and never deleted; the rest of the collection keeps working. A document newer than the app is returned read only and is never rewritten, moved or deleted. Quarantine applies to collection documents only, never to single JSONC files |
| Limits | A document file over 64 MiB (`MAX_DOCUMENT_BYTES`) is quarantined without being read and a larger value is refused on write; settings files over 16 MiB are refused and saving is blocked; settings are read as strict UTF-8 (another encoding blocks saving and is never rewritten); a put over an unreadable document is refused when the old bytes cannot be moved to the quarantine; above 1000 documents the index file is written by the next listing instead of after every put and a stale file is detected by comparing ids and sizes |
| Concurrency | Access inside one process is serialised by a per collection lock; across processes a single writer is assumed (no cross process lock); there are no transactions across documents, a batch of puts can stop half way |
| Determinism | The same documents produce the same index bytes; listing order is by id unless a type declares another order |

### 16.3 The Collection API

`collection::Collection<T: Versioned>` is the typed entry point (the signature sketch is in the [crate catalog](crate-catalog.md) under `rimstudio-io`):

1. `Collection::open(&DataRoots, RootKind, name, Arc<dyn Clock>)` creates or opens the collection (the clock stamps backups and quarantine files); `open_dir(dir, clock)` takes an explicit folder. `with_summarizer`, `with_options(CollectionOptions)` and `with_registry` configure it; `Clone` is cheap and shares the lock and index.
2. `put(id, &T)` writes one document atomically and updates the index; `get(id) -> Result<Option<Loaded<T>>>` reads, migrates in memory and type checks one document (the migrated form is written back lazily, best effort); `scan()` iterates all documents without writing; `delete(id) -> Result<bool>` removes it and its index row; `exists`, `len` and `list_ids()` (sorted) are cheap.
3. `list_summaries() -> Vec<IndexEntry { id, v, size, summary }>` returns the index rows; `rebuild_index()` forces a rebuild; `quarantined() -> Vec<QuarantineEntry { id, path, reason, at_ms }>` returns the moved aside files.
4. Errors are `StoreError` variants with stable codes (for example `store.document-corrupt`, `store.newer-than-app`, `store.id-invalid`); content problems are never panics.

### 16.4 What it is used for

| Collection | Owner class | Content |
|---|---|---|
| `designer-drafts` | user data | A designer item in progress: the design spec, the mode (simple or quiz), the quiz answers and the CE patch block (absent by default) |
| `projects` | user data | Project metadata (the opt in JSONC form for hand editing is not built) |
| `designer-calibration` | derived cache | Calibration metrics keyed by game version, reference set hash, index definitions and harness version; always safe to delete |
| `workspace-types` | derived cache | The def type table of a reference set (section 3.3) |
| `profiles`, `notes`, `history` (later) | user data | Replace the single files of section 3.2 only if measurement shows a need; until then the files above stay |

### 16.5 The foreign format exception

A dependency that only reads a foreign format is acceptable. The single instance is `rusqlite` behind the feature `aux-db` in `rimstudio-datasets`, opened read only and used only to import RimSort's `aux_metadata.db` into RimStudio's own JSON (section 12.1). It is never used for app data and the feature is off by default until the datasets crate exists.
