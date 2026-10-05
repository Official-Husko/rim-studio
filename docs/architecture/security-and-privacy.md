# RimStudio security and privacy

This document defines the threat model, the enforced security rules and the privacy stance of RimStudio. It covers hostile mod folders and archives, hostile datasets, path traversal, link games, hostile Workshop text, supply chain and update tampering, then the Tauri capability and CSP policy, path-root validation, secret storage, dataset validation, staging and link-farm safety, sidecar isolation, description rendering, logging redaction, telemetry, dependency policy and the repository checks that enforce licence hygiene (R11). It closes with a draft privacy statement. It builds on the decisions D-026, D-030, D-032, D-039, D-040, D-047, D-048, D-059, D-060, D-061, D-067 in the [decision register](decision-register.md) and the crate layout in [overview](overview.md) and [crate catalog](crate-catalog.md); platform specifics are in [cross-platform](cross-platform.md).

Status: draft | Last updated: 2026-10-05

Evidence: [webview and IPC performance](../research/webview-and-ipc-performance.md) (capabilities, CSP, isolation), [rules fetch and merge design](../research/rules-fetch-and-merge-design.md) (dataset limits, quarantine), [workshop publishing research](../research/workshop-publishing-research.md) (sidecar, credentials), [rust crate research](../research/rust-crate-research.md) (cargo-deny, keyring, archives), [frontend stack research](../research/frontend-stack-research.md) (rich text, sanitiser), [steam and game detection](../research/steam-and-game-detection.md) (link farm rules).

## 1. Security posture in one page

RimStudio is a local desktop tool that reads large amounts of third-party content (mods, Workshop descriptions, community datasets, game logs) and writes into a small set of places the user owns. The main risks are therefore not network attackers against a server but untrusted files and text reaching a privileged process, and the app damaging the user's files by mistake. The posture:

1. Untrusted input is data. It is parsed into typed structures with size caps, never executed, never interpolated into HTML, never used as a path without validation.
2. The webview is a low-privilege renderer. It has no filesystem, shell or HTTP access of its own; every capability goes through typed commands that validate arguments in Rust (invariant I-14, D-047).
3. The write surface is small and fenced. App-owned files live in four roots; the game folder accepts only owned links and `ModsConfig.xml` after a backup (I-05).
4. Network access exists in exactly one crate (`rimstudio-datasets`, the only HTTP client) and carries no user identifiers.
5. No secrets of the user's Steam account are ever handled. No telemetry exists.

## 2. Threat model

Assets: the user's mod library and load orders, `ModsConfig.xml`, the user's own mod sources (possibly months of work on an external drive), Steam account state, the app's settings and caches, the update channel.

Actors: authors of malicious or sloppy mods, authors of hostile Workshop pages, a compromised or careless dataset source, a network attacker on an unencrypted path, a compromised dependency or CI action, a local user or process with the same privileges (out of scope beyond file permissions).

| # | Threat | Entry point | Impact | Primary control | Section |
| --- | --- | --- | --- | --- | --- |
| T1 | Malicious mod folder: huge files, deep trees, loops, device files, crafted XML (entity bombs, deep nesting) | scanner, XML boundary, def indexer | hang, memory exhaustion, crash | caps, depth limits, no entity expansion, loop detection, jobs with cancellation | 3 |
| T2 | Malicious archive (zip bombs, absolute paths, `..`, symlink entries) | mod pack import, dataset archives, tool downloads | file written outside target, disk fill | never extract blindly; member and size caps; path and entry-type rejection | 3, 7 |
| T3 | Hostile dataset (wrong shape, collapsed content, oversized, ratio bomb) | RemoteDataset pipeline, imports of RimSort user rules | corrupted sorting, DoS | validation, quarantine, last good copy kept | 7 |
| T4 | Path traversal through IPC arguments or data (About.xml names, rule files, dataset entries, project names) | commands that take paths or names | read or write outside allowed roots | RootGuard on every path-taking command; name sanitisation | 5 |
| T5 | Symlink or junction games: a mod folder that is a link to a sensitive location; a link swapped after validation | scanner, staging, deploy | disclosure, deletion outside owned paths | do not follow links outside roots; canonicalise then re-check at use; unlink-only cleanup | 9 |
| T6 | Hostile Workshop content: HTML, script, tracking images, BBCode abuse in descriptions | description renderer, preview images | script in webview, tracking | AST to vnodes, sanitiser backstop, remote images only via backend | 11 |
| T7 | Supply chain: malicious crate or npm package, compromised CI action, typosquat | build | code execution on user machines | pins, lockfiles, cargo-deny, audit, pinned action hashes | 13 |
| T8 | Update tampering | updater channel | arbitrary code | signed updates verified against an embedded public key, HTTPS enforced | 14 |
| T9 | Secret leakage: tokens or SteamIDs in logs, settings or bug reports | logging, settings | account exposure | credential store, redaction, no Steam passwords at all | 6, 12 |
| T10 | Compromised sidecar or hostile helper output | Steam helper protocol | crash, spoofed result | process isolation, protocol validation, exactly one terminal event | 10 |
| T11 | Webview compromise (XSS through a bug) | any rendered untrusted text | calls to backend commands | minimal capability, CSP, command argument validation, no ambient authority | 4 |
| T12 | Licence contamination (copying GPL or non-commercial material into the repo) | contributors | legal exposure | repository checks | 15 |

Out of scope: a malicious game or Steam client, an attacker with local code execution under the user account, and side channels. Mods can contain compiled assemblies that RimWorld itself runs; RimStudio never loads, executes or reflects over mod assemblies in-process (the planned type-table reader parses metadata from files only, spike S-07) and says in the UI that "active mods run code in the game" is the user's trust decision, not something the manager can sandbox.

## 3. Parsing untrusted files

1. XML. All XML goes through `rimstudio-xml` (quick-xml, Game and Tolerant modes). Entity and DTD handling: custom entities are not expanded beyond the five predefined and numeric ones; DOCTYPE content is skipped and reported as a diagnostic `xml.doctype-ignored`. Nesting depth, attribute count per element, text size per node and file size have caps (initial values: 64 MiB per file, depth 256, tunable in settings with hard maximums) beyond which the file is skipped with a diagnostic and the rest of the library continues. Content problems are `Diagnostic`, never a panic or an abort (I-10).
2. JSON and JSONC. App files are read with size caps and schema versions; a file newer than the app is read-only; a parse failure keeps the previous file (backups, D-026) and reports an error with the file name.
3. Scanner. Two-phase and bounded: at most 8 workers, directory depth capped per source kind, loop detection by `(device, inode)` or file id, special files (devices, pipes, sockets) ignored, and every IO error turned into a diagnostic. Each scan is a job with a cancel token checked between units of work.
4. VDF and ACF. The in-house reader has no recursion without a depth cap, tolerates malformed input and returns partial results with warnings; it was differentially tested against a reference parser on real fixtures. `#base` and `#include` are recorded and never followed.
5. Logs. Player.log classification reads with line length and total size caps and never evaluates log text.
6. Images. Previews are decoded with size and dimension caps in Rust and served through the `rsimg` scheme as thumbnails; a decoder failure yields a placeholder.
7. Native code. Parsers are safe Rust; the only native code loaded is the Steam library inside the sidecar (section 10).

## 4. Tauri capability and CSP policy

### 4.1 Capability

One capability file, `apps/desktop/src-tauri/capabilities/main.json`, scoped to the `main` window, containing:

1. `core:default`.
2. Window permissions (`start-dragging`, minimise, maximise, close) only on platforms where the custom titlebar is used.
3. `dialog:allow-open` for folder and file pickers (the chosen path then goes through RootGuard when used).
4. A scoped `opener` permission allowing only `https:` URLs from an allow list (project, documentation, Workshop item pages) and the reveal-in-file-manager action for paths inside known roots.

Never granted to the frontend: `fs`, `shell`, `http`, `store`, `sql`, `log`, `process` (relaunch is a Rust command after update). Plugin commands are blocked until a capability grants them; app commands are restricted with `AppManifest::commands`, generated from the single registry (D-005), so a command not in the registry cannot be invoked. Secondary windows get their own capability file with the minimum set. Permissions are generated from the registry by `xtask`, and a test asserts that `capabilities/` contains no permission outside the allow list above.

### 4.2 CSP

Configured in `tauri.conf.json` (the CSP is enforced only when configured):

| Directive | Value | Reason |
| --- | --- | --- |
| `default-src` | `'self' ipc: http://ipc.localhost` | nothing else is reachable |
| `script-src` | `'self'` | no inline script, no eval |
| `style-src` | `'self'` plus `'unsafe-inline'` only if dynamic style attributes require it (decision recorded when the first need appears) | Tailwind output is a file |
| `img-src` | `'self' data: rsimg: http://rsimg.localhost` | previews come only from the custom scheme; `data:` for tiny inline icons only |
| `font-src` | `'self'` | fonts are self-hosted |
| `connect-src` | `ipc: http://ipc.localhost` | the frontend cannot open network connections |
| `object-src` | `'none'` | |
| `frame-src` | `'none'` | no iframes |
| `base-uri`, `form-action` | `'none'` | |

The webview never loads remote content. Remote images (Workshop previews, avatars if ever shown) are fetched by the backend, validated, cached and served as `rsimg://localhost/<id>?w=128` (D-048). The scheme handler uses an id allow list (ids minted by the backend, not paths), so it cannot be used to read arbitrary files, and responds with content type, `Cache-Control` and ETag. The Tauri asset protocol stays disabled unless a narrow user-chosen file scope is needed, and then it is scoped to that file.

### 4.3 Isolation and devtools

Isolation is off in the first release (D-047) because every command validates its arguments in Rust and the webview loads only first-party code. It is re-evaluated if the lab measures under 1 ms per command and over 100 MB per second raw throughput, or if third-party code ever runs in the webview. Release builds exclude DevTools; a diagnostics build feature includes them. The embedded WebDriver server sits behind an `e2e` Cargo feature that the release check verifies is absent.

### 4.4 Frontend rules that support the policy

Only `shared/platform` imports `@tauri-apps/*`; no component calls `invoke` directly; no `innerHTML`, `dangerouslySetInnerHTML` or `eval` on untrusted strings (an oxlint restriction and a code-review checklist item); no XML library in the frontend; `window.open` and navigation are intercepted and routed to the scoped opener.

## 5. Path-root validation

Every command whose request contains a path or a name passes through RootGuard (in `rimstudio-io`) before any filesystem call. This is a hard rule, enforced by a registry attribute and a test.

1. The registry declares per command which request fields are paths and which root class each may fall in: `LibraryRoot` (configured custom folders, Mods, Workshop), `ProjectRoot`, `DataRoot` (config, data, cache, logs), `GameRoot` (install and game config, read-only except the fence), `UserPick` (a path just chosen in a dialog, valid for that one command, then registered if the command adds it as a root).
2. Validation steps: reject relative paths, reject paths containing NUL or, on Windows, reserved names and alternate data stream syntax; expand nothing (no `~`, no environment variables after input time); canonicalise the longest existing prefix; require the result to be inside an allowed root by component comparison (never string prefix); reject a result that crosses a link whose target is outside the allowed roots; compare case-folded only on volumes probed as insensitive.
3. Names (project names, mod folder names, file names from the designer and scaffolder) are sanitised to a safe single path component: no separators, no `..`, no reserved names, no trailing dots or spaces, length capped.
4. TOCTOU: validation and use are done in the same function through a `ValidatedPath` newtype that carries the canonical path and the root class; destructive operations re-check immediately before acting and use handle-based calls where the OS offers them (open the directory, check, act relative to the handle) in `rimstudio-platform`.
5. Write commands additionally require the write class of the root: the game root accepts only the two fenced operations (section 9); library roots are read-only for the manager; project roots are writable only for the project tools; the data root is writable only through the stores.
6. Violations return the error code `io.path-outside-roots` and are logged with the command name and root class, not the full path (section 12).
7. Tests: a property test feeds traversal strings (`..`, mixed separators, long names, Unicode lookalikes, links) to every path-taking command through the registry and asserts no filesystem effect outside the root; `xtask check-docs` fails if a registry command has a path field with no declared class.

## 6. Secrets and credentials

1. Steam passwords are never requested, stored or logged, under any feature (D-061 rule). Publishing uses the Steam client's existing login through the sidecar; if the client is not logged in, the UI says so and stops. The optional SteamCMD route only generates a VDF and instructions the user runs themselves.
2. Which secrets can exist at all: an optional token for a dataset host or the GitHub API (rate limits), and nothing else is planned. If no feature needs a token the credential store stays unused.
3. Storage: the OS credential store through the `CredentialStore` port (macOS Keychain, Windows Credential Manager, Secret Service on Linux), implemented in `rimstudio-platform` with the keyring crate (4.2.0 per the research, verified on crates.io on 2026-10-04). Where no store exists (headless Linux, some Steam Deck states), a fallback file `secrets.json` in the config root with mode 0600 (user-only access control on Windows) is used, clearly labelled as weaker in Settings. The decision to use the keychain at all is an owner decision (D-030).
4. Secrets never appear in `settings.jsonc`, project files, caches, exports, error envelopes, logs or the Playwright mocks. Settings store only a reference name.
5. The frontend can set or clear a secret and read only whether one exists; it can never read the value back.
6. Process arguments: secrets are never passed on a command line (visible to other processes); the helper receives data on stdin.
7. SteamIDs and account names are treated as personal data (section 12).

## 7. Dataset validation, limits and quarantine

The `RemoteDataset` pipeline in `rimstudio-datasets` (D-032) is the only code that downloads dataset content, and the only HTTP client in the workspace.

1. Transport: HTTPS only, certificate validation through the platform verifier, redirects limited to a small count and to HTTPS, an allow list of hosts per dataset in the dataset descriptor, a fixed user agent that identifies the app and version only.
2. Limits (from the design note): per dataset `maxCompressedBytes` 2 MiB, `maxDecodedBytes` 32 MiB and `maxRatio` 50 for the typical rules dataset, with a larger configured `maxBytes` (20 MiB) for the slim SteamDB source; the response is streamed to a temporary file in the cache root while counting bytes, and gzip is decoded through a limiter. Exceeding any cap aborts the download.
3. Conditional GET with ETag so unchanged data costs one small request.
4. Archives: never extracted to disk. The reader opens the archive, reads only the named member, and enforces a member count of at most 10,000, declared and actual uncompressed size under the cap, ratio under the cap, and rejects absolute paths, `..` components, symlink entries and encrypted members.
5. Validation: UTF-8, expected root shape, entry count floor, and a collapse guard: fewer than 70 percent of the previous entry count, or fewer than the absolute floor, or an older dataset timestamp, is rejected. Unknown keys are preserved (lossless foreign data, I-18) but never interpreted as code or paths.
6. Quarantine: a rejected download is kept in `quarantine/<sha>.<ext>` with a `<sha>.reason.json`, capped at three entries, never retried until the upstream bytes change; the previous good copy stays active and the dataset panel shows the reason with an "inspect" and a "force accept" action (force accept requires an explicit confirmation and is logged).
7. Rotation: temp file, verify, then atomic rename with the last good copy kept; a crash at any point leaves either the old or the new file.
8. Imported user data (RimSort user rules, lists) is validated with the same shape checks and never overwrites app files directly; it creates a new layer with provenance.
9. Dataset content is never rendered as HTML and never used as a path. Mod names and notes from datasets go through the rich-text renderer (section 11).
10. No mirror: datasets with unclear licences are fetched at runtime only and never stored in the repository or hosted by the project (R11, D-034).
11. Trust statement shown in the dataset panel: source host, retrieval time, hash, entry count, and "community data, unreviewed by RimStudio".

## 8. Network surface

1. The only outbound destinations are: dataset hosts (per descriptor), the update manifest and artifacts on GitHub Releases, and (user-triggered) Workshop item pages opened in the system browser. Nothing else is contacted.
2. Every outbound feature has an off switch in Settings; an "offline mode" disables all of them, and the app is fully usable offline with the last good datasets.
3. Requests carry no cookies, no account identifiers, no machine identifiers and no list of installed mods.
4. Proxy settings follow the system; there is no custom TLS root store.
5. The `xtask check-deps` rule bans HTTP client crates outside `rimstudio-datasets` and the updater plugin.

## 9. Staging and link-farm safety rules

These rules restate D-039 and D-040 as testable requirements. A violation of any of them is a release blocker.

### 9.1 Game-folder write fence

1. The only writes under the install folder or the game's config folder are (a) creation and unlinking of owned link-farm entries and owned copies in `<install>/Mods`, and (b) `ModsConfig.xml` after a timestamped backup and a running-game check.
2. A write outside the fence is impossible by construction: all writes go through `rimstudio-io`'s `GameWriteFence`, which takes a typed operation (`CreateOwnedLink`, `RemoveOwnedLink`, `CreateOwnedCopy`, `RemoveOwnedCopy`, `WriteModsConfig`) and refuses anything else. Tests use `RecordingFs` to assert the complete set of operations of every deploy scenario.
3. The user's mod folders are never modified by the manager: not written, not renamed, not deleted, not touched for timestamps. Source folders are opened read-only.

### 9.2 Ownership manifests

1. Every link or copy created is recorded in a JSON manifest in the data root, keyed by install id: link path, target, target id, creation time, kind.
2. A folder in `Mods` is owned only if it is in the manifest and either is a link whose current target equals the recorded target, or is a copy with a valid `.rimstudio.json` marker. Anything else is foreign: it is never replaced, never removed, and a name collision is resolved by choosing another name or reporting.
3. Nothing is ever written into the user's source folder (no markers in link targets).

### 9.3 Unlink-only cleanup

1. Removal of an owned link uses the link-removal primitive of the platform (`remove_file` on Unix symlinks, `remove_dir` on Windows junctions). `remove_dir_all` is never called on a path that is a link or whose link status could not be determined; the clippy `disallowed-methods` policy bans `remove_dir_all` outside `rimstudio-io`, where it is reachable only through the fence operation `RemoveOwnedCopy`, which first verifies ownership and that the path is not a link.
2. Owned copies are removed only after the marker is verified and the path is confirmed to be inside `<install>/Mods`.
3. The sentinel test: a link target containing a sentinel file is unlinked, and the test asserts the sentinel and the whole target tree are intact.
4. Cleanup is idempotent and journaled; a crash mid-apply is repaired at next start from the journal and manifest.

### 9.4 Apply rules

1. A dry-run plan is always shown first ("will create 14 links, remove 2, copy 1") and the user confirms.
2. No changes while the game runs; an `unknown` running state warns and requires confirmation.
3. Targets inside the Mods folder, the Mods folder itself, or an ancestor are refused; links are created only at depth one.
4. Link names carry a stable source suffix so two sources never collide.
5. Pre-launch check: every active id must resolve; otherwise launch is blocked and the ids are listed, because the game would silently deactivate them. `ModsConfig.xml` is backed up before every launch and diffed after exit.

### 9.5 Publish staging

1. Upload content comes only from a staging copy produced by the ignore rules from the project JSONC (`uploadIgnore`), with defaults excluding `.git`, build outputs and raw asset folders.
2. The plan lists every included file and the total size; the staged tree is verified against the plan (path, size, blake3) before the first Steam call.
3. Staging never follows links out of the project root and never includes files outside it; links inside a project are copied as the files they refer to only when the target is inside the project root, otherwise they are reported and skipped.
4. The staging folder is in the cache root and is deleted by removing the files that the plan created, not by recursive removal of an arbitrary path.

### 9.6 Linking one project for testing

D-170 and D-171 ([ADR 0050](../adr/0050-link-one-project-into-the-game.md)). This is the one write under the install that a person triggers from the project page, and it is the entry kind of rule 9.1 (a): a link (or, on request, a marked copy) directly inside `<install>/Mods`, recorded in the ownership manifest before it is made.

1. The name is the project folder name made safe as one path component; names that start with a dot, contain a separator or traversal, or are device names never reach the fence (they are changed or refused), and the fence refuses them again.
2. The target is the canonical real folder of the project. A target inside the game folder, equal to it or containing `Mods` is refused. A project folder that is a link is followed to its real folder and checked as such.
3. An existing entry is never replaced. A removal needs a manifest record, a link that still points at the recorded target and the link primitive; a real folder is never removed, a marked copy only after the fence verified the marker.
4. A running game refuses the create unless confirmed; the remove is never blocked.
5. A refusal because the `Mods` folder is missing or read only shows the command to run by hand; the app never changes permissions and never creates the `Mods` folder.
6. Tests: `rimstudio-io/tests/fence_link_allowance.rs` (every other write under the install is refused, no name leaves `Mods`, no target inside the game folder), `rimstudio-library/tests/deploy_links.rs` (foreign entries, stale and retargeted links, a link into the install, a read only folder, a running game, a record without an entry, a damaged record) and the CLI and toolkit tests.

## 10. Sidecar isolation

`rimstudio-steam-helper` exists so that Valve's native library never loads into the main process (I-15).

1. The main app starts and works with no Steam library present; absence disables only Publish.
2. The helper is spawned through the `Launcher` port, not through the Tauri shell plugin, with a cleared environment except what Steam needs, its own work directory containing `steam_appid.txt` with `294100`, and no inherited file handles.
3. Protocol: newline-delimited JSON on stdin and stdout, version 1, one operation per process, validated against the DTOs in `rimstudio-ipc-types`, line length capped, unknown message types ignored with a warning, exactly one terminal event (`done` or `error`); a missing terminal event is a distinct error code. A watchdog kills the process on silence beyond the configured timeout.
4. The helper never binds a network port, never calls `SteamAPI_RestartAppIfNecessary`, never writes into the RimWorld install (a filesystem assertion test compares the install tree before and after a mocked run) and receives only paths already validated by the app and a staging folder.
5. Output is treated as untrusted: ids are strings validated as digits, messages are length capped and rendered as text.
6. The helper binary is notarized with the macOS app (Windows builds are unsigned, D-087); `binaries/` is git-ignored and the build verifies its hash against a value recorded by CI. Valve's library is never shipped: the helper loads it from the user's own Steam or RimWorld installation (D-062, D-086) and may be absent.
7. Logs from the helper redact SteamIDs the same way as the app (section 12).

## 11. Description rendering

Workshop descriptions, About.xml descriptions, dataset notes and mod names are untrusted text.

1. Pipeline: BBCode (via `@bbob/core`), Unity rich text and Markdown (via `marked`) are parsed to an AST by `shared/rich-text`, and the AST is converted to Preact vnodes through a closed list of node types (text, emphasis, strong, list, link, code, image placeholder, colour from a validated palette). There is no `innerHTML` anywhere on the main path, so no sanitiser is needed for correctness.
2. Raw HTML in input is rendered as literal text, never as markup.
3. Links: scheme allow list (`https`, `http`); `javascript:`, `data:` and others are dropped; links open only through the scoped opener in the system browser with a visible domain; `rel` attributes are set.
4. Images: remote images are not loaded by the webview. They appear as placeholders with an explicit "load" action that makes the backend fetch, validate and cache the image behind `rsimg` (size and type caps). Tracking pixels therefore never fire from the webview.
5. Colour and size directives are clamped to allowed ranges; deep nesting is capped; total node count is capped with a "show more" control.
6. Backstop: where raw HTML must be inserted (none planned in v1), DOMPurify 3.4.16 runs with a strict allow list and the result is also constrained by CSP. A test renders a corpus of hostile strings (script tags, event attributes, `javascript:` links, SVG with script, CSS expressions, huge nesting) and asserts that no element with an event attribute or script appears in the DOM.
7. The same renderer is used by logs, notes and the dataset panel so there is one place to audit.

## 12. Logging, redaction and diagnostics

1. Logging is `tracing` with JSON lines to rolling files in the logs root (D-060); the Tauri log plugin and error-reporting services are not used.
2. Levels: info by default; debug on demand; payload bodies are not logged by default.
3. Redaction layer: a `tracing` layer rewrites fields before writing: the home directory prefix becomes `~`, the OS user name becomes `<user>`, SteamIDs (64-bit ids and account ids) become a short hash prefix, tokens and anything stored via `CredentialStore` are removed, and long free text from mods is truncated. A unit test feeds sample secrets and paths and asserts none survive.
4. The error envelope `{code, message, errorId, details}` shows the user a stable code and an id; details with paths are kept in the log under that id and are redacted in the same way.
5. "Copy diagnostics" produces a JSON bundle: app version, OS and webview versions, detection report with paths redacted, settings keys (not values of path fields unless the user ticks a box), and recent log lines with redaction applied. The user sees the bundle before it is saved or copied. Nothing is sent automatically.
6. Log retention is capped by size and count; logs live only on the user's machine.
7. Crash handling writes a local crash marker and a redacted log excerpt; no minidump upload.

## 13. Dependency and supply-chain policy

### 13.1 Rust

1. `Cargo.lock` is committed. Versions follow the pins in the decision register; pre-release crates (for example notify 9.0.0-rc.5, zip 9.0.0-pre3, tauri 3.0.0-alpha) are not used in releases, except those named explicitly in the register with a recorded reason (tauri-specta `=2.0.0-rc.25`).
2. `deny.toml` (tooling config in TOML, as cargo-deny requires) contains:
   1. licences: an allow list of permissive licences (MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, CC0-1.0 (needed by `notify` 8.2.0, whose only licence on crates.io is CC0-1.0) and similar, MPL-2.0 for file-level copyleft dependencies if accepted by the owner), and a deny for GPL, AGPL and non-commercial licences so that nothing from the references can enter by dependency; the policy concerns dependency licences only, because the project's own licence is custom and added later (D-088);
   2. bans: every XML crate other than quick-xml (roxmltree, xml-rs, xmltree, minidom, libxml), with `wrappers` limiting quick-xml to `rimstudio-xml` and the known transitive users inside the Tauri tree; HTTP client crates outside `rimstudio-datasets`; OpenSSL where rustls suffices; SQLite crates except `rusqlite` behind the `aux-db` feature of `rimstudio-datasets`, a read only foreign format import accepted by the owner (D-031, D-083);
   3. advisories: yanked and vulnerable crates fail the build; `cargo-deny advisories` covers what `cargo audit` would, and a scheduled workflow runs it daily so new advisories surface without a commit;
   4. sources: only crates.io; no git dependencies without a recorded exception.
3. `cargo xtask check-deps` mirrors the bans on workspace manifests so the failure is explained in project terms, and checks that `steamlocate` and `keyvalues-parser` appear only as dev-dependencies of `rimstudio-steam` (D-037).
4. Licences of shipped dependencies are collected into a third-party notices file at packaging time by `xtask package`.
5. `cargo-machete` and `cargo update` runs are part of the scheduled maintenance workflow; updates arrive as reviewed pull requests, never automatically merged.
6. Build scripts and proc macros are the main risk; new dependencies with build scripts are reviewed (a short checklist in the contributor guide) and `cargo vet` or `cargo-crev` adoption is deferred (not evaluated).

### 13.2 Frontend

1. Exact version pins and the committed `pnpm-lock.yaml`; `pnpm audit` in CI; lifecycle scripts disabled by default except an explicit allow list (pnpm's `onlyBuiltDependencies`); the dependency count is kept minimal (frontend research risk 9).
2. `knip` finds unused dependencies; dependency-cruiser enforces folder boundaries; oxlint forbids restricted imports.
3. Fonts and assets are self-hosted; no CDN at runtime.

### 13.3 CI and release

1. GitHub Actions are pinned by full commit hash with a version comment; a scheduled job proposes bumps.
2. Workflows use least-privilege `permissions:`; the release job alone has `contents: write` and the signing secrets, runs only on tags from the protected branch, and secrets are never available to pull requests from forks.
3. The release job writes `SHA256SUMS` and build provenance attestations.
4. Reproducibility: toolchain pins (`rust-toolchain.toml`, Node and pnpm versions) are verified by `xtask check-tools`.
5. Two-person review for changes to `deny.toml`, `.github/workflows/`, `capabilities/`, `tauri.conf.json` security fields and `xtask/layers.jsonc` (a CODEOWNERS entry; effective when there is more than one maintainer).

## 14. Update integrity

1. Updates are verified against the public key embedded in the app (`plugins.updater.pubkey`); an unsigned or wrongly signed artifact is rejected by the updater.
2. Manifest URLs are HTTPS (enforced in release builds); channel selection changes only the URL, never trust roots.
3. The private signing key exists in CI secrets and one offline backup; compromise response is a bridging release signed with the old key that carries a new public key, plus an advisory.
4. The app never downloads executable code except through the updater (no plugin system, D-066; no scripts fetched from datasets).
5. Platform signing is an additional layer: macOS notarization (path still open); Windows builds are unsigned by owner decision (D-087) and the SmartScreen warning is documented as expected.
6. The updater does not run during imports, deploys or `ModsConfig.xml` writes.

## 15. Licence hygiene as repository checks (R11)

R11 is enforced by tooling so it does not depend on memory.

| Rule | Check | Where |
| --- | --- | --- |
| RimSort (GPL-3.0), RimCrow (MIT) and Combat Extended (CC BY-NC-SA 4.0) are read-only concept references | the reference trees live in `reference/` (after the D-011 rename), are git-ignored and excluded from every build input; `xtask check-licences` fails if any tracked file lies under `reference/` or if a workspace manifest has a path dependency into it | `xtask` |
| No copied code | a content fingerprint check compares each tracked source file against shingled hashes of the reference trees (computed locally on the maintainer's machine from the ignored references; the hash list itself is not committed, because it would encode reference content, and CI runs the check only when the references are present); reviewers use a pull request checklist item | `xtask check-licences`, review |
| No reference data files | tracked files with extensions of data files (`.xml`, `.json`, `.jsonc`, `.csv`) are limited to an allow list of directories (`tests/fixtures`, `tests/vectors`, `tests/golden`, schemas, locales, configuration); any new data directory needs a registry entry | `xtask check-licences` |
| No vanilla or CE values in the repo | test vectors use fictional numbers; a denylist of token patterns characteristic of real vanilla and CE def names and stat tables (maintained as a short list of identifiers, not values) fails the build when found outside documentation research notes | `xtask check-licences` |
| Datasets never bundled or mirrored | a check fails if any tracked file matches a dataset signature (root shape of the five datasets) or exceeds a size threshold in data directories; dataset descriptors contain URLs only | `xtask check-licences` |
| Real-install tests opt in | tests that read a real game install are `#[ignore]` and require an environment variable; the check greps for tests referencing install paths without the attribute | `xtask check-licences` |
| Dependency licences | `cargo deny check licenses` with GPL, AGPL and non-commercial denied | CI quality leg |
| Own licence | custom and added later (D-088): no `license` field, no `LICENSE` file and no headers until the owner supplies it; third-party notices are generated at packaging | owner supplies the text |
| Research notes | notes may describe references in their own words and cite file names; they contain no verbatim code blocks from references over a short length (a line-count lint on fenced blocks with a reference path citation) | `xtask check-docs` |
| Fixtures | created by the fixture builder from synthetic descriptions; no real Workshop mod content | `xtask fixtures` |

Where a check needs the reference trees and they are absent (CI), it degrades to the structural checks only and says so in its output. The owner action to request licences for the community datasets (D-034) stays open and does not block anything because nothing is bundled.

## 16. Review checklist for new features

1. Does it take a path? Declare the root class in the registry.
2. Does it parse untrusted data? Add caps, a fixture and a cancel point.
3. Does it show untrusted text? Use `shared/rich-text`.
4. Does it write? Name the root and the fence operation.
5. Does it touch the network? It belongs in `rimstudio-datasets` with a descriptor and an off switch.
6. Does it handle an identifier of a person or account? Redact it.
7. Does it add a dependency? Run cargo-deny and read its build script.

## 17. Privacy statement (draft)

This statement is written for the application's About page and the repository, and is a draft pending owner review.

RimStudio works on your computer with your files. It does not collect analytics, usage statistics, crash reports or advertising identifiers, and it contains no telemetry code. It does not create an account and does not ask for or store your Steam password.

What RimStudio reads. It reads your RimWorld installation, your Steam library description files, the Workshop and mod folders you point it at, your RimWorld configuration and logs, and any files you open or import. This data stays on your machine.

What RimStudio writes. Settings, caches, project data and logs are stored in folders owned by RimStudio (visible in Settings, and movable with the portable marker file). In your RimWorld folder it only creates and removes the links or copies it created for your custom mod folders, and it edits the active mod list file after making a backup. It never modifies your own mod folders.

What RimStudio sends over the network. Only when the feature is enabled: it downloads community sorting data and related lists from their public hosts, checks for application updates on the project's release page, and, when you ask, opens Workshop pages in your browser. These requests contain no information about you, your mods or your computer beyond what any web request reveals (your IP address, and the application name and version in the user agent). You can turn each feature off, or use offline mode.

Publishing to the Workshop. The publish tool uses the Steam client that is already running and signed in. RimStudio does not see your password. The process that talks to Steam is separate from the main application, and your Steam account number is hidden in logs.

Logs and diagnostics. Logs stay on your computer with your user name, home folder and Steam numbers masked. If you share diagnostics, you see exactly what will be shared first.

Secrets. If a feature needs a token, it is stored in your operating system's credential store, or, where none exists, in a file readable only by you, and that is shown in Settings.

Third-party data. Community datasets are downloaded at runtime from their authors and are not changed or redistributed by RimStudio.

Changes. Any change to this statement is listed in the release notes, and any new network destination requires a new release and a note here.

## 18. Owner decisions

1. Keychain use and the fallback file for secrets (D-030).
2. Acceptance of MPL-2.0 file-level copyleft dependencies in the cargo-deny allow list.
3. Approval of the privacy statement text and of the rule that any new network destination requires a documented release.
4. Whether to enable two-person review rules once there is more than one maintainer.

## 19. Write path review, 2026-10-05

RimStudio writes into folders that belong to people and that may be uploaded to the Workshop, so a bug in the write path loses work or ships junk. This section records an adversarial review of `rimstudio-io` (guard, fence, atomic, backup, collection, store), `rimstudio-xml` (edit, load folders, render) and `rimstudio-toolkit` (guarded writer, designer apply and plan, project create), the attacks that were tried, the defects found and the rules that now hold. The tests are named after the attack and live in each crate's `tests/` folder: `rimstudio-toolkit/tests/write_path_attacks.rs`, `apply_write_safety.rs`, `project_create_attacks.rs`, `plan_hostile_files.rs`, `rimstudio-xml/tests/hostile_documents.rs` and `rimstudio-io/tests/store_attacks.rs`.

### 19.1 Threats considered and where each is stopped

| Threat | Stopped by | Test |
|---|---|---|
| Plan path that is absolute, has `..`, a drive prefix, UNC, `\\?\`, backslashes, a NUL or control character, an alternate stream `:` or a wildcard | `is_safe_relative_path` in the planner, then `check_relative` in the writer (`sanitize_name` on every segment) | `hostile_relative_paths_are_refused_and_nothing_is_written` |
| Trailing dot or space, Windows device names (`CON`, `NUL`, `COM1` to `COM9`, `LPT1` to `LPT9`, the superscript digit forms, `CONIN$`, `CONOUT$`) with any extension, on every platform | `sanitize_name` (the mod may be played on Windows even when authored on Linux) | same test, `guard.rs` unit tests |
| Segment over 255 bytes, relative path over 200 bytes | `check_relative` (`MAX_REL_PATH_BYTES`) | same test |
| Symlink inside the project that points outside it, for a file and for a folder, and a dangling link | `RootGuard` (canonical path must stay under the canonical root), then a re-check right before the bytes go down | `a_directory_link_that_leaves_the_project...`, `a_file_link_that_leaves_the_project...`, `a_dangling_link_in_the_project_is_refused` |
| Link inside the project that points inside it, as the target file | refused: a write would replace the link with a plain file; links to folders inside the project are followed | `a_file_link_that_stays_inside_the_project_is_not_replaced...` |
| Project folder that is itself a link | refused by `GuardedWriter::new` ("open the real folder"); a link in an ancestor is resolved and the real folder is used | `a_project_folder_that_is_itself_a_link_is_refused`, `..._reached_through_a_linked_parent...` |
| Project inside the game install, the game config folder, a reference mod folder or any Steam library | protected folder list (resolved, compared per component, with letter case folded) and a `steamapps` component check on the resolved root | `a_project_inside_a_steam_library_is_refused...`, `a_protected_folder_inside_the_project_is_refused...` |
| Name that differs only by letter case from an existing entry (a case sensitive disk would grow a twin folder that breaks on Windows and macOS; a case insensitive disk would silently update another file) | `case_twin` in the writer refuses the write; `check_plan_paths` refuses two plan paths that fold to the same text and a plan path that is the folder of another | `a_name_that_differs_only_by_case...`, `two_plan_paths_that_differ_only_by_case...` |
| Time of check to time of use | the writer validates, creates the parent folders, validates again, and validates a third time after the backup, then does all input and output through the canonical path; a file that no longer holds the text the plan was made from is refused (`Expect::Previous`) | `a_file_that_changed_on_disk_after_planning...`, `a_file_that_appeared_after_planning...` |
| Read only file | refused before the first write of the whole plan (`preflight`) | `a_read_only_file_in_the_plan_refuses_before_the_first_write` |
| Hostile or unusual existing files (UTF-16, Latin-1, binary, truncated, two roots, trailing text, another root name) | `SpanEditor::open` fails closed; the plan reports `designer.merge-failed` and leaves the file out | `files_that_are_not_utf8_text...`, `defs_files_with_another_root...`, `hostile_documents.rs` |

The three layers agree: for every hostile path of the table that a layer can see, `RootGuard`, `GameWriteFence::check_project_write` and `GuardedWriter` all refuse (`the_root_guard_the_fence_and_the_writer_agree_on_every_hostile_path`). The writer is the strictest because it also knows the names. The app does not yet install the optional fence into the project environment (`ProjectEnv::with_fence` has no caller); the protected folder list of the designer context and the Steam library check stand in for it, and the fence refusal is covered through the toolkit tests only.

### 19.2 Apply: atomicity, order and recovery

1. Per file: the file is the old version or the new version at every instant. The write is `atomic-write-file` (temporary file in the same folder, fsync, rename, folder fsync); the injected crash tests of `rimstudio-io` fail every step and assert old or new bytes. The backup of a replaced file is taken first, verified by reading it back, and a failed backup stops the write (`a_backup_that_cannot_be_made_stops_the_write...`).
2. Per plan: files are written definitions first, patches next, `LoadFolders.xml` last. A stop after any number of files (cancel, error) leaves whole files only, and the gate file is present only when every patch it enables exists (`a_stop_after_any_file_leaves_whole_files_and_the_gate_only_after_its_patch`, four stop points). A rerun of the same request writes only what is missing.
3. Before the first byte, every pending file is preflighted: path, read only flag and that the file still holds exactly the text of the plan. Any problem refuses the whole plan with nothing written.
4. A write error after at least one file was written does not discard the report: the result lists what was written and carries the error diagnostic `designer.apply-write-failed`; the files after the failing one are not written and no dry apply runs. A write error on the first file is an error and writes nothing.
5. Disk full and permission denied surface as store errors from `atomic_write`; the target is untouched because the failure happens in the temporary file or in the rename. Permission denied is covered by a read only folder test on Unix (skipped when the test process ignores permission bits).
6. Windows semantics (rename over a file another program holds open) cannot be run here. The code path is the same as a refused rename: `atomic_write` returns the error with the old file intact and the temporary file removed on drop; `atomic_write_with` (collections) falls back to rename-aside and, since this review, a crash inside that window is repaired by `cleanup_stale_temps` (roll forward from the synced temporary file, or move the aside file back) instead of deleting both copies.
7. Backups of replaced project files go to `<data root>/project-backups/<projectId>/<folder of the file>/`, never into the mod folder, so an upload built from the mod folder cannot contain them (`backups_of_an_apply_never_land_in_the_mod_folder`, `an_accepted_write_leaves_no_temporary_or_backup_file_in_the_project`). Two folders whose names differ only by `/` against `__` share a backup folder; retention then treats their same-named files as one series (low, accepted).
8. A scaffold (`project_create`) writes `About/About.xml` last, so a scaffold that stopped is not taken for a mod, and creating the same scaffold again completes it: files that already hold exactly the planned text are kept while `About/About.xml` is missing, and a folder that has the About file or any file with other content refuses the call.

### 19.3 Span edits and `LoadFolders.xml`

`SpanEditor` splices bytes and re-indexes the result before committing, so an edit that would break the structure returns an error and leaves the text unchanged. The tests run twelve document styles (CRLF, mixed line endings, BOM, tabs, comments that contain markup, single quoted attributes, `>` and `/>` inside attribute values, DOCTYPE with an internal subset, CDATA and processing instructions, one line files, Unicode, blank lines and trailing spaces) and assert that one splice changes and every other byte stays, that adding then removing an attribute is a byte for byte no-op, and that second roots, trailing text, truncated and empty documents are refused by every `LoadFolders` helper. A document nested 20000 deep and a 2 MB file with 100000 entries are handled within seconds (the editor is iterative; each edit re-indexes the whole text, so a plan with many edits to one huge file costs one pass per edit).

A regenerated Combat Extended patch section used to run from its banner comment to the next comment or the end of the file, so an operation the person wrote by hand right after it, without a comment, was deleted. The section now ends at the first element that does not mention the item name (every generated operation does); the hand written operation stays. A hand written operation placed between generated operations of one section is not detected (the later generated operations would then be left as stale copies); the plan diff shows the change before anything is written.

### 19.4 Document store and settings

1. Truncation at every byte, empty files, NUL and binary noise, deep nesting (200000 levels) and a wrong or newer envelope never panic: unreadable documents are moved to `.quarantine/` with their bytes, newer documents are read only and never moved, rewritten or deleted.
2. A file larger than `MAX_DOCUMENT_BYTES` (64 MiB) is judged by its length and quarantined without being read; a value that would serialise larger is refused on write. Settings files are limited to `MAX_SETTINGS_BYTES` (16 MiB) and read as strict UTF-8: a file in another encoding blocks saving and is never rewritten with replacement characters.
3. A put over an unreadable document first moves the old bytes to the quarantine; when the quarantine is not writable the put is refused and the bytes stay.
4. Ids are `[a-z0-9._-]` only, at most 128 bytes, so case tricks, Unicode lookalikes, separators, device names and the reserved name `index` never reach the file system, and no two ids collide on a case insensitive volume.
5. Two instances over one folder (the single writer assumption broken): each document is always whole (atomic rename), the index is rebuilt when it disagrees with the folder, the last write to a document wins, and the settings store of one instance can overwrite another instance's change with its own value (documented, tested as "the last save wins"). `.lock` is informational and is not checked.
6. A collection of 100000 documents: ids list in 0.13 s, the first summaries in 2.2 s, summaries from the index file after a reopen in 0.7 s, a full scan in 1.2 s (debug build, no fsync); the test is ignored by default. With a loaded index every put used to rewrite the whole index file (540 ms per put at this size); above 1000 documents the file is now written by the next listing and a crash leaves a stale file that the next listing detects.

### 19.5 Windows and macOS

Checked by `cargo check --all-targets --target x86_64-pc-windows-gnu` for `rimstudio-io`, `rimstudio-xml` and `rimstudio-toolkit`; they compile. Verified by reading: path comparison is per component through `camino` (so a `\\?\` prefix is a prefix component and `dunce` removes it from canonical paths); deny lists fold letter case on every platform (`FOLD_CASE_PATHS`, so no `cfg` is needed outside the platform crate), allow lists compare exactly, which can only refuse more. Not verifiable here: rename over an open file, junction handling in `symlink_metadata`, NTFS alternate streams on disk, 8.3 short names, macOS Unicode normalisation (a name that differs from an existing one only in normal form is not caught by `case_twin`), reserved names with trailing spaces on the real file system, and the read only attribute on Windows directories.

### 19.6 Defects found

| Id | Severity | Defect | State |
|---|---|---|---|
| W-01 | high | A file changed by another program between planning and applying was overwritten (a backup existed, the edit was lost from the file) | fixed: `Expect::Previous` in preflight and at write time |
| W-02 | high | A hand written operation right after a generated Combat Extended section was deleted by the next apply | fixed in `shared/merge.rs` (section ends at the first element without the item name) |
| W-03 | high | A put over an unreadable collection document replaced it even when the quarantine move failed, destroying the bytes | fixed: the put is refused |
| W-04 | medium | A target file that is a link inside the project was replaced by a plain file; a project folder that is a link was accepted | fixed: both refused |
| W-05 | medium | Windows device names with extensions, trailing dots and spaces, 255 byte segments and wildcard characters were not refused by the toolkit writer | fixed: `sanitize_name` per segment; the superscript digit device names and `CONIN$` and `CONOUT$` were also missing from the guard |
| W-06 | medium | Case twins were created on case sensitive disks and a case only difference was written through on case insensitive disks | fixed: refused with the existing spelling named |
| W-07 | medium | A write error after some files were written lost the report of what had been written | fixed: partial report with `designer.apply-write-failed` |
| W-08 | medium | A read only file was replaced on Unix and failed late on Windows (after earlier files were written) | fixed: refused before the first write |
| W-09 | medium | Settings read with `from_utf8_lossy`: a Latin-1 or similarly encoded file was loaded with replacement characters and written back that way | fixed: strict UTF-8, saving blocked |
| W-10 | medium | A crash inside the rename-aside fallback left the target missing and `cleanup_stale_temps` then deleted both remaining copies | fixed: roll forward or restore |
| W-11 | medium | Every put or delete with a loaded index rewrote the whole index file (O(n) per write) | fixed above 1000 documents |
| W-12 | medium | No size limit on documents and settings files | fixed: 64 MiB and 16 MiB |
| W-13 | low | A file that cannot be read as UTF-8 was reported as `designer.path-refused` | fixed: `designer.merge-failed` |
| W-14 | low | Two plan paths that differ by case or are folders of each other were not refused | fixed: whole plan refused |
| W-15 | low | `project_create` accepted a folder name that is not valid on Windows, a scaffold that stopped half way could not be completed | fixed |
| W-16 | low | `to_safe_component` returned names such as `con .txt_` that are still device names | fixed: reserved names get a leading `_` |
| W-17 | low | A project inside a Steam library was refused only when the reference set listed it | fixed: any `steamapps` component refuses |

### 19.7 Residual risks

1. No handle based file operations: the checks narrow the window between validation and use to three look-ups but a link planted by another process inside that window is still followed. A link swapped in for a parent folder can only redirect a write to a folder the person can already write, and the target file is never a link at the moment of the final check. Handle based calls belong to `rimstudio-platform` and are not built.
2. `atomic-write-file` is built without `unnamed-tmpfile`, so a temporary file `.<name>.<six letters>` exists in the mod folder during a write; a hard crash (power loss, kill) leaves it there and a Workshop upload of the mod folder would include it. The publish staging rules (section 9.5) must exclude dot files that match `.<name>.<six alphanumeric characters>`; this is an open item for the publisher.
3. The index of a collection can serve a stale summary when another process rewrites a document to the same byte length (the index is validated by ids and sizes only). Single writer assumption.
4. The project environment does not install the optional `GameWriteFence`; project writes rely on the protected folder list and the `steamapps` check. Without a loaded game (no reference set) the list holds only the explicitly protected folders.
5. `case_twin` compares with Unicode lower casing; a name that differs only in Unicode normal form is not caught on macOS.
6. A very large `LoadFolders.xml` or def file costs one full re-index per edit.

## 20. Layout fixes: moves, edits and the undo journal

`project_layout_fix_apply` is the one command that moves existing files of a mod ([mod layout](../features/mod-layout.md) section 14, [ADR 0046](../adr/0046-layout-fixes-with-an-undo-journal.md)). Where each threat is stopped:

| Threat | Stopped by |
|---|---|
| A plan reviewed earlier no longer matches the project (a file edited by another program) | The apply builds the plan again and refuses a different plan id (`designer.plan-stale`); a hash taken just before each move must still match |
| A destination outside the mod (`..`, an absolute path, a link in the path, a case twin, a device name, a protected folder) | Every source and destination goes through the guarded writer and the `RootGuard` of `rimstudio-io::rename::move_path`, before the move and again after the parent folders exist |
| A move overwrites a file | `move_path` never overwrites: a hard link first (it fails when the name exists), the existence check otherwise; a conflict is refused unless the caller accepts a free numbered name |
| A link is moved or followed | A link is refused as a source, never followed in a path, and a folder that holds a link is refused by the copy fallback; the scan lists links as empty files and plans nothing for them |
| A move across volumes leaves half a copy | Copy, byte for byte verification, then removal; a failed copy removes the partial destination and leaves the source |
| A crash between two items | The journal is written to the data root before the first change and updated before each edit and after each item; an undo infers the state of an interrupted item from the disk |
| A tampered or forged journal | A checksum against damage; the journal is also checked against the project root, every path (same rules as the apply), the recorded hashes against the disk, and only `LoadFolders.xml` can be restored from it, so a forged journal can move back only content that matches what it records, inside the project |
| An undo overwrites work done since | The undo checks every moved path, the original places and `LoadFolders.xml` before it changes anything and refuses, naming the path |
| Loss of `LoadFolders.xml` | The previous text is backed up in the data root before it is replaced and is also kept in the journal |

Residual: the journal is not secret, so an attacker who can write the data folder can forge a journal that passes the checksum; the checks above bound what such a journal can do. The window between a path check and the rename is the one of section 19.7 item 1.

## 21. Editing the basics of a mod

The commands of [mod layout](../features/mod-layout.md) section 15 change `About/About.xml`, `LoadFolders.xml` and `About/Preview.png`, create version folders and remove the preview image. They add no new way to write: every path goes through the guarded writer of section 19 (safe relative path, inside the root, no link, no case twin, outside the protected folders, the optional fence).

| Threat | Stopped by |
|---|---|
| An edit made against an old text overwrites a change made since (another editor, the game's uploader) | The form passes the SHA-256 of the file it read; a different file is refused (`project.file-stale`), and the write compares the old bytes again right before the bytes go down (`Expect::Previous`) |
| A hostile About or LoadFolders file (not XML, UTF-16, a document type declaration with entities, 2 MiB of text, nesting) | Read through `read_limited` (1 MiB); a file that is not UTF-8, not well formed or larger is shown and never rewritten (`project.file-not-editable`); the editor splices bytes and never expands an entity, and re-indexes the result before committing |
| A form value breaks the document (a line break in a name, markup in a value, a control character) | Single line fields refuse line breaks and control characters; every value is written escaped by the span editor; element names come from enums, never from the request |
| An edit loses the author's work | Byte span edits keep comments, order, unknown elements, line endings and the byte order mark; the replaced file is copied to `<data root>/project-backups/<project id>/` before the write; a change of no byte writes nothing |
| The preview source is a link, a folder, a huge file or not an image | The source must be an absolute regular file, never a link, at most 8 MiB, a complete PNG header with at most 4096 pixels a side; the copy is hash checked and read back; a different file at the target is backed up first |
| Removing the preview deletes something else | `GuardedWriter::remove_file` refuses a folder, a link and a read only file, checks the path again after the backup, and removes nothing without a verified backup; `rimstudio_io::remove::remove_regular_file` is the only removal and is not recursive |
| A version folder name escapes the mod | The version must be `major.minor` numbers; the folder is made through `make_dir` of the guarded writer; `LoadFolders.xml` is written last |
| A scan from the form leaks or stalls | None starts: the library scan is an input of the findings, never run by these commands |
