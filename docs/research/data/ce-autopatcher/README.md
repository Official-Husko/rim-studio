# ce-autopatcher data and scripts

Supports `docs/research/ce-autopatcher-formulas.md`. Everything runs from any directory; set `PYTHONDONTWRITEBYTECODE=1`.

| File | Purpose |
| --- | --- |
| `extract_presets.py CE_DIR OUT` | parses CE's gun and apparel auto-patcher preset defs into `presets_gun.json`, `presets_apparel.json` |
| `ce_load.py` | loads vanilla and vanilla + CE with the def-engine prototype (`../def-engine`); re-implements CE's gun patch operation in simplified form (the engine leaves it unknown) |
| `build_pairs.py OUT` | writes the paired dataset `pairs_guns.json`, `pairs_apparel.json`, `pairs_melee.json` |
| `autopatch.py` | reference implementation of the auto-patcher formulas (guns, apparel, tools, toughness, race armor) |
| `tests/vectors.json`, `tests/test_autopatch.py` | independent hand-derived test vectors (no CE data needed); run `python3 -m unittest discover -s tests` |
| `fidelity.py DATA_DIR` | formula vs hand-tuned error statistics and data-driven baselines: `fidelity_summary.json`, `fidelity_*.csv` |
| `class_analysis.py DATA_DIR` | archetype and single-reference experiments: `class_analysis.json` |
| `make_figures.py DATA_DIR` | `fig1` to `fig4` PNG files |

Environment for `build_pairs.py`: `CE_DIR` (Combat Extended folder), `TYPES_JSON` (def type table that includes the CE classes, built with
`../def-engine/build_type_table.py` from Assembly-CSharp.dll and CombatExtended.dll), optional `RIMWORLD_DIR`, `ENGINE_DIR`.

Run order: `extract_presets.py`, `build_pairs.py`, `fidelity.py`, `class_analysis.py`, `make_figures.py`.

## Licence note

Combat Extended is CC BY-NC-SA 4.0 (Combat Extended team). `presets_*.json`, `pairs_*.json` and the `fidelity_*.csv` files contain numeric
values read from a CE checkout and are kept here only as research evidence so the analysis is reproducible. They must not be copied
into RimStudio's source or shipped. The product derives its own tables from the user's installed CE at run time. The scripts contain no CE
code or data.
