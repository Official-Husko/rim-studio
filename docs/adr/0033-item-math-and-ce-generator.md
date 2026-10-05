# ADR 0033: Item math and Combat Extended generator placement

Status: accepted | Last updated: 2026-10-04 | Register: D-064, D-065

Update 2026-10-05: implemented in `rimstudio-design` and `rimstudio-toolkit::designer`; the as built notes are in [Combat Extended patching](../features/combat-extended-patching.md) section 14 and [items toolkit](../features/items-toolkit.md) section 15.

## Context

R7 asks for an item designer with vanilla references, CE compatibility and patch generation, and a mathematical way to judge fit. R11 forbids copying CE or vanilla value tables into the repository.

## Decision

Item math is the pure crate `rimstudio-design`. All coefficients and class tables are derived at run time from the user's install and cached as JSON keyed by a CE version hash; nothing from CE or vanilla value tables is in the repository; test vectors use fictional numbers; with CE absent the designer degrades with an explanation. The CE reader, class statistics, formulas, patch generator and CEP lint live in the `design::ce` module and produce JSON node trees from RimStudio-authored templates; orchestration with write plans is in `toolkit::designer`; XML is rendered only by `rimstudio-xml`. Output follows CE's hand-tuned forms (`MakeGunCECompatible` for guns, plain operations otherwise) inside a `LoadFolders.xml` folder gated by `IfModActive`; existing conversions switch to update mode.

## Consequences

- A separate `rimstudio-ce` crate is split off only when a second consumer appears (trigger in the catalog).
- The calibration quiz and simple mode share the same baseline model.
- Real install checks are `#[ignore]` tests.

## Alternatives rejected

- Embedding presets.
- The generator inside the xml crate (blurs the boundary).
- A separate CE crate now.

## Evidence

- [Combat Extended autopatcher formulas](../research/ce-autopatcher-formulas.md) implications 1, 6 and 9
- [CE patch conventions](../research/ce-patch-conventions.md) implications 1, 2, 4, 5 and 11
- [Combat Extended model](../research/combat-extended-model.md)
