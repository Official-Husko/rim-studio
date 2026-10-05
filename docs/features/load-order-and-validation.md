# Load order and validation

This document specifies how RimStudio decides the order of the active mod list and how it judges a list: the rule layers (About.xml, community, user, with suppression) and the order graph with provenance on every edge, the tiers, the two sort algorithms (canonical layered and game style), cycle handling, hard force flags versus soft hints, manual reordering, the validation catalogue with stable codes, the explain contract, sort preview and undo, incremental revalidation, and the test plan. It covers requirement R5 (ordering and validation) and the consumption side of R6 (what the community rules feed into the engine; fetching and editing are in [community datasets](community-datasets.md)). It builds on the [mod manager specification](mod-manager.md) (requirements MM-016, MM-017, MM-018) and the research notes cited inline. RimSort and RimCrow are described in our own words as concept references only (R11).

Status: draft | Last updated: 2026-10-04


Milestone numbering follows the [roadmap, section 1.1](../roadmap.md#11-mapping-to-the-milestone-names-in-the-register) (older mentions of M3 to M6 use the decision register's numbering).
## Contents

1. [Purpose and boundaries](#1-purpose-and-boundaries)
2. [Rule layers and the order graph](#2-rule-layers-and-the-order-graph)
3. [Tiers](#3-tiers)
4. [Sort algorithms](#4-sort-algorithms)
5. [Manual reordering](#5-manual-reordering)
6. [Validation catalogue](#6-validation-catalogue)
7. [Explain and the why-is-this-here contract](#7-explain-and-the-why-is-this-here-contract)
8. [Sort preview and undo](#8-sort-preview-and-undo)
9. [Incremental revalidation](#9-incremental-revalidation)
10. [Commands, events and settings](#10-commands-events-and-settings)
11. [Test plan](#11-test-plan)
12. [Owner decisions and open points](#12-owner-decisions-and-open-points)

## 1. Purpose and boundaries

The order engine answers three questions for the working list of a library: which order satisfies the rules, what is wrong with the current order, and why is a mod where it is. All three are pure functions over (ordered ids, mod index, rules). They live in `rimstudio-rules` (layers and graph), `rimstudio-sort` (tiers and algorithms) and `rimstudio-validate` (diagnostics), and the manager use case crate wires them to the list session ([crate catalog](../architecture/crate-catalog.md), invariants I-04 rules-in-rust, I-10 diagnostics-not-errors, I-12 deterministic-output). The UI renders results and never recomputes a rule.

Sources of truth, for orientation: the game builds its own sort graph from loadBefore, loadAfter and the two force variants only, treats `modDependencies` as warnings, and silently drops active ids it cannot find ([mod format](../research/rimworld-mod-format-and-corpus.md), sections 1 and 4; [core domain](../research/rimsort-core-domain.md), section 3.4). RimSort adds community and user rules, tier flags and an optional dependency order; RimCrow adds user-orderable rule priority and explainable cycle breaking ([RimCrow analysis](../research/rimcrow-analysis.md), implication 4). RimStudio keeps the game's hard semantics, adds the layered model and closes the known gaps: silent dropping of active mods, silently ignored cross-tier edges, unreported cycles, loss of `incompatibleWith`, and no way to cancel a community rule.

Requirement format: ids `LO-nnn`, each with a milestone tag (MVP, v1, later; milestones M0 to M6 are in the [decision register](../architecture/decision-register.md)) and acceptance criteria (AC).

| Term | Meaning |
|---|---|
| Edge | A constraint "earlier must load before later" between two active mods, with provenance |
| Hard edge | An edge the game itself enforces (force flags, Core and expansion order) |
| Soft edge | A hint the game shows as a conflict but does not enforce |
| Dormant edge | An edge whose other end is not in the active set; kept, not used |
| Tier | One of four bands of the final list: core, frameworks, normal, bottom |
| Dropped edge | An edge left out of the graph so that it stays acyclic; always reported |

## 2. Rule layers and the order graph

#### LO-001 Pure and headless engine (MVP)

`sort` and `validate` take plain data and return plain data: no file, network, clock or random access, no thread-count dependence. Both crates build and test with no UI or Tauri dependency. Content problems are `Diagnostic` values, never errors; the crate error types cover invalid input only.

AC: `cargo test -p rimstudio-sort -p rimstudio-validate -p rimstudio-rules` runs with no webview or network; the output of every public function is byte identical across 100 runs and between 1 and 8 rayon threads.

#### LO-002 Identity and reference matching (MVP)

Rules refer to mods by package id. Matching is case-insensitive and ignores the `_steam` postfix, as the game does (`SamePackageId` with postfix ignoring; [community datasets analysis](../research/community-datasets-analysis.md), section 12). The lookup index lowercases ids; display and write-back keep the original spelling (two mixed-case keys exist in the community file today). A reference resolves to every active mod whose normalised id matches. When two active mods share an id, an edge applies to both nodes (deterministic, and the duplicate is reported by `list.duplicate-id`); the alternative of binding to the first only was rejected because it hides the problem on the second copy. Mods without a package id have no edges and sit at a defined default place (LO-014).

AC: a fixture with the 228 differently spelled reference shapes of the corpus (case, `_steam`) resolves every reference; a reference to an id that matches nothing becomes a dormant edge, not an error.

#### LO-003 Rule layers (MVP About and user, v1 community)

Edges come from five layers. Their default insertion priority is the order shown, except that the two hard layers always rank first and are not configurable because the game enforces them.

| Rank | Layer id | Source | Kind of edge | Configurable rank |
|---|---|---|---|---|
| 1 | `core` | Built-in rule: `ludeon.rimworld` first, then official expansions after it in the order the install reports | Hard | No |
| 2 | `about-force` | `forceLoadBefore` and `forceLoadAfter` in About.xml (after `ByVersion` replacement for the running version) | Hard | No |
| 3 | `user` | The user rules file and the user layer of the rule editor ([community datasets](community-datasets.md), section 7) | Soft; also the flags top and bottom | Yes |
| 4 | `about-soft` | `loadBefore` and `loadAfter` in About.xml (after `ByVersion` replacement) | Soft | Yes |
| 5 | `community` | The community rules dataset: loadAfter, loadBefore, loadTop, loadBottom, incompatibleWith | Soft | Yes |
| 6 | `derived` | Dependency implied order (LO-009), opt-in | Soft | Yes (always lowest by default) |

The configurable layers (3 to 6) follow the setting `sorting.layerPriority` (default `["user", "about-soft", "community", "derived"]`). This differs from the research proposal, which placed `about-force` between `user` and `about-soft`: a user edge that contradicts a force flag cannot be honoured by the game, so letting it win would only create a list the game rewrites. The rule editor therefore rejects such an edge at entry with the hard edge shown, and the engine drops it with a report if it arises later (for example after an About.xml update).

`ByVersion` blocks replace the base list exactly (not additively) for the running major.minor ([mod format](../research/rimworld-mod-format-and-corpus.md), section 1.5); the library crate delivers the already resolved lists, the rules crate never reads XML.

AC: a fixture mod with both a base `loadAfter` and a `loadAfterByVersion` for the running version produces only the versioned edges; a user edge opposing a force edge is rejected by the editor and, when injected directly, appears in the dropped list with reason `hard-wins`.

#### LO-004 Edge model and provenance on every edge (MVP)

```text
Edge   { earlier: PackageId, later: PackageId, kind: Hard | Soft,
         sources: [ Source ] }
Source { layer, ruleRef, comment?, datasetVersion? }
ruleRef = { file or dataset id, subject, key, target }   // enough to open the rule or the About.xml line
```

An edge is normalised to earlier-then-later: `A loadAfter B` becomes `B -> A`; `A loadBefore B` becomes `A -> B`. Identical edges from several layers merge into one edge with several sources, which is how redundancy is detected ([community datasets](community-datasets.md), CD-016). If any source is hard, the merged edge is hard. Edges referencing the same mod on both ends are dropped as `self` and reported once per source. The provenance of every edge, kept and dropped, is part of every sort result and every explanation; there is no edge without a source.

AC: serialising the graph of a fixture shows at least one source per edge; the merged edge for a pair declared in About.xml and in the community layer lists both sources; `datasetVersion` equals the version recorded in the dataset state at build time.

#### LO-005 Hard flags versus soft hints (MVP)

| Aspect | Hard (`core`, `about-force`) | Soft (everything else) |
|---|---|---|
| Source of truth | The game: it refuses a manual reorder that breaks a force rule and uses the Core and expansion order at activation | A hint: the game shows a conflict tooltip, does not enforce |
| In the corpus | `forceLoad*` appears in 6 of 756 mods and is meaningful only for official expansions | `loadAfter` totals 3,511 references, up to 979 on one mod |
| Violation diagnostic | `list.order-hard`, severity Error | `list.order-soft`, severity Warning |
| Manual move | Blocked by default (LO-017) | Allowed with an immediate warning |
| Sort | Always satisfied; a hard edge is never dropped for a soft one | Satisfied when no higher edge forbids it |
| Cycle among hard edges | Reported as `sort.cycle` with severity Error | Reported as `sort.cycle` with severity Warning |
| Cancelable by user suppression | No | Yes (LO-007) |

AC: with a hard and a soft edge in conflict, the sort keeps the hard edge and drops the soft one with both sources in the report.

#### LO-006 Dormant edges (MVP)

Edges whose other end is not active are not part of the graph. They are kept in a dormant list per mod so the UI can say "this rule applies if X is enabled" and so enabling X later activates the edge without a rescan. An edge to an id no installed mod has is dormant forever until that mod appears.

AC: toggling a mod on adds its dormant edges to the graph in the same revision; the graph of a fixture with 1,241 referenced ids and 64 installed subjects (the shape of the owner's library) has only edges between active mods.

#### LO-007 Suppression (v1)

A person can suppress an edge of any soft layer (community, about-soft, derived; also a user edge, which is simply deleted). A suppression is stored as a record `{ subject, kind, target, layer?, reason, since }` (`kind` and `target` may be a wildcard to cover every rule of one subject, [community datasets](community-datasets.md), CD-015) in `userdata/rules/suppressions.json` ([data and persistence](../architecture/data-and-persistence.md), section 3.2), so the RimSort compatible user rules file stays clean. The engine removes matching soft edges after merging and before insertion (LO-008), remembers them in the result as `suppressed` with their sources and the reason, and the explanation can say so. A suppression with no layer applies to every soft source of the pair; if a hard source exists, the suppression only removes the soft sources and the report states that a hard edge remains. Suppressing an edge one end of which is not installed is allowed and stays dormant.

AC: a community edge that is suppressed no longer orders mods, appears under "suppressed rules" in the preview with its reason, and returns after the suppression is removed; a suppression naming a hard edge does not remove it.

#### LO-008 Graph build and cycle-safe insertion (MVP)

Procedure, shared by both sort algorithms and by the validator:

1. Collect edges from the enabled layers for the active set (settings `sorting.includeCommunityRules` and `sorting.includeUserRules` switch layers off for one sort; the validator uses the same switches).
2. Resolve references (LO-002); move edges with an inactive end to the dormant list; drop and report self edges.
3. Merge identical edges and union their sources (LO-004).
4. Apply suppressions (LO-007).
5. Fast path: run Kahn's algorithm over all remaining edges. If every node is emitted, the graph is acyclic; go to step 8.
6. Otherwise find the strongly connected components (Tarjan, iterative). Only components with more than one node need work.
7. Inside each cyclic component, insert edges one by one in a total order: hard edges first, then by layer rank, then by (earlier id, later id) lowercase. Before inserting `a -> b`, test whether `b` already reaches `a` through inserted edges (depth-first search limited to the component). If it does, do not insert; record `Dropped { edge, sources, reason: cycle, blockingPath: [edge...] }` with the sources of every edge on the blocking path. The reason is `hard-wins` when the blocking path is hard and the dropped edge is soft, and `same-rank` when the dropped edge and the last blocking edge are in the same layer (the choice among equals is then made only by the deterministic insertion order, and the report says so).
8. Return `Graph { nodes, edges (acyclic), dormant, suppressed, dropped, cycles }` where `cycles` lists the component nodes and the dropped edges that broke each.

Complexity: collecting and sorting edges is O(E log E); the fast path is O(V + E); step 7 costs O(Ec times (Vc + Ec)) inside each cyclic component only, which is small because cycles are rare (the owner's library has one two-node cycle among 3,289 constraints). The result is always acyclic and deterministic. This generalises RimSort's one narrow rule (an inferred dependency edge loses against an explicit one) and RimCrow's weighted cycle breaking, and removes the "sort abandoned on a cycle" failure of both the game and RimSort ([rules fetch and merge](../research/rules-fetch-and-merge-design.md), section 7.2; [core domain](../research/rimsort-core-domain.md), section 3.2).

AC: on the owner's 610-mod fixture the one existing two-cycle is reported with both ends, the dropped edge and the sources of the surviving edge; every dropped edge in every random-input property test has a blocking path whose edges are all inserted edges of equal or higher rank.

#### LO-009 Dependencies as warnings by default, opt-in order (MVP)

By default `modDependencies` contribute no edges: the game treats them as warnings only, and on the owner's list adding them as edges changes the largest topological level from 150 to 106 mods while leaving violations at 0, so they mostly add ordering that authors did not ask for ([core domain](../research/rimsort-core-domain.md), section 3.1). The setting `sorting.dependenciesAsLoadAfter` (default off) adds a `derived` soft edge `dependency -> mod` for each dependency whose providing mod is active (an alternative id from the same dependency entry counts, first active alternative wins in a fixed order). Derived edges are not created when either end is a tier 0, 1 or 3 seed, and because they are inserted last (LO-008) an explicit edge in the opposite direction removes them with a `cycle` report instead of creating a conflict.

AC: with the setting off the graph of a fixture contains no `derived` edge; with it on, a fixture with an explicit opposite edge produces a dropped derived edge and no cycle.

## 3. Tiers

#### LO-010 Four tiers (MVP)

The final list is partitioned into tier 0 (core), tier 1 (frameworks), tier 2 (normal) and tier 3 (bottom). Tiers are a documented rule, not an accident of the algorithm, and can be switched off with `sorting.tiersEnabled` (default on), in which case the whole graph is one tier.

| Tier | Seeds | Membership |
|---|---|---|
| 0 core | Active mods in the core seed list: Core, the installed official expansions (detected from the install, never listed in data), and the short set of libraries that must precede everything (default Harmony and the pre-patcher, editable in settings as `sorting.tierSeeds.core`) | Seeds plus the transitive closure of their predecessors (what they must load after) |
| 1 frameworks | Active mods in `sorting.tierSeeds.frameworks` plus every mod flagged load-top in the user or community layer, minus tier 0 | Seeds plus closure of predecessors, minus tier 0 |
| 3 bottom | Mods flagged load-bottom in the user or community layer | Seeds plus the closure of successors (what must load after them), minus tiers 0 and 1 |
| 2 normal | Every other active mod | The rest |

Closure directions follow load semantics: whatever a core or framework mod must load after has to be in the same or an earlier tier, and whatever must load after a bottom mod has to be in the same or a later tier. (The RimSort user guide describes tier 0 the other way round; its code does what is stated here, [core domain](../research/rimsort-core-domain.md), section 3.1.) The default framework seed list ships as a small JSON resource in the app (not downloaded data, not a copy of a RimSort file) and its content is an owner decision (section 12); tier flags can also be set per mod in the rule editor.

AC: the tier partition of any list is disjoint and exhaustive (property test); on the owner's fixture the replica tier sizes were 8, 15, 560 and 35 (618 before overlaps are removed from 610 mods), and the implementation reports its own sizes in the preview.

#### LO-011 Cross-tier conflict reporting (MVP)

RimSort silently discards every edge that crosses tiers in the wrong direction. RimStudio checks every edge `a -> b` after tier assignment and requires tier(a) less than or equal to tier(b). A violating edge is reported as `sort.tier-conflict` with its sources and the tiers of both ends. Resolution:

1. A hard edge never loses to a tier flag. If a hard edge `a -> b` has tier(a) greater than tier(b), tier(b) is raised to tier(a) and the raise propagates along hard edges to successors until stable (monotone, so it terminates). The report names the flag that caused the conflict.
2. A soft edge loses: it is excluded from the tiered sort, stays in the list of unsatisfied constraints, and the validator will flag the resulting order as `list.order-soft` with an explanation that a tier flag caused it.
3. A mod claimed by the closures of tier 1 and tier 3 is placed in tier 1 and reported (`kind: top-and-bottom`).

AC: a fixture with a load-bottom mod that must load before a normal mod pulls that mod into tier 3 by closure and reports nothing; a fixture with a framework seed that must precede a core seed reports one conflict with both sources and the sort completes.

## 4. Sort algorithms

Both algorithms take the same `SortInput { active: [ModIdx] in current order, index, graph, tiers, pins, options }` and return the same `SortOutput { order, moves, dropped, cycles, tierConflicts, unmapped, explanations }`. The user chooses between them in the sort preview; the default is canonical.

#### LO-012 Canonical layered sort (MVP)

Goal: a trustworthy, reproducible order that does not depend on the current order. The output depends only on the active set, the rules and the options.

1. Build the graph (LO-008); the graph is acyclic.
2. Assign tiers (LO-010) and report tier conflicts (LO-011).
3. For each tier in order 0, 1, 2, 3, take the induced subgraph on its nodes (edges that leave the tier are satisfied by tier order).
4. Compute longest-path levels: level 0 has no predecessors inside the tier; level k has all its predecessors in levels below k. This is a Kahn pass by levels, O(V + E).
5. Order the nodes of each level by the total key `(casefolded display name with natural number ordering, lowercase package id, ModId string)`. Natural ordering compares digit runs numerically. The key is a total order because the ModId is unique, so ties are impossible and the result is independent of input order and hash seeds (RimSort breaks equal names by set iteration order, which varies between runs).
6. Concatenate the tiers. Apply pins (LO-016).
7. Verify: every kept edge goes forward; assert in debug builds, report as an internal error diagnostic otherwise.

Complexity: O(E log E) for edge handling, O(V log V) for the level sorts, so O((V + E) log (V + E)) overall; the replica of the same approach took about 3 ms on the owner's list and the Rust target is under 5 ms (benchmark `sort_610`). Cycle handling is entirely in step 1: every dropped edge with its sources is returned, none is hidden. Compared with the research proposal (tie-break by current order, then name), this specification ignores the current order so that shuffled input gives identical output, which is the property the owner asked to test; staying close to the current order is the job of the game-style sort.

AC: byte identical output across 100 runs and across 100 random shuffles of the input; zero violations of kept edges on the owner's 610-mod fixture; the result contains exactly the active set (LO-014).

#### LO-013 Game-style minimal-movement sort (MVP)

Goal: fix violations while keeping the user's order as much as possible, in the same style as the game's own auto sort (a depth-first postorder from the current list, stable with respect to the existing order; [core domain](../research/rimsort-core-domain.md), section 3.4).

1. Build the graph and assign tiers as in steps 1 and 2 of LO-012.
2. Partition the current list into tiers, keeping the current relative order inside each tier. A mod keeps its tier even when the current list had it elsewhere.
3. For each tier, visit its mods in current order with an explicit stack (no recursion, so a chain of 979 predecessors cannot overflow). To visit a mod: if already visited, return; mark it; visit each of its in-tier predecessors, ordered by current index, then emit the mod.
4. Concatenate the tiers. Apply pins (LO-016).

Properties: if the current order already satisfies every edge and tier, the output equals the input (when each mod is visited all its predecessors precede it and are already emitted), so the sort is idempotent and a no-op on a valid list; the output always satisfies every kept edge; the result depends on the input order by design but is deterministic for a given input. Complexity O(V + E) plus the predecessor ordering O(E log d). On the owner's list the game's own algorithm changed 506 positions (mean displacement 4.55, maximum 504) against 600 of 610 for a layered sort ([core domain](../research/rimsort-core-domain.md), section 3.1); the preview reports the number of positions that change and the number of mods that actually move (LO-025), computed as the list length minus the longest common ordered subsequence with the current list, because moving one mod far shifts everything between and is not 100 positions of work for the user.

Rejected alternative: Kahn's algorithm with a priority queue on current index (lexicographically smallest order). It is plausible to move fewer mods, but only the depth-first variant has measured evidence and matches what the in-game sort produces; the benchmark in section 11 compares both on the 610 fixture and the choice is revisited if the queue variant moves measurably fewer mods (section 12).

AC: on a list that satisfies all rules the sort returns the identical order; on the owner's fixture the game-style sort leaves zero violations and moves fewer mods than the canonical sort by the subsequence measure; running it twice gives the same order as running it once.

#### LO-014 Determinism, completeness, idempotence (MVP)

Invariants, tested as properties:

1. Determinism: same input gives the same output across runs and thread counts; canonical output is also invariant under shuffles of the input.
2. Completeness: the output list is a permutation of the active set. No mod is dropped. A mod that cannot take part in ordering (no package id; reported by `list.missing-properties`) is placed in tier 2 level 0 by the total key and listed in `unmapped` with the reason. This closes RimSort's silent loss of unmappable mods and of a second mod with the same id.
3. Idempotence: canonical sort of the canonical result is unchanged; game-style sort of any result of itself is unchanged.
4. Soundness: every violation present in the output corresponds to an entry in `dropped`, `suppressed` or `tierConflicts`; the validator run on the output reports no `list.order-*` diagnostic beyond those.

#### LO-015 Cycle handling and report (MVP)

A cycle never aborts a sort and never alters the list silently. The result has one `cycles` entry per strongly connected component:

| Field | Content |
|---|---|
| `members` | Mod ids and names in the component |
| `dropped` | Every dropped edge: earlier, later, kind, all sources (layer, rule reference, comment, dataset version), reason (`cycle`, `hard-wins`, `same-rank`) |
| `blocking` | For each dropped edge, the path of inserted edges that made it impossible, each with its sources |
| `severity` | Error if any dropped edge was hard, otherwise Warning |

The preview lists cycles by mod names with the lowest-priority edge named first and a button that opens the rule editor on it (suppress, delete or keep). A cycle found in input data alone (two exist in the community dataset, one in the owner's own About.xml set) is shown the same way so authors and dataset maintainers can fix the source ([community datasets analysis](../research/community-datasets-analysis.md), section 3.3).

AC: a fixture with a three-node cycle across three layers drops exactly one edge, the lowest ranked, and reports all three edges with their layers; a fixture with a cycle among force edges reports severity Error and still returns a complete list.

#### LO-016 Pins and groups (v1)

A pinned mod keeps its current index when the rules allow. After step 6 of LO-012 (or step 4 of LO-013) the engine removes pinned mods, then reinserts them at their previous indices clamped to the list length, in ascending index order. A pinned mod whose reinsertion would violate a kept hard edge or tier is released and reported as `list.pinned-conflict`; a soft violation is allowed only when `sorting.treatPinnedAsFixed` is on, and the preview marks it. Groups stay contiguous unless a hard edge forces a split, in which case the preview shows the group as split with the reason. Pins and groups are user data of the manager ([mod manager](mod-manager.md), MM-019).

AC: a fixture with two pinned mods and a rule that would move one of them reports exactly one pinned conflict and leaves the other at its index.

## 5. Manual reordering

#### LO-017 Constraints and warnings on manual moves (MVP)

Dragging, keyboard moves and "move to" are `list_move` commands. Before applying, the engine computes the new relative order and evaluates only the constraints incident to the moved mods (section 9), returning the new diagnostics in the same response.

1. Soft violations are allowed. The row shows the warning immediately with the rule that is broken and its source.
2. A hard violation is blocked by default (setting `sorting.hardRuleMoves` is `block`, or `warn` to allow it with an Error diagnostic). The block message names the force flag or Core rule and the mod it concerns and offers the allowed range.
3. For one or more selected mods the UI can ask for the allowed range: the interval of positions between the last hard predecessor and the first hard successor. This is a query over the position array (`list_move_bounds`, proposed row for the registry) and lets the drag target show where a drop is legal.
4. Moving a block moves its members together and checks them together; constraints between two members of the block cannot change.
5. Moving a mod across a pin or into a group boundary changes nothing silently: the pin or group relation shows as a warning (`list.pinned-conflict` for pins).
6. Every move is one undoable command ([mod manager](mod-manager.md), MM-014).

AC: moving a mod before its force predecessor is refused with a message naming the rule and its About.xml source; with the setting on `warn` the move is accepted and `list.order-hard` appears on both rows in the same revision; moving a mod across 400 rows revalidates in under 1 ms (benchmark).

## 6. Validation catalogue

#### LO-018 Validation contract (MVP)

`validate` is a set of pure producers returning `Diagnostic { code, severity, mod, file?, message, args, sources }`. Codes are stable strings of the form `<area>.<kebab-name>` (D-046); renaming one is a breaking change that needs a migration of stored ignore entries. Messages are English templates in `locales/en.json` under the key `diag.<code>`, with `{mod}`, `{other}` and similar slots filled from `args`; the UI never builds a sentence. Severity is Error, Warning or Info; counts in the status bar are of unmuted Error and Warning only. "Blocking" means the diagnostic stops a save or launch until resolved or overridden as described.

#### LO-019 The catalogue (MVP unless noted)

| Code | Name | Severity | Trigger | Message template | Suggested action | Ignorable |
|---|---|---|---|---|---|---|
| `list.missing-dependency` | Missing dependency | Error | A dependency of an active mod is not satisfied by an active mod; satisfaction accepts the dependency id, any of its listed alternatives, or an active mod named as the replacement for it in the replacement dataset. `args.state` is `inactive-available`, `alternative-available` or `not-installed` | `{mod} requires {dependency}, which is not active` | Enable dependency (inactive available), Use alternative, Open Workshop page, Sort and add (MM-025) | Yes |
| `list.incompatible` | Incompatible mods active | Error | Two active mods are declared incompatible by the About.xml of either, or by the community or user layer; reported on both mods, `args.declaredBy` names the side and layer | `{mod} is incompatible with {other} (declared by {declaredBy})` | Disable one of them; Mute | Yes |
| `list.order-hard` | Hard order violation | Error | An active pair violates a hard edge (force flag, Core first, expansion order) | `{mod} must load {direction} {other} (force rule from {layer})` | Move after or before X (fix command), Re-sort | Yes (not recommended) |
| `list.order-soft` | Soft order violation | Warning | An active pair violates a soft edge from any layer; also reported for a tier-conflict edge with `args.cause = tier` | `{mod} should load {direction} {other} ({layer}: {comment})` | Move after or before X, Sort, Suppress the rule | Yes |
| `list.version-mismatch` | Version mismatch | Warning | The running game major.minor is not in the mod's supported versions (rules in LO-020) and the mod is not in the No Version Warning list and not Core or an official expansion | `{mod} lists {versions}, not {gameVersion}` | Look for update, Mute; the No Version Warning list may hide it | Yes |
| `list.duplicate-id` | Duplicate id active | Error | Two active entries share a normalised package id (two copies, or one mod listed twice) | `{mod} and {other} share the package id {id}` | Keep one (opens Duplicates, MM-027), apply the duplicate policy | No |
| `list.replacement-available` | Replacement available | Info | The mod's workshop id is the old id of a rule in the replacement dataset (matched by workshop id first; a package id match alone is only a hint and is labelled as such) | `{mod} has a suggested replacement: {replacement}` | Use replacement (enable it, disable old, keep position), Mute | Yes |
| `list.missing-properties` | Missing properties | Warning | No package id, an invalid id format, or no declared supported versions; the mod takes part in order only through its folder identity (LO-014) | `{mod} has no usable package id` or `{mod} declares no supported versions` | Open About.xml, Mute | Yes |
| `list.core-inactive` | Core not active | Error | `ludeon.rimworld` is not in the active list | `Core is not active` | Enable Core (the game would force it) | No |
| `list.pinned-conflict` | Pin conflicts with a rule | Warning | A pin contradicts a hard edge, or a manual move crosses a pin | `{mod} is pinned at {position}, but {other} must load {direction} it` | Release pin, Move | Yes |
| `list.unresolved-id` | Unresolved active id | Error | An id in an imported list or in ModsConfig.xml matches no mod in any enabled source; the entry stays as a ghost row, is never written to ModsConfig.xml, and the game would silently deactivate it | `{id} is in the list but no installed mod has it` | Remove from list, Locate (rescan sources), Subscribe (Workshop page) | No (blocks save until removed or confirmed) |
| `deploy.not-visible` | Mod not loadable (unlinked) | Error, blocks save and launch | An active mod lives in a custom folder and has no link or copy in the game Mods folder (the game reads only Data, Mods and Steam items) | `{mod} is in {folder}, which the game cannot see until it is linked` | Link now (deploy plan), Disable | No |
| `deploy.source-offline` | Source offline | Error, blocks save and launch | An active mod's source folder or drive is unreachable (unmounted drive, broken link) | `{mod}'s folder {source} is not reachable` | Reconnect and rescan, Disable | No |
| `deploy.modsconfig-version` | ModsConfig version mismatch | Warning when reading, Error blocking when writing | The `version` in ModsConfig.xml differs in major.minor from the installed `Version.txt` (the game would discard the whole list at its next start), or RimStudio cannot read the install version it must stamp | `ModsConfig.xml was written for {fileVersion}; the game runs {gameVersion}` | Save (RimStudio rewrites with the install version after a backup), Check the install path | No |
| `sort.cycle` | Rule cycle | Warning, Error if a hard edge was dropped | Strongly connected component found while building the graph (LO-015); preview only | `Rules {members} contradict each other; {dropped} was ignored` | Open rule editor, Suppress, Delete | n/a (preview) |
| `sort.tier-conflict` | Tier conflict | Warning | An edge crosses tiers wrongly, or a mod is claimed by top and bottom (LO-011); preview only | `{mod} is flagged {flag} but must load {direction} {other}` | Change flag, Suppress | n/a (preview) |
| `sort.unmapped` | Not part of ordering | Info | A mod without a usable id was placed by default (LO-014); preview only | `{mod} could not be ordered by rules and was placed by name` | Fix About.xml | n/a (preview) |

Notes on the catalogue.

1. Version mismatch compares the running game's major.minor with each supported version's first two integers (build and revision are never compared; a value such as `1.6.4871` is read as 1.6; a value such as `v1.6` is unparsable and reported under `list.missing-properties`, matching the game's behaviour in [mod format](../research/rimworld-mod-format-and-corpus.md), section 1.2).
2. Incompatibility is a Warning in the game's UI tooltip and an Error here because a list with a declared incompatibility is rarely intended; the severity is a presentation default, not a rule, and the setting `sorting.incompatibleSeverity` (`error` default, `warning`) changes it.
3. The community `incompatibleWith` edges (5 in today's file) are consumed here; RimSort's typed model drops them ([community datasets analysis](../research/community-datasets-analysis.md), section 3.4).
4. `list.duplicate-id` and `list.unresolved-id` cover entries the game would deactivate (the game rejects a duplicate active instance and silently removes an unknown id; [mod format](../research/rimworld-mod-format-and-corpus.md), sections 4.2 and 4.3).
5. Author-side lints (`author.*`), publish preflight (`publish.*`) and log classification (`log.*`) are separate producers in `rimstudio-validate` and are catalogued in their own documents.

AC: a golden file per code exists (one fixture list producing exactly that code on exactly the expected mods); the code list in the registry test equals the table above; every code has a translated template whose slots are all populated by `args`.

#### LO-020 Version compatibility rules (MVP)

1. Compatible if any supported version equals the running major.minor. `ludeon.rimworld` and official expansions are always compatible.
2. An empty or missing list is not a mismatch; it is reported once under `list.missing-properties`.
3. A mod in the No Version Warning list for the running major.minor (ids lowercased, derived from the dataset; [community datasets](community-datasets.md), section 2) is not flagged. For an uninstalled-mod hint the SteamDB `gameVersions` may add information, never override About.xml for an installed mod.
4. The game version is the install's `Version.txt` (including revision), read by `rimstudio-steam`; the compared value is its major.minor.

AC: fixtures for `1.6`, `1.6.4871`, `v1.6`, an empty list, a newer-only list, and an id in the No Version Warning list give the expected presence or absence of the diagnostic.

#### LO-021 Muting and ignore entries (MVP, editor v1)

The popover action Mute stores `{ mod id, code, since }` in `userdata/ignore.json`; a muted diagnostic is hidden from counts and badges and visible with the filter `is:muted`. Muting is per mod and per code; a global Reset muted warnings and an ignore list editor exist from v1. Codes marked not ignorable cannot be muted. A muted diagnostic that stops occurring leaves its entry (harmless); the editor flags such entries as stale. Muting never changes sort results.

AC: muting changes counts without recomputing other rows; the ignore file keeps unknown keys on save.

#### LO-022 Fixes as commands (MVP)

Every diagnostic with a fix offers it as an undoable command: Enable dependency, Disable incompatible mod, Move after or before X (a single move that respects the other constraints, computed from the allowed range), Use replacement. Fixes never edit rules or mod files.

AC: applying Enable dependency changes the list by one entry and one undo step and removes the diagnostic in the same revision.

## 7. Explain and the why-is-this-here contract

#### LO-023 `explain(x, y)` (MVP simple, v1 full)

`explain` is a query over the graph snapshot of the current rules revision and active set; it does not depend on the current positions, so it answers for the list as it would be sorted. It returns the first applicable answer:

| Kind | Condition | Result |
|---|---|---|
| `chain` | A directed path from x to y exists in the graph | The shortest path (fewest edges; ties broken by higher layer rank, then lowercase ids), as steps `{ earlier, later, kind, sources }`, so each step shows its layer, rule reference, comment and dataset version |
| `reverse` | A path from y to x exists | The chain, stated as "y must load before x"; the UI says that x above y would violate it |
| `tier` | No path, different tiers | "x is in tier N because of seed or flag (layer, comment)" and the same for y |
| `dropped` | A dropped edge or a suppression relates the pair | The edge, why it was dropped (cycle, hard-wins, same-rank, suppressed) and the blocking path |
| `none` | No rule relates them | "No rule orders these mods; their positions come from the current order or the name tie-break" |

If the current positions violate the answer, the result carries `violated: true`. The query is cheap (breadth-first search over at most the active graph) and returns within 1 ms on 610 mods (benchmark `explain_pair`). Unknown cases say none; the engine never guesses a reason.

AC: for a fixture pair with a three-edge chain, the three edges are listed in order with their layers; a pair separated only by a dropped edge returns kind `dropped`; a pair in different tiers with no path returns kind `tier`.

#### LO-024 Why is this here (MVP simple, v1 full)

For a single mod the detail panel and the context menu entry "Why is this here?" call `explain_position(x)` and render:

1. The tier and the reason (core seed, framework seed, flag top or bottom, closure of which mod).
2. The binding predecessor and successor: of the mods that must load before x, the one currently nearest above x and, symmetrically, the one that must load after x and is nearest below; each with the edge sources. "Slack" (how many positions x could move without violating a kept edge) is shown as a range, computed from the position array.
3. Constraints currently violated and their diagnostics.
4. If x has no edges at all: "no rule; position is user choice".
5. Each source is a link: About.xml sources open the mod's About line, community sources open the rule in the editor read-only, user sources open it editable, and a suppressed edge offers Restore.

The v1 Relations tab shows the same data as a small graph around x (Cytoscape behind `shared/graph`). Results carry the rules revision so a stale panel can detect that it needs a refresh.

AC: for a mod with one hard predecessor, the panel names that predecessor, the layer and the allowed range; clicking a source opens the matching rule or file.

## 8. Sort preview and undo

#### LO-025 Sort preview (MVP)

Sort is preview first ([mod manager](mod-manager.md), MM-017). `sort_preview` returns:

| Field | Content |
|---|---|
| `order` | `ModIdx[]` of the proposed list |
| `moves` | Moved mods grouped into blocks with before and after positions; each move has a short reason (the binding edge: layer and neighbour) |
| `counts` | Positions changed, mods moved (list length minus the longest common ordered subsequence), tier sizes |
| `dropped`, `cycles`, `tierConflicts`, `suppressed`, `unmapped` | As defined in LO-008, LO-011, LO-014, LO-015 |
| `remaining` | Count of order diagnostics the proposed list would still have (zero unless dropped hard edges exist) |
| `token` | Hash of the proposed order and the revision it was computed against |
| `stats` | Algorithm, option digest, rules revision, duration |

The preview is computed from a snapshot and pushes no undo entry. Changing an option (algorithm, layers on or off, dependencies imply order, pins) recomputes the preview, debounced. The panel shows added, removed and moved lists, never silently reorders, and a sort over 610 mods yields a preview in under 100 ms end to end. The registry lists `sort_preview` as a query; because anything over 1 ms of Rust work should be a job (D-045), the handler must be written so that its kind can be promoted to a job without changing its inputs (flagged in section 12).

AC: preview of the owner's fixture shows zero remaining diagnostics; changing from canonical to game-style changes the moved counts shown; cycles are listed by mod name with sources; the preview output is byte identical across 100 runs.

#### LO-026 Apply, undo and redo (MVP)

`sort_apply` takes the token. A token whose revision is stale returns `list.revision-conflict` and the UI recomputes the preview. Applying pushes exactly one command on the list session's undo stack, so one undo restores the previous order and pins; redo reapplies it. Apply records an automatic history snapshot (v1, [mod manager](mod-manager.md), MM-022). "Sort and apply" from the palette skips the panel, and a setting makes it the default for users who trust it. The unsaved marker is raised; nothing is written to ModsConfig.xml until the person saves (and the checks of LO-019 pass).

AC: apply, undo, redo and apply again compare equal as lists at each step; an apply with a token computed before a manual move is refused.

## 9. Incremental revalidation

#### LO-027 Incremental engine (MVP)

State kept per list session (rebuilt from the list in O(V + E) when the rules or the index change):

| Structure | Purpose |
|---|---|
| `pos[ModIdx] -> u32` | Position in the active list, a sentinel when inactive |
| Constraint table in compressed rows (CSR) by `ModIdx`: outgoing and incoming edges with kind and source index | Order checks touch only a mod's incident edges |
| Reverse dependency index (dependency id to mods that need it) | A toggle revalidates the dependents |
| `id -> count` for active normalised package ids | Duplicate detection |
| Incompatibility pair table | Both directions in one lookup |
| Diagnostic slots per mod, each a small list | Delta computation |

Key observation: moving a set M of mods changes the relative order only of pairs with at least one member in M, so only edges incident to M are re-evaluated; positions of everything between shift but no pair among the others changes. A toggle re-evaluates the toggled mod's edges, its dependents, its incompatibility pairs and its id count. Changes to rules, datasets or version context trigger a full recompute of the affected producers as a job. After each command the engine returns only mods whose diagnostic slot changed, as a `library.diagnostics {rev, byMod}` delta ([ipc and state](../architecture/ipc-and-state.md), snapshot plus delta).

Performance targets (benchmarked with criterion against `xtask/budgets.jsonc`; a 2x regression fails):

| Benchmark | Input | Target |
|---|---|---|
| `validate_full_610` | 610 active mods, 1,500 constraints, dependency and incompatibility tables | under 1 ms per full recalculation |
| `validate_move_1` | One mod moved across 400 rows | under 100 microseconds (target, unverified) |
| `validate_move_50` | A block of 50 mods moved | under 500 microseconds (target, unverified) |
| `validate_toggle` | One mod toggled | under 100 microseconds (target, unverified) |
| `sort_610` | Canonical and game-style over the 610 fixture | under 5 ms each |
| `sort_5000` | Synthetic 5,000 mods | recorded, 2x regression gate |
| `explain_pair` | One pair over 610 mods | under 1 ms |

The research baseline for the full recalculation is a replica with O(n) `index` lookups costing about 600 x 2.5 x 600 comparisons; the position array replaces every search with one array read ([core domain](../research/rimsort-core-domain.md), sections 4 and 7).

AC: after any random sequence of toggles, moves, pin changes and undo steps, the incremental diagnostics equal a full recomputation (property test, debug assertion in the engine); `validate_full_610` meets the 1 ms target on the CI reference machine.

#### LO-028 Budgets for the whole path (MVP)

Preview under 100 ms; one list command (move or toggle) with diagnostics delivered under 16 ms end to end (the frame budget; the Rust part under 1 ms); first diagnostics for a cached library together with the first list render ([mod manager](mod-manager.md), section 11). A drag over a long list never waits on revalidation: the move is applied optimistically in the UI and the response reconciles by revision.

## 10. Commands, events and settings

| Command | Kind | Purpose | Registry status |
|---|---|---|---|
| `sort_preview` | query | Preview (LO-025) | Row 26 |
| `sort_apply` | action | Apply by token (LO-026) | Row 27 |
| `diagnostics_for_mod` | query | Diagnostics with provenance for one mod | Row 28 |
| `list_move`, `list_toggle`, `list_undo`, `list_redo` | action | List edits with incremental diagnostics (LO-017, LO-027) | Rows 20 to 24 |
| `diagnostics_list` | query | Full diagnostics for the list (MM-016) | Proposed row |
| `diagnostics_mute` | action | Mute or unmute (LO-021) | Proposed row |
| `sort_explain` | query | `explain(x, y)` and `explain_position(x)` (LO-023, LO-024) | Proposed row |
| `list_move_bounds` | query | Allowed range for a move (LO-017) | Proposed row |

Events: `library.diagnostics {rev, byMod}` delta; `job.progress` when preview is promoted to a job. Settings (all in `settings.jsonc` under the `sorting` group of the [settings specification](settings.md), which already defines `sorting.dependenciesAsLoadAfter`, `sorting.alternativeIdsSatisfyDependencies` and `sorting.checkDependenciesOnSort`; the other keys are proposed additions; defaults not written, [data and persistence](../architecture/data-and-persistence.md), section 3.1):

```jsonc
{
  "sorting": {
    "algorithm": "canonical",              // canonical | game-style; last choice, remembered
    "tiersEnabled": true,
    "includeCommunityRules": true,
    "includeUserRules": true,
    "dependenciesAsLoadAfter": false,
    "treatPinnedAsFixed": false,
    "tierSeeds": { "core": [], "frameworks": [] },   // additions to the shipped defaults
    "sortAndApplyByDefault": false,
    "layerPriority": ["user", "about-soft", "community", "derived"],
    "hardRuleMoves": "block",              // block | warn
    "incompatibleSeverity": "error"        // error | warning
  }
}
```

## 11. Test plan

Fixtures contain only fictional ids (for example `fixture.alpha`); the owner's 610-mod library is a private fixture for `#[ignore]` tests selected by an environment variable and is never committed (R11, I-07). A synthetic generator reproduces its measured shape: 610 active mods, about 3,289 ordering pairs, loadAfter lists up to 979 entries, one two-cycle, tier sizes near 8, 15, 560 and 35.

| Area | Tests |
|---|---|
| Determinism (property) | Canonical output identical for 100 random shuffles of the input; both algorithms identical across 100 runs and at 1 and 8 threads |
| Idempotence (property) | Canonical of canonical equals canonical; game-style of its own result unchanged; game-style on a valid list returns the input |
| Completeness (property) | Output is a permutation of the active set; unmappable mods appear in `unmapped`; duplicate-id lists keep both copies |
| Soundness (property) | Every `list.order-*` diagnostic on a result corresponds to `dropped`, `suppressed` or `tierConflicts`; every dropped edge has a blocking path of equal or higher rank edges |
| Graph | Random layered rule sets give an acyclic graph; fast path and insertion path give identical graphs on acyclic inputs; a hard edge is never dropped for a soft one |
| Rule fixtures | User add; user edge versus about-force (rejected and injected); user suppression of a community edge; suppression restored; community two-cycle; `ByVersion` replacement; `incompatibleWith` kept; dependency opt-in with and without an explicit opposite edge; mixed-case and `_steam` references (the 228 shapes) |
| Tiers | Closure into tier 1 and tier 3; claimed by both; framework seed before core seed (conflict reported); hard edge across tiers raises the tier; disjoint and exhaustive partition |
| Catalogue | One golden fixture per code in section 6; registry test equals the table; message slots all filled; ignorable flags enforced |
| Version | Fixtures from LO-020 |
| Owner fixture (ignored) | Canonical: 0 violations; game-style: 0 violations and fewer moved mods than canonical by the subsequence measure; preview under 100 ms; one recalculation under 1 ms |
| Incremental | Random command sequences: incremental diagnostics equal full recompute after every step, including undo and redo |
| Explain | Golden tests for the five answer kinds; a three-edge chain; stale-revision detection |
| Preview and undo | Apply, undo, redo, apply; stale token refused; preview does not push undo |
| Manual moves | Hard block with named rule; `warn` mode; allowed range correct on a fixture |
| Benchmarks | The seven benchmarks of section 9, tracked in CI |

Acceptance summary: AC1 identical results across runs, shuffles and thread counts; AC2 no active mod is ever lost by a sort; AC3 every dropped edge, suppression and tier conflict is reported with all its sources; AC4 the owner's fixture sorts to 0 violations with both algorithms; AC5 one full recalculation of 610 mods with 1,500 constraints under 1 ms; AC6 every code in the catalogue has a golden fixture.

## 12. Owner decisions and open points

Owner decisions.

1. Default framework and core seed lists: the short lists of ids that always load first (section 3). A proposal derived from the most depended-on libraries in the corpus is available at M2; the owner confirms the shipped default.
2. Incompatibility severity default (Error here, Warning in the game's tooltip): confirm the Error default.
3. Whether hard-rule manual moves are blocked (proposed) or only warned by default.

Open points (engineering, not owner).

1. Compare the depth-first game-style sort with the min-index Kahn variant on the 610 fixture by the subsequence measure and switch if the variant moves measurably fewer mods (LO-013).
2. `sort_preview` is a query in the registry but may exceed 1 ms on large lists; promote to a job if the benchmark says so (LO-025, D-045).
3. The research placed `about-force` below `user` in priority and tie-broke by current order; this document differs on both for the reasons given in LO-003 and LO-012, and the research note should be reconciled.
4. The game's version comparison routine and its leniency for `v1.6` strings are unverified ([core domain](../research/rimsort-core-domain.md), open question 2); LO-020 follows the corpus-verified rule.
5. Whether replacement satisfaction of a missing dependency should be silent or shown as Info (currently shown as `args.state = alternative-available` on the Error); verify with users in M2.
6. Registry rows `diagnostics_list`, `diagnostics_mute`, `sort_explain` and `list_move_bounds` are proposed and must be added to `rimstudio-app::registry` with their DTOs.
