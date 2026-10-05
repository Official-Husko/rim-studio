# Community datasets analysis

Scope: the community sorting rules dataset and the related datasets that RimSort consumes (SteamDB, Use This Instead, No Version Warning, RimWorld versions), plus one adjacent dataset (Multiplayer Compatibility) that RimCrow ingests. For each: purpose, real schema (including fields and type quirks that the documentation does not mention), counts, transport behaviour, update cadence, licence terms and what they allow, data-quality findings, and measured coverage against the owner's mod library. It also compares rule semantics with the game's own About.xml semantics, derives Rust serde models that survive round trips, and measures JSON parse cost for the 49 MB SteamDB. It serves requirement R6 (compatibility, auto-fetching) and R10 (JSON only for app data). The companion design is `docs/research/rules-fetch-and-merge-design.md`.

Status: research note | Last verified: 2026-10-04

All web sources were accessed on 2026-10-04 with the header `User-Agent: rimstudio-research`. Numbers come from the scripts in `docs/research/data/community-datasets/` and `docs/research/data/rules-fetch-design/`; their outputs are committed under `docs/research/data/community-datasets/results/`. Raw dataset copies are deliberately not committed (see the licence section). Evidence for RimSort behaviour is the source tree under `RimSort-main/` (read-only, concepts only).

## 1. Dataset catalogue at a glance

| Dataset | Canonical raw URL | Format | Raw size | gzip on the wire | Entries | Version field | Licence in repo |
|---|---|---|---|---|---|---|---|
| Community rules | https://raw.githubusercontent.com/RimSort/Community-Rules-Database/main/communityRules.json | JSON, 4-space indent, ASCII-escaped | 394,716 B | 49,277 B (12.5%) | 631 subject entries, 1,568 edges | `timestamp` (epoch int) | none found |
| SteamDB | https://raw.githubusercontent.com/RimSort/Steam-Workshop-Database/main/steamDB.json | JSON, 4-space indent, ASCII-escaped | 49,312,694 B | 3,512,663 B (7.1%) | 57,679 workshop entries | `version` (epoch int) | none found |
| Use This Instead (UTI) | https://raw.githubusercontent.com/emipa606/UseThisInstead/main/replacements.json.gz | JSON inside gzip, UTF-8 BOM | 1,384,108 B decompressed | 185,498 B (file is already gzip) | 2,718 rules | `version` (ISO 8601 string) | MIT (Mlie, 2020) |
| No Version Warning (NVW) | https://raw.githubusercontent.com/emipa606/NoVersionWarning/main/1.6/ModIdsToFix.xml (one file per game version folder 1.3 to 1.6) | XML list of package ids (third-party file, read through the XML boundary) | 22,706 B for 1.6 | not measured | 318 ids for 1.6 | none (use the folder name) | MIT (Mlie, 2020) |
| RimWorld versions | https://raw.githubusercontent.com/bukforks/rimworld-versions/main/rimworld_versions.json | JSON | 142,968 B | not measured | 587 version records | none | none found |
| Multiplayer Compatibility (adjacent, not RimSort) | repository https://github.com/rwmt/Multiplayer-Compatibility | JSON list (source file path inside the repo not re-verified) | 375,578 B (earlier capture) | n/a | 3,402 records | none | MIT (Meru, 2019) |

The default RimSort URLs are the `archive/refs/heads/main.zip` forms of these repositories (`RimSort-main/app/models/settings.py`, fields `external_*_url`); the raw URLs above were verified to return HTTP 200 and to carry the same bytes (the zip for Community-Rules-Database is 50,458 B, for SteamDB 3,513,035 B). RimCrow uses `blob` URLs that it rewrites to raw (`RimCrow-main/backend/managers/mgr_download.py`, `RimCrow-main/backend/settings.py`).

## 2. Transport behaviour (measured)

| Observation | Value | Evidence |
|---|---|---|
| raw.githubusercontent.com response to a gzip request | `content-encoding: gzip`, weak ETag `W/"..."`, `cache-control: max-age=300` | scratch `community-datasets/headers/communityRules.get.gzip.txt` |
| Same URL with identity encoding | strong ETag `"..."`, a different value from the weak one | `rules-fetch-design/cr/hdr_identity.txt` vs the gzip capture |
| brotli and zstd requested | server ignored them and returned identity bytes (394,716 B in both cases) | `rules-fetch-design/cr/body_br.bin`, `body_zstd.bin` |
| Conditional GET with `If-None-Match` (strong tag) | HTTP 304, no body, `max-age=300` repeated | live check 2026-10-04 |
| Conditional GET on the Atom feed `commits/main.atom` | HTTP 304 with the feed ETag | live check 2026-10-04 |
| Range request on steamDB.json | HTTP 206 with 100 bytes | live check 2026-10-04 |
| Rate-limit headers | none present on raw or Atom responses | all captured header files |
| Edge cache | `x-cache: HIT`, `via: varnish`; the freshness window is 300 s, so polling faster than every 5 minutes is pointless | headers |
| SteamDB gzip download (warm edge cache, this machine) | 3,512,663 B in 0.17 s | `rules-fetch-design/steamdb/download_log.txt` |
| SteamDB through the repository zip | 302 to codeload.github.com, 3,513,035 B in 0.63 s; no gain over gzip | same log |
| UTI file | served as `application/octet-stream`, strong ETag, no content-encoding (already gzip) | `community-datasets/headers/uti.raw.get.txt` |
| jsDelivr mirror, communityRules.json | HTTP 200, 394,716 B, `cache-control: public, max-age=604800, s-maxage=43200` | live check |
| jsDelivr mirror, steamDB.json | HTTP 403 "File size exceeded the configured limit of 20 MB." | live check |

Consequences worth remembering: the weak ETag belongs to the gzip representation and the strong one to the identity representation, so a client must store the ETag together with the `Accept-Encoding` it used and always replay the same pair. The 49 MB SteamDB cannot be mirrored through jsDelivr at all (20 MB limit), while the 395 KB rules file can, with up to about 12 hours of edge staleness for `@main`.

## 3. Community rules (communityRules.json)

### 3.1 Purpose and lineage

A hand-curated list of extra load-order rules for mods whose authors did not declare them in About.xml, with free-text notes. The RimSort editor docstring calls the format "Paladin communityRules.json style" (`RimSort-main/app/windows/rule_editor_panel.py`), and the RimPy documentation describes the same rules-based approach (a sorting model built on pairwise rules rather than category groups). RimSort's user guide says the file is "compatible" with the user rules file `userRules.json`, which has the identical schema (`RimSort-main/docs/user-guide/databases.md`).

### 3.2 Complete observed schema

```text
root            { "timestamp": int (epoch seconds), "rules": { <packageId>: Rule } }
Rule            { "loadAfter"?: { <packageId>: Edge }, "loadBefore"?: { <packageId>: Edge },
                  "incompatibleWith"?: { <packageId>: Edge },
                  "loadTop"?: { "value": bool, "comment"?: string|[string] },
                  "loadBottom"?: { "value": bool, "comment"?: string|[string] } }
Edge            { "name": string | [string], "comment"?: string | [string] }
```

Observed counts (`results/community_rules_summary.json`, generated by `docs/research/data/rules-fetch-design/analyze_community_rules.py`):

| Item | Value |
|---|---|
| File timestamp | 1789239946 = 2026-09-12T19:05:46Z |
| Subject entries (`rules` keys) | 631 |
| Entries by key combination | loadAfter only 400, loadBefore only 137, both 76, loadBottom only 5, incompatibleWith only 5, others 8 (9 distinct combinations) |
| Edges | loadAfter 1,124; loadBefore 439; incompatibleWith 5; total 1,568 |
| loadTop / loadBottom flags | 1 / 12 (all `true`) |
| Edges with a comment | 543 of 1,568 |
| `name` as list vs string | loadAfter 1,073 list / 51 string; loadBefore 376 / 63 |
| `comment` forms | string or list of strings, never an object |
| Distinct package ids (subjects and targets) | 1,241 (631 subjects, 818 targets) |
| Largest in-degree | 47 (`seohyeon.optimizationmeats`); largest out-degree 30 (`ceteam.combatextended`) |
| Unknown rule keys | none; `incompatibleWith` is the only key that RimSort's typed schema does not model (see 3.4) |
| Serialisation | equals Python `json.dumps(indent=4, ensure_ascii=True)` plus a trailing newline, except one whitespace-only line of 17 characters (verified: byte-identical after removing that line) |

The `name` field is the display name of the target mod (a hint for humans), not a rule parameter, which is why it can be a list when a package id appears under several names.

### 3.3 Data-quality findings

| Finding | Detail |
|---|---|
| Mixed-case package ids | 2 subject keys contain uppercase (for example `MOFSLBLECN209.rimfacehforhfacialhstuffh1v0`) and 2 targets; no pair collides after lowercasing (0 lowercase collisions), so lowercasing is lossless today |
| Rule-induced cycles | 2 two-cycles inside the dataset alone (for example `armorguy1.fapatches` with `daemon976.facialanimationplus`), largest cyclic component 8 nodes; a real topological sorter must tolerate them |
| Same pair in both directions | 2 pairs appear as both loadAfter and loadBefore of each other |
| Many edges to absent mods | the rules reference 1,241 ids; only 64 subjects and 82 targets are installed in the owner's library |
| Empty or missing fields | no edge lacks `name`, none has an empty `name` list |
| Key order | neither case-sensitive nor case-insensitive sorted, so upstream diffs depend on insertion order |
| No CRLF, no BOM, ends with a newline | simplifies byte-faithful editing |
| Stale rules | not measurable from the file alone; the design adds a "redundant with About.xml" detector (design doc section 8) |

### 3.4 Where RimSort's typed model loses data

`docs/research/data/rules-fetch-design/rimsort_semantics_probe.py` re-implements RimSort's msgspec schema (`ExternalRule`, `ExternalRulesSchema` in `RimSort-main/app/models/metadata/metadata_structure.py`) and decodes the real file. This is a simulation of RimSort's schema, not a run of RimSort itself:

* All 5 `incompatibleWith` edges are invisible after the typed decode (0 of 5 survive), because the schema has no such field and unknown fields are silently dropped (`unknown_field_rejected: false`).
* A save through the editor path adds lowercase twins of the 2 mixed-case keys (631 become 633 entries) and updates the timestamp even when nothing was edited.
* Re-encoding through the typed model shrinks the file from 394,716 B to 217,293 B on one line (defaults omitted, formatting lost).
* Adding an incompatibility to a mod that already had one lost the pre-existing target in the probe.
* Wrong shapes (a `name` object, a list as `rules`, a string timestamp) are rejected with a decode error, which RimSort reports as "could not be decoded" and then runs with no rules (`read_rules_db` in `RimSort-main/app/models/metadata/metadata_factory.py`).

RimStudio therefore must not copy this model: it must keep unknown fields and never silently drop `incompatibleWith`.

## 4. SteamDB (steamDB.json)

### 4.1 Purpose

RimSort uses it mainly to supply dependency data for workshop mods that are not downloaded (`RimSort-main/docs/user-guide/databases.md`), and for name, author, tags and game-version lookups by workshop id. It is crawled and rebuilt by RimSort's DB builder (`RimSort-main/docs/user-guide/db-builder.md`).

### 4.2 Observed schema

Root: `{ "version": int epoch, "database": { "<workshopId digits>": Entry } }`. `version` is 1789309137 = 2026-09-13T14:18:57Z. All 57,679 keys are digit strings.

| Entry field | Present in | Type observed | Note |
|---|---|---|---|
| `url` | 57,504 | string | workshop page URL |
| `steamName` | 57,487 | string | differs from `name` in 2,141 entries |
| `tags` | 56,977 | list of objects | 32.1% of entry bytes |
| `dependencies` | 32,401 | object keyed by workshop id, value always a list of strings (53,820 of 53,820) | RimSort's schema also allows an object with `name` and `url` |
| `packageId` | 18,633 | string | only 32% of entries carry a package id |
| `packageid` (lowercase key) | 6 | string | the same 6 entries also have `packageId`: both keys in one object |
| `name`, `authors` | 18,578 each | string (authors is a comma-joined string, never a list) | 177 entries have no name at all |
| `gameVersions` | 18,572 | list of strings (6 are `[null]`) | |
| `unpublished` | 256 | bool | 171 entries carry only this flag |
| `appid` | 6 | bool | undocumented |
| `blacklist` | 0 | in RimSort's schema, absent from the data | dead field today |

Placeholder package ids that must never be treated as real: `scenario.rsc` (1,621 entries), `missing.packageid` (878), `invalid.item` (5). One package id maps to up to 1,621 workshop entries; 155 package ids map to more than one workshop entry (reuploads, scenarios). Only 15,980 distinct package ids exist for 57,679 entries.

Size by field (minified JSON bytes): tags 7.15 MB, dependencies 6.18 MB, url 4.19 MB, steamName 2.37 MB, packageId 0.69 MB, name 0.66 MB, gameVersions 0.58 MB, authors 0.43 MB. A "slim" projection keeping only package id, name, authors, game versions, dependency ids and flags for the 18,848 entries that have an id is 2,516,068 B minified, 759,836 B in gzip-9, 573,745 B in zstd-19. Full-file compression: gzip-6 3,512,663 B, gzip-9 3,323,156 B, xz-6 2,042,124 B, zstd-3 3,085,070 B, zstd-19 1,975,479 B (`results/steamdb-summary.json`, from `analyze_steamdb.py`).

### 4.3 Parse cost, JSON only (R10)

Method: `docs/research/data/rules-fetch-design/jsonbench/` (a throwaway benchmark, build outputs deleted). Release build with `target-cpu=native`, serde 1.0.229, serde_json 1.0.151, simd-json 0.18.1, Intel Core i9-9900K (16 threads), Linux, file in the page cache (read takes 7 to 12 ms), 3 runs each, peak RSS from `VmHWM` (includes the 49 MB input buffer).

| Strategy | Parse time (ms) | Peak RSS (MB) |
|---|---|---|
| serde_json into `Value` | 235 to 244 | 293 |
| serde_json into a map of `Value` entries | 258 to 298 | 296 |
| simd-json owned value | 164 to 174 | 212 |
| serde_json typed slim struct (unknown fields skipped) | 84 to 101 | 92 |
| simd-json typed slim struct | 74 to 75 | 155 |
| serde_json on the pre-built slim index (3,665,215 B JSON, 18,639 entries) | 23 to 26 | 18 |

Supporting measurements: gunzip of the 3.5 MB download takes 44 ms with `zcat`; Python with the orjson parser parses the full file in 0.27 s (`python_json_parse_seconds`). Cold-disk reads and slower CPUs were not measured.

Reading of the numbers: a typed, skipping parse of the full file already costs under 0.1 s on this machine and 92 MB of transient memory, so JSON alone meets a sensible startup budget (for example under 200 ms off the critical path). The decisive win is not a faster parser but not parsing most of the file: building a slim index once after each download and storing it as JSON drops later startups to about 25 ms and 18 MB. SIMD parsing gains 12% over serde_json for typed work at higher memory, which does not justify a dependency. A binary cache would only beat the slim JSON index by a few milliseconds and conflicts with R10, so it is not recommended; if profiling on low-end hardware later disagrees, keep it as an optional derived cache that is rebuildable and never authoritative.

A serde pitfall found by this benchmark: deriving a field with `alias = "packageid"` next to `packageId` fails with "duplicate field `packageId`" on the 6 entries that carry both keys. Model them as two separate optional fields and merge.

### 4.4 Cadence and contributors

Atom feed of the main branch (`commits/main.atom`, last 20 entries): 2025-09-01 to 2026-09-13, median gap 3.5 days, 4 distinct authors; the feed is 16 KB and answers 304 to a conditional request. The repository front page showed 45 commits (parsed from HTML, approximate).

## 5. Use This Instead (replacements.json.gz)

Purpose: maps outdated mods to maintained replacements; RimSort loads it indexed by `oldWorkshopId` (`_load_use_this_instead` in `RimSort-main/app/models/metadata/metadata_mediator.py`).

Schema: `{ "version": "2026-10-04T09:20:54Z", "rules": [ Rule ] }` where every one of the 2,718 rules has the same 10 keys:

```text
oldWorkshopId, oldName, oldAuthor, oldPackageId, oldVersions[],
newWorkshopId, newName, newAuthor, newPackageId, newVersions[]
```

Type quirks (counted by the script `analyze_small_datasets.py`, plus a type tally): `oldPackageId` is a string in 2,715 rules, an integer in 2 rules (for example the value 1797397487, equal to the workshop id) and `null` in 1; `oldAuthor` and `newAuthor` are `null` in 8; 437 rules have an empty `oldPackageId`; ids are strings in `oldWorkshopId` and `newWorkshopId`. Old workshop ids are unique (2,718 of 2,718). One rule maps an id to itself. `newVersions` contains 1.6 in 2,492 rules (versions seen: 0.19 up to 2.0, including 1.7 to 2.0 on a handful of rules). The file starts with a UTF-8 BOM, and the gzip header carries no modification time.

Cadence: Atom for the file path, last 20 commits 2026-09-06 to 2026-10-04, median gap 1.2 days, a single author (looks automated). Front page: 484 commits. Licence: MIT.

Matching trap: matching installed mods by package id alone is wrong, because many replacements keep the old package id (for example a "(Continued)" reupload under the same id). In the owner's library 53 workshop mods match some `oldPackageId`, but only 22 match by `oldWorkshopId`. Match by workshop id first.

## 6. No Version Warning (ModIdsToFix.xml)

Purpose: package ids for which the version-mismatch warning should be suppressed. The upstream source is a RimWorld mod whose data folders are named by game version (1.3, 1.4, 1.5, 1.6); RimSort reads a single `ModIdsToFix.xml` and falls back to the versioned subfolder (`_load_no_version_warning`, `RimSort-main/app/models/metadata/metadata_mediator.py`).

Format: `<ModIdsToFix>` containing `<li>packageId</li>` items, each preceded by an XML comment holding the mod name, UTF-8 BOM. Counts (`results/small_datasets_summary.json`):

| Folder | Bytes | `li` entries | Distinct lowercase | Entries with uppercase |
|---|---|---|---|---|
| 1.3 | 6,405 | 87 | 87 | 52 |
| 1.4 | 10,139 | 142 | 142 | 110 |
| 1.5 | 18,682 | 269 | 269 | 208 |
| 1.6 | 22,706 | 318 | 318 | 244 |

Cadence: the 1.6 file's Atom feed shows 20 commits from 2026-03-13 to 2026-10-03, median gap 7.3 days, one author. Licence: MIT. Since RimStudio keeps XML only at the boundary, the cache is a JSON list of lowercase ids derived from this XML.

## 7. RimWorld versions (rimworld_versions.json)

Root `{ "depots": { <group>: { <platform>: depotId } }, "versions": { <platform>: [ { "version", "manifest_id", "status", "dlcs" } ] } }`. Depot groups: base_game, anomaly, odyssey, royalty, ideology, biotech; platforms win32, mac, linux, win64. 587 version records (150 each for win32, mac, linux; 137 for win64), every `status` is `OK`; the newest linux label is `1.6.4871 rev598`, which equals the installed game. RimSort uses it for its "Download RimWorld Version" feature (`RimSort-main/docs/user-guide/databases.md`). Cadence: 2 commits, both 2026-07-08, one author. For RimStudio it is low priority (a version label to depot lookup); the game version itself must come from the install's `Version.txt`.

## 8. Adjacent datasets

| Dataset | Finding |
|---|---|
| Multiplayer Compatibility metadata | 3,402 records with keys `name`, `status`, `workshopId` and optional `notes` (1,356 have notes); status values 0 (3), 1 (293), 2 (160), 3 (231), 4 (2,715). RimCrow downloads the repository zip of the `master` branch for it (`RimCrow-main/backend/settings.py`, `mgr_multiplayer_compat.py`). The meaning of the status numbers is unverified. Not required by R6; MIT licensed (Meru, 2019). |
| RimPy community rules | RimPy documents the same rules concept (`rimpy.custom.org` page captured in scratch); no separate dataset beyond what RimSort's file continues. |
| Other RimWorld sorting lists found by name search (modlist-consultant, RimDocPlus) | cloned in earlier research, no machine-readable rule dataset suitable for R6 was identified; treat as unverified candidates. |

## 9. Licence analysis

| Repository | Licence file found (raw URL, branch main) | What it allows | Implication for RimStudio |
|---|---|---|---|
| RimSort/Community-Rules-Database | none: `LICENSE`, `LICENSE.md`, `LICENSE.txt`, `COPYING` and `README.md` all return 404 | no explicit grant; default copyright applies to contributions | fetch at runtime only, as RimSort's own client does; do not bundle, republish or mirror the data (including a CI-built compact copy) without written permission; ask the maintainers to add a licence |
| RimSort/Steam-Workshop-Database | none (same four names, 404) | same | same; a locally computed slim index stays on the user's machine, which is a derived cache, not a redistribution |
| emipa606/UseThisInstead | `LICENSE.md`: MIT, Copyright (c) 2020 Mlie | copy, bundle, redistribute with the notice | may be bundled as a fallback seed with attribution; fetching remains preferred for freshness |
| emipa606/NoVersionWarning | `LICENSE.md`: MIT, Copyright (c) 2020 Mlie | same | same |
| bukforks/rimworld-versions | none (404 on four names) | no explicit grant | runtime fetch only |
| rwmt/Multiplayer-Compatibility | `LICENSE`: MIT, Copyright (c) 2019 Meru | same | optional later |

RimSort itself is GPL-3.0 and is a read-only concept reference (R11); the data repositories are separate repositories and the GPL text does not appear in them. This is a technical reading of the repositories, not legal advice, and the absence of a licence file is the finding, not a grant. The practical rule for R11: never commit any of these datasets (the repo ships only synthetic fixtures), never publish a RimStudio-hosted copy of the two unlicensed datasets, and credit the sources in the dataset manager UI.

## 10. Contribution process (as documented and as observed)

* Rules: the user guide says users can open a pull request manually on GitHub, or use RimSort's optional GitHub integration after cloning the repository through its git feature (`RimSort-main/docs/user-guide/databases.md`; the guide notes that GitHub identity is only needed to upload). The Community-Rules-Database repository has a `.github/workflows` folder (visible on the repository page); its content was not read.
* Atom feed over the last 20 commits: 12 distinct authors on the main branch, 11 on the file path of `communityRules.json`; the front page showed 86 commits.
* SteamDB is machine-built by the DB builder and committed in bursts (4 authors over 20 commits).
* UTI and NVW are maintained by one author in the mod's own repository, which the mod itself consumes.

## 11. Coverage against the owner's library

Library scan: `docs/research/data/community-datasets/library_scan.py` (743 mods: 690 workshop, 47 local, 6 official) and `coverage.py` for everything below (`results/coverage-owner-library.json`). Game version 1.6. The active list in `ModsConfig.xml` has 610 distinct package ids under `activeMods` (the file holds 615 `li` elements in total; the other 5 belong to the known-expansions list), and all 610 were found in the scan.

### 11.1 Community rules

| Kind | In dataset | Subject active | Both ends active | Both installed |
|---|---|---|---|---|
| loadAfter | 1,124 | 176 | 59 | 63 |
| loadBefore | 439 | 102 | 34 | 42 |
| incompatibleWith | 5 | 2 | 0 | 0 |
| loadTop / loadBottom flags on active subjects | n/a | 1 / 5 | | |

Only 90 distinct ordering constraints (after normalising to earlier/later pairs) bind two active mods. Of these, 34 are already implied by the mods' own About.xml data and 56 add a new constraint; none contradicts About.xml. The About.xml rules of the active set (3,199 normalised ordering pairs, including dependencies as the game's rule set defines them) contain one 2-node cycle already; the community rules add no cycle in the active set. Conclusion: for this library the community dataset is a small but real refinement (about 6% additional pairs), and its value grows with larger or more varied libraries; the SteamDB and replacement data matter at least as much.

### 11.2 SteamDB agreement with About.xml

| Measure | Value |
|---|---|
| Workshop folders present in SteamDB | 682 of 690 (8 absent) |
| Entries without a package id | 227 |
| Package id equal ignoring case | 454 (1 differs) |
| Equal ignoring case but different in exact case | 363 of the 454 (80%) |
| Entries with `gameVersions` | 454 (228 without) |
| `gameVersions` equal to About.xml supported versions | 451 |
| SteamDB lists the current version, About.xml does not / the reverse | 0 / 2 (1 other disagreement) |
| Entries with dependencies in SteamDB / mods with dependencies in About.xml | 449 / 491 |
| Folder flagged unpublished | 1 |

Package-id case differences are the norm, so every lookup must be case-insensitive. SteamDB was fully usable as an id and version cross-check for 99% of the present entries, but About.xml remains the primary source for installed mods and SteamDB mainly serves uninstalled ones.

### 11.3 Replacements and version warnings

| Measure | Value |
|---|---|
| UTI rules matching an installed workshop id | 22 (all 22 have a replacement that supports 1.6; 6 replacements are already installed; 0 are in the active list) |
| Mods matching an `oldPackageId` | 53 (false positives for reuploads, see section 5) |
| NVW 1.6 ids installed / active | 16 / 14 |
| Active mods that do not declare 1.6 in About.xml | 12; all 12 are listed in NVW (0 unexplained) |

## 12. Rule semantics versus the game's About.xml semantics

The game's data comes from the decompiled game code; RimSort's from its source tree.

| Aspect | Game (About.xml) | Community and user rules (RimSort) |
|---|---|---|
| Order edges | `loadBefore` and `loadAfter` are soft hints shown as conflicts on the mod list; `forceLoadBefore` and `forceLoadAfter` additionally reorder mods during activation (decompiled:Verse/ModsConfig.cs, activation loop and `TryReorder`) | `loadBefore` and `loadAfter` only; no force variants |
| Built-in sort | `TrySortMods` builds one graph from loadBefore, forceLoadBefore, loadAfter and forceLoadAfter, aborts with a dialog when a cycle is found, otherwise topologically sorts (decompiled:Verse/ModsConfig.cs, TrySortMods) | RimSort sorts itself; cycles are reported, not fatal |
| Id matching | matches ignoring the platform postfix (`SamePackageId` with `ignorePostfix`), case-insensitive (decompiled:Verse/ModMetaData.cs) | lowercased everywhere (`read_rules_db` lowercases the subject keys; the sets are case-insensitive) |
| Incompatibility | a tooltip warning on the mod row, not an error (decompiled:Verse/ModsConfig.cs, conflict text builder) | modelled, but dropped by the typed decode (section 3.4) |
| Version overrides | `*ByVersion` blocks replace the base list exactly, not additively (decompiled:Verse/ModMetaData.cs, VersionedData) | not applicable to community rules, which have no version dimension |
| Top and bottom | no equivalent | `loadTop` places a mod in tier one, `loadBottom` in tier three; tier zero is a fixed set (Harmony, pre-patcher, official content) in `RimSort-main/app/utils/constants.py` |
| Dependencies | `modDependencies` do not imply order by themselves | RimSort adds them as loadAfter edges only when its option is enabled, and drops an inferred edge that contradicts an explicit one (`CompiledDependencyData.build`) |
| Merge | n/a | set union of About, community and user lists (`overall_rules` in `metadata_structure.py`); the docstring says user beats community beats About, but for order edges nothing is overridden or removed: only the dependency dictionary is overridden key by key; loadTop and loadBottom are OR-ed |
| Missing mods | ignored when the target is not active | edges to ids that are not in the mod set are skipped at graph build |

Practical conclusion: community rules are a superset of soft edges plus two tier flags, none of which the game enforces; a user cannot remove a community edge in RimSort, which is a gap RimStudio should close.

## 13. Minimal Rust serde models (round-trip safe)

The goals: accept every shape seen above, keep unknown fields, keep key order, never fail on a single odd value.

```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap; // with serde_json "preserve_order", IndexMap semantics apply to Value

type Extra = serde_json::Map<String, serde_json::Value>;

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany { One(String), Many(Vec<String>) }

#[derive(Serialize, Deserialize)]
pub struct Edge {
    #[serde(default, skip_serializing_if = "Option::is_none")] pub name: Option<OneOrMany>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub comment: Option<OneOrMany>,
    #[serde(flatten)] pub extra: Extra,
}

#[derive(Serialize, Deserialize)]
pub struct FlagRule {
    #[serde(default)] pub value: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub comment: Option<OneOrMany>,
    #[serde(flatten)] pub extra: Extra,
}

#[derive(Serialize, Deserialize)]
pub struct Rule {
    #[serde(default, rename = "loadAfter", skip_serializing_if = "Option::is_none")]
    pub load_after: Option<indexmap::IndexMap<String, Edge>>,
    #[serde(default, rename = "loadBefore", skip_serializing_if = "Option::is_none")]
    pub load_before: Option<indexmap::IndexMap<String, Edge>>,
    #[serde(default, rename = "incompatibleWith", skip_serializing_if = "Option::is_none")]
    pub incompatible_with: Option<indexmap::IndexMap<String, Edge>>,
    #[serde(default, rename = "loadTop", skip_serializing_if = "Option::is_none")] pub load_top: Option<FlagRule>,
    #[serde(default, rename = "loadBottom", skip_serializing_if = "Option::is_none")] pub load_bottom: Option<FlagRule>,
    #[serde(flatten)] pub extra: Extra,
}

#[derive(Serialize, Deserialize)]
pub struct RulesFile {
    pub timestamp: i64,
    pub rules: indexmap::IndexMap<String, Rule>,
    #[serde(flatten)] pub extra: Extra,
}
```

Notes: `Option` plus `skip_serializing_if` preserves "absent" versus "empty". Keys keep their original case in the model and are lowercased only in lookup indexes. SteamDB entries use a slim typed struct with `#[serde(flatten)] extra` only in the editing path, not in the read-only index (flatten defeats the skip-parse speed shown in 4.3), and separate optional fields `package_id` and `package_id_lower_key` for the 6 double-key entries. UTI rules use `serde_json::Value` for `oldPackageId` (string, integer or null) and normalise to `Option<String>` after reading. All timestamps are modelled as an enum of integer epoch and ISO string because the three datasets use different forms.

Byte-faithful writing: the file is `json.dumps(indent=4, ensure_ascii=True)` plus a newline, so a serde_json pretty formatter with 4 spaces and a custom escape of non-ASCII characters (uppercase or lowercase hex digits must match the original, to be tested) reproduces it, apart from the one stray whitespace line.

## Implications for RimStudio

1. Treat communityRules.json as the schema `{timestamp, rules}` with the five keys above and preserve every unknown key and the original key order on read and write (test: round trip of the real file and of synthetic fixtures with unknown keys is byte-identical after one normalised pass).
2. Never drop `incompatibleWith`: model it, surface it as a warning type, and add a regression test using the 5 real edges' shape in a synthetic fixture.
3. Accept `name` and `comment` as string or list of strings, `oldPackageId` as string, integer or null, `gameVersions` entries as string or null, and both `packageId` and `packageid` keys in one object (test each shape; a single odd record must degrade to a warning, never fail the whole dataset).
4. Lowercase package ids only in lookup indexes; keep the original case for display and for writing back to community format (2 mixed-case keys exist today).
5. Do not bundle or mirror the Community Rules and SteamDB data (no licence file); fetch at runtime from the user's machine, and build the slim SteamDB index locally. Bundling UTI and NVW as a seed is permitted under MIT with attribution but is optional.
6. Use the slim JSON index (package id keyed, about 2.5 to 3.7 MB, 18 MB RSS, about 25 ms parse) for runtime lookups and avoid keeping the 49 MB document in memory; budget 100 ms and 100 MB transient for the one-time rebuild after a download.
7. Store ETag together with the `Accept-Encoding` used and replay both on revalidation; polling faster than 300 seconds per dataset is pointless.
8. Match Use This Instead by workshop id first and by package id only as a hint (53 package-id hits versus 22 id hits in the owner's library).
9. Use `Version.txt` of the install for the game version and treat rimworld_versions.json as optional.
10. Make the rule engine tolerate cycles in input data (2 two-cycles in the dataset alone, 1 in the owner's own About.xml set) and report them with their edges' provenance instead of failing.

## Open questions

1. Can the RimSort maintainers confirm a licence for Community-Rules-Database and Steam-Workshop-Database (and permit a RimStudio-hosted compact mirror)? A request should be filed before any hosted artifact is built.
2. What do the Multiplayer-Compatibility status values 0 to 4 mean, and which file in that repository is the machine-readable source? Not re-verified in this pass.
3. What does the `.github/workflows` automation in Community-Rules-Database do (validation of pull requests, timestamp updates)? It was not read.
4. How often do the 6 double-key SteamDB entries change, and which tool produces them? No evidence in the data.
5. Do the weak ETag of the gzip representation and `If-None-Match` revalidate correctly on repeated requests? The strong-ETag path was verified with a 304; the weak gzip path was not re-tested.
6. Cold-start and low-end hardware timings for the parse benchmark were not measured; the budget in this note is from one fast desktop.
