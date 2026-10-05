# Scan and index performance spike

Scope: measurements with real Rust code on the real RimWorld mod library (690 workshop mods, the game install, the owner's mods on an external drive, the Combat Extended source) to set RimStudio's performance budgets for directory scanning, About.xml and Defs parsing, persisted caches, memory and thread scaling. All benchmark source is saved under `docs/research/data/perf-spike/` and raw result tables under `docs/research/data/perf-spike/results/`. Numbers are indicative: other jobs ran on the machine throughout.

Status: research note | Last verified: 2026-10-04

## Method and environment

| Item | Value |
|---|---|
| CPU | Intel Core i9-9900K, 8 cores / 16 threads, 16 MiB L3, max 5.0 GHz (`results/env.txt`) |
| RAM | 32 GB (about 13 GB page cache at start) |
| Internal storage | the workshop and game install live on `/home`: btrfs on a LUKS volume over a SATA SSD (Samsung 860 EVO, ROTA=0) |
| External storage | owner mods and the CE source live on `/run/media/pawbeans/project_drive`: btrfs on a 2.5 inch spinning disk (Seagate ST1000LM035, ROTA=1) |
| Load | load average 6.3 to 9.9 while measuring (about 6 cores busy with other jobs); a "quiet gate" waited at most 0.5 s per run, so most runs happened under that background load |
| Toolchain | Rust 1.96, release profile with fat LTO, one Cargo project (`rimstudio-perf-spike`), edition 2024 |
| Crates | std, walkdir 2.5, jwalk 0.9, ignore 0.4, rayon 1.12, quick-xml 0.42, roxmltree 0.21, serde_json 1, sonic-rs, simd-json, bincode 2.0.1, rkyv 0.8, lasso 0.7 (multi-threaded), memchr, rustc-hash |
| Runs | at least 5 per cell, variants interleaved round-robin, median reported with min/max where useful |
| Cold cache | approximate: `posix_fadvise(POSIX_FADV_DONTNEED)` on every file that is read. No root, so `echo 3 > drop_caches` is impossible. fadvise drops file data pages only: inode and directory metadata stay cached by the kernel, so cold numbers undercount a truly cold boot (metadata seeks on the HDD would add more). |

Datasets (counts from my scripts):

| Dataset | Files | Dirs | XML | Bytes |
|---|---|---|---|---|
| workshop (690 mods) | 306,394 | 59,348 | 46,705 | 31.9 GB |
| game `Data` (Core + 5 DLC) | 1,887 | 562 | 1,672 | 0.77 GB |
| game `Mods` | 42,154 | 9,935 | not counted | 3.3 GB |
| owner mods (27 mod folders) | 5,241 | 1,806 | 170 | 0.79 GB |
| CE source | 12,312 | 4,517 | 9,867 | 92 MB |

The Def index stages use the workshop plus `Data` (697 mod folders), with a deliberately generous "superset" rule: every `*.xml` below any `Defs` directory of the mod root, any `Common`, any version-like folder and any folder named in `LoadFolders.xml`, duplicates removed. That is 23,717 def files, 221.5 MB, 134,324 defs (128,376 with a `defName`; the rest are abstract bases or non-def children). Many mods ship the same defs for 1.4, 1.5 and 1.6, so a version-resolved index (what the game would load) is smaller than this superset.

## S1 Directory walk

Task: collect (path, size, mtime, type) for every file, warm cache. All stat-collecting variants were verified to produce an identical digest (file count, byte sum, hash). "rayon" is a hand-rolled parallel recursion using `read_dir` plus `DirEntry::file_type()` plus one `metadata()` per file; "rayon-names" is the same without the per-file stat (names and types only). "jwalk-tuned" is jwalk with sorting off and the stat read from the entry. Times in ms, median of 5.

| Walker | workshop (306k files) | Data (1.9k) | game Mods (42k) | owner, HDD (5.2k) | CE, HDD (12k) |
|---|---|---|---|---|---|
| std recursion, 1 thread | 603 | 4.5 | 86 | 13.1 | 32.9 |
| walkdir, 1 thread | 814 | 6.4 | 118 | 16.0 | 44.0 |
| jwalk default, 16 threads | 670 | 4.4 | 86 | 10.3 | 25.3 |
| jwalk tuned, 16 threads | 215 | 1.6 | 28 | 3.8 | 9.9 |
| ignore, 16 threads | 124 | 3.2 | 21 | 4.5 | 8.9 |
| rayon recursion, 1 thread | 639 | 4.9 | 92 | 14.0 | 33.1 |
| rayon recursion, 4 threads | 183 | 1.6 | 31 | 4.1 | 10.1 |
| rayon recursion, 16 threads | 94 | 1.1 | 15 | 2.5 | 5.2 |
| rayon names only, 16 threads | 50 | 0.7 | 8.6 | 2.1 | 3.9 |

Source: `docs/research/data/perf-spike/results/s1.txt`.

Findings:
- Winner: the hand-rolled rayon recursion. 94 ms for 306k files (3.3 M files/s) with full metadata, 6.4x faster than std and 8.7x faster than walkdir.
- jwalk with default settings is no faster than single-threaded std (it sorts and builds per-directory results); it only helps when tuned, and even then is 2.3x slower than the rayon recursion.
- `ignore` scales well but is built for gitignore filtering. It was the second fastest and reported errors correctly (see pitfalls).
- A single `metadata()` per file costs about as much as the directory read itself: dropping the stat (names only) halves the time (94 to 50 ms). RimStudio should not stat files it does not need.
- The walk is metadata-bound, not CPU-bound: 1 thread to 16 threads gives 6.8x on the SSD. On the external HDD the warm walk is equally fast (kernel dentry cache), but see S6 for cold behaviour.
- A full walk of everything is wasteful anyway: the pruned scan used from S3 on (only `About/`, `LoadFolders.xml` and the target subfolders `Defs`, `Patches`, `Languages`, `Assemblies` per candidate folder) visits 26.7k instead of 306k files and takes 19 to 21 ms for all 697 mods with 16 threads (S6, "walk-only").
- Only 15 percent of the 306k files are XML (46.7k); the rest are textures, sounds and similar assets. A mod manager needs the XML plus assembly names.

## S2 About.xml parsing

Task: lenient parse of every `About/About.xml` into a typed struct (name, author, packageId, supported versions, dependencies, load-before and load-after lists, description), tolerating BOMs, tag-case variants and malformed files without aborting. Three implementations produce the same struct: quick-xml event reader, quick-xml with serde, roxmltree DOM. The event parser has a lenient mode (recovers partial data from broken files) and a strict mode.

Real data: 691 About files found below the 690 workshop mod folders (one mod folder holds two, not investigated), 1.1 MB total, median file 1,136 bytes. All 691 parse with every implementation except serde (1 failure). 691 have a packageId, 486 have `modDependencies`. No file in this library is malformed; the library is cleaner than the robustness matrix below suggests, but the matrix is what real users see in the wild.

| Parser | Parse only, per file | Parse only, 691 files x 30 | End to end 691 files, 1 thread | 16 threads |
|---|---|---|---|---|
| quick-xml events (lenient) | 6.5 us | 134 ms | 8.4 ms | 1.2 ms |
| quick-xml events (strict) | 6.5 us | 136 ms | n/a | n/a |
| quick-xml + serde | 10.6 us | 219 ms | 12.4 ms | n/a |
| roxmltree DOM | 12.7 us | 262 ms | 13.6 ms | n/a |

Robustness matrix on 231 sampled real About files, each damaged in a synthetic way (cells: same result / changed result / partial recovery / failed). Source: `results/s2robust.txt`.

| Damage | events lenient | events strict | serde | roxmltree |
|---|---|---|---|---|
| BOM added, UTF-16 with BOM | 231 same | 231 same | 231 same | 231 same |
| tag names upper-case | 220 same, 11 changed | 220/11 | 0 same, 231 changed | 220/11 |
| HTML in description | 2 same, 229 changed | same | 229 failed | 229 changed |
| duplicate element | 231 changed | 231 changed | 231 failed | 231 changed |
| stray ampersand | 231 changed (recovered) | 231 partial | 231 failed | 231 failed |
| undefined entity (nbsp) | 229 changed | same | 229 failed | 229 failed |
| mismatched end tag | 231 same | 231 partial | 231 failed | 231 failed |
| missing root close, truncated | 231 partial | 231 partial | 231 failed | 231 failed |
| invalid UTF-8 byte (latin1) | 231 changed | 231 changed | 231 changed | 231 changed |

"Changed" here means the typed output differs from the undamaged parse (for example, the description text is altered), not that parsing failed.

Winner: quick-xml event reader in lenient mode. It is the fastest, tolerates most damage (matches the game's tolerant loader, which itself skips problem files with an error log rather than aborting), and gives partial data instead of failures. serde derive is unusable for About.xml: any case change, duplicate or entity problem fails the whole file. roxmltree is strict and fails on the common real-world errors (stray `&`, mismatched tags, undefined entities, HTML in descriptions). Parse cost is irrelevant for the budget anyway: the whole library is 8 ms single-threaded.

Mod-list start-up path (list the mod root, locate `About/About.xml` per mod, read, parse), 697 folders, `spike s6-about`, median of 7:

| Threads | Warm | Cold (approx, data pages dropped) |
|---|---|---|
| 1 | 9.6 ms | 24.5 ms |
| 16 | 2.0 ms | 3.7 ms |

## S3 Def index build

Task: parse all Defs XML of the 697 folders into an index record per def (def type tag, `Class` attribute, `defName`, `Name`, `ParentName`, `Abstract` flag, byte offset, source file). Three parsers with identical output (verified: 0 differing files, 134,324 defs in each, 0 errors): roxmltree DOM, quick-xml streaming, and a hand-written structural scanner on top of memchr ("scan") that only reads element boundaries, attributes and the `defName` child text. The scanner is the aggressive option: it is not a validating parser, so its result must be treated as an index hint (the full XML boundary crate parses a def when it is actually needed).

Totals: 23,717 files, 221.5 MB, 134,324 defs. Medians of 5, ms (`results/s3.txt`).

| Stage | roxmltree 1t | roxmltree 16t | quick-xml 1t | quick-xml 16t | scan 1t | scan 16t |
|---|---|---|---|---|---|---|
| A) parse only, text in memory, owned records | 1,708 | 308 | 638 | 99 | 188 | 26 |
| A2) parse only, no records allocated | 1,649 | 297 | 627 | 93 | 183 | 26 |
| B) end to end: read + decode + parse + records | 1,907 | 346 | 864 | 125 | 395 | 52 |

Derived throughput for stage B (warm):

| Parser | 1 thread MB/s | 1 thread defs/s | 16 threads MB/s | 16 threads defs/s |
|---|---|---|---|---|
| roxmltree | 116 | 70k | 640 | 388k |
| quick-xml | 256 | 155k | 1,772 | 1.07 M |
| scan | 561 | 340k | 4,260 | 2.58 M |

Read strategy with the scan parser, 16 threads, end to end: `fs::read` 57 ms, reusable per-thread buffer with `read_exact` 53 ms, `mmap` 149 ms. mmap is 2.8x slower (page-fault and mapping cost on 23.7k small files); do not use it for bulk scans.

Heavy tail: the slowest 1 percent of files account for 34 to 42 percent of total parse time. The largest def files are 2.4 to 3.4 MB (for example a mod's `Site_Base.xml` at 3.4 MB, and an ancient-market map data file at 2.5 MB, each duplicated across 1.4, 1.5 and 1.6 folders: 25 XML files exceed 1 MB in the workshop). The worst single file takes 22.6 ms in roxmltree, 7.0 ms in quick-xml and 2.2 ms in scan. Per-file parallelism is therefore fine, but a UI that parses on demand must not block on a DOM of a multi-megabyte file on the main thread.

Winner: for the background index, the streaming/structural approach. quick-xml streaming is the safe choice (a real XML tokenizer, 3.5x faster than the DOM at 16 threads); the memchr scanner is a further 2.4x faster end to end and is justified only if the full Def index must be rebuilt often. roxmltree DOM remains right for the detail views (one def, one file) where tree access matters. The XML boundary crate (requirement R10) can expose both: a fast index scan and a DOM parse.

## S4 Incremental manifest and cache formats

Design measured: a manifest with one record per mod, one per def file with (relative path, size, mtime in ns) and the extracted def records. A re-scan walks the pruned mod directories (stat of About, LoadFolders and files in the target subfolders), compares each file's (path, size, mtime) with the manifest, reuses unchanged records and re-parses only changed files. Changes are simulated by bumping the stored mtimes of chosen mods (no real file was touched). 697 mods, 23,717 files, 134,324 defs, 16 threads, medians of 5.

| Scenario | Wall ms | Files re-parsed | Bytes re-read |
|---|---|---|---|
| Full build, no manifest | 130 | 23,717 | 221.5 MB |
| Warm re-scan, nothing changed | 23.2 | 0 | 0 |
| Warm re-scan, one mid-sized mod changed | 25.8 | 12 | 0.13 MB |
| Warm re-scan, 1 percent of mods changed (4 mods) | 26.2 | 94 | 0.98 MB |

The re-scan cost is almost entirely the metadata walk (about 21 ms); the re-parse of 1 percent is 3 ms. So the cache turns a 130 ms build into about 25 ms, and the floor is the stat walk, not parsing.

Cache formats. The same manifest serialized in each format, in-process encode/decode medians of 5, plus load in a fresh process (file read included) with resident memory after the load. Source: `results/s4.txt`.

| Format | File size | Encode ms | Decode ms | Load incl. file read ms | RSS held after load | Peak RSS |
|---|---|---|---|---|---|---|
| JSON rows, serde_json | 20.8 MB | 27 | 67 | 69 | 41.5 MB | 106 MB |
| JSON rows, sonic-rs | 20.8 MB | 24 | 42 | 48 | 41.5 MB | 106 MB |
| JSON rows, simd-json | 20.8 MB | 31 | 54 | 59 | 31.5 MB | 197 MB |
| JSON compact, serde_json | 7.4 MB | 38 (+26 build) | 21 | 24 | 10.9 MB | 34 MB |
| JSON compact, sonic-rs | 7.4 MB | 37 (+26 build) | 21 | 23 | 10.9 MB | 35 MB |
| bincode 2 (rows) | 8.5 MB | 10 | 21 | 20 | 31.5 MB | 74 MB |
| rkyv, full deserialize | 13.7 MB | 10 | 19 | 21 | 31.5 MB | 94 MB |
| rkyv, validated zero-copy access | 13.7 MB | 9 | 6.8 | 9.4 | 13.4 MB | 44 MB |

"JSON compact" is a JSON layout with one deduplicated string table and integer tuples instead of objects (see the recommended design). Its table has 89,981 unique strings for 134,324 defs (tags, classes, parents and paths repeat heavily). Expanding it back to the row structure costs 12.7 ms and round-trips exactly.

Warm start end to end with the plain JSON rows manifest and serde_json (read file, decode, re-scan with nothing changed): 100 ms.

Findings:
- JSON is fast enough: the plain row layout loads in 48 to 69 ms; a compact JSON layout loads in 23 to 24 ms, which is on par with bincode (20 ms) and rkyv full deserialization (21 ms). The layout matters far more than the parser: compact versus row JSON is a 3x gain in load time, 2.8x in size and 3.8x in resident memory, while sonic-rs versus serde_json is 1.4x.
- The binary baseline buys little. Only rkyv zero-copy access is clearly faster (9 ms against 23 ms), and it requires validated archived types plus lifetimes in the data layer; it saves about 14 ms once per start. That does not justify breaking requirement R10 (all app-owned data is JSON) and losing diffability and debuggability of the caches.
- simd-json has the highest peak RSS (197 MB, it needs a mutable copy of the input) and no speed advantage over sonic-rs here; avoid it.
- sonic-rs needs a CPU with SIMD (AVX2/PCLMUL on x86-64) for its fast paths. It is a performance option, not a requirement: serde_json with the compact layout already meets the budget.

## S5 Memory and string interning

Peak resident memory of the complete def index (134,324 defs from 23,717 files), measured in a separate process per run, 3 runs, median. "held" is the growth of resident set after the build with the index kept alive; "peak" is the process high-water mark minus the baseline before the build. Source: `results/s5.txt`.

| Mode | Threads | Held MB | Peak MB | Build ms | Notes |
|---|---|---|---|---|---|
| discard (parse, keep nothing) | 16 | 19.3 | 20.6 | 64 | the cost of buffers, thread stacks and allocator arenas alone |
| owned strings (5 `String` fields per def) | 16 | 55.4 | 57.9 | 87 | 36 MB above the discard floor |
| interned (lasso, 4-byte keys) | 16 | 32.0 | 32.0 | 67 | 12.7 MB above the floor |
| interned | 1 | 14.3 | 17.2 | 439 | no allocator arena overhead |

Findings:
- Interning cuts the index payload from about 36 MB to about 13 MB (2.8x). Owned strings carry 24 bytes of header per field plus allocator rounding for strings that are mostly repeats (def type tags, `Class` names, parent names).
- A large part of the apparent memory of a parallel scan is not data: 16 threads with glibc malloc leave about 18 MB more resident than a single-threaded build of the same index (32.0 versus 14.3 MB held). Allocator arenas and per-thread buffers matter as much as interning. Pick one allocator behaviour deliberately (for example a bounded arena count, or an allocator such as mimalloc, unverified here) and re-measure.
- The whole Def index for a 690-mod library is small in absolute terms (32 to 58 MB). Memory is not a constraint; interning is still worth it because the same table doubles as the compact JSON string table (S4) and makes name comparison an integer comparison.
- Parallel interning did not slow the build (67 versus 64 ms); the sharded `ThreadedRodeo` scales.

## S6 Thread scaling, drives, cold cache

End to end full def index build (pruned walk, read, parse with the scan parser, owned records), medians of 5, in ms. "Cold-approx" drops the data pages of every def file before each run. "Walk-only" is the pruned scan without parsing. Source: `results/s6.txt`.

| Dataset | Cache | 1 thread | 2 | 4 | 8 | 16 |
|---|---|---|---|---|---|---|
| workshop + Data, 23.7k files, 221.5 MB, SSD | warm | 558 | 333 | 172 | 139 | 122 |
| workshop + Data, SSD | cold-approx | 2,256 | 1,464 | 624 | 384 | 253 |
| workshop + Data, SSD | walk-only (warm) | 119 | 64 | 37 | 22.6 | 20.8 |
| owner mods, HDD, 38 def files, 0.2 MB | warm | 1 | 1 | 0 | 0 | 1 |
| owner mods, HDD | cold-approx | 10 | 7 | 6 | 6 | 6 |
| CE source, HDD, 817 def files, 5.4 MB | warm | 23 | 22 | 22 | 23 | 22 |
| CE source, HDD | cold-approx | 1,032 | 1,101 | 1,071 | 1,011 | 1,064 |
| CE source, HDD | walk-only (warm) | 9.1 | 9.5 | 9.3 | 8.7 | 11.3 |

Findings:
- Scaling on the SSD is good up to 4 threads (3.2x warm, 3.6x cold), then flattens: 8 physical cores plus the background load of about 6 busy cores on this machine. Cold scaling is better than warm (8.9x at 16 threads) because parallelism hides read latency on the SSD. Use `min(available cores, 8)` workers for the scan; more threads add nothing measurable, and fewer than the core count leaves room for the UI.
- Internal versus external: warm cache hides the drive completely (CE at 22 ms regardless of thread count, from 817 files), as expected. Cold, the HDD is bound by seeks: 817 CE def files (5.4 MB) take about 1.0 s, 1.3 ms per file, and extra threads do not help at all (1.03 s at 1 thread, 1.06 s at 16). The SSD reads 23.7k files in 0.25 s cold at 16 threads (about 10 us per file).
- The owner's own mods folder holds only 38 def files (most of those mods contain patches, assemblies and textures), so the HDD cost for that dataset is small: 6 to 10 ms cold.
- The cold numbers are a lower bound for a truly cold machine (metadata stayed cached; see method). Extrapolating the HDD per-file seek cost to a user with a large custom-mod folder on a spinning disk is only indicative: roughly 1 ms per uncached def file.

## Pitfalls found

Checks on the real data (`results/pitfalls.txt`) and on a synthetic fixture (a symlink loop, a symlinked directory, a symlinked file, a file with an invalid UTF-8 name, a mode 000 directory):

| Pitfall | Evidence | Mitigation |
|---|---|---|
| Huge XML files | 25 workshop XML files over 1 MB, largest 3.4 MB (def files); DOM parse of the 2.5 MB file takes 22.6 ms | stream or scan for indexing; DOM only on demand and off the UI thread; size cap or warning for files over 10 MB |
| Slowest 1 percent of files carry 34 to 42 percent of parse time | S3 stage D | schedule large files first, per-file work stealing (rayon does this) |
| UTF-8 BOM | 16,577 of 46,705 workshop XML files (35 percent), 861 of 1,672 in `Data`, 5,443 of 9,867 in the CE tree | strip BOM in the one decode function; also handle UTF-16 BOM and invalid UTF-8 (lossy decode) |
| Deep trees and long paths | max depth 12 in the workshop, longest path 233 chars (260 in the CE tree, exactly the old Windows MAX_PATH) | use iterative or rayon recursion without stack growth; on Windows use extended-length path prefix handling |
| Duplicate paths | the superset folder rule reached 19 files twice (LoadFolders entries that overlap) before de-duplication | de-duplicate by path before indexing; the game itself lets the first folder win per relative path |
| Permission errors | fixture: std, walkdir, ignore and the rayon recursion count the locked directory as 1 error; jwalk silently reports 0 errors | count and surface errors per mod; do not use jwalk if error reporting matters |
| Symlinks | fixture: symlinked directories are not followed by any walker used here (recorded as non-regular entries); a link to a parent causes no loop | never follow symlinks by default; Windows junctions need a separate check (not tested here) |
| Non-UTF-8 file names | fixture: all walkers handle the name; the library itself has none (0 of 306k) | keep paths as `OsString`/`PathBuf` internally, convert lossily only for display and JSON (store as UTF-8 with a lossy flag) |
| Case variants | no `defs` or `about` folders in non-canonical case in the data (counter empty in `pitfalls.txt`) | the game is case-sensitive on Linux for some folders (verify per `def-engine-semantics.md`); match the canonical case first, then a case-insensitive fallback with a warning |
| Malformed About.xml | none in 691 real files, but stray ampersands, mismatched tags and undefined entities defeat strict parsers (S2 matrix) | lenient event parser with partial recovery and a per-mod "warnings" field |
| Cold HDD seek cost | CE: 1.3 ms per uncached def file regardless of threads | show progress immediately, scan About files first, defer Def indexing and use the cached manifest while the refresh runs |
| mmap for bulk reads | 2.8x slower than read into a reused buffer | plain reads |
| Allocator arenas | 16-thread scan holds 18 MB more than a 1-thread one | measure with the production allocator |

## RimStudio budgets

Derived from the tables above, for a library of the size measured (about 700 mods, 306k files, 134k defs) on an SSD and a 16-thread CPU under moderate background load. Budgets are given with headroom of about 2x over the measured value; a spinning-disk, 4-core laptop should be assumed to be 3x to 10x worse and the UI must still be responsive.

| Scenario | Measured | Budget (SSD, 8+ threads) | Source |
|---|---|---|---|
| Cold start to a usable mod list (list mod roots, read and parse every About.xml, nothing cached) | 3.7 ms cold-approx at 16 threads, 24.5 ms at 1 thread; 2.0 ms warm | 100 ms to show the list; first paint must not wait for anything else | S2 start-up path |
| Warm start with persisted manifest, mod list and index available | JSON manifest load 23 to 69 ms (compact versus row layout) plus 23 ms re-scan = 46 to 100 ms | 150 ms to a fully usable, verified index | S4 |
| Refresh after one mod changes (watcher event or manual refresh) | re-scan 26 ms; re-parse of the changed files under 3 ms | 50 ms including updating the in-memory index | S4 |
| Full Def index build, no cache, warm file cache | 122 ms (scan parser, 16 threads); 125 ms quick-xml; 346 ms roxmltree | 300 ms | S3, S6 |
| Full Def index build, no cache, cold SSD | 253 ms (scan, 16 threads) | 600 ms, progress bar after 200 ms | S6 |
| Full Def index build on cold HDD | not measured at library scale; 1.3 ms per uncached file | indexed in the background, never blocks the mod list | S6 |
| Full directory walk (only for an optional "all files" feature such as size accounting or asset browser) | 94 ms for 306k files with stats | 250 ms, run lazily | S1 |
| Memory of the complete Def index | 32 MB interned (58 MB with owned strings) | under 100 MB for 700 mods | S5 |
| Manifest on disk | 7.4 MB compact JSON, 20.8 MB row JSON | under 15 MB, rewritten at most once per second | S4 |

Reading the budget: the mod list is the user-visible start-up path, and it costs about 2 ms of CPU. Everything slow (Defs, patches) is background work that fits inside a second even without a cache.

## Recommended cache design

What to cache: only what is expensive to recompute and cheap to store, namely the extracted per-file records of the def index (def type, `defName`, `Name`, `ParentName`, abstract flag, byte offset) and the parsed About data per mod. Not cached: file contents, raw XML trees, and the list of assets.

Key: (path relative to the mod folder, size in bytes, mtime in nanoseconds), per file, as measured in S4. The mod folder itself is keyed by its absolute path. Optional cheap guards per mod: file count and sum of sizes of the target folders, to catch deletions. mtime granularity differs between filesystems (btrfs and NTFS have sub-second values, FAT 2 s): store the raw value and compare for equality only, never "newer than".

JSON layout (compact, validated in S4: 7.4 MB, 21 ms decode): one document with a `version` number and a string table, with all repeated strings replaced by indices, mods, files and defs stored as arrays of integer tuples. Example shape, abbreviated:

```json
{ "v": 1,
  "strings": ["Ludeon.RimWorld", "ThingDef", "Apparel_Parka"],
  "mods":  [[0, 0, 12]],
  "files": [[1, 0, 4096, 1735689600000000000, 3]],
  "defs":  [[1, 2, 2, 2, 2, 0, 128]] }
```

Rules: tuple field order is documented by the schema; the file starts with a short human-readable header line (as JSONC comments are not valid in plain JSON, keep metadata in fields); a per-mod split is better than one monolithic file once edits must be written incrementally, see below.

Invalidation:
1. The manifest `v` or the app's parser version changes: discard and rebuild.
2. The game version changes (the `Version.txt` value) or the active load-folder resolution rules change: rebuild the affected mods.
3. A file key differs: re-parse that file only; a vanished file is dropped; a new file is added.
4. Corrupt or truncated file (JSON parse error): treat as absent, rebuild, never abort.
5. Write atomically (temp file then rename).

When to store per mod: one manifest file per mod keyed by mod folder (about 700 small files) lets a refresh rewrite one file, at the price of 700 file opens at start (not measured; at the S1 rate of 3 M stat per second this is likely a few ms). A single compact manifest of 7.4 MB written after a change takes about 40 ms (encode plus build) and is acceptable if writes are debounced. Recommendation: a single manifest at first (simplest), measure the write cost in the real app and split only if needed.

Is a binary cache justified? No. The best binary result (rkyv zero-copy) saves about 14 ms of an already sub-100 ms warm start, and bincode and full rkyv are no faster than compact JSON (20 versus 21 to 24 ms). Requirement R10 is satisfied at no measurable cost, provided the compact layout is used. Revisit only if profiling a library 10x larger shows a manifest load above 250 ms.

## Reproduction

All commands run from any directory. The source is in `docs/research/data/perf-spike/` (Cargo project, no target directory committed).

1. Build and run everything (takes about 10 minutes on this machine):
   `docs/research/data/perf-spike/run_all.sh /tmp/perf-results` (uses `CARGO_TARGET_DIR=/tmp/perf-spike-target`, delete it afterwards).
2. Individual stages after `cargo build --release --manifest-path docs/research/data/perf-spike/Cargo.toml`:
   - S1: `spike s1 --roots workshop,data,localmods,owner,ce --threads 1,4,16 --runs 5`
   - S2: `spike s2 --runs 5` and `spike s2-robust`
   - S3: `spike s3 --runs 5 --threads 1,16` (add `--cold` for approximate cold)
   - S4: `spike s4 --dir /tmp/spike-cache --runs 5`
   - S5: `spike s5 --runs 3`
   - S6: `spike s6 --runs 5` (and `--no-cold`, `--sets owner,ce`, `--threads 1,2,4,8,16`) and `spike s6-about --threads 16 --runs 7`
   - pitfalls: `python3 docs/research/data/perf-spike/pitfalls.py <root>...`
3. Dataset roots default to the paths of the owner's machine; override with `SPIKE_ROOT_WORKSHOP`, `SPIKE_ROOT_DATA`, `SPIKE_ROOT_LOCALMODS`, `SPIKE_ROOT_OWNER`, `SPIKE_ROOT_CE`. `SPIKE_GATE_MS` sets how long a run may wait for a quiet machine (default 500).
4. Raw outputs of the run reported here are in `docs/research/data/perf-spike/results/`; the `.jsonl` timing files (about 1 MB) were not committed.
5. Environment: `lscpu`, `lsblk -d -o NAME,ROTA,MODEL`, `df -T`, `uptime` (saved in `results/env.txt`).

Caveats: the machine was busy (load 6 to 10), so absolute numbers are likely 10 to 30 percent pessimistic and run-to-run noise is visible (for example the one-mod-changed row is slower than the unchanged row by 2.6 ms, which is noise plus 12 re-parsed files). Windows and macOS were not measured: NTFS directory enumeration with `FindFirstFileEx` returns size and times in the directory read itself (so the stat is free), but Defender scanning can dominate; treat those as unverified.

## Implications for RimStudio

1. The mod list must appear after reading only directory listings and `About.xml` files, without waiting for Defs, patches or assemblies. Test: with an empty cache on the 690-mod library, the mod list data is ready in under 100 ms on an SSD (measured 3.7 ms).
2. Walk with a purpose-built rayon recursion using `read_dir` plus `DirEntry::file_type()`, with one stat only where size or mtime is needed; do not adopt walkdir or jwalk for hot paths (walkdir 8.7x slower, jwalk default no faster than std). Test: pruned scan of 697 mods in under 50 ms warm.
3. Scan only the folders that matter (`About`, `LoadFolders.xml`, `Defs`, `Patches`, `Languages`, `Assemblies`, and the version/Common folders named by the game rules), never all 306k files by default.
4. The XML boundary crate exposes a lenient event-based About parser (partial results plus warnings, BOM and invalid UTF-8 tolerant) and a streaming def indexer; DOM parsing (roxmltree) is only for opening a single def or file. serde derive must not be used for About.xml.
5. The full Def index must build in under 300 ms warm (measured 122 ms with the structural scanner, 125 ms with quick-xml streaming); choose quick-xml streaming as the default and treat the memchr scanner as an optional optimization that needs the same conformance tests (identical output on 23,717 real files).
6. Persist a manifest keyed by (relative path, size, mtime ns) per file and re-parse only changed files. Test: warm re-scan with nothing changed under 50 ms and a one-mod change under 50 ms for the measured library.
7. The cache is JSON (R10) in the compact string-table layout; target load under 50 ms and size under 15 MB for 134k defs. A binary cache is not adopted; revisit only if a 10x library shows a manifest load over 250 ms.
8. Intern strings (def types, classes, parent names, paths) in the in-memory index with u32 keys and reuse the same table when writing the cache; target under 100 MB for the full index of 700 mods (measured 32 MB).
9. Use at most 8 scan worker threads by default, configurable in settings, and keep one core free for the UI; do not use mmap for bulk reads.
10. Never block the UI on a cold spinning disk: show the cached list immediately, report progress, and refresh in the background (HDD cold def reading measured 1.3 ms per file with no benefit from threads).
11. Treat scan errors as data: per-mod counts of unreadable directories and malformed files, never abort a scan, never follow symlinks, keep paths as OS strings internally.
12. Automate this spike as a regression benchmark in the workspace (a bench crate that runs against a configurable library path) and fail CI on a more than 2x regression of the budgets table.

## Open questions

1. Windows and macOS: the same tables are needed for NTFS (with and without antivirus) and APFS. Is the rayon recursion still best there (unverified)?
2. Truly cold boot numbers: the fadvise method leaves metadata cached. A reboot-level measurement (or root `drop_caches`) is needed for a definitive cold-start budget, especially on the spinning disk.
3. Version-resolved index: this spike indexed a superset (all version folders). How much smaller and faster is the index when only the folders the game would load for 1.6 are read (the S3 driver has a `--scope resolved` option that was not run for this report)?
4. Patch files (`Patches/*.xml`) and `Languages` were not indexed here; their cost and cache layout still need measuring for the XML and patch tooling.
5. Which allocator to ship (system, mimalloc, jemalloc)? The 18 MB difference between 1-thread and 16-thread holds suggests it matters, but no allocator other than the system one was measured.
6. Directory watching (inotify, FSEvents, ReadDirectoryChangesW) versus polling with the stat walk: a 25 ms re-scan makes polling viable every few seconds, but the CPU and battery trade-off was not measured.
7. Is one manifest file or one file per mod better for write cost on a library with thousands of mods? Only the single-file format was written and read here.
8. The memchr scanner is not a validating parser; how often would it disagree with a strict parser on malformed user XML outside this library (it agreed on all 23,717 files here)?
