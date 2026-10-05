# ADR 0010: Two-phase scan, file watching and allocator policy

Status: proposed | Last updated: 2026-10-04 | Register: D-021, D-028, D-069

## Context

The workshop tree alone has 59,348 directories and the whole library 306k files. A list ready quickly matters more than anything else in a mod manager. inotify needs one watch per directory.

## Decision

Scanning is two-phase with a purpose-built rayon recursion over `read_dir`, targeted folders only, at most 8 workers (one core kept free), no symlink following, and scan errors reported as data. Measured: list ready in 3.7 ms of scan work, walkdir 8.7 times slower. Watching covers roots and metadata files only, debounced 300 to 500 ms, with a poll fallback on `ENOSPC`; recursion is allowed only for the active project; focus and demand trigger rescans. The system allocator is used until a measurement (spike S-10) shows a benefit for mimalloc or jemalloc; indexes use interned `u32` keys. Benchmarks gate regressions at 2 times.

## Consequences

- Allocator choice remains provisional (proposed) because only the system allocator was measured; there was an 18 MB difference between 1 and 16 thread holds.
- Windows and macOS numbers are missing until S-10.

## Alternatives rejected

- walkdir or jwalk over the whole tree.
- Recursive watching of every mod.
- Adopting mimalloc now.

## Evidence

- [Scan performance spike](../research/scan-performance-spike.md) implications 1 to 3, 8, 9 and 11
- [Rust crate research](../research/rust-crate-research.md) section 5.3
- [Performance strategy](../architecture/performance-strategy.md)
