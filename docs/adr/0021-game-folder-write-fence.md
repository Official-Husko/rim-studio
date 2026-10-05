# ADR 0021: Game-folder write fence and ModsConfig protocol

Status: proposed | Last updated: 2026-10-04 | Register: D-040, D-041

## Context

The game silently deactivates active mods it cannot find and rewrites ModsConfig.xml. Other managers write freely, which has corrupted setups. Writing while the game runs is unsafe.

## Decision

A `GameWriteFence` in `rimstudio-io` allows only two kinds of writes under the install or config folder: owned link-farm entries and `ModsConfig.xml`. Before every write and launch a timestamped backup is made; the `version` field equals `Version.txt`; ids are lowercase with `_steam` where intended; `knownExpansions` is kept; after the game exits the file is diffed; a blocking warning appears when the game would deactivate an active id. A running game is detected by process name through `ProcessProbe`, with Steam's `AppRunning` flag as a second signal (proposed; reported unknown in sandboxes). Executable names on Windows and macOS are unverified.

## Consequences

- A `RecordingFs` test proves no other path is written.
- Users see why a write was blocked.
- The fence is also the place where portable and test modes redirect.

## Alternatives rejected

- Writing freely like other managers.
- A lock file only for running detection.

## Evidence

- [Mod format and corpus](../research/rimworld-mod-format-and-corpus.md) implications 2 and 3
- [Cross-platform packaging](../research/cross-platform-packaging-research.md) implication 9
- [Security and privacy](../architecture/security-and-privacy.md)
