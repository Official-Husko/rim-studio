# ADR 0036: Release 0.1.0 scope, backend first, and the minimal manager crate

Status: accepted | Last updated: 2026-10-04 | Register: D-084

## Context

The roadmap puts the manager first and the designer fifth, but the owner wants the item designer first. The designer needs only a small part of the manager (settings, sources, custom folders, detection) and a slice of the toolkit foundation. The interface design is not finished, and the architecture already makes the CLI run the same handlers as the GUI.

## Decision

Release 0.1.0 is the designer slice: a weapons editor (ranged and melee) with the specified math, simple and quiz modes, the fit meter and the write plan, plus a Combat Extended patch generator that is opt in per item (D-085): new weapons, the Convert flow for an existing mod's weapons, update mode and lint. The Rust backend is built first and the CLI is the interim front end until the UI exists. The apparel editor, the full manager, datasets and publishing follow later. The slice is built on M0, the parts of M1 it needs, the toolkit and workspace slice of M4 and the weapons part of M5; the milestone order itself does not change. A minimal `rimstudio-manager` crate (settings and sources: custom folders, detection) belongs to the slice because the designer needs the reference set and the custom mod folders. The roadmap section "0.1.0 release (designer slice)" holds the scope, exit criteria and deferred items.

## Consequences

- Exit criteria are decidable from the CLI and tests, with no UI work.
- `rimstudio-manager` starts small; undo, sorting, profiles and the list sessions arrive in M2 and M3 without changing its public shape.
- The status page `docs/status/0.1.0-backend.md` tracks each requirement group.
- Deferred rows stay in the specifications and in the roadmap "Later" table, so nothing is lost.

## Alternatives rejected

- Wait for the full manager: delays the owner's priority by about 22 weeks.
- Build the UI first: the design is not final and the backend can be proven by tests.
- Put settings and sources in `rimstudio-toolkit`: the manager owns them in the catalog and later milestones need them there.

## Evidence

- [Roadmap](../roadmap.md) section 2.1
- [Workspace layout](../architecture/workspace-layout.md) and [crate catalog](../architecture/crate-catalog.md)
- [ADR 0003](0003-one-command-registry-and-app-root.md)
