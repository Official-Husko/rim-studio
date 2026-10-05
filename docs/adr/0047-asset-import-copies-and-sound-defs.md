# ADR 0047: Asset import, copy plans and custom sound definitions

Status: accepted | Last updated: 2026-10-05 | Register: D-130 to D-134

## Context

The designer wrote a weapon without art with a reserved `texPath` and a hint, and its sound fields only named sound definitions that already exist. A modder who has a PNG and a recorded shot still had to copy the files by hand into the right folders, write the `SoundDef` and keep the paths in step. The layout of ADR 0041 already reserves the paths (`Textures/Things/Item/Equipment/WeaponRanged`, `Sounds/Weapons/<DefName>_Shot`, `Defs/SoundDefs/World_Oneshots_Weapons.xml`). The owner asked for sound definitions and texture import.

## Decision

1. A design may import a weapon texture, an own projectile texture and a custom shot sound. The spec stores source paths and sound settings only; sizes and hashes are read at planning time and never stored in a draft (D-130).
2. A source is a regular file, never a link, read under a size limit (texture 8 MiB, clip 20 MiB) and hashed with SHA-256. The format is recognised by signature and the PNG, WAV and Ogg headers are read in house, without decoding and without an image or audio dependency. Dimensions are limited to 4096 pixels on a side (D-131).
3. The design crate stays pure: the toolkit reads the files and passes `AssetFacts`; `export_vanilla_plan_with` plans copy files (`FileKind::Copy`, actions `create`, `unchanged`, `replace`) and the `SoundDef` section. Apply copies before the definitions that name the files, with the guarantees of a text write: the guarded path, a hash checked source, a target that must still hold what the plan saw, a backup of a different existing file in the data root, an atomic write and a read back (D-132).
4. The `SoundDef` follows the vanilla form with `AudioGrain_Clip` entries and lives in the one sound file of the mod as a marked section per sound (D-133).
5. `designer_asset_info` returns the facts and a thumbnail for a small PNG, so the page needs no file access (D-134).

## Consequences

- `rimstudio-design` gains `assets` (header readers, facts), `model::assets`, `plan::assets`, `validation::assets` and 17 diagnostic codes in `validation::asset_codes`; `rimstudio-io` gains `copy` and `sha256`; `rimstudio-toolkit` gains `shared::assets`, `shared::writer::copy` and `designer::asset_info`.
- `rimstudio-ipc-types` gains optional fields (`assets`, `sounds`, `copy`, `kind`, `sha256`), the enum values `copy` and `replace` and the types of `designer_asset_info`; the generated TypeScript follows. One registry row.
- The rule of D-106 that the tool creates no image and the sentence of D-113 that art is never copied no longer hold for an explicit import. A texture a clone takes over from its source is still never copied.
- Combat Extended is unchanged: a patch never touches sounds or textures (D-085).
- Reload and impact sounds keep the reserved paths and are chosen by name.

## Alternatives rejected

- An image or audio crate: it adds a dependency tree and a decoder surface to read twenty bytes.
- Copying at plan time or putting the bytes in the plan: a plan writes nothing, and the IPC should not carry megabytes.
- `AudioGrain_Folder`: the plan would not name the files it copies; the single clip form lists them.
- One sound definition file per sound: nothing in the game's own layout does that.
