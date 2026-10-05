# ADR 0023: Snapshot plus delta streaming and the jobs model

Status: accepted | Last updated: 2026-10-04 | Register: D-044, D-045

## Context

A 5000-row snapshot costs 14 ms over IPC, but refetching the list after every edit would waste it. Operations over about 1 ms of work must not block a command. The CLI needs the same long-running behaviour without a webview.

## Decision

The mod list uses a snapshot plus revisioned deltas `{rev, upserts, removes, order}` on a long-lived channel; sort results are `ModIdx` arrays; paging is used only for the Def Explorer, search and logs. Chunks hold 250 to 1000 records, messages stay under 8 KiB or are rare, at most 20 messages per second are sent and the UI batches in requestAnimationFrame. Long work is a job: a caller-minted `JobId`, a registry in `rimstudio-app::JobRunner`, a progress channel, `cancel_job`, cancellation when the channel drops, and an `AtomicBool` token checked between units of work. The CLI renders the same progress records.

## Consequences

- Windows and macOS numbers are missing until S-10.
- Any command over 1 ms of Rust work must be declared a job; a lint in review checks this.
- A binary packed format was rejected because it would need an R10 exception for milliseconds of gain.

## Alternatives rejected

- Refetch after every edit.
- A packed binary list format.
- Per-feature job systems.

## Evidence

- [Webview and IPC performance](../research/webview-and-ipc-performance.md) sections 2.3 and 2.4 and implication 4
- [Reference architectures](../research/reference-architectures.md) implication 9
- [IPC and state](../architecture/ipc-and-state.md)
