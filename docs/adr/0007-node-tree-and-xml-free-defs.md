# ADR 0007: Node tree in core and an XML-free defs engine

Status: accepted | Last updated: 2026-10-04 | Register: D-014, D-016, D-017, D-020

## Context

The def engine research prototyped merge, patch, inheritance and instantiation over a JSON node tree with 38 vectors and 13 traces. Attribute order is observable after an `AttributeAdd`. Frameworks add custom patch operation classes that an engine cannot simulate, and Combat Extended has 12 def overrides, 9 of them type changes.

## Decision

The shape `{tag, attrs: [[name, value]], children: [node|string]}` with ordered attributes is defined in `rimstudio-core::tree`, with an arena view for XPath. `rimstudio-defs` operates only on node trees (merge, patch operations, inheritance, instantiate, type table, DefDatabases) and has no XML dependency, so the prototype vectors and traces run unchanged as integration tests. Provenance is kept at three levels: the def, the patch event and the element origin, and cross-mod overrides are exposed as data. Unknown and custom patch operation classes are parsed generically, carried raw and marked not simulated. `MakeGunCECompatible` stays an unknown operation in defs; the designer reads it with a typed reader in `design::ce`.

## Consequences

- File reading and parsing live in `workspace` and `xml`.
- The memory cost of element origins must be measured (spike S-08).
- The designer carries about 100 lines of its own merge logic for the Combat Extended operation.

## Alternatives rejected

- A public arena or roxmltree types as the shared tree.
- An XML dependency inside defs.
- Re-implementing Combat Extended's operation inside defs.
- Def-level provenance only.

## Evidence

- [Def engine semantics](../research/def-engine-semantics.md) implications 1, 4 and 7
- [Ecosystem survey](../research/ecosystem-survey.md) implication 9
- [Combat Extended model](../research/combat-extended-model.md) implications 1 and 7
