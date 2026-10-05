# ADR 0015: Read-only import of the RimSort aux database

Status: accepted | Last updated: 2026-10-04 | Register: D-031, D-083

## Context

RimSort keeps per-mod notes and colours in a SQLite file (`aux_metadata.db`). Requirement R6 asks for import of RimSort user data and R10 forbids SQLite for app data. Reading a foreign SQLite file is not the same as using SQLite internally, but it adds a dependency that the deny policy would otherwise reject. The owner accepted a read-only dependency on 2026-10-04 (D-083, [ADR 0035](0035-in-house-json-database.md)).

## Decision

Accepted: read the foreign file with `rusqlite` behind the Cargo feature `aux-db` in `rimstudio-datasets` only, opened read-only, as a one-time foreign-format import into RimStudio's own JSON. No SQLite for app data, ever. The feature stays off by default until the datasets crate exists and is switched on for builds that offer the RimSort import. Without the feature, notes and colours import is skipped and the rest of the RimSort import (user rules, mod lists) proceeds unchanged.

## Consequences

- A named exception in the cargo-deny wrapper list, visible in review.
- The rest of the product never links SQLite, so R10 holds for app data.
- Without the feature, users can still copy notes by hand; this is documented.

## Alternatives rejected

- Skip notes and colours entirely: loses data users care about.
- Shell out to an external `sqlite3` binary: not available on Windows by default and hard to sandbox.

## Evidence

- [Rules fetch and merge design](../research/rules-fetch-and-merge-design.md) section 8
- [RimSort settings catalogue](../research/rimsort-settings-catalog.md)
- [Decision register](../architecture/decision-register.md) D-031
