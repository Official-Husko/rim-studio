# Items toolkit: item designer specification

This document is the product specification of the RimStudio item designer (requirement R7), the module of the modding toolkit in which a modder designs a weapon or apparel item, learns whether it fits vanilla balance, and writes the vanilla item into a mod project; a Combat Extended (CE) patch is an optional extra the user chooses per item. It covers users and flows, the screens in words, modes, the inputs of each item kind with units, validation, how outputs are produced as JSON node trees that the XML boundary crate renders, how CE gating and `LoadFolders.xml` output work, what is disabled without CE, performance targets, requirement ids IT-001 and following, acceptance criteria and tests. The mathematics is in [item balance math](item-balance-math.md), the patch generator in [Combat Extended patching](combat-extended-patching.md), the project and workspace foundation in [modding workspace](modding-workspace.md). It specifies behaviour and contracts, not visual design (R8). Architecture documents under `docs/architecture/` are authoritative for crate names and invariants. RimSort, RimCrow and CE are described in our own words as concept references only (R11).

Status: draft, section 15 records the 0.1.0 backend as built | Last updated: 2026-10-05

## Contents

1. [Purpose and scope](#1-purpose-and-scope)
2. [Users and principles](#2-users-and-principles)
3. [Vanilla by default and the optional CE patch](#3-vanilla-by-default-and-the-optional-ce-patch)
4. [Flows](#4-flows)
5. [Screens](#5-screens)
6. [The item model: inputs per kind](#6-the-item-model-inputs-per-kind)
7. [Validation and warnings](#7-validation-and-warnings)
8. [Outputs](#8-outputs)
9. [What is disabled without Combat Extended](#9-what-is-disabled-without-combat-extended)
10. [Architecture mapping](#10-architecture-mapping)
11. [Performance targets](#11-performance-targets)
12. [Acceptance criteria and tests](#12-acceptance-criteria-and-tests)
13. [Phasing](#13-phasing)
14. [Owner decisions, conflicts and open points](#14-owner-decisions-conflicts-and-open-points)
15. [As built in the 0.1.0 backend](#15-as-built-in-the-010-backend)

## 1. Purpose and scope

The designer answers one question for a modder: "does my item fit?". It does so with exact game formulas (what the item will do), with a calibrated baseline (what a well-fitting item of this kind would look like, with honest error bands), and with generated files (the item definition and the CE patch). It is one tool of the toolkit registry (`tool-designer`, [ADR 0004](../adr/0004-no-feature-edges-and-toolkit-registry.md)), sits on the workspace's resolved reference set, and writes only into a mod project.

Scope of the first release (milestone M5, [roadmap](../roadmap.md)):

| In scope (v1) | Later (P2) |
| --- | --- |
| Ranged weapons: guns and bows, vanilla and CE | Grenades and throwables, turrets, mechanoid weapons |
| Melee weapons, vanilla and CE | Ammo and projectile creation for a new caliber (a picker over the install's ammo sets is v1) |
| Apparel, stuffed and fixed rating, vanilla and CE | Weapon platforms and attachments, Odyssey unique weapon traits |
| Flows: new weapon, new apparel, clone and adjust, convert an existing mod item to CE | Bulk conversion of a whole mod in one pass (the single-item flow is v1) |
| Lint of existing CE patches ([Combat Extended patching](combat-extended-patching.md)) | Patching vanilla items in place (the designer creates items; it does not edit vanilla defs) |

Evidence base: [modding toolkit scope](../research/modding-toolkit-scope.md) section 8.3, the three vanilla analyses, [Combat Extended model](../research/combat-extended-model.md), [CE auto-patcher formulas](../research/ce-autopatcher-formulas.md) and [CE patch conventions](../research/ce-patch-conventions.md).

## 2. Users and principles

| Persona | Need |
| --- | --- |
| The owner as modder | About 30 mods in folders on an external drive, several with hand-written CE patches; wants a new weapon that sits correctly in vanilla and CE, and wants existing mods converted without hand-editing patches |
| A hobby modder | Has an idea ("a revolver-sized sidearm, a bit stronger than vanilla") and no feel for the numbers; wants a sensible starting point and a plain-language verdict |
| A balance-minded modder | Wants exact formulas, the reference items and the bands, and will type every number |

Principles:

1. Exact where the game is exact, honest where it is not. Cycle time, DPS, armor mitigation, price, coverage and meets-armor are formulas and are shown as results. Suggested values are labelled estimates with bands and a named source (anchor, class median, answer, typed).
2. Every number about the game comes from the user's install at runtime (R11, I-07). The repository has no vanilla or CE value tables; test vectors are fictional.
3. A suggestion never overwrites a typed value, and editing a value changes the meter, not the other values.
4. Content problems are diagnostics, never errors (I-10); only structural impossibilities block writing.
5. The designer writes only into a mod project and only through a write plan the user has seen (diff first, I-05 for the game folder: never touched).
6. XML text appears only in the boundary crate; every internal artifact (drafts, calibration, templates, plans) is JSON (R10, I-03).

Requirements:

| id | requirement | acceptance |
| --- | --- | --- |
| IT-001 | The designer reads all reference values from the workspace's resolved defs; no vanilla or CE value table exists in the repository | repository scan test finds no value table; the designer opens against a fixture install built from fictional numbers |
| IT-002 | Every estimated value shows its source, band and pool label; exact results show no band | UI test on a fixture: estimated field has source chip and band, exact field has none |
| IT-003 | Typed values are never overwritten by suggestions | property test: any sequence of answers leaves typed fields unchanged |
| IT-004 | Writing is only through a reviewed write plan inside the project root | integration test: apply refuses a path outside the project root (RootGuard) |

## 3. Vanilla by default and the optional CE patch

Owner rule (D-085, [ADR 0037](../adr/0037-vanilla-default-optional-ce-patch.md)): the designer always writes vanilla definitions. Combat Extended is only an optional patch that the user chooses; it is never enabled automatically, even when CE is installed, and it is never mixed into the vanilla definition. The earlier design with three equal modes (Vanilla, Both and CE) and a proposal that Both is the default when CE is present is withdrawn.

| what the user does | what is produced | needs |
| --- | --- | --- |
| Design an item (always) | the vanilla item definition (and a new projectile definition when asked) in the mod's `Defs` folder | game install with resolved Core |
| Turn on the off by default toggle "Add a Combat Extended patch (optional)" for that item | additionally a CE patch in its own files in a folder selected by `LoadFolders.xml` and gated by `IfModActive`; the vanilla design is the twin for the CE conversion (identity mass, ratio range); the vanilla definition is unchanged | CE installed or CE data in the reference set |
| Convert an existing item to CE (Flow D) | only patch files for an item whose definition already exists (the project's own or any loaded def); the existing definition is the twin and is never edited | CE installed |

The toggle is stored per item in the draft (`cePatch`, default `false`). It is not a project default and does not follow the presence of CE: a project that names CE in `loadAfter` still starts every new item with the toggle off. Calibration (simple or quiz) is a separate choice and is called the calibration mode where the distinction matters.

Availability matrix:

| capability | no game install | game, no CE | game and CE |
| --- | --- | --- | --- |
| Open designer, edit drafts | yes, read-only banner | yes | yes |
| Exact formulas on typed numbers | yes (pure math) | yes | yes |
| Reference items, charts, anchors, quiz (vanilla) | no, explains why | yes | yes |
| Material and quality matrix | no | yes | yes |
| CE patch toggle, CE pools, caliber picker, meets-armor, CE lint defRef checks | no | no, with reason | yes |
| Write to project | yes (drafts export) | yes | yes |

IT-005: with no CE install, the CE patch toggle and the Convert flow are disabled with a plain reason and a link to the settings page where the CE folder can be added; the vanilla designer is unaffected. IT-007: the CE patch toggle is off for every new draft, whether or not CE is installed, and a write plan with the toggle off contains no CE file. IT-006: with a CE install present but no reference conversions in some class, the pool widens along the fallback chain and the UI says so.

## 4. Flows

Each flow is a numbered procedure; every step is reversible until step "write".

### 4.1 Flow A: new weapon

1. The user opens Item Designer, chooses New item, picks kind (ranged or melee) and the target project. A draft is created in the document store (D-083) with the CE patch toggle off.
2. Setup: tier and role (two taps). The designer shows the pool label and its size ("5 Industrial rifles").
3. Choice of starting point: Simple (strength choice: weaker, typical, stronger), Calibrate (quiz stepper, section 5.5), or Start from an item (pick a reference; its numbers fill the form and the dialogue is skipped).
4. The design form fills with suggested values. Exact readouts (cycle, DPS, P4, price) update live. The fit meter shows bands and typicality.
5. The user edits fields; ruler-tick sliders show the pool range and neighbours. Warnings appear inline.
6. Only when the user turns on the toggle "Add a Combat Extended patch (optional)": the CE tab shows the twin conversion: mass by identity, range by ratio, class constants by predictor, caliber picker, magazine and reload, tags, tools. The meets-armor preview shows damage after the user's reference armor tiers. The tab is filled by `designer_ce_suggest` (section 15.9): per field the suggested number, its source, band and rating, and the choices that only the user can make. Suggestions are computed only when asked and never switch the toggle on.
7. Output preview: the vanilla definition (and the CE patch when the toggle is on) as XML text and as a diff against what is already in the project.
8. Write to project: the write plan lists the files (create, update region, unchanged); the user confirms; the app writes through the project store and re-validates by a dry apply.

### 4.2 Flow B: new apparel

1. New item, kind apparel, project.
2. Setup: tier, role (helmet, torso vest, torso outer, base clothing, legwear, full body, utility), and stuffed or fixed rating.
3. Body part groups and layers: the form lists the groups from the install's BodyDef; coverage is computed live ("covers 44.2 percent of hits"). Layers are checked for wear conflicts.
4. Strength: simple choice, or the comparison dialogue over items of the same role (up to five comparisons).
5. The form fills: armor multiplier (stuffed) or ratings (fixed), insulation, hit points, work, mass, units or cost list; price is computed.
6. The material and quality matrix shows the result per allowed stuff and quality; the armor stack chart shows surviving damage against penetration for this layer alone and over a reference outfit.
7. Only when the CE patch toggle is on: CE apparel tab: thickness in mm (stuffed) or mm and MPa ratings (fixed), Bulk, WornBulk, partial armor, with ranges from the user's CE.
8. Preview and write as in Flow A.

### 4.3 Flow C: clone and adjust a vanilla item

1. The user picks a vanilla (or any loaded) item in the reference browser and chooses Clone.
2. A new draft starts with every field of the source copied and a new defName and label asked for (and the mod prefix applied to the defName when the project has one); inherited values stay inherited from the source's parent. The name is checked at once: it must be a valid def name and no loaded def may use it.
3. The source is the first anchor and the draft's `clonedFrom`; the fit meter compares the clone against the source and peers (the calibration mode of a clone is `anchored`), and a diff view lists the fields that differ from the source with the effect on DPS or armor stack.
4. The user adjusts; the typical question is "what happens to the fit if I raise damage by 2".
5. Preview and write. The designer creates a new def; it never edits the source def.

A clone is a vanilla draft like any other: the Combat Extended toggle stays off, whatever the source def contains. A source that carries a Combat Extended conversion in the loaded defs is refused with a plain reason (clone it from a setup without Combat Extended loaded); the default reference session of the designer is the game and its expansions only, so a vanilla source is the normal case. As built, section 15.10 gives the details (what is copied, the diff, the structure defaults of a new weapon).

### 4.4 Flow D: convert an existing mod item to CE

1. Open a project and choose Convert to CE. A scan job lists the project's weapon and apparel defs with a status: not converted, already CE (update mode available), unsupported kind (grenade, turret, mech: listed, not converted in v1), target not found.
2. The user selects an item. The designer reads its vanilla-style numbers (the twin) through the def engine; the CE tab opens with the twin conversion: mass identity, range ratio, spread, warmup, cooldown, Bulk, magazine, reload by predictors, with bands.
3. Required choices that cannot be derived are asked as inputs: caliber and ammo set, weapon tag class, tool penetration for melee, armor in mm for stuffable apparel, one-handed or belt-fed (section 6).
4. Preview the patch as a diff. If the item is already CE-converted, update mode proposes per-field Replace operations instead of a second `MakeGun` ([Combat Extended patching](combat-extended-patching.md) section 6).
5. Write: only patch files are produced and the existing definition is never edited; the patch file goes into the project's CE folder; `LoadFolders.xml` is created or edited with the gated folder and a `v1.6` block when `supportedVersions` lacks it; a `loadAfter` for CE is suggested to the About editor. The app then dry-applies the patch in its own engine and reports any operation that would fail.

Requirements:

| id | requirement | acceptance |
| --- | --- | --- |
| IT-010 | Flow A produces a valid vanilla ThingDef for a ranged and a melee item from tier, role and strength alone | golden XML for a fictional fixture |
| IT-011 | Flow B computes coverage from the install's BodyDef and updates it live | test: groups sum to the install's coverage, full set 1.0 |
| IT-012 | Flow C copies all source fields and records the source as anchor; diff view lists changed fields | test on a fixture def (ranged and melee: every field copied, anchor recorded, Combat Extended off, missing source and invalid name refused, diff lists exactly the edited fields with the readout deltas); an ignored test clones a vanilla rifle and a vanilla melee weapon of the real install and compares the written definition |
| IT-013 | Flow D lists convertible defs with status and never converts an already converted def with `MakeGun` | test with converted and unconverted fixtures |
| IT-014 | Every flow can be abandoned before write with no file change | integration test: no write call before confirmation |
| IT-015 | Drafts persist in the JSON document store (`designer-drafts`) and reopen identical (answers and the CE patch toggle included) | round trip test |

## 5. Screens

The screens are described by content and behaviour. Styling is left to the design prompt (R8). All lists over 100 rows use the windowing hook (D-053); no component calls IPC directly (D-052).

### 5.1 Item list

A table of the project's drafts and its existing weapon and apparel defs. Columns: label, defName, kind, calibration mode, CE patch (off, on, already CE), status (draft, written, out of date), fit summary (share of green or amber stats), last changed. Actions: new, clone, convert, delete draft, open. A reference browser tab lists the install's items of the chosen kind with the strength index, tier and role, sortable by any stat; selecting one offers clone, compare or use as anchor. Empty state: an explanation of the three starting points.

### 5.2 Design form with live readouts

Left column: grouped fields per kind (section 6), each with unit, source chip (typed, anchor, class median, answer), band and the pool's p10, median and p90 beside it. Right column: live readouts computed by the exact formulas: ranged cycle time, nominal and hit-adjusted DPS, implied AP, P4, armor-adjusted survival against the reference armor tiers; melee stat-panel and in-fight DPS with the difference labelled, P_M; apparel coverage, API, layer survival and the computed price breakdown (ingredients, labour at 0.0036 per work point, rounding); CE: sustained DPS (a labelled RimStudio metric), mass and bulk, meets-armor table. A field with an "ask" flag shows no estimate and the pool range instead.

Ruler-tick sliders: each numeric field has a text input and a slider whose track is a ruler. Ticks mark the pool minimum, p10, median, p90 and maximum; labelled marks show the three nearest reference items; the P50 and P80 bands are shaded behind the track; the current and the suggested value have distinct markers. The slider snaps to the field's natural step (integer damage, 0.05 s, whole tiles), supports keyboard stepping and shows the effect on the headline readout while dragging. Updates are coalesced to one preview call per animation frame.

### 5.3 Reference comparison charts

Charts are drawn by a lazy Chart.js wrapper from data computed in Rust (fewer than 200 points each, [frontend architecture](../architecture/frontend-architecture.md)):

1. Scatter of any two stats (default strength index against mass or price) with the reference items labelled and the new item highlighted, with the band around the prediction.
2. Distribution strips per stat showing the pool's quantiles and the item.
3. Ranged: hit-adjusted DPS against distance for the item and its three neighbours. Apparel: surviving damage against penetration for the layer alone and over a chosen reference outfit. Melee: swing damage against cooldown with the damage and cooldown trade-off line.
4. A rank readout beside every chart ("heavier than 12 of 19 reference rifles").

### 5.4 Fit meter

A panel at the top of the design form. It shows: the share of stats that are inside the P80 band (green or blue) and the number of reference items behind the bands; a per-stat bar with the prediction, the P50 and P80 bands and the item's value (green "typical" inside P50, blue "plausible" inside P80, amber "unusual" outside; red is never used for fit because red is reserved for error-severity problems, see the [design brief](../design/design-prompt.md)); the typicality score from 0 to 100 with the sentence "low means unusual, not wrong"; the date of the calibration and a button to recalibrate; a notice "bands are rough" below about 15 items in the chosen role and "bands optimistic: many near twins" where the harness flagged it. Amber stats are listed with the nearest reference value and a one-click "set to suggestion".

### 5.5 Quiz stepper

A modal stepper, not a wizard gate. One question per step: an anchor card (name, picture where available, index components with real numbers) and three large answers (weaker, about the same, stronger), or a set of options for setup and stat questions; a text box for "type a value"; "not sure" always available. The header says "question n of about m" (m recomputed as the bracket narrows) and shows the cost up front ("about 6 questions"). Controls: Back (undoes the last answer and recomputes), Skip to result (available after every answer), Use what I have (closes the stepper with the current estimate). A side panel shows the live estimate and how much each answer tightened it. Answers are saved in the draft. The full question set is in [item balance math](item-balance-math.md) section 8.

### 5.6 Material and quality matrix

For stuffed items: rows are the allowed stuffs (stuff categories of the item; up to 47 in vanilla), columns are the seven quality levels. The cell metric is switchable: armor sharp, blunt and heat (apparel), swing damage and in-fight DPS (melee), hit points, work, cold insulation, computed price. Ranged weapons are not stuffed; the matrix reduces to the quality ladder (damage and AP multipliers, accuracy multiplier clamped to 1, price factor with caps). Highlighted cells exceed a cap (armor 2.0), fall below a threshold or leave the pool's band. All values come from the stat pipeline with the user's quality factors and stuff powers; for CE the stuffed apparel shows thickness x power in mm and MPa per material.

### 5.7 Output preview

Tabs: Definition (XML of the new def), CE patch (XML of the operations; shown only when the item's optional CE patch toggle is on), LoadFolders (also only with the toggle on), About changes, Files (the write plan). Each tab renders text produced by Rust; the frontend has no XML library. Every tab shows a diff against what the project already contains: whole-file additions for new files, line diff for updated regions, "unchanged" for identical output. Static validation results appear above the preview with their codes. For CE, a collapsible "meets armor" table shows damage after reference armor tiers for the chosen ammo.

### 5.8 Write to project

The confirm step lists the files with status (create, update region, unchanged), size and location. Blocking diagnostics disable the button; warnings are counted and expandable. On confirm the app writes (job with progress and cancel between files), backs up files it updates, re-reads the written files, dry-applies the patch in the app's engine and reports success or the failing operations. The result links to the Def Explorer entry for the new def.

Requirements:

| id | requirement | acceptance |
| --- | --- | --- |
| IT-020 | The item list shows drafts and project defs with status and fit summary | component test with fixture data |
| IT-021 | Live readouts match the formula vectors of [item balance math](item-balance-math.md) | unit tests RG, AR, ML, MV, AP, CE vectors through the preview command |
| IT-022 | Each numeric field has a ruler-tick slider with pool ticks, nearest items and bands, keyboard operable | component test, keyboard test |
| IT-023 | Preview calls are coalesced to at most one per animation frame while dragging | test with a fake clock |
| IT-024 | Charts render from Rust-computed series and lazy-load the chart library | bundle test: chart code is a separate chunk |
| IT-025 | The fit meter shows the bands, typicality, calibration date, pool size and the rough or optimistic notices | component test |
| IT-026 | The quiz stepper supports back, skip to result, use what I have, not sure and typed values, and saves answers in the draft | component test, round trip test |
| IT-027 | The matrix covers every allowed stuff and quality and flags cap and band violations | test: 47 stuffs x 7 qualities on a fixture |
| IT-028 | The preview shows diffs against project files and static validation codes | golden preview test |
| IT-029 | Write is a job with backup, re-read and dry apply, and can be cancelled between files | integration test |

## 6. The item model: inputs per kind

Each field has a unit, a source class (V from a vanilla or base def, U typed by the user, P predicted by the baseline with band, C computed exactly, A asked because it cannot be derived), and a requirement level (REQ the item misbehaves without it, OPT a default exists). Inherited values (hit points, flammability, deterioration, beauty, sell price factor, the 7 to 12 other base statBases) are copied from the chosen parent base and not asked; the generated def lists only the stats the family lists and inherits the rest ([vanilla ranged](../research/vanilla-ranged-weapons-analysis.md) implication 11).

### 6.1 Ranged weapon, vanilla

| field | unit | src | level | notes |
| --- | --- | --- | --- | --- |
| label, defName | text | U | REQ | defName must be unique among loaded defs |
| parent base | def | U | REQ | picker over the install's weapon bases; sets inherited values |
| tier | enum | U | REQ | from tech level |
| role | enum | U | REQ | pool selector |
| projectile damage | integer | P | REQ | quantised by design |
| armor penetration | fraction | C or U | OPT | implied 0.015 x damage when unset; explicit value allowed |
| burst count, ticks between burst shots | shots, ticks | P | OPT | default 1 and 15 |
| warmup time | s | P | REQ | |
| cooldown | s | P | REQ | |
| range | tiles | P | REQ | |
| accuracy touch, short, medium, long | 0 to 1 | P | REQ | profile of four copied from the anchors or role scaled by a precision slider |
| mass | kg | P | REQ | |
| work to make | work | A | REQ | about 50 percent estimate error, shown as input |
| cost list | item and count | U | REQ | price follows; the designer proposes ingredient value = target price minus 0.0036 x work as a hint only |
| market value | silver | C | | computed; an explicit override is allowed with a warning |
| research prerequisite | def | U | OPT | validated |
| weapon tags, trade tags | defs | U | OPT | validated against the install |
| texture path | text | U | OPT | |

### 6.2 Ranged weapon, CE

| field | unit | src | level | notes |
| --- | --- | --- | --- | --- |
| Mass | kg | twin identity, or P | REQ | identity when a vanilla design exists |
| Bulk | volume | P with ask flag | REQ | CE Bulk error 26 to 53 percent |
| RangedWeapon_Cooldown | s | P | REQ | class constant, median predictor |
| SightsEfficiency | factor | P | OPT | |
| ShotSpread | degrees | P | REQ | |
| SwayFactor | factor | P | REQ | |
| recoilAmount | degrees | P | OPT | |
| range | cells | twin ratio, or P | REQ | |
| warmupTime | s | P | REQ | |
| burstShotCount, ticksBetweenBurstShots, aimedBurstShotCount | shots, ticks | P or fire pattern | OPT | |
| magazineSize | rounds | A or answer | REQ | always flagged as a question |
| reloadTime | s | P | REQ | class constant |
| ammoSet | def | A | REQ | picker over the install's ammo sets |
| defaultProjectile | def | C | REQ | must be a member of the set |
| weapon tag class | defs | A | OPT | one `CE_AI_*` class tag chosen from the installed CE data |
| tools (stock, barrel) | tool entries | P | OPT | one to three `ToolCE` entries, blunt penetration from power x median ratio |

Damage and penetration come from the ammo set, not from the gun, and the designer shows them as read-only lookups with the meets-armor table.

### 6.3 Melee weapon

| field | unit | src | level | vanilla or CE |
| --- | --- | --- | --- | --- |
| tools (label, capacities, linked body part group) | text, enum | U or V | REQ | both |
| tool power | damage | P (strength x cooldown identity) | REQ | both; CE power shifts (blades up, improvised blunt down) |
| tool cooldown | s | P | REQ | both |
| explicit armor penetration | fraction | U | OPT | vanilla; default 0.015 x damage |
| chance factor | weight | U | OPT | default 1 |
| sharp penetration | mm RHA | A | REQ for Cut, Stab | CE |
| blunt penetration | MPa | A | REQ | CE |
| stuff categories, stuff count | enum, count | U | REQ | vanilla |
| mass | kg | P | REQ | both |
| Bulk, MeleeCounterParryBonus | volume, factor | P with ask flag | REQ | CE |
| crit, parry, dodge offsets | fraction | A from class ranges | REQ | CE `equippedStatOffsets` |
| work to make | work | A | REQ | both |

The editor warns when a tool has two capacities (the stat panel counts the attack twice) and when the standard handle attack pulls in-fight DPS down by more than 5 percent. Fixed-price ultratech style items offer price buckets and a warning that extra melee damage (flame, EMP) is invisible to DPS.

### 6.4 Apparel

| field | unit | src | level | vanilla or CE |
| --- | --- | --- | --- | --- |
| stuffed or fixed | enum | U | REQ | both |
| layers, body part groups | enums | U | REQ | both; coverage computed |
| armor multiplier (stuffed) | factor, vanilla; mm, CE | P | REQ | thickness in mm for CE |
| fixed ratings sharp, blunt, heat | fraction, vanilla; mm RHA and MPa, CE | P (blunt about 0.42 x sharp and heat about 0.5 x sharp as helpers) | REQ | both |
| insulation cold, heat | degrees | P | OPT | vanilla |
| hit points | points | P | REQ | both |
| work, mass | work, kg | P scaled by coverage to the power 0.25 | REQ | both |
| cost list or stuff units | count | U | REQ | price follows |
| move speed offset | offset | U | OPT | warning when zero for fixed armor above 0.9 sharp |
| Bulk, WornBulk | volume | P with ask flag | REQ | CE |
| partial armor | per part factor | A | OPT | CE `PartialArmorExt` |
| stuff categories | enum | U | REQ | stuffed |

Family helpers: matching helmet from a suit (work x0.35, mass x0.125, hit points x0.44, equal ratings), prestige variant (work x2, add gold, equal ratings), ratio helper for blunt and heat ([vanilla apparel](../research/vanilla-apparel-analysis.md) implication 9).

Requirements:

| id | requirement | acceptance |
| --- | --- | --- |
| IT-030 | The form enforces the REQ fields of each kind (and of the CE patch when its toggle is on) and cannot produce a def or patch missing one | test per kind with each REQ field removed |
| IT-031 | Units are shown on every field and CE ratings below 1 on a CE patch are flagged as vanilla-scale | unit mismatch test |
| IT-032 | Inherited values come from the parent base and are not asked | test: output omits inherited stats |
| IT-033 | Market value is computed from cost list and work; override only with a warning | test MV vectors |
| IT-034 | Ask-flag stats show no estimate and the pool range | UI test |
| IT-035 | Melee attacks are enumerated per (tool, capacity) with a duplicate-capacity warning | test with a two-capacity tool |

## 7. Validation and warnings

Validation is a pure function (`rimstudio-validate` hosts the code registry; `rimstudio-design` produces the design diagnostics). Results are `Diagnostic` records with codes `<area>.<kebab-name>` (areas `design` and `ce`, D-046), never errors.

| code | severity | condition |
| --- | --- | --- |
| design.required-missing | error | a REQ field of the kind is empty |
| design.ref-unresolved | error | a defRef (ammo set, projectile, tag, body part group, research project, parent base) does not resolve in vanilla, the install, CE data or the project |
| design.defname-conflict | error | defName already exists in the loaded defs or the project |
| design.stat-out-of-band | warning | a value is outside the P50 band of its prediction (shown blue, "plausible") or outside the P80 band (shown amber, "unusual") |
| design.stat-outside-peers | warning | a value is outside the pool's observed minimum to maximum (flagged, not clamped) |
| design.price-far-from-model | warning | computed price differs from the model price by more than a factor of 2 and the item has no chip-class components |
| design.rating-over-cap | warning | an armor rating exceeds 2.0 after quality (the game clamps) |
| design.layer-stack-weak | info | a stack of thin layers is expected to underperform one thick layer for the chosen penetration |
| design.speed-offset-missing | warning | fixed armor above 0.9 sharp with no move speed offset |
| design.range-warmup-jump | warning | range rises sharply with no longer warmup (CE range and warmup correlate at 0.81) |
| design.mass-bulk-mismatch | info | Bulk and mass disagree with the class (rank correlation only 0.56, so Bulk is checked on its own) |
| design.melee-pen-low | warning | CE sharp penetration under the lowest common armor rating, or blunt penetration far above the class p90 ratio |
| design.duplicate-capacity | warning | a tool lists two capacities |
| design.handle-drags-dps | info | the handle attack lowers in-fight DPS by more than 5 percent |
| design.explicit-price | info | an explicit market value overrides the formula |
| design.pool-thin | info | fewer than 15 reference items behind the bands |
| design.calibration-stale | info | the cache key changed (game, CE, reference set) |
| design.unit-mismatch | warning | vanilla-scale ratings on a CE patch or the reverse |
| design.ce-absent | info | the CE patch toggle is disabled because no CE data is available |
| design.name-invalid, design.defname-prefix, design.value-invalid, design.text-invalid | error or warning | an invalid name, a defName without the project's prefix, a value that is not a number or a whole number where the game parses an integer, or text with forbidden characters |
| design.draft-inconsistent, design.ce-in-vanilla, design.ignored-input, design.duplicate-entry | error, warning or info | a draft whose parts disagree, a CE value found in the vanilla spec, an input that does not apply to the kind (for example a ranged block on a melee item), a duplicated list entry |
| design.plan-path-invalid, design.plan-path-conflict | error | a planned path that is unsafe or listed twice |
| design.deferred | info | the requested kind (apparel) is not implemented yet |
| designer.ce-unavailable | warning | the item has the CE block on but no CE data is loaded; the plan stays vanilla |
| designer.ce-outside-gate | error | a CE class would be written outside the gated CE folder (IT-052) |
| designer.plan-stale, designer.apply-failed, designer.apply-cancelled, designer.merge-failed, designer.path-refused | error or warning | plan and apply problems of section 15 |
| designer.convert-needs-answer, designer.convert-unknown-def | error | a conversion that has open questions (the field pointer says which) or names a def that is not in the project |
| ce.already-converted | info | the target already carries a CE conversion; update mode is used |
| ce.dry-run-failed | error | a generated operation fails in the app's own patch engine |
| ce.cep001 to ce.cep022 | per rule | CE lint rules, listed in [Combat Extended patching](combat-extended-patching.md) section 8 |

The ce lint codes are `ce.cep<nnn>-<name>` (for example `ce.cep001-...`) with `ruleId`, `path` and `field` args, plus `ce.not-checked`, `ce.update-load-after`, `ce.about-suggestion`, `ce.update-nothing`, `ce.tag-not-found`; the design codes live in `rimstudio-design::validation::codes::REGISTRY` and the lint codes in `ce::lint::codes::REGISTRY`. The field pointer of a content problem travels in the diagnostic's `field` arg (core `Diagnostic` has no field slot).

Range checks use percentiles computed at run time from the user's CE data and pools, not shipped numbers ([CE patch conventions](../research/ce-patch-conventions.md) implication 8). Warnings never block; errors block writing and are listed with a jump to the field.

Requirements:

| id | requirement | acceptance |
| --- | --- | --- |
| IT-040 | All codes above exist in the code registry with stable names and are tested | registry test |
| IT-041 | Only errors block writing; warnings and info never do | integration test |
| IT-042 | Range flags are computed from the user's data and never clamp a value | property test |
| IT-043 | Every diagnostic carries a field pointer that the UI can focus | UI test |

## 8. Outputs

### 8.1 From draft to files

1. The draft is a JSON document (kind, calibration mode, `cePatch` toggle, inputs with source, answers, anchors) stored in the `designer-drafts` collection of the in house JSON document store (D-083, [data and persistence](../architecture/data-and-persistence.md) section 16), wrapped in the versioned envelope; the project JSONC keeps only designer defaults (D-063).
2. `designer::export_plan(draft, env) -> WritePlan` is a pure function that builds JSON node trees ({tag, attrs, children}, ordered attributes, `rimstudio-core::tree`) for each output file: the vanilla definition files always, and the CE patch files, `LoadFolders.xml` and About changes only when the item's CE patch toggle is on (or in the Convert flow). Templates and presets are RimStudio-authored JSON with typed parameters (`ce.ranged-gun`, `ce.melee`, `ce.apparel`, `vanilla.ranged`, `vanilla.melee`, `vanilla.apparel`) that follow the parameter form of the research example ([CE patch conventions](../research/ce-patch-conventions.md) section 6.4).
3. `designer::apply_plan` renders node trees to XML text through `rimstudio-xml::render` for new files and applies byte-span edits for existing files (D-013), writes through `rimstudio-io` with atomic temp-then-rename and a backup of every updated file, and returns the written paths. The frontend never builds XML.
4. After writing, the app resolves the project again and dry-applies the patch in `rimstudio-defs` to confirm that nothing fails (`MakeGunCECompatible` is simulated by a registered custom operation from `design::ce`, [Combat Extended patching](combat-extended-patching.md) section 7).

### 8.2 Files per request

| request | files (relative to the project root) |
| --- | --- |
| Vanilla (always, the default) | `Defs/Weapons/<Name>.xml` or the project's existing convention (the scaffolder's layout); inside a version folder when the project uses version folders |
| CE patch toggle on for the item | the above, unchanged, plus `CE/Patches/<ModSlug>_Weapons_Ranged.xml`, `Weapons_Melee.xml` or `Apparel.xml`, plus `LoadFolders.xml` |
| Convert flow or update mode | `CE/Patches/...` and `LoadFolders.xml` only; no vanilla file is touched |

File names use the category names of the conventions note (`Weapons_Ranged.xml`, `Weapons_Melee.xml`, `Apparel.xml`, `Ammo.xml`), with `<!-- ====== Name ====== -->` section comments and a stable defName order.

### 8.3 CE gating and LoadFolders output

CE classes are written only into a folder selected by `LoadFolders.xml` with `IfModActive` set to the lowercase CE package id `ceteam.combatextended` (the game lowercases both sides). The designer keeps two distinct fields, `ceName` ("Combat Extended", compared exactly and case-sensitively by `FindMod`) and `cePackageId`, and never mixes them ([CE patch conventions](../research/ce-patch-conventions.md) implication 3). Procedure:

1. If the project has no `LoadFolders.xml`, create one with the root folder and the CE folder under the current version block. If it has one, edit it by byte-span splice, keeping its other entries and comments.
2. Add a `v1.6` block when the mod's `supportedVersions` lacks it, copying the previous block's folders (the game uses the exact version block, else the highest older, else default). The About editor is asked to add the version and a `loadAfter` for CE.
3. Warn on id suffixes (`_copy`, `_steam`) found in existing entries and on patch files in folders the selected block never loads.
4. A fallback for projects that must not use `LoadFolders.xml`: a patch file with only vanilla classes under a `FindMod` with the exact `ceName`; CE classes (`MakeGun`) can then not be used, and the generator says so.

Example of the gated entry as written (output text from the renderer):

```xml
<loadFolders>
  <v1.6>
    <li>/</li>
    <li IfModActive="ceteam.combatextended">CE</li>
  </v1.6>
</loadFolders>
```

### 8.4 Update mode

If the target already carries a CE conversion (a CE verb class, a CE tool class or the ammo comp), the designer does not emit `MakeGun` again. It proposes per-field Replace operations in a RimStudio-owned file and lists them in the diff; foreign files are never rewritten (rules in [Combat Extended patching](combat-extended-patching.md) section 6).

### 8.5 The write plan

`WritePlan { files: [{path, action (create | update-region | unchanged), tree, rendered, diff}], diagnostics }` is the engine form, with `kind` (`vanilla-defs`, `ce-patch`, `load-folders`, `about`) and `sections` (comment headers) as extra fields of each file. The DTO form `WritePlanDto { planId, files, diagnostics, hasErrors }` carries rendered text, a diff and a byte size, not node trees (section 15); sizes are small (a few files, under 100 KB). No file outside the project root is ever in a plan (RootGuard). The game install and config folders are never written by the designer (I-05).

Requirements:

| id | requirement | acceptance |
| --- | --- | --- |
| IT-050 | Plans are JSON node trees; XML is produced only by `rimstudio-xml` | architecture check (xtask) plus a test that the toolkit and design crates have no XML dependency |
| IT-051 | Generating template `ce.ranged-gun` from the example node tree equals the golden XML | golden test |
| IT-052 | No file outside the gated CE folder contains a `CombatExtended.` class | test over every generated plan |
| IT-053 | Existing files are changed by byte-span edit; unrelated bytes are identical | round trip test on a fixture with comments and odd whitespace |
| IT-054 | `LoadFolders.xml` is created or edited with the gated folder and a `v1.6` block when needed | golden tests: none, existing without CE, existing with CE |
| IT-055 | `ceName` and `cePackageId` are distinct and a `FindMod` entry that looks like a package id fails validation | validator test |
| IT-056 | After writing, a dry apply of the patch in the app's engine succeeds or reports the failing operation | test with a fixture def lacking `statBases` |
| IT-057 | Update mode is selected automatically for already converted targets and never emits a second `MakeGun` | test |
| IT-058 | The designer never writes under the game install or config folders | test with a fake fence |

## 9. What is disabled without Combat Extended

With no CE install, no CE data is available, and no CE number exists anywhere in the product (R11):

1. The toggle "Add a Combat Extended patch (optional)" and the Convert flow are disabled with the reason "no Combat Extended found" and a link to the settings page for the CE folder.
2. The CE tab, the CE reference pools, the caliber and ammo set pickers, the meets-armor table, the CE weapon tag picker and the CE value ranges are absent.
3. Lint of existing CE patch files still runs the structural rules (parse and shape, operation class spelling, exact `FindMod` name, `MayRequire` misuse, duplicates, version folders, loaded-folder checks) and marks the data-dependent rules (ammo set, projectile, tag and class existence, field names) as "not checked: CE not installed".
4. The vanilla designer, quiz, matrix, charts and writing are unaffected, and a vanilla design remains usable later as the twin when CE appears.
5. If CE disappears after a draft was made, the draft stays editable; its CE tab shows the stored CE inputs read-only with the reason.

Requirements: IT-060 (all of the above, one test per item with CE removed from the fixture); IT-061 (a repository scan test finds no CE value table, and the designer opens with a fixture install that has no CE).

## 10. Architecture mapping

| concern | location |
| --- | --- |
| math, bands, quiz, leave-one-out, CE reader, formulas, patch generator, lint | `rimstudio-design` (pure, depends on core and defs); modules `stats`, `armor`, `ranged`, `melee`, `apparel`, `price`, `fit`, `baseline`, `quiz`, `loo`, and `ce::{reader, classes, formulas, patchgen, lint}` ([crate catalog](../architecture/crate-catalog.md)) |
| orchestration, drafts, write plans | `rimstudio-toolkit::designer` behind feature `tool-designer` |
| workspace snapshots, reference set | `rimstudio-workspace` |
| XML render and byte-span edit | `rimstudio-xml` |
| files, atomic writes, backups | `rimstudio-io` |
| DTOs | `rimstudio-ipc-types` |
| command table | `rimstudio-app::registry` |
| frontend | `apps/desktop/src/features/designer/` with `DesignerPage.tsx`, `store.ts`, `api.ts`, `components/` |

Commands (declared once in the registry; names `<area>_<verb>`; the catalog lists the module functions, the registry rows below are the proposed table, not yet in the decision register):

| command | kind | purpose |
| --- | --- | --- |
| designer_reference_list | query | reference items of a kind and class with index, tier, role, stats |
| designer_preview | query | exact readouts, suggestions, bands for a draft state; at most 1 ms |
| designer_fit | query | fit meter data: bands, ranks, typicality |
| designer_ce_suggest | query | suggestions for the optional CE block of a draft: value, source, band, rating, asks; the toggle may be off and stays off |
| designer_quiz_next | query | next question for a draft |
| designer_quiz_answer | action | apply an answer, return the updated estimate |
| designer_material_matrix | query | stuff by quality grid for the item |
| designer_calibrate | job | build pools and run the leave-one-out harness; cached |
| designer_convert_scan | job | list convertible defs of a project with status |
| designer_export_plan | query | build the write plan and rendered preview |
| designer_apply_plan | job | write, back up, re-read, dry apply |
| designer_draft_save, designer_draft_list, designer_draft_delete | action, query | drafts in the `designer-drafts` collection of the JSON document store (D-083) |
| designer_clone | action | flow C: a new stored draft that copies every modelled field of a loaded weapon, the source as first anchor |
| designer_clone_diff | query | the changed fields of a clone against its source with the effect on the exact readouts |
| designer_structure_defaults | query | parent, projectile, cost list and stuff of a new weapon from the nearest reference weapon, as suggestions |

Jobs follow the model of D-045: caller-minted id, progress at most 20 messages per second, cancel between units of work (between items in calibration, between files in apply). Cache: calibration results in the cache root as JSON (`v` field, lowercase kebab-case names such as `design-calibration.json`, name proposed), keyed by (game version, CE version hash, reference set hash, index definition hash, harness version); always deletable; the designer opens from cache instantly and recalibrates in the background when the key changes. Persistence of drafts and quiz answers is in the project JSONC, never in browser storage.

Frontend rules apply: the feature imports only `shared`, `rimstudio-ui` and `rimstudio-ipc-types`; Chart.js comes through the lazy `shared/charts` wrapper; no XML library; strings through i18n keys; all lists over 100 rows use `shared/lists`.

## 11. Performance targets

Targets are budgets to be confirmed by benchmarks in M5 (criterion benches against `xtask/budgets.jsonc`, a 2x regression fails). The math itself is trivial; the budgets protect the interactive feel.

| operation | target | note |
| --- | --- | --- |
| `designer_preview` for one item | at most 1 ms Rust work, round trip at most 8 ms p95 | below the job threshold, so a query |
| slider drag | one preview per animation frame, frame work under 16 ms | coalesced in `shared/ipc` |
| `designer_material_matrix` | at most 1 ms (about 330 evaluations: 47 stuffs x 7 qualities) | if the bench exceeds 1 ms it becomes a job |
| `designer_quiz_answer` to next question | under 50 ms | pools are in memory |
| open designer from cache | under 100 ms | no calibration on open |
| `designer_calibrate` cold, vanilla pools | under 5 s with progress (the Python prototypes take 20 to 60 s for all arms and replicates) | to be measured |
| `designer_export_plan` for 20 files | under 50 ms | pure |
| `designer_apply_plan` | under 500 ms for 20 files on local disk | job |

Memory: pools and the matrix are small (hundreds of rows); the reference set comes from the workspace snapshot and is not copied.

Requirements: IT-070 (benches exist for preview, matrix, quiz answer, calibration and export plan, wired to the budget file); IT-071 (calibration output is byte-identical at 1 and 8 threads).

## 12. Acceptance criteria and tests

Test layers follow [testing strategy](../architecture/testing-strategy.md): nextest, proptest, goldenfile with JSON goldens, Vitest, Playwright with mockIPC.

| layer | tests |
| --- | --- |
| Unit, `rimstudio-design` | every vector of [item balance math](item-balance-math.md) (RG, AR, ML, MV, AP, CE, PI, FM); curve helper semantics (single point constant, inclusive ranges, clamped ends, missing stat reads 0) with the 43 hand-derived vectors that use no CE data; percentile and rank helpers; bucket learning |
| Property | typed values never overwritten (IT-003); preview is deterministic; scaling is monotone in strength for strength-driven stats; bands widen when answers are dropped |
| Harness | `loo::validate` reproduces the prototype numbers within 2 points when configured as A or B; hybrid gate of [item balance math](item-balance-math.md) section 12 |
| Golden, toolkit | node trees to XML for each template; template 01 from the example node tree equals its golden XML; byte-span edit round trips; `LoadFolders` goldens |
| Integration | flows A to D on a fixture workspace with fictional numbers; write plan applies; dry apply passes; RootGuard and fence refusals; no CE class outside the gated folder |
| Install-backed (`#[ignore]`, `RIMSTUDIO_GAME_DIR`) | revolver and assault rifle cycle and DPS, plate armor in steel (sharp 0.81, value 460), longsword and mace DPS, CE meets-armor examples, one real CE conversion reads back identical values |
| Frontend | component tests for list, form, slider, stepper, meter, matrix, preview; Playwright over mockIPC for flows A to D including back and skip; gallery screenshots in both themes; keyboard parity |
| Architecture | xtask layer check (design depends only on core and defs; toolkit modules do not import each other); repository scan for value tables; XML dependency ban |

Acceptance of the module as a whole:

1. A modder can create a new ranged weapon, a melee weapon and an apparel item with the default vanilla output and write valid defs from tier, role and strength alone (IT-010).
2. With CE present and the toggle on for an item (or in the Convert flow), the designer writes a gated CE folder, a `LoadFolders.xml`, and a patch that dry-applies cleanly (IT-051 to IT-057); with the toggle off nothing CE is written (IT-007).
3. A mod with an existing CE conversion is detected and handled in update mode (IT-057).
4. All requirement rows IT-001 to IT-071 have a passing test or an explicit waiver recorded in the roadmap.

## 13. Phasing

Within milestone M5 (after the def engine and workspace exist):

1. `design` math: stat pipeline, armor, ranged, melee, price, apparel; vectors green.
2. Reference pools, role assigner, baseline (simple mode), harness, bands and typicality; cache.
3. Vanilla designer UI for ranged weapons: list, form, sliders, readouts, fit meter, write plan, vanilla output.
4. Quiz stepper and the hybrid calibrated mode; the hybrid gate.
5. Melee, then apparel with the matrix and charts.
6. CE reader and class stats; the optional CE patch toggle and tab, twin conversion, patch generator, `LoadFolders` output, update mode, lint.
7. Convert-to-CE flow with the project scan.
8. P2: grenades, ammo creation, turrets, mech weapons, bulk conversion.

This order follows the owner's priority (weapons first, apparel next) and delivers a useful vanilla designer before any CE work.

## 14. Owner decisions, conflicts and open points

Owner decisions:

1. Bin edges and caps of the quiz ([item balance math](item-balance-math.md) section 13).
2. Resolved on 2026-10-04 (D-085): vanilla is always the default, the CE patch is an opt in toggle per item and never automatic; the question of a Both default is withdrawn.
3. Whether the owner's own CE patches may serve as optional reference items and as the held-out check.
4. Whether the first release includes the CE convert flow for apparel, the least derivable CE kind ([CE auto-patcher formulas](../research/ce-autopatcher-formulas.md) section 7.5), or ships weapons first.

Conflicts and notes:

1. The registry rows in section 10 extend the module function list in the crate catalog (drafts, matrix, convert scan); they need a register entry when M5 starts.
2. `FitReport` gains a typicality field relative to the catalog sketch.
3. The modding toolkit scope note says the CE formulas note is a stub; that note is complete and is the evidence for the optional CE patch here.
4. Update mode for patches written by another author is resolved conservatively (never rewrite foreign files); the research note left this open.

Open points: role assignment rules and the hybrid calibrated mode are unbuilt and unmeasured; hit chance in CE is not modelled; weapon quality, attachments, bipods and unique-weapon traits are base values only; the default CE ammo set assignments and tag mappings must be read from the installed CE data and this was not verified for every CE version ([CE patch conventions](../research/ce-patch-conventions.md) open question 1).

## 15. As built in the 0.1.0 backend

The backend of release 0.1.0 (weapons, ranged and melee, plus the optional CE patch and the conversion of an existing mod's weapons) is implemented in `rimstudio-design` and `rimstudio-toolkit::designer`; apparel is not. Where this section differs from the earlier sections, this section describes what the code does and the earlier text is the plan for the full release. Owner rules did not change: vanilla by default, CE only as an optional patch in its own file under a folder gated by `LoadFolders.xml`, never mixed into the vanilla definition.

### 15.1 The draft and the CE toggle

1. A draft is `model::Draft { schema_version, kind, calibration, spec, answers, anchors }` (kind `designer-draft`, version 1, mode simple or quiz). The toggle `cePatch` of section 3 is represented by the `ce` block of the spec: absent (`None`) means off, which is the default and the only state a new draft has; no function ever creates the block. Presence of the block means the user turned the patch on. The draft DTO is JSON identical to the engine type.
2. Each value of the spec that can be suggested is a `Sourced` value with a source `suggested`, `anchor`, `answered` or `typed`; a typed value is never replaced by another source (IT-003 as built by `offer`).
3. The tier is the spec's `tech_level` (Neolithic is 0) and the role an optional free text; the tier and the role are REQ in the table of section 6 and produce `design.required-missing` when absent although they do not change the written def, so every front end must supply them. Pools derive their own tier and role per reference item: tier is the explicit field, else the stat tier or tech level, else terciles of the strength rank; role is the explicit field, else the most common shared tag, else `any`. Melee roles come from tool capacities and ranged roles from the weapon classes, tags and two stat thresholds, both through the editable `RoleRules` (bow, sniper, shotgun, smg, heavy, pistol and rifle for ranged; on the owner's install the ranged roles agree with the research role table on all 19 shared weapons, which meets the 90 percent gate); a weapon no rule claims takes the most common shared tag.
4. A melee weapon needs a stuff spec (categories and count) or a non empty cost list (the table of section 6.3 says stuff REQ); a ranged weapon needs a cost list. Whole number fields that the game parses as integers (cost counts, stuff count, `ticksBetweenBurstShots`, the damage of an inline projectile) are errors when fractional. An inline projectile is written to its own file `Defs/Projectiles/<defName>.xml`, not into the weapon file.

### 15.2 REQ set of a CE patch and sanity thresholds

`validate_ce_patch` reports `design.required-missing` for these CE fields: a gun needs ammo set, default projectile, magazine size, reload time, bulk, sway factor and shot spread; a melee weapon needs bulk, the crit, parry and dodge offsets, a blunt penetration per tool and a sharp penetration for tools with a Cut or Stab capacity. `validate_vanilla` never looks at the CE block, so an incomplete CE block does not block the vanilla files. The values the block leaves open come from the predictors and fall back to the vanilla design (identity); mass is always written explicitly when known; costs, research and work to make stay in the vanilla definition. The sanity limits of the validation (`MAX_PLAUSIBLE_RANGE` 100, `MAX_PLAUSIBLE_SECONDS` 60, `MAX_PLAUSIBLE_MASS` 100) are plausibility limits chosen by the implementation, not game values; the owner should review them (open point).

### 15.3 Plans, files and apply

1. Planning is pure. The vanilla plan is always built (`design::plan::export_vanilla_plan`: `Defs/Weapons/<defName>.xml`, inside the version folder when the project uses one; on any error it has no files). The CE plan joins only when `spec.ce` is present and CE data is loaded: the patch file under the CE folder (`CE/Patches/<ModSlug>_Weapons_Ranged.xml` or `..._Weapons_Melee.xml`, or the `_Update` form of section 15.5) plus `LoadFolders.xml`. With the block on and no CE data the plan stays vanilla with the warning `designer.ce-unavailable`.
2. A new file is rendered by `rimstudio-xml`; an existing file is merged by byte span edit: a section is replaced by its `====== Name ======` comment, a same named def is replaced as an element, new sections are appended with the file's indent and line ending, and `LoadFolders.xml` is changed through `ensure_block` and `add_entry` (the `UpdateRegion` plan action). The comment of a generated patch file says it was generated by RimStudio for the draft, without a date.
3. The plan id is a blake3 hash of path, kind, action and final text of every file. `WritePlanDto` carries the id, the rendered text, a unified diff and a byte size per file and `hasErrors`; the apply request re-sends the export inputs plus the plan id, the toolkit rebuilds the plan and refuses a different hash (`designer.plan-stale`) or any error diagnostic (`designer.apply-failed`).
4. Apply checks every path with the guarded writer before the first write (relative, inside the project root, not under the protected paths: the game install of Core and the DLC, the reference mod roots and explicit folders; the optional `GameWriteFence` also applies), writes definitions, then patches, then `LoadFolders.xml` last, each with a backup and a read back check, then dry applies the patch in the app's engine with the gun conversion simulated (IT-056). Backups of replaced project files go to `<data root>/project-backups/<projectId>/<folder of the file>/`, not into the mod folder, and the reported backup path is absolute. Cancellation is observed between files: cancelled after at least one file apply returns the written list with the warning `designer.apply-cancelled`; cancelled before the first file it is cancelled. Before the first write every pending file is preflighted: its path, its read only flag and that it still holds exactly the text the plan was made from (a file edited by another program after planning, or one that appeared since, refuses the apply with `designer.apply-failed` and nothing is written); two plan paths that differ only by letter case, or one that is the folder of another, refuse the whole plan. The writer also refuses Windows device names, trailing dots and spaces, segments over 255 bytes, paths over 200 bytes, a name that differs only by case from an existing entry, a target file that is a link, a read only file, a project folder that is a link and any project inside a Steam library. A write error after at least one file was written returns the report of what was written with the error diagnostic `designer.apply-write-failed` and writes nothing further (the `LoadFolders.xml` gate is last, so it never lists a patch that was not written). A regenerated patch section ends at the first element that does not mention the item name, so operations written by hand after it are kept. `project_create` writes `About/About.xml` last, checks the folder name like any path component and can be repeated to complete a scaffold that stopped (identical existing files are kept while the About file is missing, anything else that exists refuses). The review that produced these rules is section 19 of the [security and privacy](../architecture/security-and-privacy.md) document.

### 15.4 Convert flow

`designer_convert_scan` reads the project through a session that has the project folder as its last pack (preferring the session that has CE loaded) and lists each project weapon as not converted, already CE, unsupported kind (bows, launchers and grenades are not converted in 0.1.0) or target not found, with the questions (asks) that the conversion would need. `convert_plan` feeds the same plan path: open questions become error diagnostics `designer.convert-needs-answer` of an empty plan (the scan carries the asks, the plan DTO has no ask list); a conversion never changes a vanilla definition (a test shows the file byte identical); derived numbers carry their origin and rest on few twins on real data, so they are suggestions. Gun bash tools use the median blunt ratio of the melee conversions, which no research number backs.

### 15.5 Update mode

When the target already carries a CE conversion, no second `MakeGun` is emitted. As built, update mode always adds per field Replace operations in a RimStudio override file `Weapons_<Kind>_Update.xml` (itself merged by section like any generated file), including for a conversion that lives in a hand written project file; splicing into hand written or earlier RimStudio files by value is not done (limit of the design crate, owner decision 1 of [Combat Extended patching](combat-extended-patching.md) is open). Update mode skips the REQ checks because the existing conversion supplies what the block leaves open.

### 15.6 Commands and functions as built

The designer functions of section 10 exist as `pub fn name(ctx: &Ctx, req) -> ToolkitResult<Resp>` in `rimstudio-toolkit::designer`: `reference_list` and `reference_list_ce`, `preview` and `suggest_fill`, `fit`, `quiz_next`, `quiz_answer` and `quiz_back`, `calibrate` (job body), `ce_suggest`, `draft_save`, `draft_load`, `draft_list`, `draft_delete`, `clone_draft`, `clone_diff`, `structure_defaults` (section 15.10), `export_plan`, `apply_plan` (job body), `convert_scan` (job body) and `convert_plan`. `designer_material_matrix` is not built (apparel and matrix are out of the 0.1.0 slice). Drafts live in the `designer-drafts` collection and calibration results in the cache root collection `designer-calibration`, not in the project record (the project record keeps only the last designer settings as an opaque value). The CE settings that the 25 CE conditionals read are not read from the user's config folder; the caller passes the map.

### 15.7 Known limits

CEP010 and CEP015 report not checked without a CE type table and field tables (spike S-07 reader work); lint reports only (CP-017 fixes are not built); the calibration against the research prototypes and the install backed calibration are not measured; CE settings must be supplied; weapon platform fields of `MakeGunCECompatible` and `AllowWithRunAndGun` are parsed and listed as unsupported, not applied; a def that inherits its verbs from an abstract parent keeps the inherited vanilla verb under `MakeGun` (not detected); 17 digit floats can differ by one unit in the last place after a store round trip because `serde_json` has no `float_roundtrip` feature in the workspace.

### 15.8 Integration findings (2026-10-05)

- (Superseded by section 15.9: `designer_ce_suggest` and the plan option `acceptSuggestions` now derive the block for a designed weapon.) A new weapon designed with the CE toggle on needs the CE block typed by the user (`ce.ammoSet`, `ce.defaultProjectile`, `ce.bulk`, `ce.magazineSize`, `ce.reloadTime`, `ce.swayFactor`, `ce.shotSpread` for a gun; `ce.bulk`, the three offsets and `ce.toolPenetration.N.tool|sharp|blunt` for melee). The convert flow derives these numbers, the designer does not yet; deriving a suggested block from the user's conversions is a UI step (`suggest_fill` in the command catalog).
- (Fixed, see section 15.11.) The convert scan only listed a melee weapon that had both `tools` and `weaponTags`; it now uses the rule of the reference pools.
- A plan with Combat Extended data reports every derived number (`ce.derived-value`) and warns when a predicted range or mass is implausible against the vanilla number (`ce.derived-far-from-vanilla`).

### 15.9 CE suggestions for a designed weapon

`designer_ce_suggest` (query, `designer::ce_suggest`, `DesignerCeSuggestRequest { draft }` returns `CeSuggestionDto`) answers what the user's own Combat Extended conversions say about the optional CE block of a draft. The vanilla design is the twin. The engine is `rimstudio-design::ce::suggest::suggest_block(&DesignSpec, &CeModel) -> CeSuggestion`; it is pure and deterministic and holds no game number.

1. **The toggle.** The draft may have no CE block (toggle off): the suggestion then describes what turning the patch on would offer, `toggleOn` is false and nothing is changed or written. A suggestion never creates the block (owner rule: vanilla by default). Without CE data, or without a game install, the answer has `available: false` and a plain `reason`, and no fields.
2. **Numbers.** `fields` lists, in a fixed order, every number of the block (gun: bulk, sway, spread, sights, recoil, magazine, reload and the penetration of the gun bash tools; melee: bulk, counter parry, crit, parry, dodge and the penetration of every tool). Each has a pointer (`/ce/bulk`, `/ce/toolPenetration/<tool>/blunt`), `required`, a `status` (`held`: the draft holds a value the user decided, which is kept; `derived`: a usable suggestion exists; `ask`: none), the suggested `value`, its `source` (`typed`, `anchor`, `answered`, `identity` with the number of weapons, `predicted` with the predictor and the number of weapons, `vanilla`, `first-of-set`), its `band` (P50 and P80 intervals), its `rating` (reliable, rough, unreliable, unmeasured) and typical `error` from leave one out, and, for an ask, the rejected `reference` estimate and the `reason`. A held value still shows the estimate for comparison. The numbers come from the same derivation as the convert flow (`derive_ce_block`), so a designed weapon and a converted one get identical suggestions.
3. **Choices.** `choices` lists the ammo set, the default projectile, the weapon tag class and the one handed and belt fed flags. The ammo set and the tag class are never chosen for the user: they are asks with candidates ranked by the conversions of the nearest weapons (the sum of the tag similarity of the guns that use them, then for ammo sets the closeness of the first projectile damage to the design's damage, then the number of guns, then the name; at most 12). The default projectile follows the ammo set once the user chose it (`first-of-set`). The flags are asked only while the toggle is off.
4. **Patch numbers.** `patchNumbers` shows the numbers the patch derives although the block has no field for them (mass, and for guns range, warmup and cooldown): the usable estimate, else the vanilla number with the reason. They are shown for transparency and are not editable here.
5. **Open questions.** `asks` is the list of open choices and numbers (with `reason` and `suggestion`), `missing` the required fields the draft does not hold today and `stillMissingAfterAccept` those that stay open even when every derived suggestion is accepted (an unreliable stat, the ammo set, the tag class).
6. **Plan and apply.** `DesignerExportPlanRequest` (and so the apply request that re-sends it) has the optional member `acceptSuggestions { fields }`. It is used only when the draft already carries the CE block; an empty `fields` list accepts every derived field (numbers rated reliable or rough, and the default projectile of a chosen ammo set), a list accepts only the named ones (a pointer such as `/ce/bulk` or a short name such as `bulk`). Only fields the user has not decided are filled (an empty field or one that holds an earlier suggestion); typed, answered and anchor values are never overwritten (IT-003). Each filled field is reported as an info diagnostic `ce.derived-value` whose text starts with "taken from the Combat Extended suggestion". A named field that was not accepted is the warning `designer.ce-suggestion-skipped`; a rejected estimate is the info `designer.ce-suggestion-rejected` with the reason. While the ammo set or the tag class is unanswered the plan carries the error `designer.ce-needs-answer` (with the candidates as the arg `candidates`) and cannot be applied; an unreliable required number leaves the usual `design.required-missing` error. With the toggle off the option is ignored with the info `designer.ce-suggestion-ignored`: no CE file, no `LoadFolders.xml` change and no Combat Extended class appears in any plan built by this path. Because the option is part of the request, the plan id covers it and applying with another option than the reviewed one is `designer.plan-stale`.
7. **Front ends.** The CLI has `designer ce-suggest DRAFT [--project DIR] [--json]` (exit 0, or 3 when no CE data is loaded) and `designer plan|apply DRAFT --ce --accept-suggestions [FIELD ...]`; answers still come from `--set ce.ammoSet=...` and the other `--set ce.*` keys. `--accept-suggestions` needs `--ce`, and takes the names that follow it, so it goes after the draft.

Limits: the ratings describe the library of conversions, not the next author's style; with few converted weapons every number is an ask; bows are still modelled as guns in the pools; the candidate ranking is a heuristic over tags and one damage number, not a statement that a caliber fits.

### 15.10 Clone and adjust, the diff and the structure defaults

Flow C is `designer_clone`, `designer_clone_diff` and, for a new weapon without a clone, `designer_structure_defaults` (all in `rimstudio-toolkit::designer::clone`; the reverse mapping from a def to a spec is `rimstudio-design::reader::spec_from_def`, with `spec_from_def_inheriting` for the clone).

1. **Clone.** `designer_clone` (action, `clone_draft`, `DesignerCloneRequest { projectId, source, defName, label?, modPrefix? }` returns `DesignerCloneResponse { entry, notes }`) reads the source from the loaded defs (the main reference session, which holds no Combat Extended) and stores a new draft. Every field the design spec models is copied: parent name, projectile reference (a gun keeps pointing at the projectile def of its source), cost list, work to make, mass, market value when the def sets it itself, stuff categories and count, research prerequisite, weapon, trade and class tags, texture path, the shooting verb numbers, accuracy, cooldown, the tools with their capacities, power, cooldown, armor penetration, chance factor and linked body part group, description, tier and role (the role comes from the reference pool when the def names none). Numbers carry the source `anchor`. Stats that the def only inherits from its parent stay inherited: the reader compares the resolved def with the stats its own file declares (a def changed by a patch, or whose `statBases` resets inheritance, copies every stat), records the inherited ones in `parent.inheritedStats` and leaves them out of the own stats, so the written definition does not repeat them. The source is the first anchor and `clonedFrom` of the draft, the calibration mode is `anchored`, `ce` is absent. The new name is checked before anything is stored: an invalid name or a name that a loaded def already uses is `designer.invalid-draft`; an unknown source is `designer.reference-unavailable`; a def that is no weapon is `design.invalid-input`; a source with a Combat Extended class in its verbs, tools or comps is `designer.invalid-draft` with the reason. A mod prefix, when given, is added to a name that lacks `<prefix>_` and recorded in the identity.
2. **Notes.** The response lists, in plain words, the fields of the source that the designer cannot model and a clone therefore does not carry (they come from the parent where the source inherited them), verbs beyond the shooting verb, a source without work to make or without cost list and stuff (the designer requires them before a plan is written), and, for a gun, that the clone shares the projectile of its source.
3. **Shared projectile.** The damage and armor penetration of a gun live in its projectile def. A clone points at the projectile of its source, so editing the damage in the draft changes the readouts but not the written definition; the diff says so. Giving the clone its own projectile (an inline projectile in the spec) is the way to write a different damage.
4. **Diff.** `designer_clone_diff` (query, `clone_diff`, `DesignerCloneDiffRequest { draft }` returns `DesignerCloneDiffResponse { source, sourceLabel, changes, readouts, notes }`) re-reads the source named by the draft's `clonedFrom` and lists the changed fields in spec order as a JSON pointer, a short label, the old and the new value (a value that is only typed again with the same number is not a change; the names and label of the clone are not changes). `readouts` pairs the exact readouts of both specs: cycle time, DPS, hit adjusted DPS, implied armor penetration, strength index and price for a gun, the panel and in fight numbers for a melee weapon, each with the delta. The readouts are the vanilla ones whatever the draft's Combat Extended block says. A draft that is not a clone is `designer.invalid-draft`.
5. **Structure defaults.** `designer_structure_defaults` (query, `structure_defaults`, `DesignerStructureDefaultsRequest { draft }` returns `DesignerStructureDefaultsResponse { draft, reference?, filled, notes }`) fills the empty structure of a new weapon: the nearest reference weapon is the one of the draft's role and tier (else its role, else the whole pool) whose numbers are closest to the numbers the draft holds (gun: damage, warmup, cooldown, range, mass; melee: swing damage, cooldown, mass; each difference is scaled by the spread of the stat in the pool, ties go to the smaller def name). Its parent, projectile reference and cost list are copied into the fields that are still empty, and for a melee weapon its stuff (the count is marked `suggested`). Nothing that already holds a value is replaced and nothing is marked typed; the note says the values are suggestions to check. A draft with no number gets no structure and the note says why. Work to make, tier and role stay the user's input.
6. **Front ends.** `designer new KIND --name N --from DEFNAME [--label L] [--prefix P] [--set k=v]...` performs the clone (the kind must match the source, else nothing is kept and the exit code is 2; `--strength` does not combine with `--from`); only when the def cannot be read as a whole does it fall back to the pool numbers (tier, role and the stats of the reference list, marked `anchor`) and says so. `designer diff DRAFT [--project DIR] [--set k=v]...` prints the changed fields and the readout deltas; `--set` tries a change without saving it. `designer new KIND --name N --strength S` calls the structure defaults after the numbers are filled. The text after `new` tells the next steps (`diff` and `plan` for a clone, `preview` and `plan` for a new weapon).

Limits: a source from the project itself (a weapon of the user's own mod) is not cloned, only the reference session is read; the left behind fields are listed, not carried (sounds, `comps`, recipe details, equipped offsets); the nearest reference weapon is a plain distance over a few numbers, not a statement that the structure fits the design.

### 15.11 Gap closing: quiz back, ask reasons, tagless melee, write fence, answer groups

1. **Quiz back.** `designer_quiz_back` (action, `designer::quiz_back`, `DesignerQuizBackRequest { draft }` returns `DesignerQuizAnswerResponse { draft, step }`) removes the last answer the user gave and recomputes the step and the estimate. The setup answers that were taken from the spec (tier and role) are never undone; with nothing left to take back the error is `designer.quiz-wrong-answer`. The values the earlier answers wrote into the spec stay, because a replacement never lowers a source rank, so the returned step and estimate are the truth and the form values a starting point. The terminal quiz uses this action for `b` and keeps no client side history.
2. **Why a number is asked.** `AskItemDto` of the convert flow has two optional members: `reason` (why the number is asked although an estimate exists: rated unreliable, or no converted weapon of the kind to measure) and `suggestion` (the rejected estimate, for reference; it is never written unless the user answers with it). They are absent for questions without an estimate (the ammo set, the flags). Because the suggestion is a float, `AskItemDto`, `ConvertCandidateDto` and `ConvertScanDto` no longer derive `Eq`. The plan carries the reason in the text of the `designer.convert-needs-answer` diagnostic ("asked because ..."), and `convert scan` prints a "why it is asked" line and the "rejected estimate" under each question.
3. **Melee weapons without weapon tags.** The convert scan and the converted melee reader (`ce::reader::conversions::is_item`) use `design::reader::is_weapon_def`, the rule of the reference pools: a def that has weapon tags, or is primary equipment under a weapon category, and is neither a creature nor a building. An item that only carries tools (a log, a horn) is still not a weapon. On the owner's install the number of candidates did not change (vanilla Core and DLC: 115 candidates, 48 to convert, 21 without a target, 46 unsupported; 20 resolved melee weapons with tools before and after; no mod folder of the Workshop or of the custom folder holds a tagless weapon), so the change closes a gap for other mods and moves no number here.
4. **The write fence.** The app builds a `GameWriteFence` when the toolkit context is built (and for `project_create` before that) from the selected install and the settings: the install folder and its game root, the Workshop content folders, the extra Workshop folders and the install and user folder overrides of the settings, the user data folder and its `Config` folder. The fence is handed to the `ProjectEnv` through `with_fence` (`ProjectEnv::fence()` reads it back), so every project write is checked by it in addition to the folder lists. It never writes anything itself. Without an install there is nothing to protect and no fence. A fence that cannot be built is logged and the folder lists stay the only defence. The test `write_fence.rs` of `rimstudio-app` shows a write addressed through the install refused by the fence alone (the project root above the install, no folder list).
5. **Answer groups.** `ConvertCandidateDto.family` is the family key of a weapon: the kind, the first weapon tag (`untagged` without one) and, for a gun, the default projectile (`ranged/RS_Rifle/RS_Shot00`, `melee/RS_Blade`). Weapons of one family have the same caliber and class, which most answers depend on. `ConvertRequestDto.groups` is a list of `ConvertAnswerGroupDto { family?, defNames, answers }` where `answers` is the raw object the user wrote. A group applies to a definition when its name is in `defNames`, or its `family` equals the family key of the definition, or it names neither (the defaults). `convert_plan` merges the applying groups in order, then the request's own `answers` on top, member by member (objects merge, everything else including the tool penetration list is replaced); the two plain boolean flags of the override block that are `false` in the request count as unset so that they never hide a group's `true`. A group that is not an object is `designer.invalid-draft`. The pure functions are `designer::answers_for_def` and `designer::group_applies`. CLI: the answers file takes `default` (the first group), `groups` (`{ "family": KEY or "defNames": [...], "answers": {...} }`, a group without either is a usage error) and `defs`; order of precedence is default, groups in order, `defs`, then `--set`. `convert scan` prints the family column and a hint when several weapons with open questions share a family. The help of `convert plan` and `convert apply` documents the file shapes (`toolPenetration` as a list of `{ "tool", "blunt": { "value" } }`, overrides as `{ "value" }` objects, the groups) with one example each, and `designer plan` and `designer apply` document `ce.toolPenetration.N.tool`.
