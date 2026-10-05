# RimStudio error handling and logging

This document defines how RimStudio reports problems: typed errors per crate, the IPC error envelope, the Diagnostic model for problems in user content, the policy for user-facing messages, the tracing setup with rolling JSON lines, redaction, the in-app log viewer feed, and panic and crash handling with a crash marker and no telemetry. It implements decisions D-046 and D-060 of the [decision register](decision-register.md) and the invariants I-10, I-12, I-13 and I-16 of the [architecture overview](overview.md). Evidence: the [Rust crate research](../research/rust-crate-research.md) section 11.1, the [RimCrow analysis](../research/rimcrow-analysis.md), and the [def engine semantics](../research/def-engine-semantics.md) Implications 3. The test side is in the [testing strategy](testing-strategy.md).

Status: draft | Last updated: 2026-10-04

## 1. Two kinds of problem

RimStudio separates problems in the user's content from failures of the operation itself. The distinction is the most important rule in this document.

| | Diagnostic | Error (`Result::Err`) |
|---|---|---|
| Cause | The user's mods, files or data are damaged, missing, conflicting or suspicious | The caller used the API wrongly, the environment refused, or the operation cannot continue |
| Examples | A broken `About.xml`, a patch that matches nothing, a missing dependency, an unreadable dataset entry | A path outside the registered roots, a write refused by the fence, a full disk, a cancelled job, an unknown command |
| Effect on the operation | None: the operation completes and returns a result with diagnostics beside it | The operation stops and returns no result |
| Representation | `Diagnostic { code, severity, mod, file, message }` collected in a `DiagnosticSink` | `<Crate>Error` with a stable `code()`, converted to the IPC envelope at the edge |
| Shown to the user as | Rows in the problems view, badges on mods, lint markers | A toast, a dialog or an inline message on the action that failed |

Invariant I-10 (diagnostics-not-errors) states the rule: problems in user content never abort an operation. The game itself skips a damaged file and logs an error ([mod format note](../research/rimworld-mod-format-and-corpus.md)), and the manager must show a library with 700 mods even if 30 of them are broken. A vanilla load must produce zero diagnostics, and a test asserts it.

A third category is the **outcome that is not a failure**, for example "the game is running, so ModsConfig was not written" or "no install found". These are normal results with a typed status (a `Report` or `Outcome` type), not errors, because the UI has a designed path for them.

## 2. Error types per layer

Every library crate defines exactly one `thiserror` enum named `<Crate>Error` (for example `XmlError`, `IoError`, `LibraryError`) with a method `code(&self) -> &'static str`. Binaries, xtask and tests may use `anyhow` (1.0.104); libraries never do (D-046, crate research section 11.1). `miette`, `snafu` and `eyre` are rejected.

| Layer | Error enum | What it carries | Notes |
|---|---|---|---|
| L0 `rimstudio-core` | `CoreError` | Invalid id, bad version string, invalid node tree operation, port contract violation | Tiny; no IO variants because core has no IO |
| L1 `rimstudio-xml` | `XmlError` | Unreadable input when the caller demanded strict mode, render failure, byte-span out of range | Game and Tolerant parse modes turn damage into diagnostics (`xml.*`), not errors |
| L1 `rimstudio-io` | `IoError` | Path outside a root, fence refusal, atomic write step failed (with the step), schema newer than the app (read-only), migration failure, lock held | Wraps `std::io::Error` with the operation and a redacted path |
| L1 `rimstudio-platform` | `PlatformError` | Link creation denied (privilege or sandbox), credential store unavailable, process spawn failed | Each variant says whether a fallback exists |
| L2 engines | `XpathError`, `DefsError`, `RulesError`, `SortError`, `ValidateError`, `DesignError` | API misuse only: unsupported expression kind requested explicitly, invalid input shape | Content failures inside an engine are diagnostics; an unparseable XPath in a user patch is `xpath.parse-error` diagnostic, while a call to `parse` by the patch tester returns `XpathError` with span |
| L2 services | `SteamError`, `LibraryError`, `WorkspaceError`, `DatasetError` | Missing required argument, root not registered, network refusal after retries, cap exceeded, cancelled | `DatasetError` distinguishes offline, status, cap, parse; offline is an expected state with a status, see section 5 |
| L3 features | `ManagerError`, `ToolkitError`, `PublishError` | Use case preconditions (nothing to undo, profile not found, project not open), write plan refused, helper protocol failure | Wrap lower errors with `#[from]` and keep their codes reachable |
| Contract | `ApiError` | The envelope (section 3) | Defined in `rimstudio-ipc-types`; `rimstudio-app` converts each crate error to it |
| L4 | `AppError` | Unknown command, bad request shape, job registry failures | Only `rimstudio-app` knows every crate error |

Rules:

1. **Stable codes.** A code is a lowercase `<area>.<kebab-name>` string (same grammar as diagnostic codes, section 4). Codes are API: renaming one is a breaking change that needs a note in the release notes and an update of the locale key.
2. **No stringly errors.** No `Err(String)` and no `Box<dyn Error>` in a library signature. `#[error("...")]` text is for developers and logs; users see translated text chosen by code.
3. **Context.** An error that crosses a layer adds what was being done and to what (operation name, redacted path) through a field, not by formatting into a message. `#[source]` keeps the chain for logs.
4. **No panics for expected failures.** `unwrap` and `expect` are warned in library crates by the workspace lints, and allowed only for proven invariants with a comment. Index out of range in a hot path uses checked access and returns a typed error or a diagnostic.
5. **Cancellation is an error code**, `job.cancelled`, treated as a normal end by callers and never logged above info.
6. **Mapping tests.** Each crate has a test that lists every variant with its code, and a test in `rimstudio-app` checks that every code appears in `en.json` (or is explicitly marked developer only).

## 3. The IPC error envelope

Every command that fails returns the same JSON object (D-046, defined in `rimstudio-ipc-types`):

```json
{
  "code": "io.path-outside-roots",
  "message": "That folder is not one of your registered mod folders.",
  "errorId": "e-7f3a9c21",
  "details": { "operation": "library_scan", "path": "<root:custom-1>/Mods/Alpha" }
}
```

| Field | Meaning | Rules |
|---|---|---|
| `code` | Stable machine code | Drives the UI: which message key, which recovery action, whether to retry |
| `message` | A developer-quality English fallback sentence | Always present so the CLI and logs are readable; the webview prefers the translated string for `code` and uses `message` only if the key is missing |
| `errorId` | Random id minted when the error is created | The same id appears in the log line that recorded the error, so a user can say "error e-7f3a9c21" and the maintainer greps the log |
| `details` | Redacted structured context | Only fields listed per code; paths are redacted (section 7); never raw `std::io::Error` text, never secrets |

Conversion happens once, at the edge of `rimstudio-app::dispatch`: the handler returns `Result<Resp, ApiError>`, handlers convert their crate error with `From`, and the shell serialises the `ApiError` as the rejected promise value. The error is logged at that point (once, with the `errorId`), so lower layers do not log errors they return (section 6, rule 1). The CLI prints the envelope as JSON with `--json` and as a sentence plus the id otherwise, and maps the outcome to an exit code (the same scheme as the crate catalog): 0 success, 1 completed with findings (hard violations, or error-severity diagnostics when `--strict` is set), 2 usage error, I/O error or any failure with an envelope, 130 cancelled.

Streams and jobs fail the same way: the terminal event of a job carries an envelope, and the sidecar protocol's single terminal event (D-061) maps its error to the same code space under `publish.*` and `steam.*`.

## 4. The Diagnostic model

```rust
struct Diagnostic {
    code: DiagCode,        // "<area>.<kebab-name>"
    severity: Severity,    // error, warning, info, hint
    mod_idx: Option<ModIdx>,   // session handle, serialised as ModId in DTOs
    file: Option<FileId>,      // session handle, serialised as a relative path
    message: String,       // English fallback; the UI translates by code
    // optional: span, related, args for the message template
}
```

The shape follows the spine (`Diagnostic {code, severity, mod, file, message}`) with optional span (line and column, or byte range for editor markers) and `args`, a small map of values used by the translated message template so the UI can render "Missing dependency {packageId}" without parsing prose.

### 4.1 Codes

| Area | Producers | Examples |
|---|---|---|
| `xml` | `rimstudio-xml` | `xml.parse-error`, `xml.bad-encoding`, `xml.bom-stripped`, `xml.dtd-rejected` |
| `defs` | `rimstudio-defs` | `defs.patch-failed`, `defs.unknown-type`, `defs.inherit-cycle`, `defs.duplicate-in-mod` |
| `xpath` | `rimstudio-xpath` | `xpath.parse-error`, `xpath.unsupported`, `xpath.matches-nothing` |
| `scan` | `rimstudio-library` | `scan.about-missing`, `scan.unreadable-folder`, `scan.case-mismatch` |
| `rules` | `rimstudio-rules`, datasets | `rules.unknown-key-kept`, `rules.conflict`, `rules.suppressed` |
| `sort` | `rimstudio-sort` | `sort.cycle`, `sort.unsatisfiable-constraint` |
| `list` | `rimstudio-validate` | `list.missing-dependency`, `list.version-mismatch`, `list.incompatible-pair`, `list.duplicate-package-id` |
| `author` | `rimstudio-validate` | Authoring lints for the toolkit |
| `deploy` | `rimstudio-library::deploy` | `deploy.stale-link`, `deploy.would-deactivate`, `deploy.offline-folder` |
| `steam` | `rimstudio-steam` | `steam.manifest-unreadable`, `steam.library-offline`, `steam.dangling-link` |
| `dataset` | `rimstudio-datasets` | `dataset.quarantined`, `dataset.stale`, `dataset.partial` |
| `publish` | `rimstudio-publish`, preflight | `publish.preview-missing`, `publish.title-too-long` |
| `ce` | `rimstudio-design::ce` | `ce.missing`, `ce.lint-cep001` to `ce.lint-cep022` (CE lint ids CEP001 to CEP022 stay stable and are kept inside the code) |
| `design` | `rimstudio-design` | `design.ce-missing`, `design.no-baseline`, `design.outlier` |
| `log` | Player.log classifier | `log.harmony-patch-failed`, `log.missing-def-reference`, `log.exception-in-mod` |

Rules:

1. The def engine prototype's snake_case codes (`patch_failed`, `inherit_cycle`, and the rest of the list in def engine Implications 3) are mapped by rule to `defs.<kebab>` at the API edge (underscores become hyphens, `patch_*` and `inherit_*` families keep their prefix words). The vectors keep the original codes internally, so vector files stay unchanged.
2. Every code lives in one registry in `rimstudio-validate` (`code_registry`) with: code, default severity, area, a short description, and the locale key. A test checks that producers only emit registered codes and that every registered code has a locale key.
3. A producer chooses severity from the registry default and may only lower it through a user setting (for example the user suppresses `list.version-mismatch` for a mod, recorded as a suppression with provenance in the rules layers, never deleted from the engine output).
4. Messages are English fallbacks; the UI translates by `code` plus `args` (D-056).

### 4.2 Collection, counting and caps

1. Producers write into a `DiagnosticSink` trait object passed in (I-16, no globals). The default sink counts per code and keeps the first 100 samples per code (def engine Implications 3), so a library with 30,000 repeated warnings costs bounded memory and still reports the true totals.
2. Output order is deterministic (I-12): sorted by code, then mod order, then file path ordinal, then position, so goldens are stable at 1 and 8 threads. Sinks used by parallel workers are merged after the work, never interleaved.
3. A result type carries `diagnostics: DiagnosticSummary { counts: BTreeMap<code, u64>, samples: Vec<Diagnostic>, truncated: bool }`. Mod list rows carry only counts per severity; the full list is a paged query (paging is allowed for the def explorer, search and logs, D-044).
4. Provenance: the diagnostic names the mod and file; for def and patch problems the engine also attaches the origin ids of the three-level provenance model (D-017), so "why" links in the UI resolve.

### 4.3 When to use which

1. If the user could fix it by changing their files or list: Diagnostic.
2. If the app could not do what the user asked and no partial result makes sense: Error.
3. If the app did something other than asked because of a precondition the user can understand: an outcome status.
4. If a Diagnostic would make an operation unsafe (for example `deploy.would-deactivate` before launch), the use case reads the diagnostics and returns a typed blocking outcome. The decision is made by the use case, not by turning the diagnostic into an error.

## 5. User-facing messages policy

1. **Say what happened, what it means for the user's mods, and what to do next**, in that order, in plain language. Example: "RimStudio could not read the folder 'Mods E'. It may be on a drive that is not connected. Reconnect the drive and press Rescan."
2. **No jargon in the default view.** Error codes, error ids and raw paths sit behind a "Details" expander and a "Copy details" button that copies the redacted envelope and the last log lines.
3. **No blame, no alarm.** Missing community data is information, not an error banner: offline start shows "Rules last updated 3 days ago" with the fetch status in the datasets panel. Dataset failures are `dataset.*` diagnostics and statuses, never modal dialogs.
4. **Severity maps to presentation.** Error: blocks the action or marks the item; warning: badge and problems row; info: problems row only; hint: editor margin only. Toasts are for the result of the user's own action, never for background work, and do not stack more than three.
5. **Every message is a locale key.** `errors.<code>` and `diagnostics.<code>` keys in `en.json` with ICU arguments; a completeness script fails the build when a registered code lacks a key. Game text is never translated (D-056).
6. **Recovery actions are typed.** A code can declare a recovery (`retry`, `open-settings:<section>`, `rescan`, `open-log`) in the code registry; the UI renders the button, no component hardcodes it.
7. **Destructive or game-folder actions explain before, not after.** The pre-launch check names the mods that the game would silently deactivate (D-040) and offers the fix, because the game gives no message.
8. **The CLI uses the same messages**, rendered in English, so documentation and support show identical text.

## 6. Logging with tracing

Decision D-060: `tracing` 0.1.44 with `tracing-subscriber` 0.3.23 (`fmt`, `env-filter`, `json`) and `tracing-appender` 0.2.5 for a rolling non-blocking file writer; dependencies that use the `log` crate are bridged through `tracing-log`. Rejected: `tauri-plugin-log` (a second stack, and the webview gets no `log` permission, D-047), `fern` and `flexi_logger`, and `sentry` or any telemetry. The owner's Parallax project writes a tagged activity log with a start-up report (the logging document of the owner's Parallax project); the same ideas carry over: one line per notable action, a component tag, timed steps, and a system report at start.

### 6.1 Initialisation

`rimstudio-app::logging::init(&DataRoots, &LogConfig) -> LogGuard` is the only place that installs a global subscriber (the single permitted exception to I-16, recorded in the clippy policy). The CLI calls it with a stderr layer and no file by default (`--log-file` enables the file); the shell calls it with the file layer and, in debug builds, a console layer. Libraries only emit events; they never configure output. The returned guard flushes the non-blocking writer on drop and in the panic hook.

### 6.2 Files

| Property | Value |
|---|---|
| Location | `logs` root of `DataRoots` (portable mode: `./data/logs`) |
| Names | `rimstudio.<yyyy-mm-dd>.jsonl`, lowercase, daily rotation |
| Retention | Last 14 files and at most 50 MB in total, oldest deleted at start |
| Format | One JSON object per line: `ts` (RFC 3339 UTC with milliseconds), `level`, `target`, `msg`, `fields`, `span` (names only), `errorId` when present, `session` (random id per run) |
| Writer | Non-blocking with a bounded queue; when the queue is full, debug and trace events are dropped first and a `log.dropped` counter line is written; logging never blocks or fails the caller |
| Encoding | UTF-8, LF |

The format is JSON, so R10 is satisfied and the file is parseable by the viewer (section 8) and by tools.

### 6.3 Levels and targets

Targets default to the crate's module path, which maps one to one to crates (`rimstudio_library::scan`). Default filter: `info` for `rimstudio_*`, `warn` for everything else. Override with `RIMSTUDIO_LOG` (same syntax as `RUST_LOG`) or the diagnostics page.

| Level | Use |
|---|---|
| error | An operation failed and an envelope was returned; a panic; a dropped invariant. One line per failure, with the `errorId` |
| warn | A degraded but continuing state: fallback taken (poll watcher after ENOSPC, FAT hashing, copy instead of link), dataset quarantined, offline folder |
| info | One line per notable action with duration: scan finished (mods, files, diagnostics counts, ms), sort applied, dataset updated, deploy plan applied, launch, publish step. The start-up report (section 6.4) |
| debug | Decisions and counts inside an operation: cache hit ratio, worker count, detection candidates considered. Off by default |
| trace | Per-file detail. Never enabled by default and not compiled out; bounded by the writer queue |

Rules:

1. **Log once, at the edge.** The layer that converts an error to an envelope logs it. A lower layer that returns an error does not also log it at error level (it may log at debug with context). This prevents duplicate lines and keeps the `errorId` unique per failure.
2. **Spans for jobs.** Every job runs inside a span `job{id, kind}`; every command inside `cmd{name}`. Scan, sort and load steps are `info_span` with elapsed time recorded on close, giving the "(1234 ms)" style of the owner's activity log.
3. **Diagnostics are not logged line by line.** Counts per code are logged once per operation at info; samples appear at debug. A library with thousands of warnings does not flood the file.
4. **Structured fields, not formatted prose.** `info!(mods = 691, files = 306394, ms = 94, "scan finished")`.
5. **No `println!` or `eprintln!`** outside the CLI (workspace lint `print_stdout`, `print_stderr`).
6. **Frontend events** go through a command `log_event(level, component, message)` (the webview has no `log` plugin), rate limited and length capped, tagged `target = "webview"`.

### 6.4 Start-up report

The first lines of each session (info, target `rimstudio_app::boot`) record: build (version, commit, Rust, Tauri, target triple), OS and version, session type (Wayland or X11 on Linux), webview engine and version, CPU threads, memory, data roots and free space (redacted), install detection summary (kind and count only), portable mode, dataset status, enabled cargo features including `e2e` and `diagnostics` (which must be absent in releases), settings summary. This is the first thing asked for in a bug report and is what the diagnostics page shows (D-060, "a diagnostics page shows engine and session facts").

## 7. Redaction

Logs and envelopes leave the machine only when a user copies them into a bug report, but they must be safe to paste.

| Data | Treatment |
|---|---|
| User home and name | Replaced by `~` or `<home>` in every path; the user name never appears |
| Paths under registered roots | Written as `<root:name>/relative` (for example `<root:steam-workshop>/1234567890/About`); other paths keep only the last two components |
| Steam ids (17-digit ids) and account names | Replaced by `<steamid>` and `<account>` |
| Tokens, API keys, passwords, cookies, `Authorization` headers | Never logged; any field named like a secret is replaced by `<redacted>` by the formatting layer |
| URLs | Query strings dropped; dataset URLs are logged without credentials |
| Mod content, def values, user notes | Not logged at info or above; ids and names of mods are allowed (public) |
| Workshop ids | Allowed (public) |
| Player.log text | Never forwarded into RimStudio's log; the classifier works on the file and the viewer reads it on demand |
| `std::io::Error` text | Logged at debug only after path redaction, because OS messages embed paths |

Implementation: a `Redactor` in `rimstudio-core` (pure, testable) built from the `DataRoots`, home directory and registered roots, used by (a) the envelope builder for `details`, (b) a `tracing` field formatter layer, and (c) the diagnostics bundle export. Tests feed known secrets, a fake home, Steam ids and Windows and POSIX paths with both separators, and assert none survive, including inside nested JSON `details`. A property test generates random path strings and asserts that the home string never appears in the output.

## 8. The in-app log viewer feed

The `logs` feature has two sources, both paged because logs are one of the three large sets that page (D-044).

1. **RimStudio's own log.** Command `applog_query(request)` reads the JSONL files from the logs root: filters by level, target prefix, session and text; cursor paging, newest first; a `stream` command `applog_follow` tails the current file through a channel at most 20 messages per second (the shared rate limit). The webview never opens the files itself (no `fs` plugin, D-047); the backend validates that the files are inside the logs root through `RootGuard`.
2. **The game's `Player.log`.** `rimstudio-validate` classifies lines into `log.*` diagnostics (known error patterns, the mod named in a stack trace, repeated lines folded with counts). The viewer shows the classification beside the raw text, with "which mod" links into the manager. This is analysis of the user's file at run time and nothing is stored beyond a cache of the classification keyed by file stat.

The diagnostics page offers "Copy diagnostics bundle": a JSON object with the start-up report, the last 200 log lines, the last 50 errors with ids, and settings with paths redacted. The bundle is produced locally; nothing is sent anywhere. Bundles go through the same Redactor.

## 9. Panics and crashes

1. **Panic hook.** `rimstudio-app::boot` installs a hook that logs a `panic` error line (message, location, thread, backtrace when `RUST_BACKTRACE` is set or in debug builds), flushes the log guard, writes the crash marker (below) and then lets the default behaviour continue. A panic in a job worker is caught with `catch_unwind` at the `JobRunner` boundary, converted to an envelope with code `job.panicked` and a fresh `errorId`, and the app keeps running. A panic in a command handler is caught the same way and becomes `app.internal-error`. The mod list and user work are never lost to a handler panic; data writes are atomic (D-026) so a panic at any moment leaves old or new files, never a mix.
2. **Crash marker.** At start the app writes `crash-marker.json` in the logs root `{schemaVersion, session, startedAt, version}` and removes it on clean shutdown. If the marker exists at the next start, the previous run ended abnormally (panic abort, kill, power loss). The app then (a) logs `previous session ended abnormally` with the old session id, (b) shows a non-modal notice offering to open the diagnostics page and copy a bundle, and (c) in the panic case finds `crash-<session>.json` beside it, written by the hook with panic message, location, redacted backtrace and the last 100 log lines. Crash files are kept at most five and 1 MB each.
3. **Native crashes.** A segmentation fault in the webview or a native library cannot be hooked by Rust code. The marker still detects it. Native minidumps (`minidumper` and similar) are not adopted; they imply upload infrastructure that conflicts with the no-telemetry rule (crate research section 11.1).
4. **Sidecar failures.** The helper's exit, a missing terminal event or the 120 second watchdog yields `publish.helper-failed` with the last redacted stderr line; the main app is unaffected (I-15).
5. **Out of memory and abort.** Not recoverable; the marker covers it. Scan and def work are bounded (caps per code, interned keys, D-069) to make it unlikely.
6. **Startup failures.** A corrupt or newer-schema settings file does not stop startup: the file is read-only if newer, quarantined with a backup if corrupt, defaults are used, and a `settings.recovered` notice with the backup path is shown (D-026).

## 10. No telemetry

RimStudio never sends usage data, crash reports or logs. There is no analytics call, no error reporting service and no remote logging. The only network traffic is what the user can see in the datasets panel (rule and database fetches, with the address and last fetch time listed), the update check (which the user can turn off, and which is hidden where a package manager owns updates, D-059), and Steam through the sidecar when the user publishes. The `xtask check-deps` ban list rejects `sentry`, analytics crates and minidump uploaders, and CSP `connect-src` allows only `ipc:`, so the webview cannot reach the network (D-047). Users who want help copy a diagnostics bundle by hand.

## 11. Tests for this document

| Behaviour | Test |
|---|---|
| Every error variant has a unique code, and every code has a locale key | Per crate table test plus an app level completeness test |
| Envelope shape and redaction | Golden JSON per error code family; redaction property test |
| Diagnostics are deterministic and capped | Golden at 1 and 8 threads; cap test with 10,000 repeats keeps 100 samples and the true count |
| Vanilla zero diagnostics | Ignored real-data test |
| Log once at the edge | A test subscriber counts error events for a failing command: exactly one with the `errorId` |
| Log file rotation and retention | `assert_fs` test with fake clock |
| Panic in a job | `JobRunner` test: panic becomes `job.panicked`, other jobs continue |
| Crash marker | Start, kill without cleanup, restart: marker detected, notice produced |
| No telemetry | Dependency ban and CSP string tests |

## 12. Open items

| Item | Status |
|---|---|
| Whether the optional `diagnostics` cargo feature adds tracing console output in developer builds only | Proposed yes; release builds are checked to lack it |
| Log retention numbers (14 files, 50 MB) | Proposed; revisit after S-10 measurements of real log volume |
| Message wording review by the owner for the first 30 most common codes | Owner |
