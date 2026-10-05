# def-engine: executable specification of RimWorld's Def loading pipeline

A Python prototype (lxml, standard library) of what RimWorld 1.6 does between "list of active mods" and "resolved
Def nodes": load-folder selection, merging every mod's `Defs/` into one document, applying `Patches/`, resolving XML
inheritance, honouring `MayRequire`, and building per-type def databases with provenance. The precise rules, the
evidence in the decompiled game code and the results on real data are in
`docs/research/def-engine-semantics.md`. This folder is the executable counterpart and becomes the source of the
regression test vectors of the Rust defs crate.

Scope limits: nodes stay XML trees (no C# field parsing, cross references, PostLoad), no Harmony or runtime patches,
custom PatchOperation classes are skipped unless registered (`plugins.py` has one example).

## Layout

| Path | Purpose |
| --- | --- |
| `defengine/xmlnet.py` | DOM layer reproducing the XmlDocument calls the game makes (AppendChild, InsertBefore, ...) on lxml |
| `defengine/xpath_net.py` | XPath 1.0 with the `XmlNode.SelectNodes` context rule (relative paths start at the document) |
| `defengine/mods.py` | About.xml, game version, LoadFolders.xml, version folders, file discovery and de-duplication |
| `defengine/patches.py` | All 13 vanilla PatchOperation classes plus the unknown-class behaviour |
| `defengine/inherit.py` | XmlInheritance: registration, parent choice by load order, resolution, the merge with its quirks |
| `defengine/mayrequire.py` | `MayRequire` / `MayRequireAnyOf` on `li` items inside defs |
| `defengine/engine.py` | `load_game(LoadConfig)` -> `LoadResult` (defs with provenance, databases, patch events, timings) |
| `defengine/plugins.py` | Example custom operation (Combat Extended's settings conditional) |
| `defengine/diag.py` | Structured diagnostics with stable codes (the game's Player.log lines) |
| `build_type_table.py` | Builds the Def class table from compiled assemblies with `monodis` |
| `data/def_types_vanilla.json` | Def classes of Assembly-CSharp 1.6.4871 (names and bases only) |
| `trace_def.py` | Prints a def's own XML, its parent chain and the resolved result |
| `validate_real.py` | Vanilla and vanilla + Combat Extended run, writes `golden/real_data_summary.json` |
| `make_golden.py` | Runs all vectors, writes `golden/vector_outputs.json` |
| `tests/vectors/*.json` | 38 language-neutral test vectors (mods as JSON, expectations as XML strings or counts) |
| `tests/handtrace_vanilla.json` | 13 vanilla defs traced by hand, compared with the engine |
| `probes/Probe1.cs` | .NET/Mono probe of XmlDocument behaviours the engine copies (run: `mcs Probe1.cs && mono Probe1.exe`) |

## Usage

```
cd docs/research/data/def-engine
export PYTHONDONTWRITEBYTECODE=1
G=/path/to/RimWorld                                  # folder with Version.txt and Data/

# summary of a vanilla load (Core + installed DLC), diagnostics, timings
python3 -m defengine load --game $G --types data/def_types_vanilla.json

# a resolved def with provenance (use --json for the node tree as JSON)
python3 -m defengine def --game $G --types data/def_types_vanilla.json --def ThingDef/Gun_Revolver

# every top-level patch operation with its result
python3 -m defengine patches --game $G --types data/def_types_vanilla.json

# add mods after the DLC in load order (repeat --mod); extra ids count as active
python3 -m defengine load --game $G --types data/def_types_vanilla.json --mod /path/to/MyMod
```

Library use:

```python
from defengine import LoadConfig, load_game
res = load_game(LoadConfig(mods=[core, royalty, my_mod], game_dir=game, types="data/def_types_vanilla.json"))
d = res.get("ThingDef", "Gun_Revolver")      # DefDatabase<ThingDef> lookup with the game's override rules
d.provenance()                                # {"mod": ..., "file": ..., "parents": [...], "patched_by": [...]}
d.to_json()                                   # type, defName, provenance, canonical node tree
res.diag.summary(); res.patch_events; res.timings
```

For Combat Extended the def type table needs the compiled mod assembly, because `CombatExtended.AmmoDef` and others
only exist there:

```
python3 build_type_table.py --dll $G/RimWorldLinux_Data/Managed/Assembly-CSharp.dll --dll .../CombatExtended.dll --out /tmp/types_ce.json
```

## Tests

```
python3 -m unittest discover -s tests            # 51 tests, about 30 s (most of it the Combat Extended run)
python3 tests/harness.py                         # vectors only, prints one line per vector
CE_DIR=/path/to/CombatExtended CE_DLL=/path/to/CombatExtended.dll RIMWORLD_DIR=$G python3 -m unittest discover -s tests
```

* `test_vectors.py`: the 38 vectors. Format documented at the top of `tests/harness.py`.
* `test_handtrace.py`: 13 vanilla defs with expectations derived by hand (needs `RIMWORLD_DIR`).
* `test_units.py`: XML layer, XPath context rule, LoadFolders parsing, MayRequire on li.
* `test_real_data.py`: recomputes `golden/real_data_summary.json` (counts and hashes only; the Combat Extended part
  needs `CE_DIR` and `CE_DLL` and is skipped otherwise).

## Golden files

* `golden/vector_outputs.json`: resolved defs with provenance for every synthetic vector (64 KB).
* `golden/real_data_summary.json`: counts, def-name hashes and patch statistics for the real runs. No game or
  Combat Extended content is stored (licence hygiene); machine-dependent timings are not stored either.

## Known divergences from the game

CDATA sections are read as plain text; characters the game's reader accepts (CheckCharacters is off) but lxml
rejects make a file fail; patch XPath is evaluated eagerly (the game iterates lazily and can see nodes that the same
patch just added); the order of files inside one folder is sorted here (filesystem order in the game). All are listed
with their consequences in the semantics note.
