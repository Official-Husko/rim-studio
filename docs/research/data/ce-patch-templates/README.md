# CE patch templates: index

Annotated documentation of the XML that RimStudio must produce for Combat Extended (CE) compatibility. RimStudio itself stores these as JSON node trees (see `node-tree.example.json`) and renders XML only through the XML boundary crate. Written from verified patterns in `docs/research/ce-patch-conventions.md`; no file here is copied from CE or from a third-party mod. Placeholders look like `{{name}}`; annotations are `[REQ]`, `[OPT]` and `[SRC:...]`.

Status: research note | Last verified: 2026-10-04

| File | Item kind | One-line summary |
|---|---|---|
| `01-ranged-gun.xml` | Ranged gun | `PatchOperationMakeGunCECompatible` plus `ToolCE` list |
| `02-grenade-throwable.xml` | Grenade, throwable | One-use verb, explosive projectile fixes |
| `03-bow.xml` | Bow, crossbow | Arrow or bolt set, empty `FireModes` |
| `04-melee-weapon.xml` | Melee weapon | `ToolCE` tools, Bulk, melee offsets |
| `05-apparel.xml` | Apparel | Stuffable, fixed armour, `PartialArmorExt` |
| `06-ammo-and-projectile.xml` | Ammo and projectile | New calibre Defs file |
| `07-turret.xml` | Turret | Building class and stat changes |
| `08-mech-weapon.xml` | Mech weapon | Gun, race and pawn kind layers |
| `09-gating-and-folders.xml` | Gating | LoadFolders, FindMod, Conditional, ensure-container, Sequence (snippets, not one file) |
| `node-tree.example.json` | Storage form | Template 01 as a JSON node tree |

Tools in this folder (deterministic, paths from arguments or environment variables `RIMSTUDIO_CE_ROOT`, `RIMSTUDIO_OWNER_MODS`, `RIMSTUDIO_WORKSHOP`, `RIMSTUDIO_GAME_ROOT`; run with `PYTHONDONTWRITEBYTECODE=1`):

| File | Purpose |
|---|---|
| `survey_ce_patches.py` | Corpus survey (`survey`) and patch linter (`lint FILE_OR_DIR`); output `survey_summary.json` |
| `value_stats.py` | Numeric distributions of CE patch values; output `value_stats.json` |
