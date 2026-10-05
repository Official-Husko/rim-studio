# ADR 0037: Vanilla by default, Combat Extended as an optional patch

Status: accepted | Last updated: 2026-10-04 | Register: D-085, D-064, D-065

## Context

The first design of the item designer offered three equal modes (Vanilla, Both and CE) and proposed Both as the default when Combat Extended is installed. The owner ruled that the designer always writes vanilla definitions and that Combat Extended is only an optional patch the user chooses.

## Decision

The designer always writes vanilla definitions. A Combat Extended patch is opt in per item through an off by default toggle "Add a Combat Extended patch (optional)". It is never enabled automatically, even when CE is installed, and is never mixed into the vanilla definition. Patch files go into their own files inside a folder gated by `LoadFolders.xml` (`IfModActive` on CE's package id), so the mod stays valid for players without CE. Converting an existing item to CE stays the Convert flow and only ever produces patch files; update mode edits previously generated patch files. With CE absent the toggle is disabled and explains why. The three modes and the Both default are removed from the specifications.

## Consequences

- The write plan lists vanilla files always and CE patch files only when the toggle is on for that item.
- The CE calibration still derives its tables at run time from the user's CE install, and only when a patch is requested.
- A vanilla only designer works fully with CE absent, so the first release can be tested without CE.
- UI copy and the design brief carry one toggle and a short explanation of the gated folder instead of a mode selector.

## Alternatives rejected

- Three equal modes: lets a CE value leak into the base definition and confuses users.
- Both as the default when CE is installed: automatic behaviour the owner does not want.
- A CE only mode that rewrites the definition: breaks the vanilla first promise.

## Evidence

- [Items toolkit](../features/items-toolkit.md) sections 3 and 8
- [Combat Extended patching](../features/combat-extended-patching.md)
- [ADR 0033](0033-item-math-and-ce-generator.md)
