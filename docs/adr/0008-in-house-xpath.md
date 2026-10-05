# ADR 0008: In-house XPath 1.0 engine

Status: accepted | Last updated: 2026-10-04 | Register: D-015

## Context

Patch operations select nodes with XPath. The corpus study measured how real patches use it: a restricted subset covers the vast majority. Existing Rust options are unmaintained (sxd-xpath, last release 2018) or implement XPath 3.1 semantics (xee), and neither works over RimStudio's own node tree without conversion.

## Decision

`rimstudio-xpath` is an XPath 1.0 parser and evaluator over the arena document with index hints for common shapes. It is graded by corpus coverage gates: tier B at 97 percent and tier C at 99.8 percent of real occurrences, with a full 1.0 fallback for the remainder. No oracle crate is linked; oracle results (libxml2 and Mono behaviour) are precomputed as hashes in test vectors. Document-context semantics are followed, including the open question about union-with-parent where Mono and libxml2 disagree.

## Consequences

- We own a parser and evaluator, a standing maintenance burden.
- Coverage gates fail the build if a change lowers the measured corpus coverage.
- The Mono versus libxml2 disagreement stays a recorded open question.

## Alternatives rejected

- sxd-xpath: unmaintained since 2018.
- xee: XPath 3.1 semantics, heavier, wrong tree.
- skyscraper.
- Converting to roxmltree for evaluation: second stack.

## Evidence

- [XPath patch coverage](../research/xpath-patch-coverage.md)
- [Def engine semantics](../research/def-engine-semantics.md)
- [Rust crate research](../research/rust-crate-research.md)
