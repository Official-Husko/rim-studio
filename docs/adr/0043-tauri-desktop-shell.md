# ADR 0043: The Tauri desktop shell with live reload

Status: accepted | Last updated: 2026-10-05 | Register: D-115, D-116

## Context

ADR 0003 fixed the shape of the shell: a thin Tauri crate over the Tauri free composition root of `rimstudio-app`. Until now the UI ran only in a browser behind the development bridge (ADR 0040), so there was no native folder dialog, no window and no in process backend. The owner asked for the shell next, with live reload for both halves of the stack.

## Decision

1. **Crate.** `rimstudio-shell` lives at `apps/desktop/src-tauri` (layer `l4-shell`, a workspace member), with a library of the same name and the binary `rimstudio`. It depends on `rimstudio-app`, `rimstudio-ipc-types`, Tauri 2.12.1, `tauri-plugin-dialog` 2.8.1 and `tauri-plugin-opener` 2.7.0 (exact pins in the root manifest). It boots the application context once with the real platform data roots and the rolling log file, and keeps it in managed state.
2. **Commands.** The webview gets four commands: `rs_call(name, request, jobId?)` (a query, an action or a job; a job resolves when it ends, like the bridge), `rs_cancel(jobId)`, `rs_info()` and `rs_commands()`. Job progress and job end arrive as the Tauri event `rs-job` with the JSON of the bridge's `/dev/events`, so the frontend job store reads both transports with the same code. A failure is the `ApiError` envelope, unchanged. The command layer is plain functions over `AppContext` and is tested without a window.
3. **Frontend.** `shared/ipc` gets a real `tauri` transport over `invoke` and `listen` (it is the only module besides `shared/platform` that imports `@tauri-apps/*`, and only the core and event APIs). `shared/platform` implements `pickFolder`, `pickFile({filters})`, `revealPath` and `openUrl` with the native dialog and the opener in the shell and with the bridge based folder browser in a browser, and exports the capability probe `isDesktop()`. Transport selection at boot: Tauri when `window.__TAURI_INTERNALS__` exists, else the bridge, else the mock. The connection chip reads Desktop or Bridge ok.
4. **Security.** A strict content security policy (`default-src 'self' ipc:`, no remote origin; the development policy adds only the Vite origin and its websocket), one capability file for the `main` window (`core:default`, the four app commands, `dialog:allow-open`, `opener:allow-reveal-item-in-dir`, `opener:allow-open-url` limited to `https`), no `fs`, `shell` or `http` permission, app commands restricted through `AppManifest::commands`, the window created in Rust with a navigation policy that refuses every address that is not the bundled frontend (or, in a debug build, the dev server) and refuses every request for a new window. DevTools exist in debug builds; a release build has them only with the `diagnostics` feature. `freezePrototype` stays off: with it on, the bundled libraries fail at start (a library assigns to a prototype) and the window stays blank.
5. **Live reload.** `pnpm tauri:dev` runs the Tauri CLI with Vite as the frontend dev server (hot module reload of the UI) which watches the shell folder and (verified in its start log) the folder of every workspace crate in the shell's dependency graph, so an edit under `crates/rimstudio-*` rebuilds and restarts the app with no extra configuration. `pnpm tauri:build` makes a debug build without bundles, `pnpm tauri:check` runs cargo check and clippy, `pnpm tauri:smoke` runs the headless self test (`rimstudio --smoke`: boot, `app_ping`, `detect_get_report`, the command list, exit code 0 or 1).
6. **Test data.** `RIMSTUDIO_DATA_BASE` puts the four data roots under one folder (the same hook as the bridge's `--data-dir`), for tests, smoke runs and side by side development.
7. **Identity.** Identifier `app.rimstudio.desktop` (the placeholder of D-049), product name RimStudio, bundling inactive until the packaging milestone, an icon set generated from a Blueprint style mark kept as `icons/rimstudio-mark.svg`.

## Consequences

- The development bridge stays for browser work and the end to end suite; the shell does not depend on it and duplicates the 25 lines that map a job event to JSON (a shared helper in `rimstudio-app` would let both use one function; deferred because the app crate is shared by several tasks).
- The dialog plugin pulls `tauri-plugin-fs` as a library; the shell never registers it, so no file system command exists, and `deny.toml` lists the dialog plugin as its only wrapper.
- `rs_call` runs every command on the blocking pool, so a long query never blocks the window.
- On Windows a release build is a GUI program: `--smoke` reports only through its exit code there.

## Alternatives rejected

- Tauri channels per job instead of one broadcast event: the job store is keyed by job id already and the bridge uses one stream; one event keeps the two transports identical.
- Letting the webview open links itself: the navigation policy would have to allow a second window, which defeats it.
- Generating the command wrappers from the registry (as ADR 0003 sketches): four commands carry every registry row, so there is nothing to generate; the capability coverage reduces to four permission names.

## Evidence

- [ADR 0003](0003-one-command-registry-and-app-root.md), [ADR 0040](0040-development-bridge.md)
- [IPC and state](../architecture/ipc-and-state.md) sections 2.3, 8.2 and 14
- [Security and privacy](../architecture/security-and-privacy.md) section 4
- [Cross platform packaging research](../research/cross-platform-packaging-research.md), [webview and IPC performance](../research/webview-and-ipc-performance.md)
