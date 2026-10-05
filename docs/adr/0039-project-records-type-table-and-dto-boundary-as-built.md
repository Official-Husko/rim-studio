# ADR 0039: Project records as documents, the run time type table and the DTO boundary, as built

Status: accepted | Last updated: 2026-10-05 | Register: D-091, D-092, D-093

## Context

The backend of release 0.1.0 was built against the specifications and three of them turned out differently from the text. The project store was specified as one JSONC file per project, which a person is not expected to edit while the document store of ADR 0035 already gives typed, migrated, quarantined documents. The def type table was proposed as an open spike (S-07). The manager and the toolkit were both specified as depending on the DTO crate, but the manager works over engine types that have no serde form.

## Decision

1. Project records are documents of the collection `projects` in the data root (kind `project`, version 1; `ProjectRecord { id, path, name, package_id, created_ms, last_opened_ms, target_version, designer, extra }`). The id is `p-<8 hex>` derived from the normalised folder path. The last designer settings are an opaque JSON value. The opt in `.rimstudio/project.jsonc` and the import of the legacy `Config/rimstudio.project.json` are deferred and are not part of 0.1.0. Upload ignore patterns and publish settings move into the record when the publisher is built.
2. The def type table is built at run time by `rimstudio-workspace` from the game's managed assemblies and every DLL in the reference mods' `Assemblies` folders, in parallel, and cached as a collection document under the cache root keyed by a hash of the DLL path list and validated by stat keys. Spike S-07 passes for vanilla and the DLC (0 unknown types). The few classes of the Combat Extended assembly that the designer reader needs are added by `rimstudio-design::ce::reader::with_ce_types`; the shared engine keeps `MakeGunCECompatible` unknown unless a caller registers the operation (D-020 holds, the CE class strings exist only in `rimstudio-design::ce`).
3. The manager returns engine types and the application converts them to DTOs, so `rimstudio-manager` does not depend on `rimstudio-ipc-types` (requests and responses are named `{Name}Request` and `{Name}Response` in the manager). The toolkit uses the designer, defs and project DTOs of `rimstudio-ipc-types` directly in its function signatures, as the catalog said.

## Consequences

- Project records are migrated, backed up and quarantined like drafts; a person who wants to edit project metadata by hand has no JSONC file until the opt in form is built.
- `ProjectId` is not the lowercased package id of earlier drafts of the specification (core ids disallow dots).
- A new type table document is written for each distinct set of DLL paths and is not pruned.
- The app layer owns the mapping from manager results (detection report, sources, scan counts) to DTOs and must test that every error code is in the DTO registry.

## Alternatives rejected

- A JSONC project file per project in the data root: a second storage shape for the same kind of data.
- Shipping a pre generated type schema: it would be data derived from the game's assemblies (R11).
- Making the manager depend on the DTO crate: engine types such as the library index have no serde form.

## Evidence

- [ADR 0035](0035-in-house-json-database.md), [ADR 0009](0009-def-type-table-and-two-tier-def-data.md), [ADR 0004](0004-no-feature-edges-and-toolkit-registry.md)
- [Crate catalog](../architecture/crate-catalog.md) rimstudio-workspace, rimstudio-manager
- [Data and persistence](../architecture/data-and-persistence.md) sections 3.2, 3.3 and 16
- [Roadmap](../roadmap.md) section 2.1 and the spike table
