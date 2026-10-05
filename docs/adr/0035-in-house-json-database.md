# ADR 0035: In house JSON database and the aux-db exception

Status: accepted | Last updated: 2026-10-04 | Register: D-083, D-031

Update 2026-10-05: implemented as built in `rimstudio-io::collection`; layout, quarantine and API names are in [data and persistence](../architecture/data-and-persistence.md) section 16.

## Context

R10 says app data is JSON and configs are JSONC. Several features need many small records that are saved, listed and migrated independently: designer drafts, projects, calibration caches and, later, profiles, notes and history. A single settings file does not fit them, and the owner ruled out SQL and any new storage engine. RimSort keeps notes and colours in a SQLite file that the RimSort import wants to read once.

## Decision

`rimstudio-io` gains a directory based document store, the in house JSON database: one JSON document per id, a versioned envelope `{kind, v, data}`, atomic writes (temp file then rename, verified backup), forward only migrations per kind, a compact index file for listing without opening every document, and quarantine of unreadable or newer documents instead of failure. The typed entry point is `collection::Collection<T>`. Settings stay one JSONC file edited through CST operations. Designer drafts, projects, calibration caches and later profiles, notes and history are collections. No SQL and no new storage engine are introduced.

A dependency that only reads a foreign format is acceptable. D-031 is therefore resolved as accepted: read only `rusqlite` behind the Cargo feature `aux-db` in `rimstudio-datasets`, used only to import RimSort's `aux_metadata.db` into RimStudio's own JSON, never for app data, and still off by default until the datasets crate exists. The cargo-deny wrapper exception names that crate and feature.

## Consequences

- One mechanism for every record type: atomic writes, migrations and quarantine are written and tested once.
- Listing is cheap through the index; the index is derived and rebuilt from the documents when missing or stale.
- Cross document queries are not supported; a feature that needs them keeps its own derived cache.
- The ban list keeps rejecting SQLite crates everywhere except `rimstudio-datasets` with `aux-db`.

## Alternatives rejected

- SQLite or another embedded SQL engine for app data: breaks R10 and the owner's rule.
- A key value engine (sled, redb): a new storage engine and a binary format.
- One large JSON file per collection: rewrites everything on each save and loses documents together on corruption.

## Evidence

- [Data and persistence](../architecture/data-and-persistence.md) section 16
- [Scan performance spike](../research/scan-performance-spike.md) implication 7 (JSON caches)
- [ADR 0013](0013-jsonc-cst-atomic-writes-and-schemas.md) and [ADR 0015](0015-rimsort-aux-database-import.md)
