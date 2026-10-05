# RimStudio webview and IPC performance

Scope: how RimStudio's frontend talks to its Rust backend through Tauri 2 (commands, events, channels, raw bodies, custom protocols, capabilities, isolation), how to move thousands of mod records and progress streams, how to generate typed bindings, what the three system webviews (WebKitGTK, WKWebView, WebView2) can and cannot do for a Tailwind CSS 4 app, and a numbered performance budget. Measurements come from a small Tauri 2.12.1 test application ("the lab") built and run once on the research machine; they are indicative, not a benchmark suite.

Status: research note | Last verified: 2026-10-04

Companion: `docs/research/frontend-stack-research.md` (library choices and version pins).

Evidence conventions: lab sources and raw results are committed under `docs/research/data/frontend-stack/` (`ipc-lab/`, `lab-result-default.json`, `gen_mods_dataset.py`). Tauri documentation was read from a shallow clone of the tauri-docs repository (commit dated 2026-10-03); Rust facts were read from the crate sources of `tauri` 2.12.1, `tauri-utils` 2.10.1 and `wry` 0.57.0 in the local cargo registry; npm and crates.io metadata was queried on 2026-10-04.

## 1. Measurement setup and honesty notes

| Item | Value |
| --- | --- |
| Machine | Linux (Arch), 8 cores, 1920x1080, device pixel ratio 1 (lab result `env`) |
| Webview | WebKitGTK 2.52.6 (`pacman -Q webkit2gtk-4.1`), window 1280x800, release build, `custom-protocol` and `protocol-asset` features |
| Tauri | `tauri` 2.12.1, `tauri-build` 2.7.1, `@tauri-apps/api` 2.12.1 (matching minor versions) |
| Dataset | 3000 and 5000 mod records sampled with replacement from the real `About.xml` files of the owner's workshop folder; "full" records (with descriptions) are 4.97 MB as JSON for 3000, "slim" records (list columns only) are 0.91 MB; script `docs/research/data/frontend-stack/gen_mods_dataset.py` |
| Timer resolution | WebKit reports whole milliseconds in this setup, so medians of 0 mean "below 1 ms" |
| Not measured | the isolation pattern (the lab has an `iso` feature and an `isolation/` folder but only the default variant was run), Windows, macOS, GPU load, other WebKitGTK versions, battery or low end hardware |

All numbers below are wall clock medians (n between 4 and 1000 per case) taken inside the webview with `performance.now()`. Treat them as order of magnitude guidance and re-run the lab (`LAB_DATA_DIR`, `LAB_OUT` environment variables, see `ipc-lab/src/main.rs`) on target machines before fixing budgets.

## 2. Tauri 2 IPC in depth

### 2.1 Primitives

| Primitive | Direction | Payload | Typical use | Source |
| --- | --- | --- | --- | --- |
| Command (`invoke`) | JS to Rust, with a reply | JSON arguments, JSON or raw bytes reply; raw request body allowed (`ArrayBuffer`/`Uint8Array`) | queries, actions | docs `develop/calling-rust.mdx` |
| Event | either direction, fire and forget | JSON | lifecycle, "something changed" | docs `concept/Inter-Process Communication/index.mdx` |
| Channel | Rust to JS, ordered stream tied to one invoke | JSON or raw bytes | progress, streamed results | docs `develop/calling-rust.mdx` ("recommended mechanism for streaming data") |
| Custom URI scheme | JS (browser fetch, `<img src>`) to Rust handler | HTTP-like request and response | images, large blobs | `tauri::Builder::register_asynchronous_uri_scheme_protocol` (tauri 2.0 blog) |
| Asset protocol | browser to filesystem | file bytes | local files within a configured scope | docs `security/asset-protocol.mdx` |

Verified details:

- Command return values that implement `Serialize` are serialised to JSON, and the docs warn this can be slow for large data such as files; for bytes, return `tauri::ipc::Response` (docs `calling-rust.mdx`).
- A command can take `tauri::ipc::Request` to read the raw body and headers, and the frontend sends `ArrayBuffer` or `Uint8Array` as the payload (docs, "Accessing Raw Request").
- Channel ordering: the JavaScript `Channel` class numbers messages and reorders them if they arrive out of order, delivering them in send order (comment and code in `@tauri-apps/api` 2.12.1 `core.js`).
- Channel send path (tauri 2.12.1 `src/ipc/channel.rs`): a JSON message whose serialised string is shorter than 8192 bytes (`MAX_JSON_DIRECT_EXECUTE_THRESHOLD`) is delivered by evaluating a script directly in the webview; larger JSON messages and raw messages of 1024 bytes or more (`MAX_RAW_DIRECT_EXECUTE_THRESHOLD`) are stored and fetched by the frontend, which costs an extra round trip. This explains the throughput cliff in the lab (section 2.3).
- Events "still utilise commands under the hood" and their access is controlled by event permissions (docs). In the lab, 2000 events of 100 bytes took 92 ms versus 57 ms for 2000 channel messages of the same size.
- App commands registered with `invoke_handler` are allowed for all windows by default; `tauri_build::AppManifest::commands` restricts them (docs `security/capabilities.mdx`). Plugin commands are blocked until a capability grants them.

### 2.2 Measured IPC costs (lab, WebKitGTK 2.52.6)

| Case | n | Median | p95 | Note |
| --- | --- | --- | --- | --- |
| `ping` async command | 1000 | below 1 ms (mean 0.39) | 1 | round trip |
| `ping` with `#[command]` sync | 1000 | below 1 ms (mean 0.20) | 1 | |
| echo 4 KB JSON | 500 | below 1 ms (mean 0.45) | 1 | |
| echo 64 KB JSON | 200 | 1 ms (mean 0.86) | 1 | |
| 1000 parallel pings | 1 run | 71 ms total | | 0.07 ms per call; 5000 parallel took 348 ms |
| slim list, 3000 records, command returning JSON | 20 | 8 ms | 14 | 0.91 MB |
| slim list, 5000 records | 20 | 14 ms | 16 | |
| full records, 3000 | 10 | 43 ms | 51 | 4.97 MB |
| full records, 5000 | 6 | 81 ms | 85 | |
| upload slim 3000 as JSON argument | 15 | 21 ms | 24 | JS to Rust |
| raw bytes 1 MB / 8 MB / 32 MB download | 20 / 8 / 4 | 3 / 27 / 162 ms | | about 200 to 330 MB per second |
| raw bytes upload 1 MB / 8 MB | 20 / 6 | 2 / 12 ms | | |

Decode inside the webview (pre-serialised 3000 record payloads):

| Format | Size (3000 records) | Median decode |
| --- | --- | --- |
| JSON full | 4,971,154 B | `JSON.parse` 22 ms |
| JSON slim | 912,944 B | `JSON.parse` 4 ms |
| MessagePack full / slim | 4,485,414 / 689,798 B | 43 ms / 11 ms (`@msgpack/msgpack`-style decoder in JS) |
| Own columnar binary slim | 360,680 B | 3 ms to open and decode all strings, 0.06 ms for a 50 row window |

Findings: (a) for list-column data the JSON path costs about 8 ms for 3000 records and 14 ms for 5000, far below one frame budget of 100 ms for a "loading" interaction, so a binary format buys back about 6 ms and is not justified; (b) MessagePack decoded in JS is slower than `JSON.parse` for this data (11 ms versus 4 ms slim) because the JSON parser is native; (c) the expensive part is not transport but descriptions: full records are 5.5 times larger and take 5 to 10 times longer, so never send descriptions in the list snapshot; (d) raw bytes reach 200 to 330 MB per second, so file contents and images should always use raw responses or protocols, never JSON arrays or base64.

### 2.3 Channels and progress streams (lab)

| Stream | Messages | Median total | Note |
| --- | --- | --- | --- |
| 3000 slim rows in chunks of 50 | 60 | 15 ms | |
| chunks of 250 | 12 | 10 ms | |
| chunks of 1000 | 3 | 8 ms | each message about 300 KB, above the direct threshold |
| 2000 small JSON messages of 100 B | 2000 | 57 ms | about 35,000 messages per second |
| 2000 JSON messages of 4000 B | 2000 | 64 ms | below 8192 B: direct path |
| 2000 JSON messages of 9000 B | 2000 | 352 ms | above 8192 B: stored and fetched, about 5.7 times slower |
| raw 1000 x 512 B | 1000 | 81 ms | about 6 MB per second, per message overhead dominates |
| raw 300 x 64 KB | 300 | 81 ms | about 231 MB per second |
| raw 40 x 1 MB | 40 | 132 ms | about 303 MB per second |

Guidance:

1. A progress stream should coalesce on the Rust side to at most one message per 50 to 100 ms (10 to 20 per second), and each message should be either clearly below 8 KiB or large and rare; avoid streams of many messages around 9 KB.
2. A scan of 600 to 5000 mods should stream results in chunks of 250 to 1000 records (8 to 10 ms in the lab for 3000 records), followed by an end marker; the UI applies each chunk in one `requestAnimationFrame`.
3. The JS side must not do per message work proportional to total list size; apply chunks to a `Map` and bump a version signal.

### 2.4 Snapshot plus deltas, paging, cancellation

Design for mod list data (R3, R5):

1. `list_mods_snapshot()` returns `{ rev, rows }` where each row has only list fields (id, name, authors, flags, version bitmask, load index, error and warning counts, size, update time). Approximate size at 3000 rows: 0.9 MB, 8 ms.
2. Detail data (description, full dependency list, preview path, file listing) is fetched per mod on selection with `get_mod_detail(id)`; hovering or selecting a row issues at most one call, cached by id.
3. Changes are pushed as `{ rev, upserts: Row[], removes: id[] }` on one long lived Channel opened by `subscribe_mods(channel)`. A revision gap (client sees `rev` not equal to previous plus 1) triggers a new snapshot. Sorting results are returned as a compact id order (array of numbers), not as rows.
4. Paging is unnecessary for list rows at this size (a 5000 row snapshot is 14 ms). Use paging or cursors only for genuinely large sets: def explorer results (tens of thousands of defs from 615 mods), search hits and log views; there use `{offset, limit, total}` with the backend holding the query result.
5. Cancellation: every long operation takes a client generated `jobId`; the frontend calls `cancel_job(jobId)`; Rust checks an `AtomicBool` or a cancellation token between units of work; closing a Channel from the JS side (dropping it) must also stop producers (a failed `send` is the signal in Rust, as in the lab's streaming commands).
6. Backpressure: Channel `send` is non blocking from the command's perspective; producers should still bound their rate (coalescing, section 2.3) because the webview main thread is the bottleneck, not the pipe.

Formats and requirement R10: R10 lists IPC among the things that must be JSON (and states XML appears only for RimWorld's own files). Therefore the default for all structured IPC is JSON. The measurements show no need for an exception. Raw byte bodies are used only for opaque non-app data (image bytes, file contents, downloads); if the owner ever wants a packed binary format for lists, it needs an explicit R10 exception. The lab's columnar format (360 KB, 2 to 3 ms) is kept only as evidence that the upside is small.

### 2.5 Serving preview images

| Method | Lab result (200 images of about 45 KB, WebKitGTK) | Notes |
| --- | --- | --- |
| custom scheme `modimg://localhost/<id>.jpg` | sequential 120 ms (0.6 ms per image, 72 MB per second); 8 concurrent 96 ms | handler registered with `register_asynchronous_uri_scheme_protocol`; allows thumbnail generation, caching headers, allow list |
| asset protocol `asset://localhost/<encoded path>` | sequential 96 ms (0.48 per image); 8 concurrent 36 ms (0.18 per image, 87 MB per second); 20 big images sequential 68 ms (3.4 each) | needs `protocol-asset` feature, `assetProtocol.enable` and a file scope; no transformation |
| command returning base64 string | 100 sequential in 59 ms (0.59 each, 60,022 characters average) plus 69 ms to decode 100 images afterwards | 33 percent larger, held as JS strings, no HTTP cache or lazy loading |
| `<img>` decode, protocol or asset | about 0.45 to 0.53 ms per image | same cost |

Conclusion: transport time is similar and small for all three, so the choice is about behaviour. A custom scheme (for example `rsimg://localhost/<modid>?w=128`) is best: the browser's image pipeline gives native `loading="lazy"`, `decoding="async"`, HTTP caching with `Cache-Control` and ETag, off main thread decode, and the Rust handler can serve resized thumbnails from an on disk cache and check the id against the known mod set (no arbitrary path access). Use the asset protocol only for user chosen files with a narrow scope; never use base64 for lists of images. Platform URL forms: on Linux and macOS `scheme://localhost/...`, on Windows `http://scheme.localhost/...` (default `useHttpsScheme: false`, which `tauri-utils` documents as changing the storage location if toggled between releases); the lab CSP lists both forms. A helper `previewUrl(id)` must hide this. The CSP `img-src` must include the scheme.

### 2.6 Isolation pattern

Per the Tauri docs: the isolation application runs in a sandboxed iframe, every IPC message from the frontend passes through it and is encrypted with AES-GCM using a key generated at each start, so there is overhead "even if the secure Isolation application doesn't do anything"; on Windows external files in the sandboxed iframe do not load, so the build inlines scripts. The docs recommend it "whenever it can be used" to defend against compromised frontend dependencies. It needs the `isolation` Cargo feature on `tauri` (present in 2.12.1's `Cargo.toml`). Cost was not measured here. Position for RimStudio: RimStudio ships only bundled local assets and renders untrusted text through a vnode renderer, so the primary controls are a strict CSP, minimal capabilities and no remote content; plan to measure the isolation overhead with the lab's `iso` build on a 3000 row snapshot and a progress stream before deciding. Decision rule: enable it if added latency per command is under 1 ms and large payload throughput stays above 100 MB per second, else leave it off and rely on capabilities.

### 2.7 Capabilities and permissions for RimStudio

Capabilities are JSON or TOML files in `src-tauri/capabilities/`; all files there are enabled automatically unless `tauri.conf.json` lists capabilities explicitly; windows in several capabilities get the union of permissions (docs `security/capabilities.mdx`). Plan:

1. One `main.json` capability for the `main` window with `core:default` (the lab uses exactly this) plus only: window permissions for the custom titlebar on platforms that use it (`core:window:allow-start-dragging` and the minimise, maximise, close permissions), `dialog:allow-open` for folder pickers, `opener:allow-open-url` with a scope of `https` (Workshop and mod pages) and `opener:allow-reveal-item-in-dir`.
2. No `fs`, `shell` or `http` plugin on the frontend. All file access, process launching (starting RimWorld) and network access (dataset fetching, Workshop) happen inside Rust commands with their own path validation. This keeps the webview unable to read arbitrary files even if compromised.
3. Restrict app commands with `AppManifest::commands` in `build.rs` so a future second window (for example an inspector or an editor popout) does not automatically inherit every command.
4. Secondary windows get their own capability file.
5. CSP set in `tauri.conf.json` (the CSP is only enforced when configured, per docs): `default-src 'self' ipc: http://ipc.localhost`, scripts from self only, `style-src 'self' 'unsafe-inline'` only if unavoidable (Tailwind output is a file, so inline styles are needed only for dynamic style attributes), `img-src 'self' data: blob:` plus the image scheme, `connect-src` limited to `ipc:`. Tauri appends nonces and hashes automatically.

## 3. Typed bindings

| Option | Version (crates.io, 2026-10-04) | What it generates | Status |
| --- | --- | --- | --- |
| tauri-specta | 2.0.0-rc.25 (2026-05-08); 1.0.2 is Tauri 1 only | TypeScript functions for commands, event types, Channel types | repository active (commits through 2026-07-20); still release candidates since 2023-24; depends on `tauri ^2`, `specta =2.0.0-rc.25`, `specta-serde ^0.0.12`, `specta-typescript ^0.0.12` (optional) |
| specta + specta-typescript | 2.0.0-rc.25 (2026-05-07), 0.0.12 (2026-05-07) | TypeScript types only | same maintainers, pre 1.0 exporter |
| ts-rs | 12.0.1 (2026-01-31) | TypeScript types from `#[derive(TS)]`, exported by tests | stable major, types only, no command wrappers |
| typeshare | 1.0.5 (2026-01-02), CLI 1.13.4 | types for several languages through a CLI | types only, annotation based |
| schemars | 1.2.2 (2026-07-27) | JSON Schema | not TS, but useful for JSONC config schemas |

tauri-specta's generator handles `Channel` arguments (the exporter checks for channel types in arguments and results, `src/lang/js_ts.rs`) and typed events. A search of its README and source found no mention of `tauri::ipc::Response` or `InvokeResponseBody` (unverified how a raw response command is typed), so raw commands should be few, hand typed in one module (`shared/ipc/raw.ts`) and covered by tests.

Recommendation: `tauri-specta = "=2.0.0-rc.25"` with `specta = "=2.0.0-rc.25"` and `specta-typescript = "=0.0.12"` (exact pins because pre-release semver resolution is surprising), a `bindings` export step that runs in a test or a `cargo xtask` and writes `packages/rimstudio-ipc-types/src/bindings.ts`, committed, with a CI check that regeneration produces no diff. Keep the data types in a `rimstudio-ipc-types`-style Rust crate (plain serde structs with `specta::Type`), separate from the Tauri glue crate, so the domain crates do not depend on Tauri. Fallback if tauri-specta breaks on a Tauri upgrade: ts-rs 12.0.1 (derive on the same DTOs) plus a hand maintained typed `invoke` map (one function per command, about 5 lines each, verified by a test that compares the command list in Rust to the TS map). Add `schemars` 1.2.2 to emit JSON Schema for `settings.jsonc`, theme and project files so editors can offer completion; this respects R10 (JSON Schema is JSON).

Rules for command design that keep bindings stable: input and output types are named structs (no anonymous tuples), enums use `#[serde(tag = "kind")]`, ids are strings or `u32`, never 64 bit integers that exceed 2^53 (workshop ids need care: Steam workshop ids fit in 2^53 for years, but send them as strings in DTOs to be safe, a rule the lab data dodged by using a float column), errors are a serialisable enum with a stable `code` field plus message parameters (so the UI can translate).

## 4. Webview realities per operating system

### 4.1 Engines and minimum versions

| OS | Engine | Update model | Floor to assume | Evidence |
| --- | --- | --- | --- | --- |
| Windows 10/11 | WebView2 (Chromium) | evergreen, Tauri installer ensures it is present | any current runtime satisfies Tailwind 4 | Tauri docs `reference/webview-versions.md` |
| macOS | WKWebView (system WebKit) | updated only with the OS; unsupported macOS versions get no WebKit updates | macOS 13.3 or newer for Safari 16.4 level WebKit | the Tauri table lists Ventura 13.3 as WebKit 615.1.26 with Safari 16.4 and 13.0 as Safari 16.1 |
| Linux | WebKitGTK via `webkit2gtk-4.1` | distribution package | see below | Tauri AUR and Debian pages list `webkit2gtk-4.1`; `wry` 0.57.0 uses the `webkit2gtk` crate 2.0.2 |

Tauri's default `bundle.macOS.minimumSystemVersion` is 10.13 (`tauri-utils` 2.10.1 `config.rs`), far below what a Tailwind 4 app works on, so set it explicitly (13.3 or newer) and have the same check at runtime.

WebKitGTK versions per distribution on 2026-10-04 (Repology data fetched that day; several entries per repository mean release and security updates):

| Distribution | Versions listed |
| --- | --- |
| Arch | 2.52.6 (testing 2.54.1) |
| Debian 13 / 14 | 2.52.6 and 2.54.0 / 2.52.6 |
| Debian 12 | 2.42.2, 2.50.6 |
| Ubuntu 24.04 | 2.44.0, 2.52.6 |
| Ubuntu 22.04 | 2.36.0, 2.50.4 |
| Ubuntu 26.04 / 26.10 | 2.52.0, 2.52.6 / 2.52.6 |
| Fedora 43 / 44 | 2.50.0, 2.52.5 / 2.52.1, 2.54.0 |
| openSUSE Tumbleweed | 2.52.6 |
| Nix unstable | 2.54.0 |

Tailwind 4's floor is Safari 16.4 and Vite 8's default build target is also Safari 16.4 (`Baseline Widely Available`, Vite docs). The mapping from WebKitGTK release numbers to Safari versions was not verified (unverified; WebKitGTK 2.40 is believed to correspond to Safari 16.4), so do not hard code a WebKitGTK number from this table: use feature probes (section 4.2) and show a clear "your system webview is too old" screen. The practical statement: current releases of all mainstream distributions (rolling and the latest LTS) carry 2.50 or newer; releases that only carry 2.36 or 2.42 are outside the support promise.

### 4.2 Feature availability (lab, WebKitGTK 2.52.6)

Probed with `CSS.supports`, `CSS` rule classes and API checks (`ipc-lab/dist/features.js`). All of the following were true: container queries (`container-type`), `:has()`, `:is()` and `:where()`, `:focus-visible`, `color-mix()`, `oklch()`, `oklab()`, `light-dark()`, relative colour syntax, `@layer`, `@property`, `@scope`, `@starting-style`, CSS nesting, subgrid, `content-visibility`, `contain-intrinsic-size`, `contain: strict`, `scrollbar-gutter`, `scrollbar-width`, `scrollbar-color`, `overscroll-behavior`, `aspect-ratio`, `text-wrap: balance` and `pretty`, `field-sizing`, anchor positioning (`anchor-name`, `position-area`), view transitions, scroll driven animations, `backdrop-filter`, `mask-image`, `accent-color`, individual transform properties, `dvh` units, `popover` attribute, `<dialog>.showModal`, `inert`, `Element.checkVisibility`, and the media features `prefers-color-scheme`, `prefers-contrast`, `prefers-reduced-motion`, `forced-colors`.

Reported false on that engine: `interpolate-size`, `Element.moveBefore` (Preact 11 uses it when present), `requestIdleCallback`, `scheduler.postTask` and `scheduler.yield`, `Temporal`, `SharedArrayBuffer` and cross origin isolation, WebGPU, `showOpenFilePicker`, and the Sanitizer API (`setHTML`).

What a Tailwind 4 app needs and how to handle gaps:

| Need | Used by | Handling on an older engine |
| --- | --- | --- |
| `@layer`, `@property`, `color-mix()`, `oklch()` | Tailwind 4 core and the token scheme | hard requirement; startup probe fails fast with an explanatory screen |
| container queries | responsive panels | hard requirement (probe `CSS.supports('container-type: inline-size')`) |
| `:has()` | selection and row state styles | hard requirement |
| subgrid | form and table alignment | use only where a plain grid fallback is acceptable |
| `content-visibility: auto` | long non virtualised pages | progressive enhancement; ignored when absent |
| `scrollbar-gutter`, `scrollbar-color` | stable scrollbars and theme-coloured bars | progressive enhancement; also style `::-webkit-scrollbar` as a fallback |
| popover attribute, anchor positioning | menus and tooltips | do not depend on them; Zag plus Floating UI work everywhere (companion note) |
| `requestIdleCallback` missing | any idle work | use a `MessageChannel` or `setTimeout` scheduler utility, never call `requestIdleCallback` directly |
| no `SharedArrayBuffer`, no WebGPU | workers | design workers around message passing; no WebGPU or WebGL dependence |
| no Sanitizer API | rich text | vnode renderer, DOMPurify as backstop |

A startup probe (`shared/platform/probe.ts`) runs the `CSS.supports` and API checks above before mounting the app and reports the engine to a diagnostics page; it costs under 1 ms.

### 4.3 Known WebKitGTK problems and workarounds

From Tauri's "Linux Graphics Issues" page (docs `develop/Debug/linux-graphics.md`):

| Symptom | Cause | Workaround, in the order the docs advise |
| --- | --- | --- |
| blank or white window, flicker while resizing, death on resize, console line "AcceleratedSurfaceDMABuf was unable to construct a complete framebuffer", "Gdk-Message: Error 71 (Protocol error) dispatching to Wayland display" | WebKitGTK's DMABUF renderer asking for buffer formats the NVIDIA driver does not offer | 1. kernel mode setting on (`nvidia_drm.modeset=1`, older than driver 545); 2. `__NV_DISABLE_EXPLICIT_SYNC=1`; 3. `WEBKIT_DISABLE_DMABUF_RENDERER=1` (loses the faster path); 4. `WEBKIT_DISABLE_COMPOSITING_MODE=1` (last resort, disables accelerated compositing) |
| WebGL or canvas silently slow | the context is created even when software rendered; the renderer string is masked (reports "Apple GPU" on every Linux machine) | give WebGL views a non WebGL fallback and an in-app switch; RimStudio avoids WebGL |

The docs also say not to ship an unconditional override unless verified, because it slows everyone. RimStudio design for Linux:

1. The Rust `main` reads a tiny launch configuration (`launch.jsonc` in the app config folder, JSONC per R10) before the webview exists and sets the variables above from it; Settings (Appearance, Compatibility) edits that file and says a restart is required.
2. A `--safe-graphics` command line flag and a crash marker: write a "starting" marker file at launch and delete it after the first frame is reported by the frontend; if the marker exists at the next launch, offer safe graphics (DMABUF renderer off) automatically with an explanation.
3. Optional default heuristic: NVIDIA proprietary driver present plus a Wayland session proposes the DMABUF override on first launch (heuristic, user can reverse); do not apply it silently.
4. Report the engine version, session type and graphics variables on the diagnostics page so bug reports carry them.

Rendering costs measured in the lab (60 Hz display): scroll frame time medians were 17 ms for plain, 17 ms with box shadows, 16 ms with `backdrop-filter: blur` and 17 ms with `will-change`, all at the vsync interval, so none of the variants dropped frames on this hardware. That is not evidence that blur and shadows are free elsewhere; the rule in section 5 is to avoid them on large surfaces and test on low end machines. What did measure large is layout: 3000 rows of 9 nodes (27,000 nodes) cost 876 ms of layout and 54,000 nodes cost 1611 ms, versus 196 ms and 494 ms with `content-visibility: auto`. Memory: after the bench the WebKitWebProcess went from 104 MB to 347 MB proportional set size and the app process from 112 MB to 213 MB (idle versus after all cases, payload buffers included).

Startup: in the lab the webview reached DOMContentLoaded 1.94 s and its first animation frame 2.45 s after process start, while the Rust side was ready after 0.35 s. The roughly 1.6 s gap is webview process creation on this machine (cold, release build) and is outside the app's control; the app should show its shell chrome and skeleton immediately and load data afterwards.

### 4.4 Custom titlebar, drag and drop, DevTools

Custom titlebar (docs `learn/window-customization.mdx`): set `decorations: false`, add `data-tauri-drag-region` (applies only to the element itself, not its children) or call `startDragging` manually, and grant `core:window:allow-start-dragging` plus the window control permissions. Behaviour of undecorated windows on Linux window managers (resize borders, shadows, Wayland client side decoration quirks) is not covered by that page and was not tested (unverified). The community plugin `tauri-plugin-decorum` 1.1.1 was last released 2024-09 and is not recommended. Decision: use the native titlebar on Linux by default and a custom one on Windows and macOS only if the design needs it, behind one `Titlebar` component with a setting.

File drag and drop versus HTML5 drag and drop: Tauri's webview option `dragDropEnabled` (default true) enables the native handler that produces `DragDropEvent`s with file paths; the `tauri-utils` documentation states that disabling it is required for HTML5 drag and drop on Windows because Tauri replaces the WebView2 drag handler; `wry` repeats that on Windows this disables HTML drag and drop APIs like `draggable="true"` when the handler is on. Consequences for RimStudio: (a) dropping a folder or a mod archive from the file manager onto the window works only with `dragDropEnabled: true`, and then arrives as a Tauri event with absolute paths (not through the DOM `drop` event), which the frontend subscribes to with the webview API; (b) in-app reordering and drag between lists must therefore be implemented with pointer events, not HTML5 DnD (see the companion note); (c) the two never conflict: OS file drops use the Tauri event, in-app drags use the pointer engine, and pointer based dragging also behaves identically on all three engines. Blocking the browser's default file drop navigation (dropping a file on the page) is handled by the same native handler on Windows; on other platforms add a `dragover` and `drop` preventDefault guard at the document level. The community crate `tauri-plugin-prevent-default` 6.0.0 (2026-09-27) disables default browser shortcuts (reload, print, context menu) and can be considered for release builds.

DevTools in release builds: the webview inspector works in debug builds by default and needs the `devtools` Cargo feature of `tauri` to be enabled in release (documentation comment on the `devtools` config option in `tauri-utils` 2.10.1); on macOS it calls private APIs, so it must be off for any App Store build. Decision: ship release builds without it, and produce a "diagnostics" build or Cargo feature (`rimstudio-app/devtools`) that enables it, plus a hidden `--devtools` flag documented for bug reports; CrabNebula DevTools (`tauri-plugin-devtools` 2.0.0 per the docs) is an optional dev only tool for command timing.

## 5. Performance budget and UI rules

Budget (targets for release candidates, to be verified on a mid range machine; measured lab values in parentheses are on the research machine):

1. Cold start to visible shell (window chrome plus skeleton): under 1.5 s on the research machine class; the webview process alone took about 1.6 to 2.4 s in the lab, so the shell must paint with no data dependency.
2. Mod list snapshot (3000 rows, list columns) from invoke to first rendered rows: under 100 ms (lab transport 8 ms plus parse 4 ms).
3. Scan or sort progress: at most 20 progress messages per second, each below 8 KiB; UI updates at most once per animation frame.
4. Input to paint for list interactions (select, toggle, drag step): under 50 ms for 3000 rows; frame time under 16 ms during scroll and drag.
5. Rendered DOM: no list or table holds more than 3000 DOM nodes; whole page DOM under 10,000 nodes; every list over 100 rows is virtualised (lab: 27,000 nodes cost 876 ms layout).
6. IPC payloads: list snapshot under 2 MB; detail records fetched lazily; descriptions never in list payloads; file and image bytes only through raw responses or protocols; JSON for all structured data (R10).
7. Memory: webview process under 400 MB with a 3000 mod list and previews loaded (lab after a heavy bench: 347 MB proportional set size); total app under 600 MB.
8. Bundle: initial JavaScript under 300 KB gzip, each heavy feature (editor, graph, charts, designer) a lazily loaded chunk; fonts latin subset only.
9. Images: previews served through the custom image scheme as thumbnails of at most 256 pixels for lists (full size on demand), lazy and async decoded.
10. Commands under 1 ms of Rust work run inline; anything longer is `async` and cancellable (docs: asynchronous commands are preferred for heavy work to avoid freezes).
11. Channel and event counts per user action: one channel per long operation, no per row events.
12. Regression gates: the lab or an equivalent script runs in CI on one runner per OS family with these budgets as warnings first, failures once calibrated.

UI rules that follow:

1. Use `content-visibility: auto` with `contain-intrinsic-size` for long non virtualised content (settings pages, logs) and windowing for lists.
2. Prefer fixed row heights; measure only rows that really change height.
3. No `backdrop-filter`, large `box-shadow` stacks or filters on full panels or on every row; allowed on small overlays and only after testing on WebKitGTK with the DMABUF renderer off.
4. Animate only `transform` and `opacity`; respect `prefers-reduced-motion`.
5. Do not use WebGL, WebGPU or `SharedArrayBuffer`.
6. No `requestIdleCallback`; use the scheduler utility.
7. Avoid layout thrash in lists: no reading layout properties inside row render; one `ResizeObserver` per container.
8. Update through signals so a cell change touches one text node; never replace the entire rows array for a single delta.
9. Keep Tailwind class names static and use `data-*` variants for row state.
10. All colours come from tokens; light and dark are attribute switches (`data-theme`), user accent is one custom property.
11. Text selection, context menu and keyboard shortcuts must work without a mouse; every drag operation has a keyboard command.
12. Avoid the native browser context menu in release builds (custom menus), but keep it in the diagnostics build.

## Implications for RimStudio

1. All structured IPC payloads are JSON (R10). Raw byte responses and bodies are used only for opaque file and image data. A packed binary list format requires an explicit owner exception and is not needed: 3000 slim records cost about 8 ms as JSON in the lab.
2. The mod list uses snapshot plus delta: `list_mods_snapshot` returns list columns only, `subscribe_mods` streams `{rev, upserts, removes}` over a Channel, detail comes from `get_mod_detail(id)`; a revision gap triggers a re-snapshot. Test: a 5000 row snapshot round trip stays under 50 ms on the research machine.
3. Progress and result streams use Channels, coalesced to at most 20 messages per second, with chunks of 250 to 1000 records for result streams and a hard rule that individual JSON messages are either below 8 KiB or rare and large (threshold verified in `tauri` 2.12.1).
4. Every long operation has a `jobId`, a `cancel_job` command and a Rust side cancellation check; dropping the channel also cancels.
5. Preview images are served by an asynchronous custom URI scheme with thumbnails, caching headers and an id allow list; the asset protocol is limited to user chosen files with a narrow scope; no base64 images. `previewUrl(id)` hides the per OS URL form (`scheme://localhost` versus `http://scheme.localhost`).
6. Capabilities: one `main` capability with `core:default`, window control permissions only where a custom titlebar is used, `dialog` open permission and a scoped `opener`; no `fs`, `shell` or `http` plugin permissions for the frontend; app commands restricted through `AppManifest::commands`; CSP configured with `connect-src` limited to `ipc:`.
7. Isolation stays off in the first release unless a lab run shows under 1 ms added per command and over 100 MB per second for raw payloads; record the measurement in the repository.
8. Typed bindings come from `tauri-specta =2.0.0-rc.25` with `specta-typescript =0.0.12`, written to a committed file with a regeneration diff check in CI; DTOs live in a Tauri free crate; the fallback (ts-rs 12.0.1 plus a typed invoke map) is documented and kept compatible by using plain serde structs; Steam and workshop ids travel as strings.
9. Set `bundle.macOS.minimumSystemVersion` explicitly (13.3 or newer) and run a startup feature probe for `@layer`, `@property`, `color-mix()`, container queries and `:has()` that shows a clear "system webview too old" page instead of a broken UI.
10. Linux graphics workarounds are user controlled through a JSONC launch file read before webview creation, a `--safe-graphics` flag and a crash marker that proposes safe mode; no unconditional environment overrides.
11. In-app drag and drop uses pointer events; Tauri `dragDropEnabled` stays true so OS folder and file drops arrive as Tauri events with paths; document that HTML5 drag attributes are not used.
12. Release builds exclude DevTools; a diagnostics build (Cargo feature) includes them, and the diagnostics page shows webview engine version, session type and graphics variables.
13. Adopt the numbered budget in section 5 as automated checks (warnings first), and re-run the committed lab on Windows and macOS before fixing the numbers.

## Open questions

1. Isolation pattern cost was not measured; run the lab with `--features iso` on 3000 row snapshots and channel streams.
2. Windows (WebView2) and macOS (WKWebView) numbers are missing; the IPC cost ratios may differ (WebView2 uses a different custom protocol URL form and a different drag handler).
3. How are `tauri::ipc::Response` commands best typed with tauri-specta 2.0.0-rc.25 (no mention found in its README or source)?
4. Which WebKitGTK release corresponds to Safari 16.4 for the "too old" message wording, and should Debian 12 and Ubuntu 22.04 with their updated 2.50 builds be declared supported? (Repology lists multiple versions per repository; the version a given user has depends on update status.)
5. Behaviour of an undecorated Tauri window on GNOME and KDE Wayland and X11 (resize borders, shadow, snapping) for the optional Linux custom titlebar.
6. Does the packed list format ever pay off for the def explorer (tens of thousands of defs), where payloads are larger than mod lists? If yes it needs an R10 decision.
7. Scroll behaviour on 120 or 144 Hz displays and on integrated GPUs was not measured (the lab display was 60 Hz); blur and shadow costs there remain open.
8. Can the Tauri `devtools` feature be toggled at runtime in a release binary through a CLI flag only, or does it need a separate build? (The config option documentation says the feature flag is required for release builds.)
