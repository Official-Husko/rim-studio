# RimStudio IPC and state architecture

This document fixes how the Rust backend and the Preact webview talk to each other and who owns which state. It covers the single command registry, naming, DTO and id conventions, the error envelope, the snapshot plus delta protocol, the jobs model, image serving, the webview security posture, the typed bindings pipeline, backend state ownership with its concurrency rules, and the first 40 commands. It implements requirements R1, R2, R5 and R10 (all IPC is JSON) and invariants I-01, I-09, I-13, I-14, I-16 and I-19 from the [architecture overview](overview.md). The frontend side of the same contract is in [frontend architecture](frontend-architecture.md).

Status: draft, DTO notes of the 0.1.0 backend added in section 4 and 7 | Last updated: 2026-10-05

## 1. Principles

1. One declaration per command. A command is written once in the registry of `rimstudio-app`; the Tauri wrapper, the CLI route, the permission entry and the TypeScript binding are generated from that row (D-005, I-09).
2. Handlers are plain functions. They take the explicit `AppContext` and a request DTO and return a response DTO or an `ApiError`. They know nothing about Tauri, so `rimstudio-cli` and the tests call exactly the code the webview calls (D-006, I-01).
3. JSON everywhere. The measured cost of 3000 slim records is about 8 ms over a command and 4 ms to parse, so no binary list format is justified; raw bytes are used only for opaque image and file data ([webview and IPC performance](../research/webview-and-ipc-performance.md) section 2.2).
4. The backend owns truth. Ordering, validation, conflicts and warnings are computed in Rust; the webview renders results and sends intents, which is the lesson of both reference managers ([RimSort UX inventory](../research/rimsort-feature-and-ux-inventory.md) pain points P1, P2, P9).
5. Anything longer than 1 ms of Rust work is a job with progress and cancellation (I-13).
6. Content problems are data (`Diagnostic`), never errors (I-10). Errors are for things that stopped an operation.
7. No ambient authority in the webview (I-14): no file, shell or network plugin, paths validated against registered roots.

## 2. The command registry

### 2.1 Declaration

The registry lives in `crates/rimstudio-app/src/registry.rs` as one macro table, `for_each_command!`, that accepts a callback macro. Each row has a kind, a name, the handler signature and the handler path:

```rust
// shape only; names are the contract, not the exact macro syntax
query  mods_get_detail (ctx, req: ModsGetDetailRequest) -> ModsGetDetailResponse = manager::api::mods_get_detail;
action list_toggle     (ctx, req: ListToggleRequest)    -> ListEditResponse      = manager::api::list_toggle;
stream mods_subscribe  (ctx, req: ModsSubscribeRequest, sink: Sink<ModsStreamMsg>) -> SubscriptionHandleDto = manager::api::mods_subscribe;
job    library_scan    (ctx, req: LibraryScanRequest, job: JobCtx<LibraryScanResult>) -> JobHandleDto = manager::api::library_scan;
```

The handler path of a row is resolved inside `rimstudio-app`, which may name feature crates; the generated item that the shell wrapper calls is `rimstudio_app::handlers::<command>`, so the shell never names a feature crate (D-006). The four kinds differ only in how the call is wired:

| Kind | Wiring | Rust work budget | Result delivery |
| --- | --- | --- | --- |
| `query` | read only, inline async command | under 1 ms inline, otherwise promoted to a job | return value |
| `action` | state changing, inline async command | under 1 ms inline, otherwise promoted to a job | return value with the new revision |
| `stream` | opens a long lived `Channel<T>` owned by the caller | producer coalesces to at most 20 messages per second | channel messages until closed |
| `job` | registers a `JobId`, returns a handle at once, runs on the job runner | unbounded, cancellable | channel of `JobEvent<R>` with exactly one terminal event |

A row that is used by the webview but has no CLI meaning (for example `app_list_tools`) still goes through the same table; the CLI generic `call <command> <json>` can reach every row, and named CLI subcommands wrap a subset (D-007).

### 2.2 What is generated from a row

| Artefact | Where | Check |
| --- | --- | --- |
| `#[tauri::command]` plus `#[specta::specta]` wrapper | `apps/desktop/src-tauri/src/commands.rs` (macro expansion, no hand written wrappers) | compile error if a row is missing a DTO |
| Handler adapter for `Channel<T>` | same file; adapts `tauri::ipc::Channel<T>` to the app's `Sink<T>` trait | job lifecycle tests run with a fake sink |
| CLI route | `rimstudio-cli` dispatch table via `rimstudio_app::dispatch(&ctx, name, json)` | CLI parity test (every row reachable through `call`) |
| Permission list | `build.rs` passes `AppManifest::commands(&[...])` built from the table, one `allow-<command>` permission per row | capability coverage test (section 8.3) |
| TypeScript bindings | `packages/ipc-types/src/bindings.ts` from `tauri-specta`, committed | drift check (section 9) |
| Tool table mirror | `ToolDescriptor` rows in `rimstudio-app::tools` against `apps/desktop/src/app/tools.ts` | `cargo xtask check-tools` (D-066) |

### 2.3 Adapter responsibilities (the shell)

The shell wrapper does four things and nothing else:

1. Pull `Arc<AppContext>` from Tauri managed state.
2. Convert the incoming `Channel<T>` into a `Sink<T>` whose `send` returns `Err` when the webview dropped the channel.
3. Map `ApiError` to the serialisable envelope of section 5 (the type already derives `Serialize`, so this is a pass through).
4. Run the handler on the right executor: `query` and `action` on the async runtime with `spawn_blocking` for anything that touches the disk; `job` on the job runner.

The shell stays under its 3000 line budget because it contains no business rule. A rule found in the shell is a defect.

### 2.4 Rules for handlers

1. Signature `pub fn name(ctx: &AppContext, req: Req) -> Result<Resp, ApiError>`; stream and job handlers add a sink or `JobCtx` parameter.
2. A handler never reads a global, an environment variable or the clock directly; it goes through `ctx` and the ports of `rimstudio-core` (I-16).
3. A handler never holds a lock while sending on a sink or awaiting.
4. A handler returns diagnostics in the response DTO when a content problem is the answer, and `Err` only when it could not answer.

## 3. Naming

| Thing | Rule | Example |
| --- | --- | --- |
| Command (Rust and wire) | `<area>_<verb>` snake_case, area singular or plural as the domain noun | `mods_get_detail`, `sources_add_folder`, `list_move` |
| Command (TypeScript) | camelCase of the same name, exported from `rimstudio-ipc-types` | `commands.modsGetDetail` |
| Reserved verb first name | exactly one: `cancel_job`, the infrastructure call every job screen uses | `cancel_job` |
| Request and response DTO | `<Command>Request`, `<Command>Response` in PascalCase; `Dto` suffix for shared nouns and on collision | `ListMoveRequest`, `ModRowDto` |
| Stream message union | `<Area>StreamMsg`, tagged by `kind` | `ModsStreamMsg` |
| Job events | `JobEvent<R>` generic over the result | `JobEvent<LibraryScanResult>` |
| Broadcast event (Tauri event) | `<area>:<noun>` lowercase, kebab inside the noun | `settings:changed`, `game:running-changed` |
| Channel | never named on the wire; it is a function argument, so the command name identifies it | `mods_subscribe(req, onMsg)` |
| Diagnostic and error code | `<area>.<kebab-name>` | `list.revision-conflict`, `io.path-outside-roots` |

The earlier research drafts used `list_mods_snapshot` and `subscribe_mods`; those map to `mods_snapshot` and `mods_subscribe` here so that the area always comes first and commands sort together in the bindings file. The full alias list is in [command catalog section 3](command-catalog.md#3-names-that-were-renamed-or-merged).

Areas in use: `app`, `log`, `applog`, `settings`, `detect`, `sources`, `library`, `mods`, `list`, `sort`, `diagnostics`, `profiles`, `datasets`, `rules`, `deploy`, `launch`, `defs`, `project`, `designer`, `publish`, `logs`. A new area needs a line in the registry module docs and an entry in `ToolDescriptor` when it belongs to a tool.

### 3.1 Broadcast events

Tauri events are only for low rate, whole-app facts that no single caller owns. High volume data uses channels.

| Event | Payload | Emitted when |
| --- | --- | --- |
| `settings:changed` | `{ rev, sections: string[] }` | any settings write, so other windows and panels refetch the named sections |
| `game:running-changed` | `{ state: "running" or "stopped" or "unknown" }` | process or Steam flag change (D-041); `unknown` inside sandboxes |
| `sources:changed` | `{ sourceIds: string[] }` | the file watcher saw a change in a registered root (D-028) |
| `app:second-instance` | `{ args: string[] }` | the single instance plugin forwarded a second launch |

The proposed event `game:list-changed` and the mapping of the dotted event names of the mod manager spec are in [command catalog section 5](command-catalog.md#5-broadcast-events).

OS file and folder drops are not app events: the platform adapter listens to the Tauri drag-drop event and hands the paths to the feature, which sends them to `sources_probe_folder`.

## 4. DTO and id conventions

DTOs live only in `rimstudio-ipc-types` (D-043). The rules are mechanical so they can be linted by the bindings test.

1. Field names are camelCase on the wire (`#[serde(rename_all = "camelCase")]`). Enums are kebab-case strings (`"workshop"`, `"load-after"`). Tagged unions use `kind` as the tag.
2. All 64-bit integers cross as strings: Steam and Workshop ids, file sizes above 2^53 are never needed, timestamps are RFC 3339 strings or `number` of milliseconds below 2^53, labelled by field suffix `Ms`. `WorkshopId` and `SteamId` are strings end to end, including the About `PublishedFileId` (research: [workshop publishing](../research/workshop-publishing-research.md) implication 4).
3. Id vocabulary:

| Id | Type on wire | Stable across restarts | Used for |
| --- | --- | --- | --- |
| `ModId` | string, `w<workshopId>` or `<source>:<packageId>` | yes | persisted lists, profiles, URLs, `rsimg` ids |
| `PackageId` | lowercased string | yes | rules, dependencies |
| `ModIdx` | number (u32) | no, session only | hot path rows, sort results, order arrays, deltas |
| `SourceId` | string | yes | install, workshop and custom folders |
| `ProjectId`, `ProfileId` | string | yes | project and profile files |
| `JobId` | string minted by the caller | no | job registry |
| `SessionId` | string | no | library, def and workspace sessions |

   `ModIdx` is never persisted and never shown to the user; every snapshot carries `sessionId` so a stale index from before a rescan cannot be applied to a new library.
4. Optionals: `Option<T>` serialises as an absent field (`skip_serializing_if`) and arrives as `field?: T`. `null` is not used. A value that may be unknown has a named enum variant (`"unknown"`), not an optional, so the UI cannot confuse unknown with empty.
5. Revisions are `u32` counters that restart at 1 with each session; they stay far below 2^53 and are typed `number`. This is the single documented exception to the string rule for counters.
6. List rows carry list columns only: id, name, authors, flags, supported version bitmask, load index, diagnostic counts, size, updated time. Descriptions, dependency lists, previews and file listings come from `mods_get_detail` (budget rule: no descriptions in list payloads, section 5 of the performance note).
7. Sizes: a snapshot under 2 MB, an individual stream message either under 8 KiB or rare and large (above 8 KiB the Tauri channel switches to a slower path, measured 5.7 times slower for 9000 byte messages).
8. Paths: a DTO field typed as a filesystem path is allowed only in the short list of section 8.4; everywhere else the request names an id.
9. Evolution: the webview and backend ship together, so there is no wire version negotiation. Changes are additive within a release line (new optional fields, new variants only at the end of a union); a removal or rename is a normal change that regenerates bindings and breaks the TypeScript build. `app_get_info` returns `contractHash`, a hash of the bindings file, so a development webview that is out of date shows a banner. Persisted files carry `schemaVersion` and migrate forward only; the helper protocol carries `v` (D-061).
10. Every DTO has a JSON round trip test and, for row types, a size test (3000 rows under 1 MB).
11. As built in `rimstudio-ipc-types` (2026-10-05): serde only, no `specta` derives yet; `ApiError.details` is an optional JSON object omitted when absent; `DesignSpecDto` is JSON identical to the engine `DesignSpec` (the `ce` block absent by default); a DTO enum value must be kebab-case, so the detection `How` value `libraryfolders.vdf` is `library-folders-vdf` and the conversions map it; golden JSON of the DTOs is regenerated with `RIMSTUDIO_UPDATE_GOLDEN=1` and reviewed. Known issue: `serde_json` has no `float_roundtrip` feature in the workspace, so an `f64` that the webview echoes back (a `DraftDto` value of 17 digits) can differ by one unit in the last place (open design issue OD-17). The command handlers built for 0.1.0 are listed in the [command catalog](command-catalog.md) section 4.12.

## 5. The error envelope

Every rejected call carries the same JSON object (D-046):

```json
{ "code": "list.revision-conflict", "message": "The list changed since revision 41.", "errorId": "e-8f3a21c9", "details": { "expectedRev": 41, "currentRev": 44 } }
```

| Field | Meaning |
| --- | --- |
| `code` | stable `<area>.<kebab-name>`, produced by the crate error's `code()`; the UI maps it to the catalog key `error.<code>` and never matches on `message` |
| `message` | English text for logs and the diagnostics page; not shown directly when a catalog entry exists |
| `errorId` | random short id minted when the error is created and written to the log with the full source chain and a redacted context, so a user can quote it |
| `details` | optional JSON object of structured parameters, used for message interpolation and for "fix it" buttons (for example the offending path under `details.path`) |

Rules:

1. One `thiserror` enum per crate (`ManagerError`, `LibraryError`, and so on), each with a `code()`; the handler converts with `From` into `ApiError`. Source chains stay in the log, not on the wire.
2. A failed `Channel` send is not an error to report; it is the cancellation signal.
3. A job that fails sends `JobEvent::Failed { error }` (same envelope) as its terminal event; the `JobHandleDto` returned at start never carries a late failure.
4. Content problems (`xml.unclosed-tag`, `scan.missing-about`, `sort.cycle`) are `Diagnostic { code, severity, mod, file, message }` items in responses and deltas. The registry of codes lives in `rimstudio-validate` and is mirrored as catalog keys by a check.
5. Path or permission failures never reveal paths outside registered roots in `details`.

Frequently used infrastructure codes: `ipc.invalid-request` (DTO failed to deserialise or a field is out of range), `ipc.unknown-command`, `job.duplicate-id`, `job.not-found`, `io.path-outside-roots`, `io.not-a-directory`, `settings.newer-schema` (the file was written by a newer app and is read only), `list.revision-conflict`, `session.stale` (the `sessionId` no longer exists), `game.running` (a write was refused because the game is running, D-040).

## 6. Snapshot plus delta

### 6.1 Why

The mod list is the largest and most edited data set in the app. A refetch per edit would send up to 0.9 MB per toggle at 3000 rows. The backend therefore holds the one source of truth (the `LibrarySession`), the webview takes one snapshot, and from then on only small revisioned deltas travel, typically under 1 KiB for a toggle ([overview](overview.md) section 6).

### 6.2 Protocol

1. Snapshot: `mods_snapshot({ libraryId? })` returns `{ sessionId, rev, rows: ModRowDto[], order: ModIdx[] , counts }`. `order` is the active list as an array of `ModIdx`; inactive rows are ordered by the client from a sort key because their order is cosmetic. Rows are in `ModIdx` order, so `rows[idx]` is the row.
2. Subscription: `mods_subscribe({ sessionId, sinceRev })` opens a channel and returns `{ subscriptionId }`. The first message is always `hello { sessionId, rev }`.
3. Messages (`ModsStreamMsg`, tagged by `kind`):

| kind | Fields | Meaning |
| --- | --- | --- |
| `hello` | `sessionId`, `rev` | subscription established at this revision |
| `delta` | `rev`, `upserts: ModRowDto[]`, `removes: ModIdx[]`, `order?`, `counts?` | one revision of change |
| `resync` | `rev`, `reason` | the backend cannot replay from `sinceRev`; the client must snapshot again |
| `closed` | `reason` | the session ended (library rescan replaced it, app closing) |

   `order` is absent when the active order did not change, `{ kind: "moves", moves: [{ idx, to }] }` for edits (a drag of five rows is five small moves, a typical payload below 200 bytes), or `{ kind: "full", active: ModIdx[] }` after a sort, a profile load or an undo of those; a full order for 5000 mods is about 30 KB and is rare, so it stays an allowed large message.
4. Revisions are consecutive. Every state change, including undo and redo, produces a new revision; undo does not rewind the number.
5. The backend keeps a ring of the last 256 deltas per session so a reconnecting client with a recent `sinceRev` replays instead of re-snapshotting.
6. Coalescing: edits arriving within the same 50 ms window merge into one delta (upserts keyed by `idx`, last write wins; moves concatenated). During a scan the producer sends chunks of 250 to 1000 upserts at most 20 times per second, then a final delta that carries `counts`.

### 6.3 Client rule set (implemented once in `shared/ipc/snapshot.ts`)

1. Hold `lastRev`. On `delta`:
   - if `rev == lastRev + 1`: apply inside the next animation frame and set `lastRev = rev`;
   - if `rev <= lastRev`: drop it (duplicate after a resubscribe);
   - if `rev > lastRev + 1`: a gap. Stop applying, buffer later deltas, call `mods_snapshot`, install it, discard buffered deltas with `rev <= snapshot.rev`, apply the rest in order, resubscribe with `sinceRev = snapshot.rev`.
2. On `resync` or `closed` (or a changed `sessionId`): same path as a gap.
3. Row storage is a `Map<ModIdx, Signal<ModRowDto>>` plus an `order` signal of the active array and a version signal for the inactive filter; a delta touches only the signals of upserted rows (one cell updates one text node).
4. The client never computes load order, validity or conflicts. It sends an intent (`list_toggle`, `list_move`) and waits for the delta; an optimistic local move is allowed for drag feedback only and is overwritten by the next delta.
5. In flight edits carry `expectedRev` optionally; the backend rejects a mismatch with `list.revision-conflict`, which the client turns into a resync and a toast. Omitting it means "apply on top of the newest state", which is what keyboard repeat wants.

### 6.4 Concrete command lists

The rows themselves (kind, DTOs, owning crate, milestone, requirements, root class, aliases) are kept in the [command catalog](command-catalog.md) and are not repeated here. The protocol facts that the catalog does not carry are these.

- **Mod list (library session).** `mods_snapshot` returns full rows and the active order; `mods_subscribe` streams the deltas of 6.2; `mods_get_detail` returns the description, a dependency graph slice, the preview id, file facts and rule provenance for one `ModId`; the `list_*` actions edit through the undo stack; `sort_preview` returns the proposed order as `ModIdx[]` plus a diff with reasons and `sort_apply` pushes one undo step; `diagnostics_for_mod` returns the full diagnostics of one mod (rows carry counts only). See [catalog section 4.4 and 4.5](command-catalog.md#4-the-catalog).
- **Def explorer (def session, paging because result sets reach tens of thousands of defs).** `defs_open_session` is a job that builds or loads the def index for a reference set (progress by pack, result `{ sessionId, defCount }`); `defs_search` takes `{ sessionId, queryId?, text, filters, sort, offset, limit }` and returns `{ queryId, total, offset, items: DefRowDto[] }`, where the first call computes and caches the match list in the session and later pages pass `queryId` and only slice; `defs_get_resolved` and `defs_find_references` are queries; `defs_close_session` frees the snapshot (sessions also expire on idle). See [catalog section 4.9](command-catalog.md#4-the-catalog).
- **Datasets (small, so snapshot plus delta without paging).** `datasets_status` returns `{ rev, items: DatasetStatusDto[] }` for the five datasets (state, source, last checked, last changed, size, entries, error code); `datasets_subscribe` streams `{ rev, upserts }`; `datasets_refresh` is a job doing a conditional GET per dataset (ETag), rotating files and reporting per dataset; `rules_import_rimsort` is a job that imports RimSort user rules into the user layer. See [catalog section 4.7](command-catalog.md#4-the-catalog).

Other streams (`logs_follow`, `applog_follow`, publish progress) follow the same message union shape: `hello`, domain messages, `closed` ([catalog section 6](command-catalog.md#6-channels-and-message-unions)).

Search must answer in under 20 ms for vanilla plus DLC (13,212 defs, target in the [toolkit scope](../research/modding-toolkit-scope.md) implication 12). Page size is 200 rows; the client keeps pages it has seen in a `Map<pageIndex, rows>` and requests a page when the windowing hook reaches an unloaded range. A new `text` or filter creates a new `queryId`; stale pages are dropped by `queryId` comparison. If the session's `rev` changes (a patch file saved while the project watcher is on), `defs_search` returns `session.stale` for the old `queryId` and the client re-queries.

Datasets (small, so snapshot plus delta without paging):

| Command | Kind | Purpose |
| --- | --- | --- |
| `datasets_status` | query | `{ rev, items: DatasetStatusDto[] }` for the five datasets (state, source, last checked, last changed, size, entries, error code) |
| `datasets_subscribe` | stream | `{ rev, upserts: DatasetStatusDto[] }` as downloads progress or finish |
| `datasets_refresh` | job | conditional GET per dataset (ETag), rotate files, report per dataset result; `datasetIds` optional |
| `rules_import_rimsort` | job | imports RimSort user rules into the user layer, result lists imported, merged and skipped counts |

Other streams (`logs`, publish progress) follow the same message union shape: `hello`, domain messages, `closed`.

## 7. Jobs

### 7.1 Model (D-045)

1. The caller mints the `JobId` (a string from `crypto.randomUUID()` in the webview, a ULID-like string in the CLI) and passes it in the request. Because the id exists before the call returns, the UI can show a task row and a cancel button immediately and can cancel even if the start call is still in flight.
2. `JobRunner` in `rimstudio-app` keeps a registry keyed by `JobId`: state (`queued`, `running`, `cancelling`, `finished`), label key, started time, `CancelToken`, and the sink. A duplicate id is rejected with `job.duplicate-id`. Finished jobs stay in the registry for 60 seconds so a late `cancel_job` gets a truthful answer.
3. The command returns `JobHandleDto { jobId, startedAtMs }` once the job is registered; all later information arrives on the channel the caller passed in.
4. Work runs on the blocking pool (`rayon` work is bridged through it). The job code checks `CancelToken` between units of work (per mod folder, per file batch, per dataset, per def file) and returns `Cancelled` promptly; no thread is killed.
5. Concurrency policy per job kind is declared in the registry: `library_scan` is exclusive per library (a second start cancels or rejects per row policy), `datasets_refresh` is exclusive globally, `defs_open_session` is exclusive per reference set, others are free.
6. Shutdown: the shell asks the runner to cancel everything, waits up to 2 seconds, then exits; files are written with temp then rename (D-026), so an interrupted job leaves the previous good state.

### 7.2 Cancellation

| Trigger | Effect |
| --- | --- |
| `cancel_job({ jobId })` | sets the token; returns `{ state: "cancelling" or "finished" or "unknown" }`; idempotent |
| Channel dropped by the webview (component unmounted, window closed) | the failed `send` sets the token |
| Owner of the work disappears (library replaced) | the runner cancels dependent jobs and the stream receives `closed` |
| CLI interrupt (Ctrl+C) | the CLI cancels through the same runner and exits with code 130 |

### 7.3 Progress record shapes (shared by webview and CLI)

All job events are one tagged union, `JobEvent<R>`:

| kind | Fields | Notes |
| --- | --- | --- |
| `started` | `jobId`, `labelKey`, `phases?: string[]` | first message, never coalesced |
| `progress` | `jobId`, `phase`, `done`, `total?`, `unit` (`items` or `files` or `bytes`), `detail?` | coalesced, latest wins |
| `chunk` | `jobId`, `seq`, `items: T[]` | optional partial results (scan rows) in groups of 250 to 1000; ordered, not coalesced (not implemented in `rimstudio-ipc-types` yet: no 0.1.0 job streams partial rows) |
| `finished` | `jobId`, `result: R`, `elapsedMs`, `diagnostics?: Diagnostic[]` | terminal |
| `failed` | `jobId`, `error: ApiError`, `elapsedMs` | terminal |
| `cancelled` | `jobId`, `elapsedMs`, `partial?: R` | terminal |

Rules:

1. Exactly one terminal event per job (`finished`, `failed` or `cancelled`). A job that panics is converted to `failed` with code `job.panicked` by the runner; a missing terminal event is a test failure, mirroring the helper protocol rule.
2. The progress channel emits at most 20 messages per second per job: the runner's sink keeps the latest `progress` record and flushes it on a 50 ms tick; `started` and terminal events bypass the limiter and flush any pending progress first.
3. Each message is below 8 KiB except `chunk` and the terminal `result`, which are bounded by chunk size and designed as rare.
4. `detail` is a short string key plus parameters, not formatted prose, so the webview localises it; the CLI formats the same fields in English.
5. The CLI implements `Sink<JobEvent<R>>` as a stderr renderer (one line per phase change and a final summary) and prints the `result` as JSON on stdout. The record types are the same, so a golden transcript test covers both transports.
6. A job's long lived side effect is never the progress stream itself: when a scan finishes, the library session already holds the new index and has emitted its snapshot or resync, so a client that missed the whole job stream still converges.

## 8. Images, capabilities and path validation

### 8.1 Image protocol (D-048)

| Aspect | Decision |
| --- | --- |
| Scheme | custom asynchronous URI scheme `rsimg`, registered in the shell (`protocol_rsimg.rs`); the request logic is a Tauri-free function in `rimstudio-app` (proposed module `images`) so tests and the CLI can exercise it |
| URL form | `rsimg://localhost/mod/<ModId>?w=128`; on Windows and Android the engine rewrites it to `http://rsimg.localhost/...`; `previewUrl(id, width)` in `shared/platform` hides this ([performance note](../research/webview-and-ipc-performance.md) section 2.5) |
| Kinds | `mod` (preview image of a library mod), `tex` (a texture of a project or reference set, designer and workspace, later milestones) |
| Allow list | the id must exist in the current `ModIndex` or the open reference set; the handler resolves the path itself and never accepts a path from the URL |
| Widths | clamped to 64, 128, 256 or `full`; list rows use at most 256 (budget item 9) |
| Thumbnails | generated on the blocking pool, stored in the cache root under `thumbnails/` keyed by stat key plus width; cache is deletable (I-17) |
| Headers | `Cache-Control: max-age=31536000, immutable` for hashed URLs, `ETag` from the stat key, `Content-Type` from the decoded format, `404` for unknown ids |
| Not used | base64 in JSON (33 percent larger, no HTTP cache, no lazy loading), the `asset:` protocol for library images |

The `asset:` protocol is enabled only for user chosen files with a narrow scope if a feature needs one (for example previewing a candidate Workshop image from a folder the user picked). It is off by default.

Components use `<img loading="lazy" decoding="async" src={previewUrl(id, 128)}>`; the browser pipeline handles caching and off thread decode, which keeps scrolling at frame rate.

### 8.2 Capabilities and CSP (D-047)

1. One capability file, `apps/desktop/src-tauri/capabilities/main.json`, for window `main`. It contains `core:default`, the window permissions needed for the custom title bar where used, `dialog:allow-open`, `opener:allow-open-url` scoped to `https`, `opener:allow-reveal-item-in-dir`, and the generated `allow-<command>` entries for every registry row.
2. No `fs`, `shell`, `http`, `store`, `sql` or `log` plugin permission is granted to the webview. File access, process launching and network access happen only inside Rust handlers with their own validation.
3. `build.rs` uses `AppManifest::commands` so a future secondary window starts with no commands; any new window gets its own capability file.
4. CSP in `tauri.conf.json`: `default-src 'self' ipc: http://ipc.localhost`; `script-src 'self'`; `style-src 'self' 'unsafe-inline'` only for dynamic `style` attributes (Tailwind output is a file); `img-src 'self' data: rsimg: http://rsimg.localhost`; `font-src 'self'`; `connect-src ipc: http://ipc.localhost`. Nothing loads from the network in the webview, so fonts are self hosted and untrusted descriptions render to vnodes with remote images blocked (frontend rule).
5. Isolation stays off in the first release. It is reconsidered only if the lab shows under 1 ms added per command and more than 100 MB per second raw throughput (spike S-10); the measurement is recorded in the repository.
6. Release builds exclude DevTools; the `diagnostics` Cargo feature enables them and the diagnostics page.
7. A browser mode HTTP bridge is not built. If a dev server bridge is ever added it must bind to loopback on a random port with a per-session token and an origin check (the RimCrow bridge lacks both, [RimCrow analysis](../research/rimcrow-analysis.md) section 7.2).

### 8.3 Capability coverage test

A test in `rimstudio-shell/tests` computes four sets and asserts they are equal (apart from a short, explicit exclusion list in the registry for commands the CLI does not name):

1. command names in the registry table;
2. `allow-*` permission names in `capabilities/main.json`;
3. command keys in the exported `bindings.ts`;
4. routes reachable through `rimstudio_app::dispatch` (generic `call`).

It also asserts that no capability grants `fs:`, `shell:` or `http:` permissions. This is the executable form of I-14 and I-09.

### 8.4 Path validation

1. Prefer ids. Requests name `ModId`, `SourceId`, `ProjectId` and the backend resolves them to paths it already trusts.
2. The registry row declares, per path field, the root class it may fall in (`LibraryRoot`, `ProjectRoot`, `DataRoot`, `GameRoot`, `UserPick`; [security and privacy](security-and-privacy.md) section 5), and a test fails when a path field has no class. Fields allowed to carry a path: `sources_probe_folder.path`, `sources_add_folder.path`, `detect_set_override.path`, `project_*` open and create calls, and export or import targets chosen through the platform adapter's dialog. Each is a `UserPathDto`.
3. A `UserPathDto` is canonicalised in Rust, must exist, must be a directory or file as the command requires, and is rejected if it points into the app's own config, data or cache roots (a user cannot register the cache as a mod folder). Writes happen only through handlers that go through `RootGuard` against the registered roots (mod folder sources for reading, project roots for writing, data and cache roots for app files).
4. Under the install or config folder only the game write fence applies (D-040): owned link farm entries and `ModsConfig.xml` after a timestamped backup and a running game check. `list_save` and `deploy_apply` are the only commands that reach it.
5. `details` in errors never contains a path outside a registered root.

## 9. Typed bindings pipeline

1. Source of truth: the DTO structs in `rimstudio-ipc-types` (derives behind features `bindings` for specta, `ts-fallback` for ts-rs, `schema` for schemars) and the registry table.
2. Generator (D-042): `tauri-specta` `=2.0.0-rc.25` with `specta-typescript` `=0.0.12`, driven from `apps/desktop/src-tauri/tests/bindings.rs`, which writes `packages/ipc-types/src/bindings.ts`. The file is committed and contains command functions, DTO types, event and channel types.
3. Entry point: `cargo xtask bindings` regenerates the file; `cargo xtask bindings --check` regenerates into a temporary path and fails on any diff. `cargo xtask check` runs the check, and CI runs `cargo xtask check` on every pull request (I-19). The same pattern covers `schemas` (schemars output for JSONC files, D-027).
4. Fallback path, kept compatible by writing only plain serde structs: ts-rs 12.0.1 exports DTO types, and `xtask bindings --backend ts-rs` emits a generated `commands.ts` holding a typed invoke map built from `registry::describe()` (a JSON description of every row: name, kind, request and response type names). The frontend imports the same names either way because it consumes only `rimstudio-ipc-types`.
5. Spike S-01 (M0) decides whether the default holds: tauri-specta rc.25 must accept the macro generated wrappers, `Channel<T>` arguments and `tauri::ipc::Response` (the one open point in the research). Exit criterion: a three command slice (`app_ping`, `mods_subscribe`, `library_scan`) generates, type checks in TypeScript and round trips in a mock. If it fails, the fallback becomes the default and D-042 is updated.
6. Runtime validation of IPC payloads is done only in development and tests (valibot or the generated types in a mock layer); release builds trust the generated types.

### 9.1 Bindings as built

The 0.1.0 pipeline uses the fallback path of item 4 (D-102). Nothing here needs `tauri`.

- Derives. Every public serde type of `rimstudio-ipc-types` (and the app owned DTOs in `rimstudio-app::dto`) carries `#[cfg_attr(feature = "ts", derive(ts_rs::TS))]`. The feature `ts` is off by default, so shipped binaries do not depend on `ts-rs`. Fields that are absent when empty or unknown (`skip_serializing_if`) carry `ts(optional)` (with `ts(as = "Option<...>")` for empty collections), so the TypeScript type is `field?: T`. `serde_json::Value` becomes `JsonValue`. The generic envelopes `JobEvent<R>` and `JobResultEnvelope<R>` stay generic.
- Generator. `crates/rimstudio-app/tests/bindings.rs` collects the declarations reachable from every request and response type of the registry rows (through the exported `for_each_command!` macro) plus a short list of extra roots (error envelope, job handle, job events, mods snapshot, material matrix). 64 bit integers are typed as `number`. Declarations are sorted by name, so the output is byte stable. A second test fails when a public type of the contract crate is missing from the output.
- Files, committed under `packages/ipc-types/src`: `bindings.ts` (all DTO types) and `commands.ts` (`CommandTable` with kind, request and response of every row, the helper types `CommandName`, `CommandRequest<N>`, `CommandResponse<N>`, `CommandKindOf<N>` and `CommandsOfKind<K>`, the `commands` constant with camelCase export names, and `commandNames`). `index.ts` re-exports both. For a job row the response is the result carried by the terminal event; the command itself returns a `JobHandleDto`.
- Refresh. After changing a DTO or a registry row run `UPDATE_BINDINGS=1 cargo test -p rimstudio-app --test bindings` and commit the changed files. Without the variable the test compares and fails with that line when the files are stale.
- Golden check. `pnpm --filter rimstudio-ipc-types bindings:check` type checks the JSON files of `crates/rimstudio-ipc-types/tests/golden` against the generated types with the TypeScript compiler API, so a DTO and the JSON it produces cannot disagree. A new golden file needs a row in `packages/ipc-types/scripts/check-golden.mjs`.
- Rule for authors. Never hand copy a DTO shape in the frontend: import it from `rimstudio-ipc-types`. Behaviour that serde applies and ts-rs cannot see (a new `skip_serializing_if`, a flatten, a custom serializer) needs a `ts(...)` attribute next to it.

## 10. Backend state ownership

### 10.1 The context

`AppContext` is created once by `rimstudio_app::boot` and shared as `Arc<AppContext>`. It is explicit, with no `static`, `OnceCell` or `lazy_static` singleton anywhere in the workspace (I-16, D-006). Every handler receives it.

| Field | Owner of | Mutability |
| --- | --- | --- |
| `roots` | the four resolved data roots (config, data, cache, logs) and portable mode flag | immutable after boot |
| `platform` | `dyn` implementations of the core ports (registry, links, processes, launcher, credentials, sandbox, install source) | immutable |
| `settings` | `SettingsStore` over `settings.jsonc` and `workspace.jsonc` | `RwLock`, writes through CST edits |
| `datasets` | `DatasetStore`: per-dataset status, current and previous copies, merged rule layers | `RwLock` on status, `Arc` swaps for data |
| `library_sessions` | map `SessionId` to `Arc<LibrarySession>` (normally one) | map under a mutex, session internals below |
| `workspace_sessions` | map `SessionId` to `Arc<WorkspaceSession>` for def sessions and projects | same |
| `jobs` | `JobRunner` registry | internal mutex |
| `events` | `EventBus` sink for broadcast events | send only |
| `clock`, `ids` | ports for time and id minting | immutable |

### 10.2 Ownership by state kind

| State | Lives in | Persisted | Rule |
| --- | --- | --- | --- |
| Library snapshot (`ModIndex`: rows, packages, duplicates) | `rimstudio-library` types held by `LibrarySession` as `Arc<ModIndex>` | derived scan manifest in the cache root | immutable once built; a rescan produces a new `Arc` and a new revision base; readers never see a half built index |
| Load order session (active list, undo and redo stack, dirty flag, merged rule snapshot, diagnostics) | `LibrarySession` in `rimstudio-manager` | the saved list is `ModsConfig.xml` through `deploy`; named lists in `userdata/profiles`; history in `userdata/history` | the only mutable thing the manager edits; edits go through the command stack |
| Def sessions | `WorkspaceSession` in `rimstudio-workspace` holding an immutable `DefDatabases` snapshot | def index and type table in the cache root | snapshot built by a job, swapped atomically, queries read it without locks |
| Dataset state | `DatasetStore` | dataset copies in the cache root, status in `state.json` beside them | refresh builds a new merged rule layer and swaps the `Arc`; sessions pick it up at the next revision and recompute diagnostics |
| Settings | `SettingsStore` | `settings.jsonc`, `workspace.jsonc`, secrets in the credential store | read as typed values, written as CST edits that preserve comments and unknown keys; secrets never appear in the DTO |
| Projects | `rimstudio-workspace` project store | `projects/<id>.jsonc` in the data root | opened projects are sessions; closing frees memory |
| Jobs | `JobRunner` | none | process lifetime only |

### 10.3 The undo and redo command stack

1. Every list edit is a value of `ListCommand`: `Toggle`, `SetActive`, `Move`, `SortApply`, `ProfileLoad`, `DuplicateResolve`, and so on. Each stores enough to invert itself (previous flags, previous positions, previous order for sort and profile load), not a full copy of the list.
2. The session holds `done: Vec<ListCommand>`, `undone: Vec<ListCommand>` and a `saved_at: usize` marker. A new edit clears `undone`. The stack is capped at 200 entries; the oldest drop off the bottom, and the dirty flag is then conservatively true.
3. Dirty is `done.len() != saved_at` with the cap adjustment, so undoing back to the saved state clears the indicator. Restoring a history entry is itself an undoable command (pain point P5).
4. `sort_apply` and `profile_load` push exactly one command each, so one undo reverts a whole sort (pain point P6). `sort_preview` pushes nothing; it computes a proposed order on the immutable index and rules snapshot and returns the diff.
5. After every command or inverse, incremental validation (`rimstudio-validate`) recomputes diagnostics only for the affected mods, and the resulting row changes are part of the same revision's delta, so counts and order are never out of step.
6. `list_save` runs the pre-launch check (`deploy::check`), takes the timestamped `ModsConfig.xml` backup, writes by byte span edit and refuses with `game.running` if the game runs; it does not clear the undo stack, only moves `saved_at`.

### 10.4 Concurrency rules

1. Immutable snapshots shared by `Arc`; mutation through one owner per session guarded by a `Mutex` held only for the duration of a command application, never across `await`, disk IO or a sink send. A handler clones what it needs, releases the lock, then works.
2. Lock order is fixed and written down in `rimstudio-app`: `library_sessions` map, then a session, then `settings`, then `datasets`. A lower lock is never taken while a higher one is wanted; a test with a lock order checker in debug builds (a wrapper type in `rimstudio-app`) panics on violation.
3. Reads (`mods_snapshot`, `mods_get_detail`, `defs_search`) take a short read lock to clone the current `Arc` and then run lock free.
4. Writes to different sessions do not block each other; writes to one session are serialised in the order they arrive, which gives the consecutive revisions of section 6.
5. Blocking IO and CPU work run on `spawn_blocking` or the job runner; the async runtime threads only dispatch. `rayon` uses one shared pool sized by the scan policy (at most 8 workers for scanning, D-021) created in `boot` and passed through the context.
6. Streams hold a `Weak` reference to their session; when the session drops, they send `closed` and end.
7. The file watcher (D-028) emits `sources:changed`; it never mutates a session. The user chooses to rescan, or a setting enables automatic rescan, which starts a normal `library_scan` job.
8. Output must be deterministic: the same inputs give byte identical sort results and snapshots at 1 and 8 threads (I-12); streams may differ in chunk boundaries but the final state is identical.

## 11. The first 40 commands

This table is the milestone M2 core subset: the 40 registered rows that the manager MVP and the CLI parity test are built on (it also lists the M0, M1 and M4 rows they depend on). The authoritative list of every command, event and channel, including proposed rows, aliases and root classes, is the [command catalog](command-catalog.md); on any difference the catalog wins (D-071).

Milestones follow the [roadmap](../roadmap.md): M0 skeleton, M1 core engine, M2 manager MVP, M3 manager v1, M4 toolkit foundation, M5 item designer, M6 Workshop publishing, M7 polish and release. Request and response names omit the `Request` and `Response` suffix pair for space: `X` means `XRequest` and `XResponse`. A job command's response is `JobHandleDto`; the result type is carried by its terminal event.

| # | Command | Kind | Request, response (result for jobs) | Milestone |
| --- | --- | --- | --- | --- |
| 1 | `app_ping` | query | `AppPing` (echo, used by the performance lab and tests) | M0 |
| 2 | `app_get_info` | query | `AppGetInfo` (version, os, contractHash, portable flag, webview probe data) | M0 |
| 3 | `app_list_tools` | query | `AppListTools` (`ToolDescriptor[]` with capability availability) | M0 |
| 4 | `cancel_job` | action | `CancelJob` (jobId, resulting state) | M0 |
| 5 | `settings_get` | query | `SettingsGet` (sections optional, `SettingsDto` without secrets) | M1 |
| 6 | `settings_update` | action | `SettingsUpdate` (section patches, returns new `SettingsDto` and rev) | M1 |
| 7 | `detect_run` | job | `DetectRun`, result `DetectionReportDto` (all candidates with how and confidence) | M1 |
| 8 | `detect_get_report` | query | `DetectGetReport` (last report, cached) | M1 |
| 9 | `detect_set_override` | action | `DetectSetOverride` (which path field, `UserPathDto`) | M1 |
| 10 | `sources_list` | query | `SourcesList` (install, workshop, custom folders with status) | M1 |
| 11 | `sources_add_folder` | action | `SourcesAddFolder` (`UserPathDto`, label), returns `SourceDto` | M1 |
| 12 | `sources_update` | action | `SourcesUpdate` (label, enabled, order) | M1 |
| 13 | `sources_remove` | action | `SourcesRemove` (`SourceId`; never touches files) | M1 |
| 14 | `sources_probe_folder` | query | `SourcesProbeFolder` (looks like a mods folder, mod count estimate, removable drive flag, warnings) | M1 |
| 15 | `library_scan` | job | `LibraryScan` (`libraryId?`, `full?`), result `LibraryScanResult` (counts, timings, diagnostics count) | M1 |
| 16 | `library_get_status` | query | `LibraryGetStatus` (session id, rev, scan state, game version) | M1 |
| 17 | `mods_snapshot` | query | `ModsSnapshot` (`sessionId`, `rev`, rows, active order, counts) | M2 |
| 18 | `mods_subscribe` | stream | `ModsSubscribe` (`sessionId`, `sinceRev`), `ModsStreamMsg` | M2 |
| 19 | `mods_get_detail` | query | `ModsGetDetail` (`ModId`), `ModDetailDto` | M2 |
| 20 | `list_toggle` | action | `ListToggle` (`ModIdx[]`, `active`, `expectedRev?`), `ListEditResponse` (rev, dirty, undo and redo depth) | M2 |
| 21 | `list_move` | action | `ListMove` (`ModIdx[]`, target position or relative step, `expectedRev?`), `ListEditResponse` | M2 |
| 22 | `list_set_active` | action | `ListSetActive` (full active order, used by import), `ListEditResponse` | M2 |
| 23 | `list_undo` | action | `ListUndo`, `ListEditResponse` | M2 |
| 24 | `list_redo` | action | `ListRedo`, `ListEditResponse` | M2 |
| 25 | `list_save` | action | `ListSave` (target `ModsConfig` write), `ListSaveResponse` (backup path id, diagnostics blocking the write) | M2 |
| 26 | `sort_preview` | query | `SortPreview` (mode: canonical or game-style), `SortPreviewResponse` (`ModIdx[]`, moves with reasons, cycles) | M2 |
| 27 | `sort_apply` | action | `SortApply` (preview token), `ListEditResponse` | M2 |
| 28 | `diagnostics_for_mod` | query | `DiagnosticsForMod` (`ModId`), `Diagnostic[]` with provenance | M2 |
| 29 | `profiles_list` | query | `ProfilesList`, `ProfileDto[]` | M2 |
| 30 | `profiles_save` | action | `ProfilesSave` (name, from current list), `ProfileDto` | M2 |
| 31 | `profiles_load` | action | `ProfilesLoad` (`ProfileId`, missing mod policy), `ListEditResponse` plus missing ids | M2 |
| 32 | `datasets_status` | query | `DatasetsStatus`, `{ rev, items }` | M2 |
| 33 | `datasets_subscribe` | stream | `DatasetsSubscribe` (`sinceRev`), `DatasetsStreamMsg` | M2 |
| 34 | `datasets_refresh` | job | `DatasetsRefresh` (`datasetIds?`, `force?`), result per dataset outcome | M2 |
| 35 | `rules_import_rimsort` | job | `RulesImportRimsort` (`UserPathDto` of the user rules file), result import counts | M2 |
| 36 | `deploy_plan` | query | `DeployPlan` (active list), `DeployPlanResponse` (links to create, remove, fallback copies, blockers) | M2 |
| 37 | `deploy_apply` | job | `DeployApply` (plan token), result links created and removed, ownership manifest id | M2 |
| 38 | `launch_start` | action | `LaunchStart` (profile launch arguments, via Steam or direct), `LaunchStartResponse` (pre-launch check outcome) | M2 |
| 39 | `defs_open_session` | job | `DefsOpenSession` (reference set, project id optional), result `{ sessionId, defCount }` | M4 |
| 40 | `defs_search` | query | `DefsSearch` (see 6.4), `DefsSearchResponse` (`queryId`, total, items) | M4 |

Notes on the table:

1. `list_*` actions return a `ListEditResponse` with the new revision so the caller can detect that its own edit arrived before the delta; the delta remains the only carrier of row changes.
2. `sort_apply` accepts a preview token (a hash of the proposed order and the revision it was computed against) so an apply cannot act on an order that no longer matches; a stale token is `list.revision-conflict`.
3. `deploy_apply` and `list_save` are the only commands that can write under the game folders; both call `RootGuard` and the running game check.
4. Later milestones add `defs_get_resolved`, `defs_find_references`, `defs_build_snapshot` (job), `project_*` (M4), `designer_*` (M5), `publish_*` (M6), `logs_*` (M3) and `app_check_update` (M7). Rows that other documents already assume (all listed with their proposed names in the [command catalog](command-catalog.md)): `settings_list_themes`, `settings_get_theme` and `settings_get_user_css` (parsed theme JSON and the optional `user.css` text, M2), `log_event` (webview error reports, rate limited), `applog_query` and `applog_follow` (RimStudio's own log), `app_webview_info` (the only command allowed before the feature probe passes). Each lands as rows in the registry plus DTOs in `rimstudio-ipc-types`, with no edits to the shell or the CLI.
5. A command that turns out to exceed 1 ms inline (for example `diagnostics_for_mod` on a pathological mod) is promoted to a job by changing its kind; the TypeScript wrapper changes shape, and the compiler finds every call site.

## 12. Verification

| Check | Where | Gate |
| --- | --- | --- |
| Registry parity (names, permissions, bindings, CLI routes) | shell tests, `cargo xtask check` | CI fails on any difference |
| Bindings and schema drift | `cargo xtask bindings --check`, `schemas --check` | CI fails on any diff |
| Job lifecycle: one terminal event, cancel, channel drop, duplicate id, 20 msgs per second cap | `rimstudio-app` tests with fake sink | CI |
| Snapshot plus delta: gap, duplicate, resync, replay from ring, concurrent edits give consecutive revisions | `rimstudio-manager` and `rimstudio-testing` | CI |
| Payload sizes: 5000 row snapshot round trip under 50 ms, 3000 rows under 1 MB | criterion bench against `xtask/budgets.jsonc` | 2x regression fails |
| Toggle in a 5000 mod list: IPC round trip under 5 ms, Rust under 5 ms, input to paint under 50 ms | `e2e_budgets` bench plus Playwright with mock IPC | budget file |
| Path rules: no command accepts a path outside the allowed list; `RootGuard` rejects traversal and symlink escape | shell and app tests | CI |
| Deterministic output at 1 and 8 threads | CLI golden tests | CI |

## 13. Open points and owner decisions

1. Spike S-01 decides the bindings generator default (section 9).
2. Isolation cost is unmeasured (spike S-10, `--features iso` in the lab).
3. The `rimstudio-app::images` module (id allow list, headers) and `rimstudio-library::thumbs` (decode, resize, cache) are now in the [crate catalog](crate-catalog.md); the shell keeps only `protocol_rsimg.rs`.
4. App identifier (D-049) fixes the data directories and therefore the roots that `boot` resolves; the placeholder `app.rimstudio.desktop` is used until the owner chooses.
5. Keychain use for secrets (D-030) affects `settings_get` only by the fact that secrets are never returned; the owner decision does not change the IPC contract.

## 14. Development bridge

The temporary test UI of release 0.1.0 runs in a normal browser (Vite), where Tauri's `invoke` does not exist. The crate `rimstudio-devserver` (layer `l4-shell`, binary `rimstudio-devserver`, [ADR 0040](../adr/0040-development-bridge.md), D-103) boots the same `AppContext` as the shell and the CLI and serves the registry over HTTP on the loopback address, so the browser UI works with the real backend and the real numbers of an install. It is a development tool: it is never packaged, no release artefact contains it and the shell does not depend on it. The UI reaches it only through `shared/ipc` (the frontend rule that no `fetch` or `invoke` exists elsewhere holds).

Run it with `cargo run -p rimstudio-devserver -- [flags]` (set `CARGO_TARGET_DIR` as for any cargo command). It prints a banner with the address, the token and the data folder.

### 14.1 Flags and defaults

| Flag | Default | Meaning |
| --- | --- | --- |
| `--port N` | 7878, or the environment variable `RIMSTUDIO_BRIDGE_PORT`; 0 picks a free port | Loopback port |
| `--data-dir PATH` | `$HOME/.local/share/rimstudio-dev` | Data folder with `config`, `data`, `cache` and `logs` inside, in portable style (`BootInput::with_data_base`). It never mixes with the CLI's folders or with the data of an older application |
| `--token-file PATH` | `node_modules/.cache/rimstudio-bridge.json` in the repository root | Written at start as `{"port":N,"token":"..."}` and removed at a clean stop; the frontend dev tooling reads it |
| `--allow-origin URL` | `http://localhost:5173` and `http://127.0.0.1:5173` | Origin a page may call from; repeatable; any use replaces the defaults |
| `--help` | | Usage text |

The token is 32 random bytes in hex, generated at each start. The server stops cleanly when `quit` is typed in its terminal. The standard library cannot catch SIGINT without unsafe code, which the workspace forbids, so Ctrl+C ends the process at once: the token file stays behind (its port is dead, so the UI reports the bridge as not running) and the crash marker of the data folder is not marked clean, which the next start reports as "previous run ended abnormally" in the development data folder only.

### 14.2 Bridge API v1

Base URL `http://127.0.0.1:PORT`. One request per connection (`Connection: close`).

| Endpoint | Answer |
| --- | --- |
| `GET /dev/health` | `{"ok":true,"bridgeVersion":"0.1.0"}`. The only call without a token |
| `GET /dev/info` | `{"bridgeVersion","platform","home","dataDir","commandCount","contractHash","roots":{"config","data","cache","logs"}}` |
| `GET /dev/commands` | `[{"name","kind","request","response"}]`, one row per registry command |
| `POST /rpc/COMMAND` | The body is the request JSON of the command (an empty body is an empty request). HTTP 200 with `{"ok":true,"data":RESPONSE}` or `{"ok":false,"error":ApiError}`. A job command is run through the job runner and the response arrives when the job ends. A job that reports diagnostics adds a `diagnostics` array next to `data`. A request may carry a top level `jobId` for a job so that it can match the events |
| `GET /dev/events` | Server-Sent Events, one JSON object per `data:` line: `{"type":"job-progress","jobId","command","message","done","total"}` and `{"type":"job-finished","jobId","command","ok"}`. A comment line every 15 seconds keeps the stream alive. `EventSource` cannot send the token header, so the client reads it with `fetch` and a stream reader |
| `GET /dev/fs/list?path=ABS[&files=1]` | `{"path","parent","entries":[{"name","kind":"dir"\|"file","isModFolder","hasAbout","symlink"}],"truncated"}`: absolute paths only (a relative path or a `..` part is a 400), directories first then by name without case, dotfiles hidden, at most 2000 entries. `isModFolder` means `About/About.xml` exists and `hasAbout` that an `About` folder exists, both without regard to case. A symbolic link is listed with the kind of its target and `symlink: true`, and is never looked into (so it reports no mod flags) |
| `GET /dev/fs/home` | `{"home","places":[{"label","path"}]}` with the home folder, the Steam libraries of the cached detection report, the RimWorld folder, the sources (install `Data` and `Mods`, Workshop, custom mod folders) and the filesystem root or the drive letters; only folders that exist |

Application errors are HTTP 200 with `ok: false` (an unknown command is `ipc.unknown-command`, a request of the wrong shape `ipc.invalid-request`). Protocol problems use a 4xx or 5xx status with the same envelope, whose `details.reason` names the cause:

| Status | Code | Cause |
| --- | --- | --- |
| 400 | `ipc.invalid-request` | Malformed request line, header or target, body that is not JSON, bad command name, relative or `..` path, repeated security header |
| 401 | `bridge.unauthorized` | Token missing or wrong |
| 403 | `bridge.forbidden` | Peer not loopback, `Host` not `127.0.0.1:PORT` or `localhost:PORT`, `Origin` not allowed |
| 404 | `bridge.not-found`, `io.not-found` | No such path, folder does not exist |
| 405 | `ipc.invalid-request` | Method not allowed for the path (an `Allow` header names the right one) |
| 408 | `bridge.timeout` | The request was not complete within 30 seconds |
| 411 | `ipc.invalid-request` | A POST without `Content-Length`, or with `Transfer-Encoding` (chunked bodies are not supported) |
| 413 | `bridge.too-large` | Body over 8 MiB, refused from the header before it is read |
| 414, 431, 505 | `ipc.invalid-request` | Target over 2048 bytes, request line and headers over 16 KiB or over 100 fields, version other than HTTP/1.0 and 1.1 |
| 415 | `ipc.invalid-request` | A POST whose content type is not `application/json` |
| 503 | `bridge.too-busy` | 32 connections are already open |

The `bridge.*` codes exist only here; they are not in the contract registry.

### 14.3 Security

The bridge reaches the real disk and the real install, and every web page in the person's browser can send requests to loopback, so the checks run in a fixed order before any body is read:

1. Loopback only: the listener binds 127.0.0.1 and a peer that is not loopback is refused.
2. `Host` must be `127.0.0.1:PORT` or `localhost:PORT` (a rebound DNS name fails).
3. A present `Origin` must be on the allow list. An allowed origin gets `Access-Control-Allow-Origin` (also on error answers, so the page can read them) and `Vary: Origin`. A `null` origin is refused.
4. The preflight `OPTIONS` is answered without a token, because a browser cannot attach one; it carries only the allowed methods and headers (`content-type`, `x-rimstudio-token`) and no data.
5. Every other call except `GET /dev/health` needs the header `x-rimstudio-token`, compared in constant time. A missing token is a 401 on every path, so no route existence leaks.
6. A POST must use `Content-Type: application/json`, which a simple cross site request cannot send without a preflight.

Limits: 16 KiB for the request line and headers, 8 MiB for a body, 30 seconds in total to deliver a request (one deadline for the whole request, so a client that sends one byte at a time cannot hold a connection), 32 connections, one request per connection, no chunked uploads, no keep alive, no pipelining. A repeated `Host`, `Origin`, `Content-Length`, `Content-Type`, token or `Transfer-Encoding` header is refused, and a line feed without a carriage return is refused. The token file holds a secret that is valid for one run on loopback; it lives under `node_modules`, which is not committed.

### 14.4 Verification

`crates/rimstudio-devserver/tests/protocol.rs` runs over a real loopback socket against an application booted with fake ports and a temporary data folder: a query, an action and a job (final result and the `job-finished` event on the stream), the unknown command and shape errors, a bad token, `Host`, `Origin` and content type, a header and a body over the limit, a slow client, a client that trickles bytes, too many connections, malformed request lines, a relative and a traversing path in the folder listing, and a listing of a fixture tree. The parser, the token comparison and the flags have unit tests.

Related documents: [command catalog](command-catalog.md), [architecture overview](overview.md), [crate catalog](crate-catalog.md), [decision register](decision-register.md), [workspace layout](workspace-layout.md), [reference architectures](../research/reference-architectures.md), [RimCrow analysis](../research/rimcrow-analysis.md).
