# ADR 0059: Attributed RimWorld and Combat Extended data

Status: accepted | Last updated: 2026-10-10 | Register: D-211, D-212 | Supersedes in part: [ADR 0034](0034-licence-hygiene.md)

## Context

ADR 0034 kept every RimWorld and Combat Extended value out of the repository (R11) and rejected bundling data with attribution. The owner has stated on 2026-10-10 that the rights holders of both projects allow their data to be included and used when the source is named and the official project is linked.

The texts that sit on the owner's machine agree with the shape of that condition. The RimWorld end user licence agreement (EULA.txt in the game folder, revision of 2022-04-07) lets people create works on the game, asks every such work to display a fixed disclaimer, asks that the name "RimWorld" is not altered or used in a confusingly similar domain or title, and keeps distribution of parts of the software for Ludeon to agree to in advance. The README of Combat Extended says the project is used under the Creative Commons Attribution-NonCommercial-ShareAlike 4.0 licence, which asks for the name of the author with a link, a link to the licence, an indication of changes, the same licence for adaptations, and no commercial use.

The repository already holds real values: the UI fixtures are recorded from a real install (D-100), the research notes carry their datasets, and the production build of the UI bundles those fixtures for its offline mock mode. What was missing was the attribution and a check that keeps it complete.

## Decision

1. Data of RimWorld and Combat Extended (definitions, values, names, tables and patch content) may be included in the repository and used by the application when the source is named and the official project is linked (D-211). Art, audio and code of the two projects are not included, and the permission does not extend to RimSort, RimCrow or community datasets: ADR 0034 and invariant I-06 stand for those.
2. The values the designer works with are still derived at run time from the user's install (I-07), because that is what keeps them correct for the user's game and mod versions. Included data serves fixtures, tests, research evidence, documents and the offline mock mode, and may serve a later shipped table.
3. The attribution is the file `NOTICE.md`: the name, holder and official link of each source, the disclaimer of the RimWorld licence word for word, the licence name and links of Combat Extended with a statement of changes, and a table of the included sets. The README repeats the disclaimer and links the notice.
4. `xtask/third-party.jsonc` declares the sources, the sets of included data by path, the reference trees and the project's own binary assets. `cargo xtask check-licences` (part of `check-all`) fails when a source lacks its holder, https link or required text in the notice, when a set is not documented there or matches no file, when a data file that names a game or Combat Extended definition lies outside every set, when a file of a reference project is tracked, and when a binary asset is tracked that is not listed as the project's own (D-212).
5. Included Combat Extended data stays in sets that can be separated, so that the terms of its licence can be met or the sets removed without touching the rest (open issue OD-20).

## Consequences

- Rule 2 of the project instructions, the invariant I-07 and principle 6 of the overview, the testing strategy, the tooling conventions, the workspace layout and the roadmap exit criterion are reworded; ADR 0034 is superseded in part.
- Fixtures and goldens recorded from a real install are normal and covered by the notice. A new data folder needs a manifest entry and a notice row, or the check fails.
- The permission is the owner's statement; the project holds no signed grant. If a rights holder withdraws it, the declared sets say exactly what to remove.
- The application should show the notice where it shows included data; the production build bundles the recorded fixtures for its mock mode.

## Alternatives rejected

- Keep everything out of the repository, as ADR 0034 did: it no longer matches what the owner may do, and the fixtures and research data are there already.
- Include the data without attribution: against the condition of the permission and against both texts.
- Include art, audio or code of the two projects: not covered, and against the key rule of the RimWorld licence.
- Generate NOTICE.md from the manifest: a second generator for a short, rarely changing text; the check compares the two instead.

## Evidence

- [ADR 0034](0034-licence-hygiene.md), [decision register](../architecture/decision-register.md) D-211 and D-212, [overview](../architecture/overview.md) invariants I-06 and I-07.
- The licence agreement in the game folder (EULA.txt) and the README of Combat Extended, as read on 2026-10-10; official projects: https://rimworldgame.com/ and https://github.com/CombatExtended-Continued/CombatExtended.
