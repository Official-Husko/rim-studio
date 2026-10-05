# ADR 0049: Custom Combat Extended ammunition

Status: accepted | Last updated: 2026-10-06 | Register: D-152, D-153, D-085

## Context

A weapon converted to Combat Extended fires an ammo set. The owner asked for two things: that the designer shows a list of all available ammo, and that a user who wants a caliber of their own gets a window with all the data of such a projectile. The install is large (307 ammo sets, 896 ammo items, 963 projectiles and 890 recipes on the owner's install), the numbers of a projectile are not derived from a weapon, and Combat Extended is licensed so that none of its data may be shipped (R11).

## Decision

- The data is read at run time. `read_conversions_with` fills `CeModel.ammo` with the ammo classes, ammo items, projectiles, recipes and categories of the load. The catalogue and the suggestions are pure functions of that model.
- The catalogue is a query: `designer_ce_ammo_catalog` returns pages of sets with their types, the weapons that use them and a suggested flag taken from the relevance ranking that `designer_ce_suggest` already computes for the design. Facets let the page filter by caliber, family and ammo class.
- A custom caliber is an optional field of the Combat Extended block, `customAmmo`. It holds every field the user's projectiles carry in a typed place, and raw nodes for the rest. Every number is a sourced value, so a typed value is never overwritten.
- Defaults come from the nearest of the user's own ammunition: the same ammo class, ranked by an energy index (damage times speed), the median of the nearest five with a rating by how closely they agree. A copy of an existing type takes every value.
- The files go to `Defs/Ammo/<prefix>_<Name>.xml` in the gated Combat Extended folder, one section per def, only when the switch is on. The weapon then uses the custom set and the projectile of the default type.
- Checks measure instead of limiting: a number far from the user's own ammunition of the class is a warning with the range found. Existence checks (derived names, ingredients, classes) are errors.
- Lists that merge with the parent's are written as replacements, so a copied type never writes an entry twice.

## Consequences

- The plan can carry a second kind of Combat Extended file (`ce-defs`); the gate test accepts it only under the gated folder.
- The rebuild of every real set of the owner's install, loaded as one more mod after the real defs, reproduces the real definitions except for values that the siblings of a def share and that look inherited (12 missing elements in 1,148 types).
- Art is not imported: an ammo item without a texture gets a reserved path and an info that names the file.
- The type table must know the ammo item and ammo class types. The test table builder adds them; a table read from the game assemblies has them.

## Alternatives rejected

- A table of ammunition in the repository: a licence problem and it would age.
- Scaling from the weapon's vanilla damage: the numbers of a Combat Extended projectile depend on the cartridge, not on the gun.
- A separate project file for custom ammunition: it would need its own merge; the block of the draft already is the place for optional Combat Extended choices (ADR 0048).
- Clamping implausible numbers: the user decides what their cartridge does.
