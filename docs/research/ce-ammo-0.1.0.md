# Combat Extended ammunition, 0.1.0

Evidence note. It records what the owner's Combat Extended holds as ammunition, how large the catalogue of ammo sets is, and how well a custom ammo spec rebuilt from the real values reproduces the real definitions (decisions D-152 and D-153). Only aggregates are recorded; no value of Combat Extended is copied (R11).

## 1. Question

Can the designer list every ammo set of the install and offer a window for a new caliber, with defaults taken from the user's own ammunition, and can the files it writes carry all the data that Combat Extended's own ammunition carries?

## 2. Method

The ignored test `crates/rimstudio-design/tests/real_ce_ammo.rs` loads the official packs and the Combat Extended workshop copy, reads the model, and then

1. asks the catalogue for all pages;
2. rebuilds every ammo set as a `CustomAmmoSpec` of typed values (`set_from_existing`), one type per pair of the set, with a copy of the real ammo item, projectile and recipe;
3. generates the definition files under another prefix and loads them as one more mod after the real defs, so the parents resolve as in the game;
4. compares each generated set, ammo item, projectile and recipe with the real one leaf by leaf on the resolved nodes (a path that repeats is compared as a sorted list). The def name, the cook off and detonation projectile and the product name are ignored because they carry the renamed defs;
5. runs the lint rules, the dry load and the spec checks over every generated file.

Run with `cargo test -p rimstudio-design --release --test real_ce_ammo -- --ignored --nocapture`.

## 3. The install

| Measure | Count |
| --- | --- |
| Ammo sets | 307 |
| Ammo types over all sets | 1,158 |
| Ammo classes (`AmmoCategoryDef`) | 95 defined, 73 in use |
| Calibers and families (facets) | 249 |
| Ammo items | 896 |
| Projectiles of the ammo sets | 963 |
| Recipes that make an ammo item | 890 |
| Thing categories of the ammo and their ancestors | 255 |
| Generic sets (other sets are similar to them) | 21 |
| Sets with `similarTo` | 199 |
| Sets used by a converted gun | 32 |
| Pairs of an ammo set | written as one element per pair |

Reading the model, including the library, takes about 66 ms on top of the load of the packs (about 20 s). The three most frequent children of a projectile's properties are the damage, the two penetrations; speed is on about half of the projectiles because the base of the cartridge family sets it.

## 4. Fidelity of the rebuild

The count is over 1,148 types of 298 sets (9 sets have no pair whose ammo item and projectile are loaded, because they belong to content that is not part of the load).

| Measure | First run | After the work |
| --- | --- | --- |
| Elements the real def has and the generated one lacks (missing) | 406 | 12 |
| Elements with another value (different) | 124 | 16 |
| Elements the generated def has and the real one lacks (extra) | 5 | 0 |
| Generated files that fail the dry load | not run | 0 of 307 |

The first run had the typed fields only (mass, bulk, graphic, damage, penetration, speed and the commonest projectile fields). What closed the rest:

| Gap of the first run | Closed by |
| --- | --- |
| Explosive and fragment components of ammo items and projectiles | the component list of a def is kept whole when it holds an entry that no base provides, and written as a replacement |
| Stats such as the market value, graphic colour and size, shader | typed `marketValue`, `drawSize`, `statBases` and the extra graphic children |
| Research prerequisites that are a list, and the empty single prerequisite that clears the parent's | `researchPrerequisites` and an empty `researchPrerequisite` |
| Ingredients with several defs or with categories, and a fixed filter that differs from the ingredients | `alternatives`, `categories` and the raw fixed filter |
| Set level children (a mortar flag, a generic label extension) | `setExtra` |
| Lists that merge with the parent's, written twice | a list of `li` entries and the mod extensions are written as replacements |
| A value that every sibling shares being taken for inherited, and the other way round | a def is measured against its siblings under the same parent when there are three or more, else against the whole kind |

## 5. What stays

All 12 missing elements and most of the 16 different ones have one cause: a value that every def under one parent carries unchanged (the same set maker tags on three shells, a screen shake factor on six napalm projectiles, a pre explosion spawn def on three) cannot be told from a value that the parent provides, because the abstract parents are not part of the resolved defs. The generated def inherits from the same parent, so the value is lost only when the parent does not provide it. The copy keeps it when a def differs from its siblings. The remaining differences are one projectile with an emission list of its own and the 9 sets without loadable pairs.

## 6. The spec checks on the real values

The checks run on the rebuilt specs give no error on a spec that the install itself contains, except for projectiles that have no damage or penetration of their own (11, flares, smoke and similar, where the damage def or the parent supplies it) and the 9 sets with no type. Plausibility warnings (a number outside half of the smallest and twice the largest value among at least 5 ammo types of the class) fire 7 times over 1,148 types, so the rule is quiet on real ammunition. The lint finds 6 recipes without an ingredient entry of the form it reads and the same 11 projectiles without damage.

## Implications for RimStudio

- Copy what differs from the siblings; do not try to tell inherited from own beyond that (D-152).
- Plausibility stays a warning (D-153).
- The catalogue is paged because 307 sets with 1,158 types are too many for one answer; the default page is 25.

## Open questions

- Art for custom ammunition is not imported: the plan reserves a texture path and says where the image goes. The asset import of the designer could take the ammo item and projectile art.
- The abstract parents of ammunition are not part of the resolved defs, so a copy cannot tell a value the parent provides from one every sibling sets. Reading the unresolved defs of the install would close the last twelve missing elements.
- A converted gun that fires an under barrel ammo set or a second set is counted only by its main set in the catalogue.
