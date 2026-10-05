# ADR 0006: XML boundary crate and byte-span editing

Status: accepted | Last updated: 2026-10-04 | Register: D-012, D-013

## Context

R10 allows XML only when reading or writing RimWorld's own files. Mod authors hand-edit About.xml, LoadFolders.xml and patch files, so rewriting a whole file destroys comments and layout. Serde derive on About.xml breaks on the tolerance the game itself shows. Transitive XML users exist in the Tauri tree (the plist crate pulls quick-xml).

## Decision

`rimstudio-xml` on quick-xml 0.42.0 is the only place that touches XML text. It offers Game and Tolerant parse modes producing node trees, a renderer from node trees to XML, codecs for About, LoadFolders, ModsConfig and save metadata, and a streaming def indexer. Existing RimWorld files are edited by byte-span splice through a `SpanEditor`, so untouched bytes survive; full rendering is used only for new files and templates. Every other XML crate is banned through cargo-deny wrappers whose list names `plist`, and no frontend XML library is allowed.

## Consequences

- Spike S-09 must confirm the wrapper lists work with workspace wrappers and the Tauri tree.
- Byte-preservation tests are mandatory for every edit operation.
- Defs, design and rules stay XML-free, which keeps them testable with JSON vectors.

## Alternatives rejected

- roxmltree DOM everywhere: a second stack and 2.8 times slower (index build 346 ms against 125 ms).
- serde derive for About.xml: fails on tolerant cases.
- Regenerating whole files on every save.

## Evidence

- [Scan performance spike](../research/scan-performance-spike.md) S3
- [Rust crate research](../research/rust-crate-research.md) section 4.3
- [Mod format and corpus](../research/rimworld-mod-format-and-corpus.md) parser checklist
