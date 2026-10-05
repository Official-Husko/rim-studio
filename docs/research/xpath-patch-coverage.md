# XPath usage in RimWorld patches: coverage analysis and engine decision

This note measures which XPath 1.0 constructs real RimWorld patches use, verifies from the decompiled game code how the game evaluates and uses them, and recommends how RimStudio's patch tooling (patch simulator, def explorer, "what does my patch change" preview, patch validation) should evaluate XPath. It covers the install, Combat Extended (CE), 690 Steam workshop folders and the owner's own mods. It does not cover the content of `<value>` payloads or the Defs inheritance engine (see `docs/research/def-engine-semantics.md`).

Status: research note | Last verified: 2026-10-04

## 1. Summary of findings

| Question | Answer | Evidence |
|---|---|---|
| How does the game evaluate xpath? | Every path-based PatchOperation calls `XmlDocument.SelectNodes` or `SelectSingleNode` with the raw string, against one unified `Defs` document. | decompiled:Verse/PatchOperationReplace.cs (ApplyWorker), PatchOperationAdd.cs, PatchOperationConditional.cs, PatchOperationTest.cs |
| How many expressions in the corpus? | 183,620 xpath occurrences, 85,859 distinct, in 14,102 patch files from 421 mod units. | `docs/research/data/xpath-corpus/summary.json` (`tiers`) |
| What does an in-house subset cover? | Subset A 89.7%, B 97.2%, C 99.82% of all occurrences; C covers 99.94% of distinct valid expressions. Everything else valid (D) is 88 occurrences. | same file, `tiers.cumulative_occurrences` |
| Are there expressions the game rejects? | 249 occurrences (0.14%). 240 of them are unexpanded template variables such as `{EggsToPatch}` that a mod framework substitutes before the game sees them. | `xpath.invalid_reasons` |
| Does a Rust XPath crate fit? | No maintained crate offers both mutation of its own tree and exact .NET behaviour. Recommendation: in-house evaluator (full XPath 1.0 parser, evaluator staged B then C, D later) over RimStudio's own node tree, with crates and Mono used only as test oracles. | sections 6 and 7 |

## 2. How the game uses xpath (verified in decompiled code)

1. Patches are loaded per mod from `Patches/` folders (through LoadFolders for versioned mods) and applied to one merged document by `LoadedModManager.ApplyPatches`, which loops over every running mod's patches in load order and catches and logs exceptions per patch (decompiled:Verse/LoadedModManager.cs, ApplyPatches).
2. `CombineIntoUnifiedXML` builds a new document with an empty `Defs` root element and imports every child of each Def file's `Defs` root into it (decompiled:Verse/LoadedModManager.cs, CombineIntoUnifiedXML). This is why the dominant expression form is the relative `Defs/ThingDef[...]` (the context node is the document node, so `Defs` is a child step) and why `/Defs/...` also works. 63.9% of corpus expressions start with `Defs/`, 35.0% with `/Defs/`, 0.94% with `*/` (any root element child), and only 21 expressions start differently (`root_forms`).
3. Def and patch files are read with `XmlReaderSettings` that ignores comments and insignificant whitespace and disables character checking (illegal XML characters tolerated), after stripping a UTF-8 BOM and decoding bytes as UTF-8 (decompiled:Verse/LoadableXmlAsset.cs, constructor taking a FileInfo). About.xml and LoadFolders.xml go through a different path, `XmlDocument.LoadXml` on the file text with no such settings (decompiled:Verse/DirectXmlLoader.cs, ItemFromXmlString; decompiled:Verse/ModMetaData.cs line near `ItemFromXmlFile<ModMetaDataInternal>`), so they are parsed strictly: any exception yields a default object and a logged error.
4. Result use differs per operation class (how the node-set is consumed matters for the data model):

| Class | Call | Use of result |
|---|---|---|
| PatchOperationAdd, Insert, AttributeAdd, AttributeSet, AttributeRemove, AddModExtension | `SelectNodes`, iterate live | Mutates each matched node (append or prepend imported children, set attributes, wrap in a modExtensions list). Success is "at least one match". |
| PatchOperationReplace, Remove, SetName | `SelectNodes(...).Cast<XmlNode>().ToArray()` | Snapshot first, then replace or remove each node through its parent. |
| PatchOperationConditional, Test | `SelectSingleNode` | Only existence of a first match matters; `Conditional` runs `match` or `nomatch`. |
| PatchOperationSequence, FindMod | no xpath | Compose other operations. |

5. The `success` field (`Normal`, `Always`, `Invert`, `Never`) rewrites the boolean after the worker returns (decompiled:Verse/PatchOperation.cs, Apply). In the corpus `Always` appears on 8,779 operations, `Invert` on 132, `Normal` explicitly on 69 (`operation_classes.success_field_values`).
6. XPath is not used by the inheritance resolver: `XmlInheritance` clones parent nodes and matches by `Name` and `ParentName` attributes, with no SelectNodes call (grep of decompiled:Verse/XmlInheritance.cs finds only `CloneNode`). The only other SelectNodes use outside patches is `DirectXmlLoader.cs` selecting children by type name.
7. The engine is whatever XPath the game's .NET runtime ships (Unity Mono's System.Xml, which implements XPath 1.0 only): no variables, no namespaces in practice (the unified document has none), no XPath 2.0 functions. (That the game ships Mono, not .NET Core, is stated here from knowledge of Unity and is unverified in this repo; the results below are consistent with it because Mono's engine was used as the oracle.)

## 3. Method

1. `docs/research/data/xpath-corpus/xpath_coverage.py` locates every folder named `Patches` (any depth and letter case) under four corpora, parses each XML file with the game's loader rules, keeps files whose root is `Patch`, walks `Operation` elements (including nested `operations`, `match`, `nomatch`, never `value`), extracts each `xpath` text and the `Class`, then runs a full XPath 1.0 lexer and parser over it and tabulates features. Run on 2026-10-04 (14.3 s): `python3 xpath_coverage.py --ce <CombatExtended-Development> --owner "<RimWorld Mods>"`, writing `summary.json` next to it (no raw corpus stored).
2. Parser validation: all 85,859 distinct expressions were also fed to libxml2 through lxml, with zero accept/reject disagreements (`parser_validation`).
3. Evaluation oracle (scratch only): an unified Defs document for the owner's active mod list (14,830 files, 110 MB) was built by `build_unified.py`, and a stratified sample of 12,433 distinct expressions (all of tiers B, C, D and X plus 3,000 random tier A) was evaluated by Mono's System.Xml (reference), lxml/libxml2, and sxd-xpath 0.4.2. Each result is reduced to node count plus a hash of node kind, name, depth and value so results compare without storing nodes.

Corpus size:

| Source | Units with Patches | Patch files | Reachable for 1.6 (by version folders and LoadFolders) |
|---|---|---|---|
| Install Data (Core and 5 DLC) | 6 | 14 | 14 |
| CE source (Patches plus ModPatches/*/Patches) | 1 unit, 765 Patches dirs | 3,795 | 3,793 |
| Steam workshop (690 folders) | 398 | 10,251 (6 unparseable) | 7,399 |
| Owner mods | 16 | 36 | 30 |

Caveat: counts include patch files that are not reachable for 1.6 (other version folders), because mod authors' habits matter for tooling too. CE contributes 37,396 + 4,268 + 490 + 12 + 4 = about 42,000 occurrences with many near-duplicate expressions, so "distinct" figures are the fairer measure of variety.

## 4. What the patches contain

### 4.1 Operation classes

117 distinct classes. 191,383 operations use a vanilla class, 13,996 a custom class (1,511 of those carry an xpath). Operations nest up to depth 5 (`nesting_depth_counts`: 120,862 at depth 0, 37,573 at 1, 30,507 at 2, 12,044 at 3, 2,761 at 4, 1,632 at 5). 673 operations carry `MayRequire` attributes. No operation has more than one `xpath` child; `fromxpath` (42) and `onlyXpathExists` (6) appear as extra child names for custom classes.

| Vanilla class | Operations | With xpath |
|---|---|---|
| PatchOperationReplace | 68,395 | 68,395 |
| PatchOperationAdd | 62,631 | 62,629 |
| PatchOperationConditional | 26,794 | 26,794 |
| PatchOperationRemove | 12,855 | 12,855 |
| PatchOperationAddModExtension | 10,267 | 10,267 |
| PatchOperationSequence | 5,384 | 0 |
| PatchOperationFindMod | 3,887 | 0 |
| PatchOperationAttributeSet | 636 | 636 |
| PatchOperationTest | 306 | 306 |
| PatchOperationInsert | 191 | 191 |
| PatchOperationAttributeAdd, AttributeRemove, SetName | 24, 7, 2 | all |

Custom classes (top by count; all are defined in mod assemblies, not in the game):

| Class | Operations | With xpath | Note |
|---|---|---|---|
| CombatExtended.PatchOperationMakeGunCECompatible | 9,727 | 0 | CE helper, takes a def name instead of an xpath |
| NQualityOfLife.XML.* (Log, Add, AddOrReplace, PatchIfExists, AddDeep, Replace, Remove, ...) | about 2,400 | most | A framework with its own xpath semantics and template variables like `{EggsToPatch}` |
| XmlExtensions.OptionalPatch and friends | 475 + 84 + 52 | 0 | Settings-driven patch wrappers |
| SafePatcher.PatchOperationSetModExtension | 370 | 370 | |
| TweaksGalore.*, ModSettingsFramework.*, LWM.DeepStorage.PatchMessage | 151, 96, 70, 54 | 0 | |

Implication: a patch simulator must treat unknown classes as opaque (report "custom operation, not simulated") and never fail the whole file. Only 1,511 custom operations (0.7%) would be affected by that for xpath evaluation.

### 4.2 Expression shape

| Metric | Value |
|---|---|
| Valid / invalid occurrences | 183,371 / 249 |
| Expression length (chars) | p50 60, p90 103, p99 258, max 13,228 |
| Steps per expression | p50 4, p90 7, p99 14, max 270 |
| Predicates per expression | p50 1, p90 2, p99 4, max 26 |
| Predicate nesting depth | 0: 963; 1: 182,088; 2: 313; 3: 6; 4: 1 |
| With a newline / leading or trailing whitespace | 6,420 / 4,902 (must be tolerated and trimmed like .NET does) |
| Namespace prefixes, variables, unknown functions | none |

Most common shapes (literals collapsed to `S`): `Defs/ThingDef[defName='S']` 6.87%, `.../statBases` 5.59%, `.../tools` 3.47%, `/Defs/ThingDef[defName='S']` 2.61%, `.../comps` 2.20%, `.../statBases/ArmorRating_Sharp` 2.14% (`top_shapes`). The item designer's "generate CE patch" output belongs to exactly this family.

### 4.3 Axes, node tests, predicates, functions

| Feature | Occurrences | Expressions | Share of valid |
|---|---|---|---|
| child axis (any step) | 890,984 | 183,371 | 100% |
| attribute axis (`@a`) | 16,256 | 14,836 | 8.09% |
| `//` (descendant-or-self) | 798 | 798 | 0.44% |
| `.` (self) | 461 | 390 | 0.21% |
| `..` (parent) | 48 | 24 | 0.01% |
| explicit `self::` / `parent::` / `descendant::` | 13 / 2 / 1 | 5 / 2 / 1 | under 0.01% |
| ancestor, following, preceding, sibling, namespace axes | 0 | 0 | 0% |
| name test | 899,943 | 183,368 | 100% |
| `text()` test | 4,832 | 2,720 | 1.48% |
| `*` wildcard | 2,481 | 2,480 | 1.35% |
| `node()`, `comment()`, `processing-instruction()` | 0 | 0 | 0% |

| Predicate kind | Occurrences | Expressions | Share of valid |
|---|---|---|---|
| child-text equality `[defName="x"]` | 232,612 | 174,451 | 95.14% |
| attribute equality `[@Class="x"]` | 16,191 | 14,813 | 8.08% |
| `text()` equality | 2,949 | 1,154 | 0.63% |
| positional `[n]` | 2,244 | 2,187 | 1.19% |
| child existence `[c]` | 1,564 | 1,314 | 0.72% |
| nested path existence `[a/b]` | 593 | 385 | 0.21% |
| `contains()` | 465 | 165 | 0.09% |
| self equality `[.="x"]` | 424 | 353 | 0.19% |
| nested path equality `[a/b="x"]` | 105 | 89 | 0.05% |
| `starts-with()` | 70 | 25 | 0.01% |
| function result equality | 34 | 12 | 0.01% |
| numeric equality / two non-literals | 22 / 22 | 22 / 22 | 0.01% |
| `last()` | 14 | 14 | 0.01% |
| attribute existence `[@a]` | 23 | 15 | 0.01% |

| Function | Occurrences | Expressions |
|---|---|---|
| `not` | 1,760 | 1,406 |
| `contains` | 465 | 165 |
| `starts-with` | 70 | 25 |
| `string` | 35 | 13 |
| `name` | 21 | 10 |
| `normalize-space` | 15 | 3 |
| `last` | 14 | 14 |
| `position` | 4 | 4 |
| `translate` | 2 | 1 |

| Operator | Occurrences | Expressions |
|---|---|---|
| `=` | 252,363 | 182,311 |
| `or` | 38,888 | 11,324 |
| `and` | 521 | 443 |
| union `\|` | 286 | 119 |
| relational `< <= > >=` | 10 | 7 |
| `!=` | 1 | 1 |

Arithmetic, `count()`, `sum()`, `concat()`, `substring*`, `id()`, `lang()`, `number()`, `boolean()` never occur in valid expressions. Observation: the dominant idiom is "child element text equals a literal", i.e. a Def lookup by `defName`.

## 5. Coverage of progressively larger subsets

Definitions (as implemented in the script, `tiers.definition`):

| Tier | Includes |
|---|---|
| A | child steps with name tests, optional final `@attribute`, predicates only of the form `(@attr or child) = "literal"` (several predicates allowed) |
| B | A plus `//`, `.`, `..`, `*`, `and`, `or`, `not()`, `contains()`, `starts-with()`, `!=`, existence and nested-path predicates, numeric equality |
| C | B plus `[n]`, `last()`, `position()`, `count()`, `text()`, relational operators and unions |
| D | any other valid XPath 1.0 (explicit axes, other functions, arithmetic, filter expressions, `node()`) |
| X | rejected by an XPath 1.0 engine (syntax error, unknown function, variable, prefix, non-node-set result) |

| Subset | Occurrences covered | Share of all (183,620) | Share of valid | Distinct covered | Share of distinct valid (85,656) |
|---|---|---|---|---|---|
| A | 164,551 | 89.61% | 89.74% | 76,426 | 89.22% |
| A+B | 178,277 | 97.09% | 97.22% | 83,463 | 97.44% |
| A+B+C | 183,283 | 99.82% | 99.95% | 85,608 | 99.94% |
| A+B+C+D (all valid) | 183,371 | 99.86% | 100% | 85,656 | 100% |
| X (invalid) | 249 | 0.14% | n/a | 203 | n/a |

Per source (share of valid occurrences, cumulative): vanilla A 96.55%, B 96.55%, C 100%; CE A 88.69%, B 98.81%, C 99.97%; workshop A 90.06%, B 96.76%, C 99.95%; owner mods A 78.75%, B 86.25%, C 100%. The owner's own hand-written patches use more `or`, `text()` and positional predicates than the community average, so tier A alone would serve them worst.

Which feature to add first after A (greedy order, `feature_adoption.greedy_after_subset_A`):

| Step | Feature added | Cumulative share of valid |
|---|---|---|
| 0 | tier A baseline | 89.74% |
| 1 | `or` in predicates | 95.09% (solo unlock 9,821 expressions) |
| 2 | `[n]` position | 95.91% (1,467) |
| 3 | `*` wildcard step | 96.84% (970) |
| 4 | `text()` step | 97.60% (1,392) |
| 5 | `text()` equality | 98.15% |
| 6 | `//` | 98.56% |
| 7 | `and` | 98.69% |
| 8 | existence predicate | 98.81% |
| 9 | `not()` | 99.53% |
| 10 | `contains()` | 99.59% |
| 11 | union | 99.65% |
| 12 | nested path equality | 99.69% |
| 13 | other predicates and `self::` | 99.74% |

What tier D contains (all 48 distinct expressions inspected): `defName = * and not(...)` style comparisons, `name()=` and `self::x` choices (`*[self::bluntStunDuration or ...]`), `translate()` lowercasing inside `contains`, `normalize-space()`, `descendant::`, `(/Defs/thingDef | /Defs/ThingDef)/...` parenthesised unions, `string()`, and the XPath truthiness traps below.

### 5.1 Truthiness traps (must be reproduced exactly)

Authors write expressions that are valid XPath but mean something different from what they intended, and the game accepts them. Examples seen in the corpus:

| Expression fragment | What XPath 1.0 actually does |
|---|---|
| `/Defs/RecipeDef[defName = "A" or "B"]/recipeUsers` | The right side of `or` is a non-empty string literal, which is true, so every RecipeDef matches. |
| `Defs/ThingDef[defName="X" and "Y"]/weaponTags` | The string literal is truthy, so it behaves as `[defName="X"]`. |
| `Defs/FactionDef[@Name="N"]/apparelStuffFilter/thingDefs/li["Steel"]` | A string predicate is truthy: selects every `li`. |
| `[defName = defName="RG_X" or ...]` | Left-associative `=` chain compares a boolean with a string, coerced to number. |
| `li[text()=MineablePlasteel]` | Bare name compares text with a child element named `MineablePlasteel` (usually absent, so empty node-set, false). |

The simulator and any linter built on the in-house engine must implement the full XPath 1.0 type-coercion rules (node-set, string, number, boolean conversions and comparison semantics), not a simplified "equality of strings" model, otherwise the preview will disagree with the game on exactly the patches that are accidentally broad. A lint rule "string literal as boolean operand" is a cheap, valuable authoring warning.

## 6. Engine comparison (Rust crates and oracles)

| Engine | Version, last release | Tree it needs | Mutation | Notes |
|---|---|---|---|---|
| In-house evaluator | n/a | RimStudio's own arena tree | native | Full control over node handles, positions, error text |
| sxd-xpath + sxd-document | 0.4.2 (2018-10-31), 0.3.2 (2019-05-26) | sxd-document arena | limited (document is built through a Package, nodes are not freely re-parentable) | Unmaintained for 7 years but complete XPath 1.0 (1.7 s to parse the 110 MB document in the scratch harness) |
| xee-xpath + xot | 0.1.5 (2025-08-21), xot 0.31.2 (2025-04-09) | xot tree | yes (xot is mutable) | Targets XPath 3.1, so it accepts constructs .NET rejects; pulls an ICU stack: building a harness with it alone produced a 736 MB target directory |
| skyscraper | 0.7.0 (2026-05-03) | its own HTML-oriented tree | no | Made for HTML scraping, XPath 1.0 subset, 2 downloads in the last 90 days |
| libxml (libxml2 binding) | 0.3.21 (2026-08-02), MSRV 1.88 | libxml2 C tree | yes (C API) | Complete and very fast, but a C dependency that must be built or bundled on three platforms, with different edge cases from Mono |
| Python lxml (test oracle only) | lxml 6.1.3 / libxml2 2.15.4 | n/a | n/a | Used in this research as a second oracle |
| Mono System.Xml (test oracle only) | via mono and a tiny .exe | n/a | n/a | Closest to the game |

Agreement with the Mono reference on the 12,433-expression sample (`scratch compare.py`):

| Engine | Same count and same node hash | Both rejected | Different |
|---|---|---|---|
| lxml / libxml2 | 12,229 | 203 | 1 (a union with `..` returned 5 nodes versus 4; cause not investigated) |
| sxd-xpath 0.4.2 (random 2,187 of the 12,433, run was stopped early) | 2,160 | 27 | 0 |

Timing on the 110 MB document (4 parallel processes, per expression): Mono median 18.7 ms, p90 46 ms; lxml median 15.6 ms, p90 32 ms; sxd-xpath median 34 ms but p90 about 2.0 s, because some `//` or union expressions are evaluated naively (the run was stopped after 2,187 expressions for that reason). So the one crate that is complete for XPath 1.0 and pure Rust is also the slowest on the long tail.

The point of the comparison: engines agree overwhelmingly on the real corpus, so the choice is driven by integration cost (mutable tree, node identity, no C dependency, error messages with spans) and not by correctness. The only well-fitting engine that mutates is libxml2, which is the wrong trade for a cross-platform Tauri app that already needs a single XML boundary crate.

## 7. Recommendation

Hybrid, with a clear bias to in-house:

1. Write a complete XPath 1.0 parser (the grammar is small: the script's parser is about 410 lines of Python including the lexical disambiguation rules) so every expression is classified exactly as .NET does (valid, invalid, which tier). The parser result is a typed AST usable by the linter, the "explain this xpath" UI and the patch generator.
2. Implement the evaluator over RimStudio's own node tree in stages, each gated by the corpus coverage test: stage 1 tier B (97.2% of occurrences), stage 2 tier C (99.82%), stage 3 the remainder of XPath 1.0 (explicit axes, remaining functions, filter expressions). Include the full conversion and comparison rules from section 5.1 from stage 1.
3. Return a node-set as stable node handles in document order (the game's `SelectNodes` returns document order) and implement Replace, Remove and Add on a snapshot taken before mutation, matching the `ToArray()` versus live-iteration distinction in section 2.
4. Where a construct is not yet supported, the simulator reports "operation not simulated: unsupported xpath feature <name>" rather than guessing. No fallback engine is shipped. sxd-xpath, libxml (through lxml in CI scripts) and Mono stay as dev-time oracles only.
5. Do not adopt xee or skyscraper: the former is a larger dependency than the whole feature and implements a different language version, the latter targets HTML.

Why not a crate: the evaluator must produce node identities in RimStudio's tree (to show diffs and to attribute a change to the patch file and line), so a foreign tree would need a full copy per run; with 110 MB of unified Defs for a 600-mod list, copying is a measurable cost. The in-house evaluator can also run an index (defName to node) that turns the 95% dominant shape `Defs/ThingDef[defName="X"]` from a linear scan into a hash lookup; that is the main performance lever, since mono needs a median of about 18 ms per expression on this document.

## 8. Regression-fixture plan

1. Corpus fixture generator: a committed script (this one, extended) writes `tests/fixtures/xpath/shapes.json`: per distinct shape (literals collapsed), one representative expression with a count and the tier. About 2,000 shapes cover all 85,859 distinct expressions; the file is small. No raw patches are committed (licence hygiene for CE and mod content).
2. Synthetic document: a hand-written mini Defs document (JSON-described, rendered to XML only inside the test) of 30 to 50 defs with duplicates, missing children and mixed `Class` attributes, so each shape has a known expected node count authored by hand.
3. Golden answers: the node count and node-signature hash from Mono (reference) and libxml2 for every shape on the synthetic document, stored as JSON next to the shape file, regenerated only by an explicit script run that requires mono and lxml on the maintainer's machine. CI runs the Rust evaluator against the stored hashes.
4. Truthiness suite: every row of section 5.1 plus the string-to-number and boolean-to-string comparison matrix from the XPath 1.0 spec, one fixture each.
5. Differential job (optional, nightly, local): evaluates the full sample against the user's real unified document, comparing the in-house engine to sxd-xpath as a dev-dependency, and fails on any new disagreement, listing the expression.
6. Property tests (`proptest`): random predicates over the synthetic document, checking that tier B expressions give the same result as the same expression with redundant parentheses or whitespace.
7. Coverage gate: a test that parses all shapes and asserts the share handled by the evaluator is at least 99.8% of occurrences, and that unsupported shapes are listed in a checked-in allowlist.

## Implications for RimStudio

1. The patch engine's input is a single unified `Defs` document; the context node for an xpath is the document node and `Defs/...` and `/Defs/...` must both work.
2. Ship one in-house XPath 1.0 parser plus evaluator crate (suggested name `rimstudio-xpath`, depending only on RimStudio's node tree), not a third-party engine.
3. Evaluator milestone gates by corpus coverage: B at least 97% of occurrences, C at least 99.8%, then full XPath 1.0.
4. Implement XPath 1.0 coercion semantics exactly, with a fixture per row of section 5.1.
5. Trim whitespace and newlines inside xpath text before parsing, as 6,420 expressions contain newlines.
6. Return results in document order and snapshot before mutating for Replace, Remove, SetName.
7. Parse every operation class generically (any child with `Class`); unknown classes are reported, not failed.
8. Build a defName hash index per Def type to make the dominant `Defs/Type[defName="x"]` shape constant time.
9. Offer authoring lints: string literal used as boolean, `defName = A or "B"`, expression that matches nothing in the current load order, expression with unexpanded `{variable}` placeholders.
10. Keep the corpus script, the shape file and the golden hashes in the repo (small); never commit raw patches.

## Open questions

1. Which XPath implementation does the shipped Unity runtime of RimWorld 1.6 use (Mono System.Xml or a newer one)? The one disagreement between Mono and libxml2 (a union containing `..`) should be checked against the game.
2. The ordering and de-duplication rule for unions that include `..` was not investigated.
3. Whether custom frameworks (NQualityOfLife.XML, SafePatcher, XmlExtensions) should be simulated at all, and with which of their own semantics for `{variables}`.
4. Reachability: the counts include patch files in non-matching version folders; a second table restricted to files reachable for 1.6 under LoadFolders and the owner's active mod list would sharpen the numbers.
5. How should the simulator treat patches whose application depends on other mods' operations that are not simulated (order dependence)?
