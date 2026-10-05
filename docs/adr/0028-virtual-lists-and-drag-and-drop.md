# ADR 0028: Virtualised lists and drag and drop

Status: accepted | Last updated: 2026-10-04 | Register: D-053, D-054

## Context

Mod lists reach thousands of rows; the lab measured 27,000 DOM nodes costing 876 ms of layout. HTML5 drag and drop collides with Tauri's native file-drop handler on Windows. RimCrow carries two virtualisers.

## Decision

One windowing hook over `@tanstack/virtual-core` 3.17.11 serves every list over 100 rows, with fixed row heights and list DOM under 3000 nodes (there is no official Preact adapter, so the hook is ours). In-app drag and drop uses pointer events behind `shared/dnd` (`@dnd-kit/dom` 0.5.0 behind it, an own pointer engine as fallback); Tauri `dragDropEnabled` stays true for OS file drops; every drag has keyboard and menu equivalents. Spike S-05 (drag between two virtualised lists with five selected rows at 16 ms frames) gates the choice before M2 builds the list UI.

## Consequences

- If S-05 fails, the fallback engine costs extra time in M2.
- Keyboard parity is part of the accessibility baseline.

## Alternatives rejected

- Two virtualisers.
- Unwindowed lists.
- HTML5 drag and drop libraries.

## Evidence

- [Frontend stack research](../research/frontend-stack-research.md) section 4.2
- [Webview and IPC performance](../research/webview-and-ipc-performance.md) section 5
- [Mod manager spec](../features/mod-manager.md)
