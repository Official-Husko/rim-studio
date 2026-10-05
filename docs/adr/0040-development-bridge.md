# ADR 0040: Development bridge for the browser test UI

Status: accepted | Last updated: 2026-10-05 | Register: D-103

## Context

The owner wants a temporary, component based, dark themed UI to test the weapon designer end to end with the numbers of a real install. The desktop shell needs the Tauri webview toolchain and a desktop session; the test UI is developed and reviewed in a normal browser, where `invoke` does not exist. A mock backend in the page could not show real numbers, and the backend is already complete behind one registry that the CLI calls through `rimstudio-app` (ADR 0003).

## Decision

1. A new crate `rimstudio-devserver` (layer `l4-shell`, binary `rimstudio-devserver`) boots the same `AppContext` through `rimstudio-app::boot` and serves the registry over HTTP on 127.0.0.1. The wire contract is "bridge API v1" in [IPC and state](../architecture/ipc-and-state.md) section 14: `POST /rpc/COMMAND`, a Server-Sent Events stream of job progress, `/dev/info`, `/dev/commands`, `/dev/health` and a read only folder listing for the folder picker.
2. The server is hand written on `std::net` (a small HTTP/1.1 parser with hard limits, a thread per connection, a connection cap). The crate has no HTTP framework, no async runtime of its own and no new third party dependency; it uses the workspace's `serde_json`, `tracing` and the `rimstudio-app` and `rimstudio-ipc-types` crates, which the matrix row `l4-shell` allows.
3. Access control: loopback peer, `Host` and `Origin` checks, a per run random token in the header `x-rimstudio-token` compared in constant time, and the content type `application/json` for every POST. The token is printed at start and written to a token file under `node_modules/.cache`.
4. The bridge boots the app with its own data roots (`BootInput::with_data_base`, a small additive hook in `rimstudio-app`: the four roots under `base/{config,data,cache,logs}` in portable style), by default `$HOME/.local/share/rimstudio-dev`, so it never mixes with the CLI's folders.
5. It is a development tool: it is never packaged, no installer or release job includes it, the shell does not depend on it and the frontend reaches it only through `shared/ipc`. `xtask/layers.jsonc` needs no change (the row `l4-shell` already allows its edges); `xtask/source-allow.jsonc` allows the crate to print its start banner and to write its token file.

## Consequences

- The browser UI can run the real designer, the Combat Extended suggestions and the file generation against the owner's install, and the numbers on screen are the backend's.
- Anyone who can run a process as the same person can read the token file; the threat model is a hostile web page, not a hostile local process.
- SIGINT cannot be handled with std alone (the workspace forbids unsafe code), so Ctrl+C ends the process without the clean shutdown mark; a typed `quit` stops cleanly.
- The bridge executes commands that write files (apply plan). It therefore refuses every request that is not from the allowed origins with the token, and the write path keeps its own fences (D-098).
- `EventSource` cannot send headers, so the client reads the event stream with `fetch`.

## Alternatives rejected

- Running the test UI in a Tauri window: needs the webview toolchain and a desktop session, and ties UI review to the shell milestone.
- A mock backend in the browser: cannot show real numbers, which is the point of the exercise (D-100).
- An HTTP framework such as axum or tiny_http: a dependency and a runtime for a loopback tool with six endpoints; the layer matrix and the third party bans would also need new entries.
- A token in the query string so that `EventSource` works: tokens in URLs end up in logs and history.

## Evidence

- [IPC and state](../architecture/ipc-and-state.md) sections 2, 5, 7 and 14
- [Security and privacy](../architecture/security-and-privacy.md) sections 2, 4.4 and 5
- [Workspace layout](../architecture/workspace-layout.md) section 4
- [ADR 0003](0003-one-command-registry-and-app-root.md) and [ADR 0025](0025-webview-security-and-image-serving.md)
