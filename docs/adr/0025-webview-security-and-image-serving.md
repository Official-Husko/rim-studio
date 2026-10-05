# ADR 0025: Webview security posture and image serving

Status: accepted | Last updated: 2026-10-04 | Register: D-047, D-048

## Context

The webview must not become a general file or process interface; a mod manager handles user paths and downloaded data. Mod lists show thousands of preview images, and base64 payloads would bloat IPC.

## Decision

One `main` capability with no `fs`, `shell`, `http`, `store`, `sql` or `log` permission; app commands are restricted through `AppManifest::commands`; every command taking a path passes `RootGuard`; the CSP limits `connect-src` to `ipc:`; the asset protocol serves only user-chosen files. Isolation stays off in v1 unless the lab shows under 1 ms per command and over 100 MB per second (S-10). Images use an asynchronous custom scheme `rsimg` with thumbnails up to 256 px, cache headers and an id allow list; `previewUrl(id)` hides the per-OS URL form; no base64 lists. Thumbnails are cached in the cache root.

## Consequences

- The frontend can only do what the registry exposes.
- Transport time is similar across image options, but lazy loading, HTTP caching and off-thread decoding favour a scheme.
- Isolation cost is measured later.

## Alternatives rejected

- `fs` plugin scopes.
- Isolation always on.
- Broad asset protocol; base64 lists.

## Evidence

- [Webview and IPC performance](../research/webview-and-ipc-performance.md) implications 6 and 7 and section 2.5
- [Rust crate research](../research/rust-crate-research.md) section 3.3
- [Security and privacy](../architecture/security-and-privacy.md)
