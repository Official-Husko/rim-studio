# RimWorld Def loading pipeline: precise semantics and reference engine

Scope: how RimWorld 1.6 (build 4871 rev598) turns the XML of an ordered list of mods into resolved defs: mod folder selection, document merging, PatchOperations, XML inheritance, MayRequire handling, duplicates and type names. It documents the executable Python specification prototype under `docs/research/data/def-engine/` (usage in its `README.md`) and the facts it was verified against: the decompiled game code, a Mono probe of `XmlDocument` behaviours, hand-traced vanilla defs, and full loads of vanilla (Core plus five DLC) and vanilla plus Combat Extended. It is the specification for RimStudio's Rust defs crate (requirements R3, R5, R7, R10, R11).

Status: research note | Last verified: 2026-10-04

Conventions: `decompiled:Verse/X.cs (Method)` points at the ILSpy output of Assembly-CSharp (Ludeon code, never copied; behaviour is described in own words). Paths without a prefix are relative to the repository root. "Vector" means a test vector in `docs/research/data/def-engine/tests/vectors/*.json`. Game fact = verified in decompiled code; probe = verified by running `docs/research/data/def-engine/probes/Probe1.cs` on Mono 6.12 (Unity ships an older Mono, so probe results are "very likely, not proven" for the game itself); (unverified) = from memory or the wiki only.

## 1. Pipeline overview

The game runs one function, `LoadedModManager.LoadAllActiveMods` (decompiled:Verse/LoadedModManager.cs), and the stages relevant here happen in this fixed order. Every stage is a function in the prototype.

| # | Game stage | What it does | Prototype |
| --- | --- | --- | --- |
| 1 | `InitializeMods`, `ModContentPack.InitLoadFolders` | one content pack per active mod, in load order; each decides which folders it loads | `mods.init_load_folders` |
| 2 | `LoadModXML`, `DirectXmlLoader.XmlAssetsInModFolder` | read `Defs/**/*.xml` of every pack, de-duplicated by relative path across the pack's folders | `mods.xml_assets_in_mod_folder` |
| 3 | `CombineIntoUnifiedXML` | deep-import every top-level node of every file into one `<Defs>` document, remembering its file | `engine.load_game` (step 2) |
| 4 | `ModContentPack.LoadPatches`, `ApplyPatches` | parse `Patches/**/*.xml` of each pack, apply the operations of all packs in order to the unified document | `patches.py`, `engine.load_game` (step 3) |
| 5 | `ParseAndProcessXML`: `XmlInheritance.TryRegister`, `Resolve` | register nodes with `Name` or `ParentName`, link parents, resolve top-down | `inherit.Inheritance` |
| 6 | `DefFromNodeNew` | per node: MayRequire / MayRequireAnyOf, Abstract, type from element name or `Class`, parse C# fields | `engine.load_game` (step 5), fields not parsed |
| 7 | `DefDatabase<T>.AddAllInMods` (called per Def subclass by `PlayDataLoader`) | per-type databases with override-by-defName rules | `engine.build_database` |

```mermaid
flowchart LR
  A[active mods in load order] --> B[load folders per mod]
  B --> C[Defs files, dedup by relative path]
  C --> D[one unified Defs document]
  D --> E[patches of all mods in order]
  E --> F[register Name/ParentName nodes]
  F --> G[resolve inheritance]
  G --> H[per node: MayRequire, Abstract, type]
  H --> I[DefDatabase per type]
```

Two consequences shape everything below. First, all `Defs` of all mods are merged BEFORE any patch runs, so a patch of mod 2 can change a def of mod 5 (vector `patch-order-across-mods`). Second, inheritance runs AFTER patching, so patches edit raw, unmerged nodes: patching a parent affects every child, patching a child never affects the parent.

## 2. Mod folders, LoadFolders.xml and version resolution

Mods are processed in the order of `ModsConfig.ActiveModsInLoadOrder`; the position becomes `ModContentPack.loadOrder` (the index used everywhere below). Core is first in a normal setup (the ordering itself lives in `ModsConfig` and was not studied here, see open question 3). A mod whose root folder is missing is deactivated with a warning (decompiled:Verse/LoadedModManager.cs `InitializeMods`).

### 2.1 LoadFolders.xml parsing (decompiled:Verse/ModLoadFolders.cs, LoadFolder.cs)

* Each child element of the root is a version key: lower-cased, and one leading `v` removed (`<v1.6>` and `<V1.6>` both mean `1.6`, `<default>` means `default`). Repeated keys merge into one list.
* Each `li` child is a folder entry. Text `/` or `\` means the mod root (empty folder name); other text has the alternate directory separator replaced.
* Attributes: `IfModActive` is any-of, `IfModActiveAll` is all-of, `IfModNotActive` is "none of"; each is a comma list, every item trimmed. A folder is loaded when (any-of empty or one active) AND (all-of empty or all active) AND NOT (not-active non-empty and one active). Package ids are compared case-insensitively; the `_steam` postfix the game appends to duplicated workshop copies is ignored by these "NoSuffix" lookups (`ModLister.AnyModActiveNoSuffix`).

### 2.2 Choosing the folder list (decompiled:Verse/ModContentPack.cs `InitLoadFolders`)

1. If the mod has a LoadFolders.xml with at least one version key:
   1. Look for the key equal to `CurrentVersionString` (for this build `1.6.4871`). It almost never exists, because mods write `1.6`.
   2. Otherwise take every key that contains a dot, is not `default`, parses as a version, and is not newer than the running version; sort the raw key STRINGS descending and take the first. Caveat: string order, so `1.10` would sort below `1.6` (the game bug is faithfully reproduced).
   3. Otherwise use the `default` key.
   4. The first list that exists and is non-empty wins and the function returns, even if no entry passes its IfMod conditions (the mod then loads nothing and the game logs "did not load any content").
2. Otherwise (no usable LoadFolders.xml): the folder named like the running `major.minor` (`1.6`) if it exists; else the closest version folder that is not newer than the game (the highest folder not above the game version; if all are newer, the lowest one is used; the folder name is rebuilt from the parsed version); then `Common`; then the mod root.
3. The resulting list is stored in descending priority: entries of a LoadFolders list are walked from the LAST to the first, so the last entry has the highest priority.

Real example: Combat Extended's LoadFolders.xml has a `v1.6` list starting with `/` followed by `Royalty`, `Ideology`, `Biotech`, `Anomaly`, `Odyssey` (each with IfModActive on the DLC package id) and about 760 `ModPatches/...` entries. With the five DLC active the engine reports the folders `Odyssey, Anomaly, Biotech, Ideology, Royalty, .` in that priority order (`golden/real_data_summary.json`, key `ce.load_folders`).

### 2.3 File discovery and de-duplication (decompiled:Verse/DirectXmlLoader.cs `XmlAssetsInModFolder`)

* Folders are visited from the highest priority. For each folder, every `*.xml` below `<folder>/Defs/` (or `Patches/`) is collected recursively; names starting with `.` are skipped.
* The key of a file is its path relative to its load folder (including the `Defs/` prefix). The first occurrence of a key wins (`TryAdd`), so a file in a higher-priority folder HIDES the same relative path in lower ones, without any duplicate error (vector `file-dedup-between-folders`).
* The result keeps insertion order, so the files of the highest-priority folder come FIRST in the merged document, and the mod root (lowest priority) comes last. Version folders therefore load before Common before the root (vector `load-folders-and-version-folders`, whose expected database order is `NoFoo, Roy, R` for a mod with entries `/`, `Royalty`, `NoFoo`).
* Order of files inside one folder is whatever `GetFiles` returns: filesystem order, not sorted. NTFS happens to give alphabetical order; on Linux ext4 it is hash order. The prototype sorts by case-folded name, files of a directory before its subdirectories. This only matters for duplicate resolution inside one mod and for patch order inside one mod. (unverified for Unity's Mono whether subdirectories are visited after the files of the parent.) Rust must pick one deterministic rule; ordinal sort of the relative path is the recommendation.
* Whether `*.xml` matching is case-insensitive on Linux is not verified; the prototype matches case-insensitively.

## 3. Merging into one document, and provenance

`CombineIntoUnifiedXML` creates a fresh `<Defs>` root and, for every parsed file in order, deep-imports each child of the file's document element and appends it. A file whose root element is not named `Defs` only logs an error: its children are still imported (vector `defs-bad-root-parse-error-unknown-type`). A file that failed to parse has no document and is skipped with an "unknown parse failure" error.

How a file is read (decompiled:Verse/LoadableXmlAsset.cs):

* A UTF-8 byte order mark is stripped; the bytes are decoded as UTF-8 regardless of the declared encoding.
* The XML reader drops comments and whitespace-only text nodes (outside `xml:space="preserve"`), has `CheckCharacters` off, and has the .NET default of prohibiting DTDs (a DOCTYPE is a parse error).
* CDATA stays a distinct node type (probe). This matters in one place: the inheritance merge only counts `Text` nodes as text (section 6).

Provenance. The game remembers `(ModContentPack, file)` per imported top-level node in a dictionary keyed by node (`assetlookup`). Nodes created by patches (a Replace of a whole def, an Add at `/Defs`) are not in it. Consequences, all game behaviour:

1. Their `mod` is null. In inheritance they behave as a pseudo-mod: a null-mod child prefers a null-mod parent, and a mod child falls back to a null-mod parent (section 6.2; vector `patch-created-nodes-have-no-mod`).
2. Their defs go into `LoadedModManager.patchedDefs` instead of a mod's list and are added to the databases AFTER all mods, overriding anything of the same name (vector `patch-replace-whole-def` expects database order `Z, R` for a replaced `R`).

The prototype records, per def: `mod` and `file` (null for patch-created), the parent chain (`mod:Name` entries from the nearest parent upwards), and `patched_by` (one entry `mod:file#index Class(xpath)` for every patch operation that touched the def's subtree, found by walking up from each node an operation mutated). This is def-level provenance. A field-level provenance (which mod or patch produced which element) is not tracked and is the main extension RimStudio needs for its explorer (section 10, implication 6).

## 4. PatchOperations

### 4.1 Loading and order

`ModContentPack.LoadPatches` (decompiled:Verse/ModContentPack.cs) reads `Patches/**/*.xml` with the same folder rules as Defs. The document element must be `Patch` and its element children must be `Operation`; anything else logs an error and is skipped. Each `Operation` is deserialised into the class named by its `Class` attribute. Operations are applied mod by mod in load order, file by file, operation by operation, each through `PatchOperation.Apply` inside a try/catch (decompiled:Verse/LoadedModManager.cs `ApplyPatches`). `MayRequire` on a top-level `Operation` is not consulted; it is honoured on the `li` entries of the lists below. A patch file with a parse error has no document and the loader dereferences it (deduced from the code, not observed: the game would throw while building the patch list); the prototype logs `patch_file_unreadable` and skips it.

### 4.2 The common wrapper

`Apply` runs the class's worker and then rewrites the result by the `success` field: `Normal` keeps it, `Invert` negates, `Always` forces true, `Never` forces false. The change itself is not undone by `Never` (vector `patch-success-modes`). Only top-level operations are checked at the end: `Complete` logs "Patch operation failed" for every top-level operation whose result was false; nested operations are not reported separately. Exceptions escaping a worker (missing `value`, invalid XPath, attribute or text nodes where an element is needed) are logged as "Error in patch.Apply()" and abort the enclosing Sequence, Conditional or FindMod up to the top level (vector `patch-exceptions`).

### 4.3 The thirteen vanilla classes

All verified in decompiled:Verse/PatchOperation*.cs. `xpath` is evaluated with `SelectNodes` on the unified document (context rules in section 5). "Targets" are the nodes the expression selects.

| Class | Fields | Result is true when | Behaviour worth knowing |
| --- | --- | --- | --- |
| `PatchOperation` (base, also used for unknown classes) | `success` | never (logs an error) | `success=Always` or `Invert` still applies, so an unknown class with `Always` succeeds |
| `PatchOperationAdd` | `xpath`, `value`, `order` (Append default, Prepend) | at least one target | copies every child node of `value`; Append adds at the end of each target; Prepend adds at the start keeping the value's order; a target that is an attribute gets text appended to its value; a text node target throws |
| `PatchOperationInsert` | `xpath`, `value`, `order` (Prepend default, Append) | at least one target | copies are inserted next to the target node (before for Prepend, after for Append). With several value children the copies end up REVERSED in both modes (vector `patch-insert-order`: values `x y` before `a` give `y x a`); with one child there is no visible effect |
| `PatchOperationReplace` | `xpath`, `value` | at least one target | for each target of a snapshot list: inserts the value's children before it, then removes it; the value may have zero or several children; a text node target (`.../text()`) is replaceable |
| `PatchOperationRemove` | `xpath` | at least one target | snapshot, removes each |
| `PatchOperationAddModExtension` | `xpath`, `value` | at least one target | finds the target's first `modExtensions` child, creating it at the END of the target when missing, and appends the value's children (vector `patch-addmodextension`) |
| `PatchOperationAttributeAdd` | `xpath`, `attribute`, `value` | at least one target lacked the attribute | does nothing where the attribute exists; the new attribute is appended last |
| `PatchOperationAttributeSet` | same | at least one target | creates or overwrites |
| `PatchOperationAttributeRemove` | `xpath`, `attribute` | at least one target had it | |
| `PatchOperationSetName` | `xpath`, `name` | at least one target | replaces the element by a new one with the same inner XML; ALL attributes are dropped (vector `patch-setname`) |
| `PatchOperationTest` | `xpath` | the expression selects something | no side effect |
| `PatchOperationConditional` | `xpath`, `match`, `nomatch` | see below | uses `SelectSingleNode` |
| `PatchOperationFindMod` | `mods` (list of display NAMES), `match`, `nomatch` | see below | compares with `ModMetaData.Name`, exact and case-sensitive, NOT with package ids (vector `patch-findmod`) |
| `PatchOperationSequence` | `operations` | every step returned true | stops at the first false; earlier steps stay applied (no rollback); an empty or missing list: missing is an exception, empty is true |

Conditional: if the xpath selects something and `match` exists, the result is `match`'s result; if it selects nothing and `nomatch` exists, `nomatch`'s result. In the remaining cases (selected but no `match`, or not selected and no `nomatch`) the result is: when `match` is absent, whether `nomatch` exists; otherwise true. Truth table in vector `patch-conditional-test-sequence`: no branches at all gives false; only `match` and no hit gives true; only `nomatch` and a hit gives true. FindMod: with `match` and a mod found, `match`'s result; with `nomatch` and none found, `nomatch`'s result; in every other case true.

### 4.4 Class lookup, unknown classes and field parsing

The `Class` attribute goes through the same type search as defs (section 8) with `Verse` as the ambiguity namespace; the search is case-insensitive for namespace-qualified and ignored-namespace names, so `PatchOperationadd` and `Verse.PatchOperationAdd` both work (vector `patch-class-case-tolerance`; CE ships 9 operations spelled `PatchOperationreplace`). A missing `Class` is the base class. A class that cannot be found logs "Could not find type" and the object becomes the BASE `PatchOperation` (decompiled:Verse/DirectXmlToObject.cs `ClassTypeOf`), whose fields are only `success`; every other child element is reported as an unknown field. Such an operation fails at apply time with "Attempted to use PatchOperation directly" (vector `patch-unknown-class`).

For fields, names match exactly, then case-insensitively with an error; `value` is kept as the `value` XML node itself; `order` and `success` are enums parsed case-sensitively (an invalid value logs an error and the default stays). `li` entries of `operations` and `mods` honour `MayRequire` (all-of) and `MayRequireAnyOf` (any-of); when `MayRequire` is present and non-empty it is the only one tested (vector `mayrequire-in-patch-lists`).

The prototype's rule for unknown custom classes: the operation is the base class, `patch_unknown_class` (warning) and `patch_base_class` and `patch_failed` (errors) are counted, and the raw `Operation` element is kept in the patch event so its parameters stay readable. Custom classes can be registered (`LoadConfig.custom_ops`); `plugins.py` shows `CombatExtended.PatchOperationSettingsConditional`.

## 5. XPath: lxml versus .NET

The game calls `XmlDocument.SelectNodes(xpath)` / `SelectSingleNode`. Both lxml and .NET implement XPath 1.0, so axes, predicates, `text()`, `@attr`, `position()`, `last()`, `starts-with`, `contains`, `and`, `or`, `not` behave alike. Differences that mattered (the first two are decisive):

| Topic | .NET (game) | lxml (prototype) | Handling |
| --- | --- | --- | --- |
| Context node | the DOCUMENT, so `Defs/ThingDef[...]` equals `/Defs/ThingDef[...]` (probe: `'Defs/A'` and `'/Defs/A'` both give 2, `'A'` gives 0) | the root ELEMENT, where `Defs/ThingDef` would look for a nested `Defs` | `xpath_net.to_document_context` rewrites a location path that is relative at predicate depth 0 into an absolute one, using a tokenizer that follows the XPath 1.0 disambiguation rules. 1657 of the 1677 xpath expressions in Combat Extended's `Patches/` folder are in the relative form (ad hoc count, 2026-10-04), so this is not optional |
| Iteration | lazy: the loops of Add, Insert, AddModExtension, AttributeX walk the live result while mutating (probe: appending a matching node during iteration kept extending the loop, 21 visits for 3 nodes before the probe's guard stopped it). Replace, Remove, SetName take a snapshot first | eager node-set | eager everywhere; differs only for a patch whose own changes add nodes that match its own xpath. Rust should also be eager and report a diagnostic for expressions of this shape (open question 5) |
| Non node-set results | `SelectNodes("count(//a)")`, `("string(...)")` and `("")` throw `XPathException` (probe) | numbers and strings are returned | treated as exceptions (`patch_exception`) |
| Attribute and text nodes | selected attribute nodes have no parent (probe), text nodes are real nodes | `_ElementUnicodeResult` objects | wrapped in `AttrRef` and `TextRef`; operations raise `PatchException` where .NET would throw |
| Adjacent text | `Normalize` merges after Insert on a text target (probe) | lxml merges text always | no difference visible after normalisation |

## 6. XML inheritance

Source: decompiled:Verse/XmlInheritance.cs. Implementation: `defengine/inherit.py`.

### 6.1 Registration

For every top-level element of the (patched) unified document, in document order: skip the node when it has neither `Name` nor `ParentName`, or when it has a `MayRequire` attribute whose ids are not ALL active (this check does not lower-case, but id lookup does). A `Name` that is already registered for the SAME mod (compared by mod identity, so two patch-created nodes also count as one mod) is an error and the node is not registered (vector `inherit-duplicate-name-same-mod`). `MayRequireAnyOf` is not consulted at registration, only at def creation, so a parent whose only requirement is an `AnyOf` is still a valid parent.

### 6.2 Choosing the parent (`GetBestParentFor`)

For a child node with `ParentName = P`, among all registered nodes named `P`:

| Child's mod | Choice |
| --- | --- |
| a real mod with load order `n` | the candidate with the HIGHEST load order that is `<= n` (so its own mod wins, then earlier mods); if none, the first candidate without a mod (patch-created); if none, error "Could not find parent" |
| none (patch-created) | the first candidate without a mod; else the candidate with the LOWEST load order |

A parent that exists only in a later mod is therefore invisible, even though it exists (vector `inherit-parent-by-load-order`: Core's child cannot see a parent defined by two later mods). Real data: in the vanilla plus Combat Extended load 7165 of 17080 defs have a parent, 2855 of those have at least one ancestor in a different mod (DLC defs derive from Core bases, Combat Extended defs from vanilla bases), and the deepest chain has 8 levels.

### 6.3 Resolution order

Nodes whose parent is null are roots; the game resolves each root and then its children depth first, in the order the children were linked (document order). A node with a missing parent is a root (it resolves to itself after the error). Cycle members are never reached from a root: each gets a "Cyclic inheritance" error, stays unresolved, is not stored in the resolved table, and later `GetResolvedNodeFor` returns the ORIGINAL node with another error, so the def still exists, unmerged (vector `inherit-cycle`). The order never changes results for acyclic trees.

### 6.4 The merge (`RecursiveNodeCopyOverwriteElements(child, current)`)

`current` starts as a deep clone of the resolved parent. `child` is the raw (patched) node. Steps in order, first match wins:

1. If `child` has `Inherit` whose lower-cased value is `false`: remove all children of `current`, append deep copies of all child nodes of `child` (text included), then append every attribute of `child` except `Inherit` to `current`. The attributes of `current` are NOT cleared, so a def-level `Inherit="false"` keeps the parent's `Name` and `Abstract="True"` on the resolved node (vector `inherit-false-root-keeps-parent-attrs`; harmless because `Abstract` is read from the original node). Nested `Inherit="False"` (vector `inherit-false-nested`) simply replaces that element's content.
2. Otherwise remove ALL attributes of `current` and copy all attributes of `child`. So `Name`, `Abstract`, `Class` and `Foo` of the parent are not inherited (vector `inherit-basic-merge`), and a `Class` must be repeated in every child that needs it (vector `class-attribute-and-type-names`).
3. If `child` has a child of node type `Text` (the LAST such node): clear `current`'s children and append a copy of that text. Quirk, verified by probe: the game clears with a `foreach` over the live child list while removing, which removes only the FIRST child node; for the usual case of a single text child nothing is visible, but a text child over a parent with several children leaves the rest in place (vector `inherit-text-over-multichild-quirk`: parent `<x><a/><b/><c/></x>` plus child `<x>t</x>` gives `<x><b/><c/>t</x>`).
4. Else, if `child` has no element children (it is empty): when `current` has element children nothing happens (`<statBases/>` keeps the parent's stats); otherwise the first child node of `current` (its text) is removed, so `<label/>` over `<label>x</label>` empties it (vector `inherit-empty-element`).
5. Else, for each element child `c` of `child` in order: if `c` is a list element it is deep-copied and appended to `current` (never merged, never de-duplicated); else, if `current` has a child element with the same name (the FIRST such), recurse with `(c, thatElement)`; else append a deep copy of `c`.

List elements are `li`, plus every child of a field whose name is in the set of fields marked `XmlInheritanceAllowDuplicateNodes`: in 1.6.4871 `descriptionHyperlinks`, `nullifyingTraitDegrees`, `forcedTraits`, `disallowedTraitsWithDegree`, `agreeableTraits`, `disagreeableTraits` (decompiled:Verse/Def.cs, RimWorld/ThoughtDef.cs, Verse/PawnKindDef.cs, RimWorld/MemeDef.cs; mods may add attributes in their own assemblies, so the set is configurable). Before merging a node, the game checks the child for duplicate non-list element names at every depth and logs an error per duplicate (vector `inherit-duplicate-node-name-error`); the merge still proceeds and the last of the duplicates wins. Real data shows this in Combat Extended: 14 such errors (for example a doubled `equippedStatOffsets` or `Bulk`), matching what the game would log.

Result order: the clone's children keep their order; replaced elements stay in place; new elements go to the end. That is why `defName` and `label` of a leaf def appear after the inherited fields in resolved nodes.

### 6.5 Hand-traced example

`Gun_Revolver` (Core, `RangedIndustrial.xml`) inherits `BaseHumanMakeableGun <- BaseMakeableGun <- BaseGunWithQuality <- BaseGun <- BaseWeapon`. `BaseWeapon` gives `statBases` = MaxHitPoints 100, Flammability 1.0, DeteriorationRate 2, Beauty -3, SellPriceFactor 0.20 and three `comps` li; `BaseGun` overrides Flammability to 0.5 in place and adds one comp; the next two levels add one comp each; the leaf adds `WorkToMake`, `Mass`, four accuracy stats and the cooldown at the end of `statBases`. Resolved: 12 stats in that order, Flammability 0.5, 6 `comps/li`, `weaponTags` = Gun (from `BaseHumanMakeableGun`) followed by the leaf's SimpleGun and Revolver. The test `tests/test_handtrace.py` encodes 13 such traces (section 9.2).

## 7. MayRequire and MayRequireAnyOf

| Where | Rule | Evidence |
| --- | --- | --- |
| Top-level node, def creation | `MayRequire` is lower-cased and split at commas: ALL ids must be active (items trimmed, ids case-insensitive). `MayRequireAnyOf`: at least one id active. Either failing skips the node. The empty string is NOT "no requirement": `MayRequire=""` splits into one empty id which is never active, so the node is skipped (vector `mayrequire-top-level`) | decompiled:Verse/LoadedModManager.cs `ParseAndProcessXML` |
| Top-level node, inheritance registration | `MayRequire` only (all-of); an unmet parent is not registered, so its children report a missing parent (and still become defs) | decompiled:Verse/XmlInheritance.cs `TryRegister` |
| `li` entries in lists (def fields, patch lists) | `MayRequire` non-empty and not all active: skipped (an empty value is ignored here, unlike the top level); otherwise `MayRequireAnyOf` non-empty and none active: skipped. Evaluated when the resolved node is read into C# objects, so the prototype offers it as a post-pass `mayrequire.prune_li` | decompiled:Verse/DirectXmlToObject.cs `ListFromXml`, `ValidateMayRequires` |
| Fields whose C# type is a Def (cross references) | an unmet requirement silently drops the reference | decompiled:Verse/DirectXmlCrossRefLoader.cs |
| Any other element | the attribute is ignored | same readers |

Active means: a mod with that package id is active; the `_steam` postfix is ignored for matching. Frequency: the vanilla Defs of Core and the five DLC contain 1991 `MayRequire` attributes, 1150 of them on `li`, so the post-pass is needed for any faithful "effective values" view, while top-level MayRequire skips nothing when all DLC are active (`may_require_skipped` is 0 in the vanilla run).

## 8. Duplicates, def types and namespaces

### 8.1 Which type is a node

`DefFromNodeNew` takes the element name and replaces it by the `Class` attribute of the RESOLVED node when present. The name goes through `GenTypes.GetTypeInAnyAssembly` (decompiled:Verse/GenTypes.cs):

1. A case-sensitive index of types whose namespace is null or one of `RimWorld, Verse, LudeonTK, Verse.AI, Verse.AI.Group, Verse.Sound, Verse.Grammar, RimWorld.Planet, RimWorld.BaseGen, RimWorld.QuestGen, RimWorld.SketchGen, System`, keyed by short name; on a clash the type found last (assembly load order) wins. This is why `<ThingDef>` works without namespace.
2. `Assembly.GetType(name, ignoreCase: true)` on the game assembly and then on every mod assembly in order (full names, case-insensitive): `<CombatExtended.AmmoDef>` and `Class="combatextended.ammodef"` work; the short name `AmmoDef` does not (vector `class-attribute-and-type-names`).
3. Each ignored namespace prefixed to the name, again case-insensitive: so `<thingdef>` works.
4. A special case for generic types.

The result must derive from `Verse.Def`, otherwise "Type X is not a Def type or could not be found" (counted as `def_unknown_type`; the node is dropped). The prototype needs the table of Def classes because it does not load assemblies: `build_type_table.py` reads the metadata of compiled assemblies with `monodis` (names, base types and abstract flags only), `data/def_types_vanilla.json` holds the 256 Def classes of Assembly-CSharp 1.6.4871, and a Combat Extended run adds 18 classes from `CombatExtended.dll` (274 in total). The `Abstract` attribute is compared case-insensitively as a value (`True`, `true`, `TRUE`) but is a case-sensitive attribute name, is read from the ORIGINAL node and is never inherited (vector `abstract-not-created`).

### 8.2 The databases

`PlayDataLoader` calls `DefDatabase<T>.AddAllInMods` for every strict subclass of `Def` (decompiled:Verse/PlayDataLoader.cs, DefDatabase.cs); `Def` itself has no database. For a type `T`:

1. Packs are ordered Core first (`OverwritePriority` 0), then all others by load order.
2. For each pack, its defs that are instances of `T` (subclasses included: `DefDatabase<ThingDef>` contains `CombatExtended.AmmoDef`) in the order they were added, which is merged-document order. A second def with an already seen `defName` INSIDE the same pack is skipped with "has multiple ...s named X" (vector `duplicate-in-same-mod`; for ancestor types this also fires when two different subclasses share a name, vector `duplicates-across-types`).
3. Otherwise, if an earlier pack already contributed that `defName`, the old def is REMOVED and the new one appended (the winner moves to the end and all indices shift). So a mod "overrides" a def by redefining it, per database (vector `merge-load-order-and-override`).
4. Defs from patch-created nodes are added last, with the same override rule.
5. A def without `defName` is `UnnamedDef` and gets a random generated name with an error.

Real data: in the vanilla load no `defName` is redefined (0 overrides in `DefDatabase<ThingDef>`, 1811 defs). With Combat Extended active, 12 ThingDef names are overridden by CE, nine of them converting a vanilla thing to `CombatExtended.AmmoDef` (`Shell_HighExplosive`, `Shell_Incendiary`, `Shell_EMP`, `Shell_Firefoam`, `Shell_Smoke`, `Shell_AntigrainWarhead`, `Shell_Toxic` (from Biotech), `Shell_Deadlife` (from Anomaly) and `Pila`) and three staying plain ThingDefs (`Pilum_Thrown`, `Gun_AutocannonTurret`, `Gun_TurretSniper`). The def type changes while the name stays, which is exactly the case where "(type, defName)" keys would be wrong and "(database type, defName)" is right.

## 9. Validation on real data

### 9.1 Method

The engine was run on the machine's RimWorld 1.6.4871 rev598 install with the six official packs in load order (Core, Royalty, Ideology, Biotech, Anomaly, Odyssey; paths under `/home/pawbeans/.steam/steam/steamapps/common/RimWorld/Data/`), and on top of it Combat Extended from `CombatExtended-Development` (package `CETeam.CombatExtended`, its own LoadFolders.xml) with the Def type table extended from the compiled `CombatExtended.dll` of the workshop copy. Counts below are in `golden/real_data_summary.json` and re-checked by `tests/test_real_data.py`; no game or CE content is stored in the repository, only counts and hashes.

### 9.2 Vanilla

| Measure | Value | Independent check |
| --- | --- | --- |
| Def files | 1558 | glob over `Data/*/Defs/**/*.xml` gives 1558 |
| Top-level nodes | 13808 | direct count of children of every file root: 13808 |
| Abstract nodes | 596 | direct count: 596 |
| Defs created | 13212 | = 13808 - 596 |
| Patch operations | 29, all true | 10 Biotech, 10 Odyssey, 8 Royalty, 1 Ideology; Core has none |
| Diagnostics | none | the game logs no XML error for a vanilla load |
| ThingDefs | 1811 (Core 800, Royalty 121, Ideology 127, Biotech 223, Anomaly 204, Odyssey 336) | 2055 raw ThingDef nodes minus 244 abstract |
| Largest types | ThingDef 1811, SoundDef 1231, ThoughtDef 923, BackstoryDef 845 | |

Hand traces (`tests/handtrace_vanilla.json`; each expectation was derived by reading the raw parent chain and applying the section 6.4 rules, before comparing with the engine; two of my first expectations were wrong because I misread where a base was defined, which is how the Core-versus-Royalty parent case below was found):

| Def | Rules exercised |
| --- | --- |
| `Gun_AssaultRifle` | in-place override (Flammability 0.5), appended stats (12 in order), li appended across 4 levels (6 comps, 3 weaponTags) |
| `Gun_Revolver` | parent chain of 5, weaponTags order Gun, SimpleGun, Revolver, costList |
| `Gun_ChargeRifle` | leaf `techLevel` Spacer replaces Industrial; `tradeTags` li from parent and leaf (2) |
| `MeleeWeapon_LongSword` | `techLevel` Medieval replaces Industrial, `relicChance` 2 replaces 1, comps 3+2 across levels |
| `MeleeWeapon_Knife` | `burnableByRecipe` false replaces true, Neolithic tech level |
| `Apparel_FlakVest` | stats 11 (MaxHitPoints 200 and Flammability 0.6 in the original slots), `apparel` has no parent so layers/bodyPartGroups are the leaf's |
| `Apparel_PlateArmor` | 10 stats, 5 body part groups, `smeltable` replaced in place, leaf-only `tradeTags` |
| `Apparel_Parka` | `tradeTags` 2 (parent plus leaf li), `canBeDesiredForIdeo` false, `techLevel` Neolithic |
| `Apparel_CowboyHat` | `apparel` merged with a parent `apparel` (`parentTagDef` stays first), 4-level chain via `HatMakeableBase` |
| `Steel` | `statBases` parent Beauty first, then 9 leaf stats; `useHitPoints` false replaces true |
| `Bullet_Revolver` | root parent `BaseBullet`, `label` and `useHitPoints` replaced |
| `Apparel_ArmorRecon` | Core def, parent in Core; leaf `apparel/tags` appended as a new element (parent has none) |
| `Apparel_ArmorReconPrestige` (Royalty) | cross-pack inheritance: Royalty child, Core parents (all four ancestors in `Ludeon.RimWorld`), `Plasteel` 80 replaced by 100 and `Gold` appended |

### 9.3 Vanilla plus Combat Extended

Combat Extended contributes `Odyssey, Anomaly, Biotech, Ideology, Royalty` and the root as folders (the DLC subfolders only because the DLC package ids are active; its ~760 `ModPatches` folders stay unloaded because the target mods are not active).

| Measure | Value |
| --- | --- |
| Def files / top-level nodes / abstract / defs | 1924 / 18177 / 1097 / 17080 |
| Patch operations in total / CE only | 2849 / 2820 |
| CE operations that apply (engine without plugin) | 2752 of 2820; 68 fail because their class is unknown to the engine: 43 `CombatExtended.PatchOperationMakeGunCECompatible` and 25 `CombatExtended.PatchOperationSettingsConditional` |
| CE operations that apply (with the settings plugin, CE defaults `genericAmmo` false, `realWeaponNames` true) | 2777 of 2820; 43 not applied (all `MakeGunCECompatible`) |
| By class (applied, engine without plugin) | Add 969, Replace 963, Conditional 442, AddModExtension 245, Remove 93, AttributeSet 29, Insert 6, FindMod 2, Sequence 2, AttributeAdd 1 |
| Failed patch operations that are a real mismatch | 0: every vanilla class operation finds its target |
| Other diagnostics | 14 `inherit_duplicate_node_name` (authoring duplicates in CE defs, the game logs the same), 280 `patch_unknown_field` (info) from the 43 unknown operations |
| `DefDatabase<ThingDef>` | 3781 defs, 12 overridden names (section 8.2) |
| CE defs by type | AmmoDef 915, AmmoSetDef 307, AmmoCategoryDef 95, RecipeDef 907, ThingDef 1063, ThingCategoryDef 260, StatDef 46, FleckDef 35 |

Resolved weapons under CE (engine output compared with the vanilla run):

* `Gun_AssaultRifle`: five patch operations touched it (a Replace of its `tools`, an Add into `graphicData`, an Add at the def level, an AddModExtension that creates `CombatExtended.GunDrawExtension`, and the settings conditional that sets the label to a real-world name because `realWeaponNames` defaults to true). Its `statBases`, `verbs` and ammo data are UNCHANGED relative to vanilla: the CE gun conversion is not an XML edit but the custom operation `PatchOperationMakeGunCECompatible`, which the engine cannot run.
* `Gun_Revolver`: three operations, same picture (label and mod extension changed, stats and verb still vanilla).
* `MeleeWeapon_LongSword`: five operations; `statBases` gains `Bulk` and `MeleeCounterParryBonus`; tools replaced (3 before and after).
* `Apparel_FlakVest`: 15 operations; gains `Bulk`, `WornBulk`, `StuffEffectMultiplierArmor` and a `CombatExtended.PartialArmorExt`; `MaxHitPoints` and `Mass` change; the three `ArmorRating_*` stats are removed.
* `Apparel_PlateArmor`: 6 operations; gains `Bulk`, `WornBulk`; `StuffEffectMultiplierArmor` changed.

Important finding for R7: for guns, the CE values live in the PARAMETERS of the 43 `MakeGunCECompatible` operations (each names a `defName` and carries `statBases`, `Properties`, `AmmoUser`, `FireModes`, `weaponTags`, `costList` blocks; CE's `Source/.../PatchOperationMakeGunCECompatible.cs` shows the field list). The engine keeps the raw `Operation` element of every skipped operation (`PatchEvent.raw`), so the item designer can read CE's intended values from the user's own install and implement the conversion concept itself (R11: concepts only, values read at runtime). The same holds for the ~3178 uses of that class across CE's `ModPatches`.

### 9.4 Timings (Python prototype, Arch Linux, one run each, single thread)

| Stage | Vanilla | Vanilla + CE |
| --- | --- | --- |
| Mod folders and About | 0.004 s | 0.08 s |
| Parse and combine 1558 / 1924 files | 1.15 s | 1.9 s |
| Patches (29 / 2849 operations) | 0.10 s | 16.5 s |
| Inheritance (13808 / 18177 nodes) | 0.60 s | 1.0 s |
| Def creation | 0.13 s | 0.30 s |
| Total | 2.0 s (1.2 s in a quieter run) | 19.8 s |

The patch time is linear scanning: each of the 2849 operations evaluates an XPath such as `Defs/ThingDef[defName="X"]` over about 18k top-level nodes (about 6 ms per operation). The game does the same scan with .NET's XPath and is also slowest here. A Rust implementation should index `defName` and `Name` and fall back to the generic evaluator (implication 5).

### 9.5 Test suite

51 tests, about 30 s, all passing: 38 vectors (merge and databases 5, inheritance 15, patches 16, MayRequire 2), 1 test with 13 hand traces, 10 unit tests and 2 real-data golden comparisons. `golden/vector_outputs.json` (64 KB) holds the resolved output of every vector for use as Rust regression data.

## 10. Limits and the hardest parts of a Rust port

Not modelled by the prototype:

* C# object semantics: no field parsing, enum or number validation, cross references, `PostLoad`, `ResolveReferences`, `ConfigErrors`; resolved defs are XML trees. MayRequire on Def-typed fields is not applied (needs field types).
* Harmony and runtime patches (for example Combat Extended's runtime weapon conversion), custom operations of other mods (XmlExtensions and similar are skipped and counted), mod assemblies as such (types come from a table).
* Mono-specific XML details: CDATA nodes are read as text; characters that the game's reader accepts with `CheckCharacters` off but lxml rejects make a file fail; adjacent text nodes are always merged; processing instructions are dropped.
* Eager XPath node-sets (section 5) and sorted file order (section 2.3).
* Hot reload, language folders, `Defs` inside `Languages`, mod settings that change patches at runtime.

Hardest parts of the port, in order:

1. Type resolution needs .NET metadata. A Rust tool must read type names and bases from `.dll` files (a metadata reader, no runtime) to know that `CombatExtended.AmmoDef` is a Def. Mods without assemblies only use vanilla types; mod assemblies must be scanned in load order with the same "later wins" rule.
2. The inheritance merge with its quirks (first-child-only removal, attributes dropped or kept depending on `Inherit`, first same-named element, list elements). It is small (about 100 lines) but every rule above is visible in real data; the vectors pin it.
3. XPath 1.0 against a mutable tree with document-context semantics. The corpus is almost entirely `Defs/ThingDef[defName="X"]/...` shapes plus `or` chains (the expressions can also contain `li[...]`, `text()`, attributes), so a fast path plus a complete fallback is realistic.
4. Patch performance and determinism: 2849 operations over 18k nodes cost 16 s in this prototype; a name index brings it to milliseconds, but must be maintained under Add, Replace, Remove and SetName.
5. Custom C# operations that carry the actual data (Combat Extended `MakeGunCECompatible`): the defs crate must expose their parameters and let a higher layer (the item designer) interpret them.
6. Provenance through three transformations (merge, patch, inheritance) at element level for the explorer, without the memory cost of cloning 18k nodes per parent resolution: structural sharing or copy-on-write nodes with origin ids.
7. Ordering rules that the game leaves to the filesystem (file order inside a folder).

## Implications for RimStudio

1. The defs crate (proposed name `rimstudio-defs`) operates on RimStudio's own JSON-serialisable tree and has NO XML dependency. Node shape: `{"tag": str, "attrs": [[name, value], ...], "children": [node | string]}` (identical to the prototype's canonical form and to `golden/vector_outputs.json`); attributes are an ordered list (order is observable after AttributeAdd and in Inherit="false" merges). `rimstudio-xml` is the single boundary crate that parses RimWorld XML into this tree (BOM strip, UTF-8 forced, comments and whitespace-only text dropped except under `xml:space="preserve"`, DTD rejected, CDATA kept as a text node flagged or merged) and serialises it back for export.
2. Proposed Rust API surface (all inputs and outputs `serde`-serialisable, so IPC and golden tests use JSON per R10):
   * Types: `Node`, `Child { Element(Node), Text(String) }`, `ModMeta { package_id, name, load_folders: Option<LoadFoldersSpec> }`, `GameVersion`, `ActiveSet`, `TypeTable { types: Map<FullName, TypeInfo{base, abstract}> }` (JSON, built from assemblies), `Origin { mod_idx: u32, file: FileId }` with an interned file table, `PatchRef { mod_idx, file, op_index, class }`, `DefRecord { type_name, def_name, origin: Option<Origin>, node: Arc<Node>, parents: Vec<ParentRef>, patched_by: Vec<PatchRef> }`, `Diagnostic { code: DiagCode, severity, mod_idx: Option<u32>, file: Option<FileId>, message }`.
   * Functions: `resolve_load_folders(&ModMeta, &GameVersion, &ActiveSet) -> LoadPlan` (folders in descending priority plus the mode used); `collect_files(&LoadPlan, Subdir) -> Vec<SourceFileRef>` (the de-duplication and ordering rules of section 2.3); `merge(files: &[(ModIdx, FileId, Node)]) -> UnifiedDoc`; `parse_patches(...) -> Vec<PatchOp>` with `PatchOp` a serde enum of the 13 classes plus `Unknown { class, raw: Node }` and `Custom`; `apply_patches(&mut UnifiedDoc, &[(ModIdx, Vec<PatchOp>)], &PatchContext) -> PatchReport` (per top-level operation result, error, touched defs); `resolve_inheritance(&UnifiedDoc, &ActiveSet, &InheritConfig) -> ResolvedDoc`; `build_defs(&ResolvedDoc, &TypeTable, &ActiveSet) -> Vec<DefRecord>`; `DefDatabases::build(&[DefRecord], &TypeTable, &ModOrder) -> DefDatabases` with `get(db_type, def_name)`, `overrides()`, `duplicates()`; `prune_li_may_require(&mut Node, &ActiveSet)`. A convenience `load(LoadInput) -> LoadOutput { defs, databases, patch_report, diagnostics, timings }` chains them.
   * Extension points: `trait CustomPatchOp { fn class(&self) -> &str; fn apply(&self, doc: &mut UnifiedDoc, ctx: &mut PatchCtx) -> Result<bool, PatchError>; }` registered in `PatchContext`; the CE settings conditional is the first implementation; `MakeGunCECompatible` stays an `Unknown` operation whose raw parameters are exposed to the item designer.
3. Error model: content problems never abort a load. Every game "Log.Error/Warning" becomes a `Diagnostic` with a stable string code identical to the prototype's (`patch_failed`, `patch_exception`, `patch_unknown_class`, `patch_base_class`, `patch_unknown_field`, `inherit_missing_parent`, `inherit_cycle`, `inherit_not_resolved`, `inherit_duplicate_name`, `inherit_duplicate_node_name`, `def_unknown_type`, `def_duplicate_in_mod`, `defs_bad_root`, `xml_parse_error`, `defs_unknown_parse_failure`, `patch_file_unreadable`, `patch_field_case_mismatch`, `patch_setting_missing`), counted per code and capped per code for memory (first 100 samples kept). `Result<_, DefsError>` is only for API misuse and I/O at the boundary. A load of vanilla Core plus five DLC must yield zero diagnostics (tested).
4. Provenance model: three levels. (a) Def level, as in the prototype: `origin`, `parents` (nearest first, `mod:Name`), `patched_by`; (b) patch events with the operation's source file and index, kept for the UI "why did this change" view; (c) element level for the explorer: every node carries an optional `origin: OriginId` set at merge time, copied on clone, set to the operation for nodes created by a patch, and to the child level for nodes written by the inheritance merge. Overrides across mods are exposed as data (`overrides()`: old def, new def, type change), because the CE result (12 overrides, 9 of them type changes) is a core manager feature, not an error.
5. Performance requirements (testable): the vanilla load (Core plus 5 DLC, 1558 files) in under 1 s on the reference machine with parallel file parsing; the Combat Extended load (2849 operations) in under 2 s, which requires indexes for the patterns `Defs/<Type>[defName="X"]` (and `or` chains of them) with a full XPath 1.0 fallback; cloning for inheritance uses structural sharing or copy-on-write. Determinism: identical output (byte-identical JSON) regardless of thread count; file order inside a folder is an ordinal sort of the relative path (documented divergence from the filesystem order of the game).
6. XPath: implement exact document-context semantics (relative paths start at the document), eager node-sets, non-node-set results as errors, and attribute and text node targets with the exceptions listed in section 4.3. Build the supported-subset decision on the corpus script `docs/research/data/xpath-corpus/xpath_coverage.py` and keep the fallback complete for XPath 1.0 so no real patch is rejected.
7. Regression tests: the 38 vectors in `docs/research/data/def-engine/tests/vectors/*.json` and the 13 traces in `tests/handtrace_vanilla.json` become Rust integration tests unchanged (the vector format is language-neutral JSON with XML only inside strings, which is the RimWorld boundary format). `golden/vector_outputs.json` is compared structurally; `golden/real_data_summary.json` is the real-data acceptance test (vanilla: 13808 nodes, 596 abstract, 13212 defs, 29 operations all true, 1811 ThingDefs, 0 diagnostics; with CE: 18177 nodes, 17080 defs, 2820 CE operations of which 43 `MakeGunCECompatible` are not applied, 12 ThingDef overrides). Each vector name is a test name; a failure message prints the vector `doc` sentence.
8. Behaviours that must be copied even though they look like bugs, each pinned by a vector: Insert reverses multiple value children; Sequence has no rollback; `success=Never` keeps the change; `MayRequire=""` on a top-level node skips it; children cannot see parents of later mods; `Inherit="false"` on a def keeps the parent's attributes; text over a multi-child parent removes only the first child; Class is not inherited; databases are per type and include subclasses, with winner-moves-to-end ordering.
9. The Def type table is a first-class data file (JSON, R10): generated by a Rust metadata reader from the game and mod assemblies in load order, cached per assembly hash, with `short_name_collisions`. Without it, defs of mod-defined classes cannot be typed; the fallback of treating every `*Def` element as a def must produce a visible warning, never silent success.
10. The item designer and CE tooling (R7) consume `LoadOutput` only: vanilla values come from resolved `DefRecord` nodes, CE gun values from the retained raw `MakeGunCECompatible` operations of the user's installed CE (read at runtime, never copied into the repository, R11); everything user-visible about "what changed" is derived from `patched_by` and element origins.

## Open questions

1. Unity's Mono is older than the Mono 6.12 used for the probe. The first-child-only removal in the merge, the attribute-append duplicate semantics and the reversed Insert were probed on 6.12 and read from code; confirm them on the game runtime (a Harmony-free log of a deliberately crafted mod, or a probe loaded in-game).
2. File order inside a folder on Linux and macOS for the game itself (readdir order versus sorted) and whether `*.xml` matching is case-insensitive there. This decides which duplicate wins inside one mod and the order of patch files; RimStudio's rule (ordinal sort) may differ from what a player sees on ext4.
3. Exact semantics of `ModsConfig` pre-processing before the load (Core forced first, dependency auto-sorting, removal of missing mods, `_steam` postfix pairs). The prototype takes the order as given and compares package ids by lower-case string.
4. CDATA and invalid characters: how many real mods rely on CDATA inside defs or on characters rejected by a standard XML parser, so that the boundary crate must reproduce the game's reader instead of a strict parser.
5. Lazy XPath iteration: how many real patches add nodes that match their own expression (an unbounded loop in the game)? A corpus scan could turn this into a lint.
6. Which XPath subset covers 100% of the real corpus (workshop mods, CE `ModPatches`) and what speedup a defName index gives; the corpus script exists from another task but its coverage was not re-run here.
7. How to read CE's `MakeGunCECompatible` and its sibling classes into a structured conversion record (parameter names, which of them also create defs), and how much of the runtime behaviour of that class (as opposed to its parameters) the item designer must reproduce.
8. Whether `DefDatabase` override order matters downstream for RimStudio (the game uses `index` for save compatibility; a mod manager only needs the winner), and whether the 12 CE overrides should be shown as "replaced" or "upgraded".
9. Type resolution for mod assemblies that are not installed (a patch or def references a type from an absent mod): the game drops the node with an error; RimStudio should decide whether the manager shows such defs as "unknown type" instead.
