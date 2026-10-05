# Rust crate research for RimStudio's backend

This note evaluates the Rust ecosystem choices for RimStudio's backend workspace (Tauri 2.12.x shell, headless domain crates, a Steam helper sidecar) in ten categories. Every recommended or rejected crate was checked against the crates.io API on 2026-10-04 (generic User-Agent, 672 crates fetched by `docs/research/data/rust-crates/fetch_crates.py` (summary in `crates_summary.tsv`)), and decisive claims were checked against the crate repositories, the Tauri documentation sources, the decompiled game code, or a small experiment. It gives a recommended-crate table, a rejected list, a risk register, a root `Cargo.toml` skeleton, a cargo-deny rule that confines XML crates to one boundary crate, and an example Tauri plugin layout. The XPath question has its own note: `docs/research/xpath-patch-coverage.md`.

Status: research note | Last verified: 2026-10-04

## 1. Reading guide and method

| Item | Convention |
|---|---|
| Version | `max_stable_version` from crates.io on 2026-10-04 (pre-releases named explicitly, never recommended unless stated) |
| MSRV | the `rust-version` field of that release; "none" means the crate declares none |
| Maintenance signal | date of the stable release, `recent_downloads` (crates.io 90 day counter) and whether a pre-release line exists |
| "(unverified)" | claim from memory or not reproducible here |
| Toolchain on this machine | rustc 1.96.0 (2026-05-25); the highest MSRV among recommended runtime crates is 1.90 (Tauri), so the workspace MSRV is 1.90 unless an optional feature raises it |
| R10 | app-owned data is JSON, configs are JSONC; TOML/YAML/SQLite/XML are rejected for app-owned data. Cargo.toml, deny.toml, `.cargo/config.toml`, `rust-toolchain.toml` and CI files are tool configuration, not app-owned data, and stay in the formats their tools require |

Cross-references: IPC typing and webview performance are in `docs/research/webview-and-ipc-performance.md`; Steam location and VDF in `docs/research/steam-and-game-detection.md`; Workshop publishing in `docs/research/workshop-publishing-research.md`; JSON parse cost in `docs/research/community-datasets-analysis.md` (section 4.3).

## 2. Capability checklist: what RimSort and RimCrow depend on

Dependency lists are from `RimSort-main/pyproject.toml` and `RimCrow-main/pyproject.toml`. "Needed" means RimStudio needs the capability, not the library.

| Python dependency (project) | Capability | Needed by RimStudio | Rust answer (section) |
|---|---|---|---|
| aiohttp, requests, certifi, pip-system-certs (RimSort, RimCrow) | HTTPS with system or bundled roots | yes (R6 dataset fetch) | reqwest with rustls + platform verifier (6) |
| beautifulsoup4 (both) | scraping Workshop HTML pages | maybe: only if the Steam Web API route is not enough | `scraper` not evaluated; prefer API/JSON endpoints (open question 3) |
| lxml (both) | XML parse, XPath | yes at the RimWorld boundary | quick-xml + in-house xpath (2) |
| msgspec, pydantic (RimSort, RimCrow) | typed (de)serialisation | yes | serde, serde_json, schemars (4) |
| loguru, colorlog, icecream | logging | yes | tracing (9) |
| click | CLI | optional (xtask, headless tool) | clap 4.6.7 (10) |
| natsort | natural sort | yes (mod name sort) | alphanumeric-sort (5) |
| networkx, toposort | graph, topological sort, cycles | yes (load order) | petgraph (5) |
| platformdirs | config and cache dirs | yes | Tauri path API plus `directories` in tools (4) |
| psutil | find running RimWorld/Steam process, memory | yes (warn "game is running") | sysinfo 0.39.6 (3, 10) |
| pygit2, pygithub (RimSort) | git clone of datasets and mods, GitHub API | partly | HTTPS fetch instead of git for datasets; git via `git` binary for projects (8) |
| PySide6 (RimSort), pywebview (RimCrow) | GUI | replaced by Tauri | (1) |
| rapidfuzz | fuzzy search | yes (instant search) | nucleo-matcher, strsim (5) |
| sqlalchemy (RimSort), peewee (RimCrow) | local database | rejected by R10 | JSON files (4) |
| steam (RimSort), steamfiles, steam-vdf (RimCrow), SteamworksPy | Steam client, VDF/ACF, Workshop subscribe and publish | yes (R3, R5) | keyvalues-parser or own parser, steamlocate, steamworks sidecar (6) |
| watchdog | file watching | yes, carefully | notify 8.2.0 (3) |
| zstandard (both) | compressed archives, dataset downloads | low | zstd 0.14.0 or ruzstd, only if needed (8) |
| packaging, python-dateutil, base58 | version parsing, dates, Steam id encoding | yes (version parse) | custom parser for "1.6.4871 rev598" (5); jiff/time not needed beyond epoch ints |
| keyring (RimCrow) | secret storage (API keys) | yes if any token is stored | keyring 4.2.0 (4) |
| pathspec (RimCrow) | gitignore-style patterns | yes (upload staging) | ignore, globset (3) |
| send2trash (RimCrow) | move to recycle bin | yes (safe delete of mods) | trash 5.2.9 (3) |
| pillow (RimCrow) | image resize, preview checks | yes | image, fast_image_resize, imagesize (7) |
| python-docx, reportlab | document and PDF export | no (non-goal) | none |
| json-repair, litellm, openai (RimCrow) | assistant features | no (not in the owner requirements) | none |
| nuitka, pyinstaller | packaging | replaced by Tauri bundler | (1) |
| pywin32 | Windows APIs | maybe | windows-sys / windows-registry (3, 10) |

## 3. Category 1: Tauri 2.12.x, plugins, capabilities, sidecar, modularity

### 3.1 Core versions (all Apache-2.0 OR MIT, edition 2024, MSRV 1.90 unless noted)

| Crate | Stable | Released | Note |
|---|---|---|---|
| tauri | 2.12.1 | 2026-09-30 | `3.0.0-alpha.4` published 2026-10-01: stay on 2.12 and treat Tauri 3 as a later migration |
| tauri-build | 2.7.1 | 2026-09-30 | |
| tauri-plugin | 2.7.1 | 2026-09-30 | build-time helper for plugin crates |
| tauri-cli | 2.12.1 | 2026-09-30 | install as a dev tool, not a dependency |
| wry / tao | 0.57.0 (MSRV 1.85) / 0.37.1 | 2026-09-08 / 2026-09-26 | webview and window layers, pulled by tauri |

Tauri's `config-json5` and `config-toml` features exist (crates.io feature list); the default `tauri.conf.json` is JSON and fits R10. Tauri itself pulls `plist` (normal, non-optional dependency of tauri and tauri-utils), which depends on quick-xml ^0.42: an XML parser is therefore always in the build graph, and the "single XML crate" rule must be a rule about our own direct dependencies (section 11.3).

### 3.2 Plugins

All are 2.x crates with MSRV 1.90 and licence Apache-2.0 OR MIT; each also has a 3.0.0 alpha line published 2026-09-30 (except where blank).

| Plugin | Stable (date) | Use for RimStudio | Decision |
|---|---|---|---|
| dialog | 2.8.1 (2026-10-01) | folder and file pickers (R4 custom mod folders, game path override), message boxes | adopt |
| opener | 2.7.0 (2026-09-29) | reveal a mod in the file manager, open Workshop pages in the browser | adopt |
| process | 2.4.0 (2026-09-26) | exit and relaunch after an update | adopt (small) |
| updater | 2.13.1 (2026-09-29) | in-app updates; signing is mandatory and cannot be disabled (Tauri docs, plugin/updater.mdx) | adopt later (release engineering) |
| single-instance | 2.5.2 (2026-10-01) | prevents two processes writing the same settings and watching the same folders; on Linux it uses D-Bus (docs list dbus interface) | adopt |
| window-state | 2.5.0 (2026-09-26) | remembers size and position. Its state file format is plugin-defined (JSON or binary, unverified), so it is an R10 question | adopt only if its file is JSON; otherwise store window geometry in `settings.jsonc` ourselves |
| log | 2.10.0 (2026-09-26) | log bridge | reject: use `tracing` (section 10) |
| global-shortcut | 2.4.0 | system-wide hotkeys | skip, not a requirement |
| notification | 2.5.1 (2026-10-01) | "download finished" toast; depends on notify-rust | optional |
| clipboard-manager | 2.4.1 (2026-10-01) | copy package ids, mod lists | optional (the webview clipboard API may suffice) |
| os | 2.4.0 | OS info | skip: `std::env::consts` plus `sys-locale` 0.3.2 |
| fs | 2.6.0 (7.8M recent downloads) | webview-side file access | reject: expose only purpose-built scoped commands, so the webview never has general file access |
| persisted-scope | 2.4.0 | persists fs/asset scope grants | skip unless fs or the asset protocol is enabled |
| shell | 2.4.0 | sidecar spawn from Rust or JS | adopt only for sidecar spawning, see 3.4 |
| store | 2.5.0 | key-value file | reject: format is not JSONC and not ours |
| sql | 2.5.0 | SQLite | reject: R10 |

### 3.3 Capabilities model

Facts from the Tauri documentation sources (accessed 2026-10-04, `tauri-docs/v2/src/content/docs/security/capabilities.mdx`): capability files grant permissions to named windows or webviews (`"windows": ["main"]`, wildcards allowed), may be limited to platforms (`linux`, `macOS`, `windows`, `iOS`, `android`), and apply to IPC calls from the webview. Plugin permissions are named `plugin:permission` and generated from the plugin's command list (`permissions/autogenerated/commands/<cmd>.toml`; the fs plugin ships the pattern, see `security/permissions.mdx`). Capability and permission files are Tauri's own tool format (JSON/TOML/JSON5 chosen per file), not app-owned data under R10.

Recommended policy:

1. One capability file per window role (`main.json`, later `splash.json`), never `"windows": ["*"]` for anything beyond `core:default`.
2. The webview receives only app-command permissions and dialog, opener, process, updater, and event/window core permissions. No `fs:*`, no `shell:*`, no `http:*` permission, so a compromised or buggy frontend cannot touch the file system or spawn processes except through RimStudio commands that validate their arguments.
3. App commands are the security boundary, so every command that takes a path validates it against the known roots (game, workshop, registered custom folders, project folders) in the Rust layer.

### 3.4 Sidecar (externalBin) for the Steam helper

Tauri docs (`develop/sidecar.mdx`): list the binary under `bundle.externalBin` (for example `binaries/rimstudio-steam-helper`), supply one file per target triple named `<name>-<target-triple>` (for example `-x86_64-unknown-linux-gnu`, `-aarch64-apple-darwin`), spawn from Rust with `app.shell().sidecar("<file name only>")`, and grant `shell:allow-execute` only for JS-side execution. The sibling note recommends a separate Rust executable `rimstudio-steam-helper`, spawned per operation, speaking newline-delimited JSON over stdio (`docs/research/workshop-publishing-research.md`, section 6.1). That matches R10 (JSON IPC) and keeps Valve's redistributable out of the main process.

Practical notes: build the helper in the same workspace and copy it to `src-tauri/binaries/<name>-<triple>` from an `xtask` step; do not give the webview any `shell:` permission and spawn it only from a Rust command (capabilities gate webview IPC; whether Rust-side `ShellExt` calls are subject to them is stated from the ACL model and not verified here). Windows needs `.exe` handling and macOS needs the sidecar signed and notarised with the app (unverified details, check at packaging time).

### 3.5 Modularising many commands: plain modules or one plugin per feature

| Option | What it is | Pros | Cons |
|---|---|---|---|
| A. Plain command modules | `#[tauri::command]` functions in `src/commands/<feature>.rs`, one `generate_handler!` list in `main.rs` (the docs show `commands::my_custom_command` paths, `develop/calling-rust.mdx`) | least ceremony | one flat permission namespace; the handler list grows to dozens; features cannot be compiled out |
| B. One Tauri plugin crate per feature | `tauri::plugin::Builder::new("mods").invoke_handler(...).setup(...).build()`, a `build.rs` calling `tauri_plugin::Builder::new(COMMANDS).build()` (verified in the official `os` plugin: `plugins/os/build.rs` lists `COMMANDS` and `src/lib.rs` has `pub fn init<R: Runtime>() -> TauriPlugin<R>` with `Builder::new("os")`), generated per-command permissions plus a `default` set | per-feature capability sets (`mods:default`), per-plugin state and setup, separate crate and tests, compile-time feature gating, IPC command names namespaced as `plugin:mods|scan` | more boilerplate: each plugin needs a build script and permission files; typed binding generation (tauri-specta) must be wired per plugin (unverified how tauri-specta handles plugin commands) |
| C. Hybrid (recommended) | domain logic in headless crates with no Tauri dependency; thin adapter crates `rimstudio-tauri-<feature>` that are Tauri plugins for the 6 to 8 big features | domain testable without a webview; permissions per feature; small main.rs | adapter layer to maintain |

Recommended layout:

```text
rimforge-studio/            (to be renamed)
  Cargo.toml                virtual workspace
  crates/
    rimstudio-core/         ids, errors, paths, settings model (no Tauri, no XML crate)
    rimstudio-xml/          THE xml boundary (quick-xml); node tree, reader, writer, About.xml editor
    rimstudio-xpath/        xpath parser and evaluator over the node tree
    rimstudio-steam/        locate Steam, ACF/VDF, workshop scan
    rimstudio-mods/         scan, About ingestion, load order graph
    rimstudio-rules/        datasets fetch/merge, user rules import
    rimstudio-patch/        patch operations simulator
    rimstudio-publish/      staging, manifest, protocol types
    rimstudio-ipc-types/    serde + specta DTOs (no Tauri)
    rimstudio-tauri-mods/   Tauri plugin: commands + permissions for the mod manager
    rimstudio-tauri-toolkit/  Tauri plugin: project workspace, def explorer, item designer
    rimstudio-tauri-publish/  Tauri plugin: publish commands, sidecar spawn
  apps/rimstudio/src-tauri/ app shell: registers plugins, capabilities/main.json, tauri.conf.json
  tools/rimstudio-steam-helper/   sidecar binary
  xtask/                    codegen, fixtures, deny checks
```

The app shell registers `.plugin(rimstudio_tauri_mods::init())` and so on; each plugin's `permissions/default.toml` lists its commands, and `capabilities/main.json` references `mods:default`, `toolkit:default`, `publish:default`. Plugin crate prefix: `rimstudio-tauri-*` keeps the "rimstudio-" prefix rule.

## 4. Category 2: XML at the boundary only

### 4.1 What the game accepts (verified in decompiled code)

| Input | Loader | Behaviour |
|---|---|---|
| Defs and Patches files | `LoadableXmlAsset` (decompiled:Verse/LoadableXmlAsset.cs) | UTF-8 BOM stripped, bytes decoded as UTF-8, comments ignored, insignificant whitespace ignored, character checking off (illegal XML characters pass), then `XmlDocument.Load`. Any exception: warning logged, document null, file skipped; the combiner then logs "unknown parse failure" (decompiled:Verse/LoadedModManager.cs, CombineIntoUnifiedXML). A root element not named `Defs` is logged but its children are still imported. |
| About.xml, LoadFolders.xml | `DirectXmlLoader.ItemFromXmlFile` then `XmlDocument.LoadXml` on `File.ReadAllText` | Default settings: comments kept in the DOM (harmless), no whitespace or character relaxations; failure yields an empty default object and an error log (decompiled:Verse/DirectXmlLoader.cs, ItemFromXmlString; decompiled:Verse/ModMetaData.cs reads About through it). A missing file also yields a default object. |

What .NET's reader rejects (well-formedness): mismatched or unclosed tags, multiple roots, unescaped `&` or `<` in text, undefined entities such as `&nbsp;`, duplicate attributes, invalid names. The corpus has 6 unparseable workshop patch files (`docs/research/data/xpath-corpus/summary.json`, `xml_parse_status_outside_ok`). Behaviour on DTDs is not verified here. Consequence: RimStudio needs two modes in the boundary crate: Game mode (fail where the game fails and report the same class of error, used by validation) and Tolerant mode (best-effort recovery, used to show and fix a broken file in the editor).

### 4.2 Parser candidates

| Crate | Stable (date) | Licence | MSRV | Signal | Fit |
|---|---|---|---|---|---|
| quick-xml | 0.42.0 (2026-08-22) | MIT | 1.86 | 125M recent downloads, edition 2024, also pulled by plist | Pull/event reader and writer; comments, CDATA, PI events kept; byte positions available for minimal-diff edits; no tree. Strict end-name checking is configurable (`check_end_names`, verify when coding) |
| roxmltree | 0.21.1 (2025-10-12) | MIT OR Apache-2.0 | 1.60 | 25M | Read-only zero-copy DOM, strict, fast; cannot mutate, cannot write; a second parser inside the boundary |
| xml-rs | 1.0.0 (2025-08-25), 0.8.29 line exists | MIT | 1.85 | 24M | Pull parser, slower than quick-xml (unmeasured here), writer included |
| xmltree | 0.12.0 (2025-11-10) | MIT | none | 5M | Mutable element tree on xml-rs; no source spans (unverified); tree shape is its own |
| xot | 0.31.2 (2025-04-09) | MIT | none | 84 | Arena tree, mutable, pairs with xee; low adoption |
| minidom | 0.19.0 | MPL-2.0 | none | 915 | Licence and scope (XMPP) wrong |
| libxml | 0.3.21 (2026-08-02) | MIT | 1.88 | 535 | C library, rejected for the cross-platform build cost |

Recommendation: quick-xml 0.42.0 alone, wrapped by `rimstudio-xml`, which exposes (1) a lightweight arena node tree built from quick-xml events with a per-node source span (used by the patch simulator, def explorer and the item designer), (2) a streaming scanner for About.xml and Defs indexing that never builds a tree, (3) writers. roxmltree would be a faster read-only path but adds a second parser and cannot serve the patch engine, which must mutate. Unmeasured: a micro-benchmark of quick-xml event parsing versus roxmltree on the 46.7k workshop XML files should be run before freezing the scanner design (open question 1).

### 4.3 Writing About.xml with comment preservation

| Approach | How | Verdict |
|---|---|---|
| Event passthrough | Read with quick-xml, write every event unchanged to `quick_xml::Writer`, substituting events of edited elements | preserves comments, attribute order and CDATA; normalises whitespace only if trimming is on (keep it off) |
| Span splice | Read once to locate the byte span of the target element or its text, then replace that slice of the original bytes | minimal diff; trivial for the About fields RimStudio edits (name, author, description, packageId, supportedVersions, loadAfter lists); needs care with entities and encodings |
| Tree rewrite | Serialise the node tree | loses comments unless the tree models comments and original whitespace as nodes; only for generated files (new mod, Defs from the item designer) |

Use span splice for edits to existing About.xml and LoadFolders.xml (user comments and layout are sacred), event passthrough for structured edits that add elements (insert after a named sibling), and tree serialisation only for newly generated files. Preserve the BOM and line endings found in the file. About.xml is always saved UTF-8; the game reads it with `File.ReadAllText`, which detects BOMs.

### 4.4 XPath engines

See the XPath note. Summary of crate facts (crates.io, 2026-10-04):

| Crate | Stable (date) | Verdict |
|---|---|---|
| sxd-xpath 0.4.2 / sxd-document 0.3.2 | 2018-10-31 / 2019-05-26 | complete XPath 1.0, agreed with Mono on all 2,187 sampled expressions, but unmaintained for 7 years, own tree, slow tail (p90 about 2 s on a 110 MB document) |
| xee-xpath 0.1.5 + xot 0.31.2 | 2025-08-21 / 2025-04-09 | XPath 3.1 engine, ICU-heavy (a harness build reached 736 MB of target output), low adoption |
| skyscraper 0.7.0 | 2026-05-03 | HTML oriented, no mutation, 2 recent downloads |
| libxml 0.3.21 | 2026-08-02 | binding to libxml2 (C), good fidelity, no |
| in-house (recommended) | n/a | `rimstudio-xpath` over the boundary crate's node tree |

## 5. Category 3: filesystem and concurrency

### 5.1 Walking

| Crate | Stable (date) | Licence | Notes |
|---|---|---|---|
| walkdir | 2.5.0 (2024-03-01) | Unlicense/MIT | 158M recent downloads, sequential, stable; no new release in 2.5 years is normal for finished crates |
| ignore | 0.4.33 (2026-08-04) | Unlicense OR MIT | gitignore semantics, parallel walker, overrides; MSRV 1.88 |
| jwalk | 0.9.0 (2026-08-05) | MIT | rayon based parallel walk with ordered output; 2.5M |
| globset | 0.4.20 (2026-08-04) | Unlicense OR MIT | glob sets |

The workshop folder in this checkout has 690 mod folders, 306k files and 59,348 directories; the common scan only needs the top level, `About/About.xml`, `Defs/**/*.xml`, `Patches/**`, `LoadFolders.xml` and version folders, so walks should be targeted per mod (read_dir on known subfolders) rather than whole-tree. Recommendation: `walkdir` for deterministic single-folder walks inside a mod, `rayon` `par_iter` over mods for the scan (files are small; blocking IO fits rayon), and `ignore` (GitignoreBuilder, OverrideBuilder) for upload staging patterns. Skip jwalk (a second rayon pool with its own ordering semantics adds nothing over rayon + walkdir for this layout) but keep it as a fallback if one huge mod folder dominates.

### 5.2 rayon versus tokio

rayon 1.12.0 (2026-04-14, MSRV 1.80) is for CPU and blocking file work (scan, parse, hash). tokio 1.53.2 (2026-10-03, MSRV 1.71) is for network, subprocess (SteamCMD, `git`, sidecar), timers and cancellation. Tauri 2 runs async commands on its own tokio runtime, so reuse it (`tauri::async_runtime`) and move blocking scans to `spawn_blocking` or a dedicated rayon pool built with `ThreadPoolBuilder` sized to leave one core free for the UI. Never call rayon from an async task without `spawn_blocking`. Use `crossbeam-channel` 0.5.17 or `tokio::sync::mpsc` for progress streams; Tauri `Channel` carries them to the webview.

### 5.3 Watching

| Crate | Stable (date) | Licence | Notes |
|---|---|---|---|
| notify | 8.2.0 (2025-08-03); 9.0.0-rc.5 exists (2026-08-30) | CC0-1.0 | MSRV 1.77; recommended stable line is 8.2 |
| notify-debouncer-mini | 0.7.0 (2025-08-03) | MIT OR Apache-2.0 | coalesces events per path with a timeout; no rename tracking |
| notify-debouncer-full | 0.7.0 (2026-01-23); 0.8.0-rc.2 exists | MIT OR Apache-2.0 | rename stitching and file ids (`file-id` 0.2.3), heavier |

Facts from this machine: `fs.inotify.max_user_watches` is 524,288, but the workshop tree alone has 59,348 directories, and inotify needs one watch per watched directory, so a recursive watch of every mod folder would use about 11% of the budget before other applications (editors, Steam) take theirs; default limits on other distributions can be far lower (the common figure is 8,192 on older kernels, unverified here). Policy: watch only roots and metadata files non-recursively (the mods folders, `workshop/appworkshop_294100.acf`, `ModsConfig.xml`, RimStudio's own settings), run targeted per-mod rescans on focus and on a "rescan" command, watch a project's folder recursively only for the active project, and fall back to `notify::PollWatcher` on `ENOSPC`. Use `notify-debouncer-mini` with a 300 to 500 ms window for the few watched roots. Network and removable drives deliver no events (consistent with `docs/research/steam-and-game-detection.md`, risk item 7).

### 5.4 Paths

| Need | Crate | Stable (date) | Verdict |
|---|---|---|---|
| Same file / hard link or link-loop detection | same-file | 1.0.6 (2020-01-11) | adopt; finished crate, 155M downloads; needed to dedupe the Mods folder when a custom folder or symlink points into it |
| Windows verbatim prefix `\\?\` removal | dunce | 1.0.5 (2024-08-04) | adopt on `canonicalize` results shown to users and passed to the game |
| Normalise without touching disk | normpath 1.5.2 (2026-09-19), `path-clean` | | normpath adopt only if needed |
| UTF-8 paths | camino | 1.2.6 (2026-09-15) | adopt in domain and JSON types (below) |
| Better IO errors | fs-err | 3.3.2 (2026-10-04) | adopt: errors name the path |

OsString versus camino: JSON (R10) and the IPC layer need UTF-8, serde cannot serialise a non-UTF-8 `PathBuf`, and the game (a .NET program) works on UTF-16 strings. Policy: `Utf8PathBuf` in all domain types, DTOs and persisted JSON; at the filesystem boundary, convert with a fallible step that records "skipped: path is not valid UTF-8" as a diagnostic rather than failing a scan; keep `PathBuf` only inside `rimstudio-fs`-style helpers that call the OS.

### 5.5 Links, trash, locks, copying

| Need | Crate | Stable (date) | Licence | Verdict |
|---|---|---|---|---|
| NTFS junction create, delete, query | junction | 2.1.0 (2026-09-24) | MIT | adopt on Windows: creating directory symlinks there needs administrator or developer mode, junctions do not (mod linking for the project workspace) |
| Symlink on Unix | std `std::os::unix::fs::symlink` | | | std |
| Move to recycle bin | trash | 5.2.9 (2026-09-13) | MIT | adopt (send2trash equivalent); on Linux it follows the FreeDesktop trash spec |
| Open file or URL | `tauri-plugin-opener` (opener 0.9.0 and open 5.4.4 exist) | | | use the plugin |
| Advisory file lock | fs4 | 1.1.0 (2026-04-28) | MIT OR Apache-2.0 | optional lock file in the data dir if single-instance is not enough |
| Reflink and copy-on-write copy | reflink-copy | 0.1.30 (2026-06-18) | | optional for staging large mods |
| Temp files | tempfile | 3.27.0 (2026-03-11) | | adopt (runtime and tests) |
| mtime handling | filetime | 0.2.29 | | optional |
| Memory-mapped read | memmap2 | 0.9.11 | | not needed for XML of this size (mean file under 10 KB) |

### 5.6 Gitignore-style matching for upload staging

RimCrow depends on `pathspec`. In Rust, `ignore::gitignore::GitignoreBuilder::add_line` accepts patterns from memory, so the patterns can live in the project JSONC (`"uploadIgnore": ["*.psd", "Source/"]`), keeping R10 intact without a `.something-ignore` text file. Matching semantics (negation, directory-only slash, anchoring) are git's, which modders already know. Always apply the built-in exclusions for secrets and VCS (`.git/`, `*.pdb`, `.vs/`) before user patterns.

## 6. Category 4: JSON, JSONC, schemas, directories, atomic writes, hashing

### 6.1 JSON engines

| Crate | Stable (date) | Licence | MSRV | Verdict |
|---|---|---|---|---|
| serde_json | 1.0.151 (2026-07-20) | MIT OR Apache-2.0 | 1.71 | default |
| simd-json | 0.18.1 (2026-08-23) | Apache-2.0 OR MIT | 1.88 | measured 12% faster typed, more memory; not worth a dependency |
| sonic-rs | 0.5.10 (2026-09-11) | Apache-2.0 | none | 3M recent downloads; not measured here; skip |

Measured in the sibling note (49 MB SteamDB JSON, i9-9900K, release): serde_json typed slim struct 84 to 101 ms and 92 MB peak, simd-json 74 to 75 ms, a pre-built slim index of 3.7 MB parses in 23 to 26 ms (`docs/research/community-datasets-analysis.md`, section 4.3). Decision: serde_json everywhere, with typed structs that skip unknown fields on read-only paths, `preserve_order` only where order is user-visible.

### 6.2 JSONC (configs) and comment-preserving edits

| Crate | Stable (date) | Licence | Verdict |
|---|---|---|---|
| jsonc-parser | 0.34.0 (2026-09-27) | MIT | adopt. Features `serde`, `serde_json`, `preserve_order` and `cst` (concrete syntax tree) exist in the crates.io feature list; the repository describes "a JSON parser and manipulator that supports comments" (dprint/jsonc-parser README). Use `serde` for reads and `cst` for programmatic edits that keep the user's comments. Verify the CST edit API on a spike before committing to it |
| json5 | 1.3.1 (2026-02-07) | MIT | reject: accepts a larger language than JSONC (unquoted keys, single quotes) and cannot preserve comments on write |
| json_comments | 0.2.2 (2023-11-03) | Apache-2.0 | stripper only, stale |
| serde_json_lenient | 0.2.4 (2024-12-10) | MIT/Apache-2.0 | lenient reader, fork of serde_json, no edits |

Policy: files that users edit by hand (settings, custom mod folders, rule overrides, themes) are JSONC and written back only through the CST editor (a settings change touches one value, comments survive). Machine files (caches, indexes, golden files, IPC) are plain JSON written by serde_json. A `$schema` property pointing to a generated schema gives editor completion.

### 6.3 Schemas, directories, atomic writes

| Need | Crate | Stable (date) | Licence | Verdict |
|---|---|---|---|---|
| JSON Schema generation | schemars | 1.2.2 (2026-07-27) | MIT | adopt (MSRV 1.74; has `preserve_order`); emit schemas from the settings structs via xtask |
| Runtime schema validation | jsonschema | 0.58.5 (2026-10-02) | MIT | optional (validating imported rule files) |
| Path errors in serde | serde_path_to_error | 0.1.20 (2025-09-15) | MIT OR Apache-2.0 | adopt: error says which JSON path failed |
| App directories | directories 6.0.0 (2025-01-12), dirs 7.0.0 (2026-09-05), etcetera 0.11.0 (2025-10-28) | | | use Tauri's `app.path()` in the shell and pass paths down; core crates never discover directories; tools and tests use `directories`. Support a portable override through an environment variable and a settings key |
| Atomic write | atomic-write-file | 0.3.1 (2026-08-11) | BSD-3-Clause | adopt for settings and user rules: temp file in the same directory, fsync, rename |
| Atomic write alternative | tempfile `NamedTempFile::persist`; atomicwrites 0.4.4 (2024-09-19) | | | fallback; persist does rename but you must fsync yourself |

Allow BSD-3-Clause in the cargo-deny licence list (atomic-write-file, zstd 0.14.0 declares BSD-3-Clause on crates.io).

### 6.4 Hashing

| Crate | Stable | Licence | Use |
|---|---|---|---|
| blake3 | 1.8.7 (2026-08-20) | CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception | content hashes for dataset integrity and change detection; has rayon support (feature name unverified) |
| xxhash-rust | 0.8.19 (2026-09-28) | BSL-1.0 | fast non-crypto hash; Boost licence is permissive but must be in the allow list |
| twox-hash | 2.1.4 (2026-08-27) | MIT | alternative to xxhash-rust |
| rustc-hash | 2.1.3 (2026-07-02) | Apache-2.0 OR MIT | `FxHashMap` for internal maps with trusted keys |
| foldhash | 0.2.0 (2025-08-23) | Zlib | hashbrown's default; alternative to rustc-hash |

Change detection for mod folders should use `(path, size, mtime)` first and hash only on demand; blake3 for downloaded datasets and the staged upload manifest.

### 6.5 Is a binary cache justified? (explicit exception request, answer: not now)

Candidates: rkyv 0.8.18 (zero-copy, MIT, MSRV 1.81), postcard 1.1.3, bincode 3.0.0 (2025-12-16), bitcode 0.6.9, rmp-serde 1.3.1. The measurement above shows JSON already meets a startup budget under 200 ms for the largest dataset, and a slim JSON index drops it to about 25 ms. No exception to R10 is requested. Trigger condition for revisiting: after the scanner exists, if cold start index load on the owner's 690-mod set exceeds 150 ms at p95 with the JSON index, request an exception for one derived, disposable cache (version header, rebuilt on mismatch, never authoritative) and then evaluate rkyv first.

## 7. Category 5: graph and text

| Need | Crate | Stable (date) | Licence | MSRV | Verdict |
|---|---|---|---|---|---|
| Topological sort, SCC, cycles | petgraph | 0.8.3 (2025-09-30) | MIT OR Apache-2.0 | 1.64 | adopt: `toposort`, `tarjan_scc`/`kosaraju_scc`, `has_path_connecting`; RimSort uses networkx and toposort for the same jobs. Note petgraph's toposort is DFS based and not stable for ties: implement tier-based Kahn on top for deterministic RimSort-like output |
| Small DAG helper | daggy 0.9.0 (2025-04-17), topological-sort 0.3.1 (2026-08-07) | | | skip |
| Fuzzy search UI | nucleo-matcher 0.3.1 (2024-02-20), nucleo 0.5.0 (2024-04-02) | MPL-2.0 | none | adopt `nucleo-matcher` for interactive search (helix editor matcher); MPL-2.0 is file-level, compatible; stale 2.5 years but stable |
| String distance | strsim | 0.11.1 (2024-04-02) | MIT | 1.56 | adopt for "did you mean" on packageIds |
| Old fuzzy | fuzzy-matcher 0.3.7 (2020-10-04) | MIT | | reject (stale) |
| Natural sort | alphanumeric-sort | 1.5.8 (2026-07-01) | MIT | 1.56 | adopt; natord 1.0.9 (2015), lexical-sort 0.3.1 (2020), natural-sort-rs (1 recent download) rejected |
| Case folding | std `to_lowercase`; unicase 2.9.0 (2026-01-06); caseless 0.2.2; icu_casemap 2.3.0 (Unicode-3.0, MSRV 1.88) | | | see below |
| Versions | semver 1.0.28; versions 8.0.1; version-compare 0.2.1; lenient_semver 0.4.2 (2021) | | | custom parser, below |

Case folding for packageIds: the game compares packageIds case-insensitively by lowercasing (RimSort keeps lower-case keys; the sibling note records that SteamDB has double-key entries). packageIds are ASCII by convention (`author.mod`). Use `str::to_ascii_lowercase` on a validated ASCII id, and `to_lowercase` with a flagged diagnostic when a non-ASCII id appears; do not add icu_casemap. Whether the game uses invariant or culture lowercasing for ids (Turkish i) is not verified here (open question 4).

Version strings such as `1.6.4871 rev598`: neither semver nor version-compare parse a trailing `revNNN`. Write a 60-line parser in `rimstudio-core` returning `GameVersion { major, minor, build: Option<u32>, rev: Option<u32> }`, and compare on (major, minor) for `supportedVersions` matching (the game's rule uses major.minor, per `docs/research/rimworld-mod-format-and-corpus.md`) while keeping build and rev for display; `semver` stays out because mod versions in About.xml are free text. Take version lists from the rimworld-versions dataset (R6).

## 8. Category 6: networking and Steam

### 8.1 HTTP

| Crate | Stable (date) | Licence | MSRV | Notes |
|---|---|---|---|---|
| reqwest | 0.13.5 (2026-09-08) | MIT OR Apache-2.0 | 1.85 | async, default features include a TLS backend (the crates.io feature list shows `default-tls`, `rustls` with `__rustls-aws-lc-rs`, `native-tls`); gzip, json, stream features optional |
| ureq | 3.4.2 (2026-09-13) | MIT OR Apache-2.0 | 1.85 | blocking, rustls feature, small; fine for the sidecar or tools |
| rustls | 0.23.45 (2026-09-14) | Apache-2.0 OR ISC OR MIT | 1.71 | |
| rustls-platform-verifier | 0.7.1 (2026-09-24) | MIT OR Apache-2.0 | 1.85 | uses the OS trust store (enterprise proxies, corporate roots) rather than a bundled bundle |
| hyper | 1.11.1 | MIT | | too low level |
| http-cache-reqwest | 0.16.0 stable; 1.0.0-alpha.9 exists | MIT OR Apache-2.0 | 1.82 | caching middleware; unnecessary |
| reqwest-middleware 0.5.2, backon 1.6.0 (retry), governor 0.10.4 (rate limit) | | | | optional |

Recommendation: reqwest 0.13.5 with `default-features = false` and `rustls` plus `rustls-platform-verifier` (check the exact feature names against 0.13's feature list at implementation time), `json`, `gzip`/`zstd` decoding as needed, `stream`. Conditional requests are hand written, about 40 lines: store `ETag` and `Last-Modified` per dataset in a JSON sidecar, send `If-None-Match`/`If-Modified-Since`, treat 304 as "no change"; the sibling note found that raw GitHub URLs answer range requests (HTTP 206) and serve the same bytes as the zip form (`docs/research/community-datasets-analysis.md`). No caching middleware is needed. Retry with `backon` and exponential delay on 5xx and connect errors; honour `Retry-After`.

### 8.2 Steam files and location

| Crate | Stable (date) | Licence | MSRV | Verdict |
|---|---|---|---|---|
| steamlocate | 2.1.1 (2026-08-13) | MIT | 1.83 | finds Steam, libraries and apps (README: `libraries()`, `find_app`); 38 recent downloads; good for a first locator but the sibling note found gaps (HKCU on Windows, workshop ACF, Proton, non-Steam) |
| keyvalues-parser | 0.2.4 (2026-05-17) | MIT OR Apache-2.0 | 1.81 | pest based, sorted map API; measured 8.85 ms for the 241 KB workshop ACF versus 0.52 ms for a 118-line prototype; fails on BOM, `#include` and empty file (sibling note) |
| keyvalues-serde | 0.2.4 | MIT OR Apache-2.0 | 1.81 | serde layer over the above |
| vdf-reader 0.3.4 (2026-07-11), steam-vdf-parser 0.1.2 (2026-07-14) | | MIT, MIT OR Apache-2.0 | 1.86, 1.88 | new, 3 and 1 recent downloads; not recommended yet |
| steam-vent 0.5.0 | | MIT | 1.88 | Steam network client; out of scope |

Verdict: follow the locator decision in `docs/research/steam-and-game-detection.md`; for the crate choice, use `keyvalues-parser` with a pre-read that strips BOM and rejects empty files, or the small in-house parser if re-reads on file events matter; `steamlocate` is acceptable behind a `GameLocator` trait as one detector among several, never as the only one.

### 8.3 steamworks-rs

| Item | Value | Evidence |
|---|---|---|
| steamworks | 0.13.1 (2026-05-05), MIT / Apache-2.0, MSRV 1.80, 98 recent downloads | crates.io |
| steamworks-sys | 0.13.0 (2026-04-14) | crates.io |
| Bundled SDK | README table: crate 0.13.0 uses SDK 1.64, 0.12.0 SDK 1.62, 0.11.0 SDK 1.58a (the table has no row for 0.13.1; assumed unchanged, unverified) | raw.githubusercontent.com/Noxime/steamworks-rs/master/README.md (accessed 2026-10-04) |
| Licence of bundled files | crate is dual MIT/Apache-2.0 "except for the files in steamworks-sys/lib/steam/" (Valve's SDK terms); the loaded library is `steam_api64.dll` / `libsteam_api.so` / `libsteam_api.dylib` from `redistributable_bin`, loaded dynamically | same README |
| UGC coverage | `ugc.rs` exposes create_item, start_item_update with builder methods for title, description, language, preview_path, content_path, metadata, visibility, tags, key-value tags and content descriptors, `submit(change_note, cb)` plus `UpdateWatchHandle::progress`, subscribe_item, unsubscribe_item, subscribed_items, item_state, item_download_info, item_install_info, download_item, query_all/query_user/query_items/query_item, delete_item, playtime tracking | raw `src/ugc.rs`, function list (accessed 2026-10-04) |

Redistribution terms: the grant covers partners who accepted Valve's SDK agreement (`docs/research/workshop-publishing-research.md`, section 5.5); RimCrow therefore does not ship the SDK and has users supply it (RimCrow README, `scripts/setup_steamworks_runtime.py`), while RimSort commits Steamworks libraries in `RimSort-main/libs/` (file listing) under its own judgement. RimStudio must decide who ships the redistributable (open question 2). Confining it to the sidecar makes either decision cheap to change.

SteamCMD alternative: `tokio::process::Command` with `+login anonymous` for downloads (RimSort uses `login anonymous` in `RimSort-main/app/utils/steam/steamcmd/wrapper.py`), parse output lines, pass `+workshop_download_item 294100 <id>` (command shape from Valve's SteamCMD documentation, unverified here); not suitable for publishing (needs credentials, Valve calls it test-only).

## 9. Category 7: images and textures

### 9.1 Which tool does RimSort drive?

`RimSort-main/app/utils/todds/wrapper.py` builds command lines for `todds`, the CPU-based DDS encoder by joseasoler (credited in `RimSort-main/docs/ACKNOWLEDGEMENTS.md`). Repository facts (raw.githubusercontent.com/todds-encoder/todds, accessed 2026-10-04): licence MPL-2.0; the README states "todds is no longer being developed" and the project is read-only, with a successor `imutate` on codeberg; formats BC7 (default), BC1, PNG output; options such as `-f`, `-af` (alpha format), `-cl` (clean), `-ss` (subfolder filter), `-t` (meaning not confirmed), `-p`. RimSort's "optimized" preset uses `-f BC1 -af BC7`, an overwrite flag, `-vf`, `-fs`, `-ss Textures`, `-t`, `-p`; the "clean" preset deletes generated DDS files. Licence consequence: MPL-2.0 is file-level copyleft; redistributing an unmodified binary with a source pointer is straightforward, but the project is archived, so a long-lived dependency on it is a risk.

### 9.2 Crates

| Crate | Stable (date) | Licence | MSRV | Signal | Notes |
|---|---|---|---|---|---|
| image | 0.25.10 (2026-03-10) | MIT OR Apache-2.0 | 1.88 | 56M | decode PNG/JPEG; disable default features and enable `png`, `jpeg` only |
| png | 0.18.1 (2026-02-14) | MIT OR Apache-2.0 | 1.73 | 80M | direct PNG IO |
| imagesize | 0.15.0 (2026-07-09) | MIT | none | 13M | dimensions without decoding (preview checks: size, format) |
| fast_image_resize | 6.1.0 (2026-07-21) | MIT OR Apache-2.0 | 1.87 | 4.7M | SIMD resize, Lanczos/mip generation |
| ddsfile | 0.6.0 (2026-04-04) | MIT | 1.73 | 141 | DDS container read/write |
| image_dds | 0.7.2 (2025-03-13) | MIT | none | 26 | BCn encode via intel_tex_2 and decode via a Rust port of bcdec; mipmaps; README warns "some targets may not build" for lack of precompiled ISPC kernels |
| intel_tex_2 | 0.5.0 (2025-07-02) | MIT/Apache-2.0 | none | 170 | bindings to Intel ISPC Texture Compressor: BC6H, BC7, ETC1, ASTC, BC1/BC3; ISPC and libclang are not required unless regenerating kernels (README) |
| texpresso | 2.0.2 (2025-05-26) | MIT | none | 46 | pure Rust, BC1 to BC4 shown in the README excerpt (BC7 not seen; unverified) |
| block_compression | 0.10.0 (2026-08-21) | MIT | 1.80 | 7 | wgpu compute shaders: needs a GPU context |
| bcndecode 0.2.0 (2017), basis-universal 0.3.1 (2023) | | | | | stale |
| oxipng | 10.2.1 (2026-09-02) | MIT | 1.88 | 551 | lossless PNG optimisation; candidate for "shrink preview" |

Recommendation: native encoding behind a `TextureEncoder` trait, default implementation `image_dds` (BC1 for opaque, BC7 or BC3 for alpha, matching RimSort's presets) with `fast_image_resize` for mips; an optional external backend that runs a user-supplied `todds` for parity; texpresso as the pure-Rust fallback if intel_tex_2 fails to build on a target. Spike items before committing: build on Windows x64, macOS arm64, Linux x64; compare output size and speed against todds on 200 textures (not done here). The texture feature is not part of the owner's R3 to R7 list, so schedule it after the mod manager and toolkit.

## 10. Category 8: archives and Git

| Crate | Stable (date) | Licence | MSRV | Notes |
|---|---|---|---|---|
| zip | 8.6.0 (2026-04-25); 9.0.0-pre3 exists | MIT | 1.88 | read and write; use the 8.x line |
| flate2 | 1.1.10 (2026-08-28) | MIT OR Apache-2.0 | 1.67 | gzip/deflate; also used for `.tar.gz` |
| tar | 0.4.46 (2026-05-18) | MIT OR Apache-2.0 | 1.63 | |
| zstd | 0.14.0 (2026-09-04) | BSD-3-Clause | 1.64 | C binding; ruzstd 0.9.0 (MIT, MSRV 1.87) is pure Rust, decoder strong, encoder basic |
| sevenz-rust2 | 0.23.0 (2026-09-18) | Apache-2.0 | 1.93 | pure Rust 7z; raises MSRV to 1.93 (sevenz-rust 0.6.1 from 2024 is superseded) |
| lzma-rs 0.3.0 (2023), bzip2 0.6.1 | | | | stale or unneeded |
| async_zip 0.0.19 | | | | pre-1.0, unneeded |
| self_update 1.3.0 | | | | replaced by the Tauri updater |

Needs: reading a zip (importing a user's mod pack, SteamDB zip if the zip URL form is used; raw JSON URLs avoid it), writing a zip (exporting a mod for sharing, project export), extracting SteamCMD or tool downloads (zip and tar.gz). Adopt zip 8.6.0 and flate2; add zstd only if a feature needs it (RimSort and RimCrow both depend on zstandard, but the reason is not established in this note); keep 7z as an optional cargo feature.

Git: RimSort uses pygit2 and RimCrow does not list a git library. Options: `gix` 0.88.0 (pure Rust, MIT OR Apache-2.0, MSRV 1.88, 12M recent downloads, still 0.x), `git2` 0.21.0 (libgit2 binding, C build, no MSRV declared), or shelling out to `git`. Recommendation: datasets are fetched over HTTPS (no git); for the modding toolkit's project git status and commits, shell out to the system `git` through `tokio::process` (users' credential helpers, signing and hooks work unchanged), and gate a `gix`-based read-only fallback (repository discovery, HEAD, status) behind an optional feature for machines without git. Use `gix` with `default-features = false` and only the needed components, because its default tree is large. Do not use git2 (C build on three platforms, OpenSSL/libssh2 options).

## 11. Category 9: errors, logging, tests, quality

### 11.1 Errors and logging

| Crate | Stable (date) | Licence | Verdict |
|---|---|---|---|
| thiserror | 2.0.21 (2026-09-23) | MIT OR Apache-2.0 | adopt for library crates (typed errors with stable codes that map to IPC error DTOs) |
| anyhow | 1.0.104 (2026-07-18) | MIT OR Apache-2.0 | adopt only in binaries, xtask and tests |
| miette 7.6.0 (2025-04-27), snafu 0.9.2, eyre 0.6.14, color-eyre 0.6.5 | | | reject; miette is a candidate later for rendering XML and xpath diagnostics with spans in a CLI |
| tracing | 0.1.44 (2025-12-18) | MIT | adopt |
| tracing-subscriber | 0.3.23 (2026-03-13) | MIT | adopt: `fmt`, `env-filter`, `json` for JSON-lines files |
| tracing-appender | 0.2.5 (2026-04-17) | MIT | adopt: rolling non-blocking file writer |
| tracing-error 0.2.1 | | | optional (span traces) |
| log 0.4.34 | | | bridged by tracing-subscriber's `tracing-log` feature so dependencies that use `log` still appear |
| tauri-plugin-log 2.10.0 | | | reject (above) |
| fern, flexi_logger | | | reject (second logging stack) |
| sentry 0.49.3, minidumper 0.11.0, human-panic 2.0.8 | | | no default telemetry: a modding tool should not phone home; crash reports are written locally as JSON and shown to the user |

Errors cross IPC as a JSON object `{code, message, details}`; the typed DTO lives in `rimstudio-ipc-types`.

### 11.2 Tests and tools

| Tool | Stable (date) | Licence | Verdict |
|---|---|---|---|
| cargo-nextest | 0.9.146 (2026-09-21) | Apache-2.0 OR MIT | adopt (tool MSRV 1.91; fine on 1.96) |
| insta | 1.49.0 (2026-10-03) | Apache-2.0 | see R10 note below |
| goldenfile | 1.11.0 (2026-02-25) | MIT | adopt for JSON golden files: plain files, regenerated with an environment variable |
| similar | 3.2.0 (2026-08-17) | Apache-2.0 | adopt for readable diffs in golden helpers |
| proptest | 1.11.0 (2026-03-24) | MIT OR Apache-2.0 | adopt (xpath, version parser, rule merge properties) |
| rstest | 0.27.0 (2026-09-06) | MIT OR Apache-2.0 | adopt (table tests over fixtures) |
| assert_fs | 1.1.4 (2026-05-26) | MIT OR Apache-2.0 | adopt (temp mod trees); `tempfile` underneath |
| criterion | 0.8.2 (2026-02-04) | Apache-2.0 OR MIT | adopt for benches; divan 0.1.21 (2025-04-10, 3.9M) has a nicer API but its last release is 17 months old |
| cargo-llvm-cov | 0.9.1 (2026-09-06) | Apache-2.0 OR MIT | optional coverage |
| cargo-deny | 0.20.2 (2026-07-09) | MIT OR Apache-2.0 | adopt: licences, bans, advisories, sources |
| cargo-machete | 0.9.2 (2026-04-15) | MIT | adopt (fast unused dependency check; cargo-shear 1.14.0 needs Rust 1.95, cargo-udeps needs nightly) |
| cargo-audit 0.22.2 | | | covered by cargo-deny advisories |
| cargo-semver-checks, cargo-mutants, cargo-fuzz | | | later, for `rimstudio-xml` and `rimstudio-xpath` |
| typos-cli 1.50.3 | | | optional |
| bacon 3.26.0 | AGPL-3.0 | | developer tool only, never a dependency |

R10 and snapshot tools: insta stores `.snap` files that begin with a YAML-style metadata header, and R10 lists "golden outputs" among things that must not be XML (and tooling YAML is awkward to defend). Use goldenfile with JSON bodies for golden outputs; use insta only in inline mode (`assert_json_snapshot!(value, @r#"..."#)`, snapshots live in the Rust source, no `.snap` files) for small expectations, or drop insta entirely. This is a design decision for the owner (open question 5).

xtask versus just: `just` 1.58.0 (CC0-1.0, MSRV 1.89) needs installation and a separate shell dialect on Windows; an `xtask` crate (`cargo xtask <task>` via an alias in `.cargo/config.toml`) needs only cargo and can share Rust types with the workspace (bindings export, schema generation, fixture generation, the XML-ban check, helper copy for the sidecar). Choose xtask; keep an optional two-line `justfile` of aliases for people who like it.

### 11.3 cargo-deny: confine XML crates to one boundary crate

```toml
# deny.toml (tooling config; TOML is required by cargo-deny)
[bans]
multiple-versions = "warn"
deny = [
  # Only the boundary crate (and the known transitive users) may depend on quick-xml.
  # The wrapper list must be completed from `cargo tree -i quick-xml -e normal`
  # once the Tauri tree exists; plist is verified (tauri -> plist -> quick-xml ^0.42).
  { crate = "quick-xml", wrappers = ["rimstudio-xml", "plist"] },
  { crate = "roxmltree" },
  { crate = "xml-rs" },
  { crate = "xmltree" },
  { crate = "xmlparser" },
  { crate = "sxd-document" },
  { crate = "serde-xml-rs" },
  { crate = "minidom" },
  { crate = "libxml" },
  # R10: no embedded databases or YAML/TOML for app-owned data
  { crate = "rusqlite" }, { crate = "sqlx" }, { crate = "diesel" },
  { crate = "serde_yaml" }, { crate = "serde_norway" },
]
```

How it works (cargo-deny docs, `docs/src/checks/bans/cfg.md`, accessed 2026-10-04): `wrappers` names the only crates allowed to depend directly on the banned crate; this field cannot be combined with `deny-multiple-versions`. Caveats: transitive users inside Tauri's tree (plist verified; others such as Linux desktop integration crates are unverified) must be listed, `roxmltree` or `xmlparser` may appear transitively through other dependencies and may need wrappers or skips after the first real `cargo deny check`, and `toml`/`toml_edit` cannot be banned because Cargo tooling and Tauri use them. Add an xtask check as a second guard: no `Cargo.toml` outside `crates/rimstudio-xml` may list an XML crate, and no `.xml` or `.toml` file may be written by the app outside the allowed RimWorld paths (a grep-based test over source for `.xml"` literals in non-boundary crates).

## 12. Category 10: typed IPC and workspace hygiene

### 12.1 Typed IPC (Rust side; cross-reference `docs/research/webview-and-ipc-performance.md`)

| Crate | Stable (date) | Licence | Verdict |
|---|---|---|---|
| tauri-specta | 2.0.0-rc.25 (2026-05-08); stable 1.0.2 is Tauri 1 only | MIT | adopt with an exact pin, as the sibling note recommends; release candidates since 2023 are the main risk |
| specta / specta-typescript / specta-serde | 2.0.0-rc.25 (2026-05-07) / 0.0.12 / 0.0.12 | MIT | pinned with tauri-specta |
| ts-rs | 12.0.1 (2026-01-31) | MIT | fallback: stable derive for DTO types; commands need a hand-written typed invoke map |
| typeshare 1.0.5, tsify 0.5.8, utoipa 6.0.0 | | | not fitted (other target languages, wasm, OpenAPI) |
| tauri-typegen 0.5.3, taurpc 0.8.2 | | | immature (1 and 2 recent downloads) |
| schemars | 1.2.2 | MIT | JSON Schema for settings and project files |

Keep DTOs in `rimstudio-ipc-types` (serde structs deriving `specta::Type`), Tauri-free, so domain crates stay testable; send Steam and workshop ids as strings (u64 exceeds JavaScript's safe integers).

### 12.2 Workspace hygiene

| Topic | Decision | Evidence |
|---|---|---|
| Edition | 2024 | the current root manifest already uses it; Tauri 2.12.1 is edition 2024 |
| Resolver | `resolver = "3"` (MSRV-aware resolution) | needed explicitly in a virtual workspace |
| MSRV | `rust-version = "1.90"` in `[workspace.package]`; the optional `sevenz-rust2` feature needs 1.93, `cargo-shear` 1.95 as a tool | crates.io rust_version fields |
| Dependency versions | one `[workspace.dependencies]` table, members use `dep.workspace = true` | |
| Lints | `[workspace.lints]`: `unsafe_code = "forbid"` (override only in the helper if unavoidable), `clippy::dbg_macro`, `todo`, `unwrap_used` warn in libraries | |
| Profiles | dev: `debug = "line-tables-only"` and `[profile.dev.package."*"] opt-level = 2` so XML parsing in debug is usable; release: `lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`, keep `panic = "unwind"` because the patch simulator and scanners isolate per-file panics with `catch_unwind` (matching the game's per-patch try/catch) | |
| Linker | nothing to configure on Linux x86_64: rustc 1.96 passes `-fuse-ld=lld` with its bundled `gcc-ld` directory by default (verified here with `cargo rustc --release -- --print link-args` on a trivial crate); Windows MSVC and macOS use their default linkers | experiment on this machine |
| Committed files | `Cargo.lock`, `rust-toolchain.toml` (channel), `deny.toml`, `.cargo/config.toml` (xtask alias) | |

Root `Cargo.toml` skeleton for the future virtual workspace (the current root manifest is a single package named `rimforge-studio` and is not modified by this note):

```toml
[workspace]
resolver = "3"
members = ["crates/*", "apps/rimstudio/src-tauri", "tools/*", "xtask"]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.90"
license = "MIT OR Apache-2.0"   # owner to decide
repository = "https://example.invalid/rimstudio"   # placeholder

[workspace.dependencies]
# internal
rimstudio-core = { path = "crates/rimstudio-core" }
rimstudio-xml = { path = "crates/rimstudio-xml" }
rimstudio-ipc-types = { path = "crates/rimstudio-ipc-types" }
# tauri
tauri = { version = "2.12.1", features = [] }
tauri-build = "2.7.1"
tauri-plugin = "2.7.1"
tauri-plugin-dialog = "2.8.1"
tauri-plugin-opener = "2.7.0"
tauri-plugin-process = "2.4.0"
tauri-plugin-single-instance = "2.5.2"
tauri-plugin-updater = "2.13.1"
tauri-plugin-shell = "2.4.0"
# data
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.151"
serde_path_to_error = "0.1.20"
jsonc-parser = { version = "0.34.0", features = ["serde", "cst"] }
schemars = "1.2.2"
quick-xml = "0.42.0"            # only rimstudio-xml may depend on this
# fs and concurrency
rayon = "1.12.0"
tokio = { version = "1.53.2", features = ["rt-multi-thread", "process", "fs", "sync", "time"] }
walkdir = "2.5.0"
ignore = "0.4.33"
notify = "8.2.0"
notify-debouncer-mini = "0.7.0"
same-file = "1.0.6"
dunce = "1.0.5"
camino = { version = "1.2.6", features = ["serde1"] }
fs-err = "3.3.2"
tempfile = "3.27.0"
atomic-write-file = "0.3.1"
trash = "5.2.9"
junction = "2.1.0"
# text and graph
petgraph = "0.8.3"
nucleo-matcher = "0.3.1"
strsim = "0.11.1"
alphanumeric-sort = "1.5.8"
# net
reqwest = { version = "0.13.5", default-features = false, features = ["rustls", "json", "stream"] }  # confirm feature names
# errors and logging
thiserror = "2.0.21"
anyhow = "1.0.104"
tracing = "0.1.44"
tracing-subscriber = { version = "0.3.23", features = ["env-filter", "json"] }
tracing-appender = "0.2.5"
# tests
proptest = "1.11.0"
rstest = "0.27.0"
assert_fs = "1.1.4"
goldenfile = "1.11.0"
criterion = "0.8.2"

[workspace.lints.rust]
unsafe_code = "forbid"
[workspace.lints.clippy]
dbg_macro = "warn"
todo = "warn"

[profile.dev]
debug = "line-tables-only"
[profile.dev.package."*"]
opt-level = 2
[profile.release]
lto = "thin"
codegen-units = 1
strip = "symbols"
```

Version numbers in the snippet are the crates.io stable versions of 2026-10-04; `cargo update` plus `cargo deny check` and `cargo machete` belong in CI. The `reqwest` feature names must be confirmed against the 0.13.5 feature list (the list on crates.io contains `rustls` and `default-tls`; whether `rustls-platform-verifier` is a separate feature is not confirmed here).

## 13. Recommended-crate table

| Crate | Version | Role | Why | Risk | Fallback |
|---|---|---|---|---|---|
| tauri / tauri-build | 2.12.1 / 2.7.1 | app shell | requirement; stable 2.x, edition 2024 | Tauri 3 alpha exists; plist drags quick-xml | pin 2.12.x |
| tauri-plugin-dialog | 2.8.1 | pickers | R4 | none | rfd 0.17.2 |
| tauri-plugin-opener | 2.7.0 | reveal, open URL | | | opener 0.9.0 |
| tauri-plugin-single-instance | 2.5.2 | one process | protects settings and watchers | D-Bus on Linux | fs4 lock file |
| tauri-plugin-updater | 2.13.1 | updates | mandatory signing is safe default | key management | manual downloads |
| tauri-plugin-shell | 2.4.0 | sidecar spawn only | documented sidecar path | wide permission surface if misconfigured | `tokio::process` with resolved path |
| quick-xml | 0.42.0 | XML boundary | one parser and writer, byte offsets | no tree (we build one) | roxmltree 0.21.1 (read only) |
| (in-house) rimstudio-xpath | n/a | patch xpath | coverage and mutation | effort, fidelity | sxd-xpath as test oracle |
| serde / serde_json | 1.0.229 / 1.0.151 | JSON | measured fast enough | none | simd-json 0.18.1 |
| jsonc-parser | 0.34.0 | JSONC read/edit | `cst` feature for comment-preserving edits | CST API unverified | custom stripper plus serde_json (loses comments on write) |
| schemars | 1.2.2 | schemas | | | none needed |
| atomic-write-file | 0.3.1 | crash-safe saves | | small user base | tempfile persist |
| rayon / tokio | 1.12.0 / 1.53.2 | CPU / IO runtime | | mixing them | std threads |
| walkdir, ignore | 2.5.0, 0.4.33 | walks, ignore patterns | | | jwalk 0.9.0 |
| notify + debouncer-mini | 8.2.0, 0.7.0 | watch roots | | inotify limits, 9.0 rc pending | PollWatcher, rescans |
| same-file, dunce, camino, fs-err | 1.0.6, 1.0.5, 1.2.6, 3.3.2 | path hygiene | | | std |
| trash, junction | 5.2.9, 2.1.0 | safe delete, links | | platform edge cases | std remove, symlink |
| petgraph | 0.8.3 | load order | | tie ordering | own Kahn |
| nucleo-matcher, strsim, alphanumeric-sort | 0.3.1, 0.11.1, 1.5.8 | search, distance, sort | | nucleo MPL-2.0 file copyleft | strsim only |
| reqwest (+rustls) | 0.13.5 | HTTP | | feature naming | ureq 3.4.2 |
| keyvalues-parser (or own) | 0.2.4 | VDF/ACF | | fails on BOM, slow | in-house mini parser |
| steamlocate | 2.1.1 | Steam detection helper | | misses cases | own locator |
| steamworks | 0.13.1 (SDK 1.64) | publish sidecar | UGC coverage | redistributable licence, SDK age | libloading to game's library |
| image, fast_image_resize, imagesize | 0.25.10, 6.1.0, 0.15.0 | images | | | |
| image_dds (+intel_tex_2, ddsfile) | 0.7.2 | DDS encode | native BC1/BC7 | build on all targets, stale releases | texpresso, external todds |
| zip, flate2 | 8.6.0, 1.1.10 | archives | | 9.0 pre-release line | |
| thiserror, anyhow | 2.0.21, 1.0.104 | errors | | | |
| tracing (+subscriber, appender) | 0.1.44, 0.3.23, 0.2.5 | logs | | | |
| goldenfile, proptest, rstest, assert_fs, criterion | 1.11.0, 1.11.0, 0.27.0, 1.1.4, 0.8.2 | tests | | | insta inline |
| tauri-specta (+specta) | 2.0.0-rc.25 | typed IPC | | release candidate | ts-rs 12.0.1 plus typed invoke map |
| cargo-nextest, cargo-deny, cargo-machete | 0.9.146, 0.20.2, 0.9.2 | CI tools | | | |

## 14. Rejected list

| Crate | Reason |
|---|---|
| sxd-xpath, sxd-document | unmaintained since 2018 and 2019; own tree; slow tail; kept only as a test oracle |
| xee-xpath, xot | XPath 3.1 semantics differ from .NET; ICU dependency weight; very low adoption |
| skyscraper | HTML scraping design; no mutation |
| roxmltree, xml-rs, xmltree, minidom, libxml | second XML stack or C build; fully banned in deny.toml |
| serde_yaml (0.9.34+deprecated), toml, rusqlite, sqlx, redb-style stores | R10 |
| tauri-plugin-store, tauri-plugin-sql, tauri-plugin-fs, tauri-plugin-log | R10 or security or duplicate stack |
| json5, json_comments | cannot preserve comments on write or accept a wider language |
| fuzzy-matcher, natord, lexical-sort, natural-sort-rs, lenient_semver | stale or negligible adoption |
| notify 9.0.0-rc.5, notify-debouncer-full 0.8.0-rc.2, zip 9.0.0-pre3, http-cache-reqwest 1.0.0-alpha.9, smallvec 2.0.0-beta.2, tauri 3.0.0-alpha | pre-releases |
| git2 | C build cost; gix or system git preferred |
| self_update | duplicates the Tauri updater |
| sentry | default telemetry is not wanted |
| block_compression | needs a GPU context |
| sevenz-rust (0.6.1) | superseded by sevenz-rust2 |
| bacon | AGPL, tool only |
| divan | last release 2025-04-10; criterion preferred |
| bincode, rkyv, postcard (as cache) | no R10 exception requested; revisit only with measured evidence |

## 15. Risk register

| # | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| 1 | tauri-specta stays on release candidates or breaks on a Tauri bump | medium | medium | exact pins, committed generated bindings, CI diff check, ts-rs plus typed invoke map as fallback |
| 2 | Tauri 3 migration pressure (alpha published 2026-10-01) | medium over 12 months | medium | stay on 2.12.x, keep domain crates Tauri-free, adapters thin |
| 3 | XML boundary rule defeated by transitive quick-xml users (plist verified) | high (certain for plist) | low | wrapper list in deny.toml, xtask guard on our own manifests |
| 4 | In-house xpath diverges from the game | medium | high for the simulator | full coercion rules, Mono and libxml2 golden hashes, corpus coverage gate (xpath note) |
| 5 | inotify exhaustion on large libraries | medium on Linux | medium | roots-only watching, PollWatcher fallback, rescans |
| 6 | steamworks redistributable licensing and SDK age (1.64) | medium | high for publishing | sidecar isolation, decision on who ships the library, spike on a real account |
| 7 | image_dds or intel_tex_2 fail to build on a target; both lightly maintained | medium | low (feature optional) | texpresso fallback, external todds backend, trait seam |
| 8 | jsonc-parser CST edit API insufficient for settings edits | low to medium | medium | spike early; fallback is rewriting via serde with comment loss warning |
| 9 | One-person crates (nucleo-matcher, atomic-write-file, steamlocate) go stale | medium | low | small surface, trait seams, vendoring is a last resort |
| 10 | Sidecar signing and notarisation on macOS and Windows | medium | medium | release engineering spike, CI matrix |
| 11 | MSRV drift (sevenz-rust2 1.93, tools 1.95) | low | low | optional features, pin toolchain channel |
| 12 | Non-UTF-8 paths and Windows long paths in mod folders | low | medium | fallible Utf8 conversion with diagnostics; `dunce` and long-path handling tests |

## Implications for RimStudio

1. The workspace is a virtual workspace (edition 2024, resolver 3, `rust-version = "1.90"`, `[workspace.dependencies]`, `[workspace.lints]`) with domain crates that have no Tauri dependency and `rimstudio-tauri-*` adapter plugins. Test: `cargo tree -p rimstudio-mods` contains no `tauri` crate.
2. `rimstudio-xml` is the only crate with a direct dependency on quick-xml; `deny.toml` bans every other XML crate and wraps quick-xml with an explicit parent list; an xtask check fails if another manifest lists an XML crate. Test: adding `roxmltree` to any crate fails CI.
3. The XML boundary offers Game mode (matches `LoadableXmlAsset` and `XmlDocument.LoadXml` failure classes) and Tolerant mode, strips a UTF-8 BOM itself, and edits existing About.xml and LoadFolders.xml by byte-span splice so comments and layout survive. Test: edit `packageId` in a fixture with comments and compare bytes outside the span.
4. XPath is evaluated by the in-house `rimstudio-xpath` crate (see the XPath note); sxd-xpath, lxml and Mono are test oracles only.
5. All app-owned files are JSON (machine) or JSONC (user-edited), written atomically (`atomic-write-file`), user-edited JSONC changes only through the CST editor, and schemas are generated by `schemars` through xtask. Test: change one setting and verify comments elsewhere in `settings.jsonc` are unchanged.
6. No binary cache and no embedded database; reopen only when a measured p95 cold start exceeds 150 ms with the JSON index.
7. Watching is limited to roots and metadata files with debounced events and a poll fallback; a per-mod recursive watch exists only for the active project. Test: a simulated `ENOSPC` falls back to polling and logs one warning.
8. Domain and DTO paths are `Utf8PathBuf`; non-UTF-8 paths are reported as diagnostics, never panics.
9. The webview gets no `fs`, `shell` or `http` permission; every path-taking command validates against registered roots. Test: a command with a path outside the roots returns a typed error.
10. Steam publishing and Workshop UGC run in `rimstudio-steam-helper` (steamworks 0.13.x, SDK 1.64) over newline-delimited JSON; the redistributable library is confined to that sidecar and its shipping decision is explicit.
11. HTTP is reqwest with rustls and the platform verifier, hand-written ETag and Last-Modified conditional fetches stored in JSON, and `backon` retries; no cache middleware.
12. Logging is `tracing` with a rolling JSON-lines file, no telemetry; errors are `thiserror` enums mapped to `{code, message, details}` IPC objects.
13. Tests use nextest, proptest, rstest, assert_fs, criterion and JSON golden files (goldenfile); tasks run through `cargo xtask`.
14. Typed IPC uses `tauri-specta =2.0.0-rc.25` with the ts-rs fallback documented, DTOs in `rimstudio-ipc-types`, 64-bit ids as strings.
15. Textures (post-v1) go through a `TextureEncoder` trait with `image_dds` as the default and an optional user-supplied `todds` backend.

## Open questions

1. Micro-benchmark quick-xml events versus roxmltree on the 46.7k workshop XML files and on the 110 MB unified Defs document before freezing the scanner design.
2. Who ships Valve's redistributable library with RimStudio (the owner as a Steamworks partner, or the user supplies it as RimCrow does), and does the owner's account allow it?
3. Is HTML scraping of Workshop pages needed at all, or are the Steam Web API and the community datasets enough (this decides whether any HTML crate is evaluated)?
4. Does the game lowercase packageIds with invariant or culture rules, and is any non-ASCII packageId present in the 690-folder corpus?
5. Owner decision: use insta in inline mode only, or standardise on goldenfile and JSON golden files to keep R10 strict.
6. Which JSONC editing operations does `jsonc-parser`'s `cst` support (insert key in order, remove, replace array element)? A one-day spike is needed.
7. Does `tauri-specta` 2.0.0-rc.25 type commands defined inside Tauri plugins (the recommended modular layout), or only app-level commands?
8. Is `tauri-plugin-window-state`'s state file JSON, or should geometry live in `settings.jsonc`?
9. Do `image_dds` and `intel_tex_2` build and run on Windows x64, macOS arm64 and Linux x64 without extra tooling, and how do their outputs compare with todds on size and speed?
10. Is zstd needed at all (why do RimSort and RimCrow both depend on zstandard), or can all payloads stay uncompressed JSON?
11. Exact feature names for reqwest 0.13.5 rustls with the platform verifier.
12. Is the Mono versus libxml2 union-with-parent disagreement a bug in one of them, and which behaviour does the shipped RimWorld runtime have?
