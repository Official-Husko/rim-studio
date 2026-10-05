# ADR 0011: JSON-only caches and the cache key

Status: accepted | Last updated: 2026-10-04 | Register: D-022, D-023

## Context

R10 forbids internal binary formats and SQLite unless measured necessary. The spike compared rkyv, bincode, postcard, SQLite and compact JSON for the scan manifest. Filesystems differ in mtime granularity (FAT rounds to 2 s).

## Decision

All caches are JSON in a compact string-table layout; no binary cache, no SQLite and no zstd until measured. The per-file key is `(relative path, size, mtime ns, file id)` compared for equality only; FAT-family volumes round mtime to 2 s and fall back to a blake3 content hash. One manifest per library id lives in the cache root and is invalidated by manifest version, parser version, game version or load-rule change; corrupt files are treated as absent. Caches live only in the cache root and are always deletable. Revisit if manifest load exceeds 250 ms on a library ten times larger.

## Consequences

- Compact JSON loads in 21 ms; the best binary result saves 14 ms of a sub-100 ms warm start.
- A split per mod is considered only if write cost is measured.
- Cache deletion is always safe (invariant I-17).

## Alternatives rejected

- rkyv, bincode, postcard: tiny gain, R10 exception.
- SQLite: gain not measured, R10.
- mtime only; content hash always; caches beside user data.

## Evidence

- [Scan performance spike](../research/scan-performance-spike.md) implication 7 and cache design
- [Cross-platform packaging](../research/cross-platform-packaging-research.md) implication 8
- [Data and persistence](../architecture/data-and-persistence.md)
