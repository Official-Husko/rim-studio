# Combat Extended weapon platforms and under barrel units, 0.1.0

Evidence note. It records how Combat Extended expresses weapon platforms, attachments and under barrel units, what the owner's install contains of them, and the round trip of the generator's conversion against the one weapon family the install converts this way. Only aggregates are recorded: element paths and counts, no value of the install (R11). The specification is in [Combat Extended patching](../features/combat-extended-patching.md) section 14.14; the decision is D-142.

## 1. How Combat Extended expresses them

Two separate features share the word "platform" in the community's use. They are modelled apart.

**Weapon platform.** The conversion operation takes three optional parameters: a platform flag, a list of attachment links and a list of default graphic parts. Read from the operation's documented merge (the reference source, read only; described here in our own words):

- The def becomes a platform when the flag is set or when links or parts are given. It takes the platform def type (a `Class` attribute on the `ThingDef`), a platform `thingClass` unless it already names a Combat Extended class, and the real time drawer.
- An attachment link names one attachment def and may carry a draw scale, a draw offset and three lists of stat entries (offsets, multipliers, replacers). Stat entries are written as `<Stat>number</Stat>` children.
- A default graphic part is drawn while its slots are free: an optional picture, an optional outline picture and a list of slot tags.
- Each part of the operation is applied only when given, so an operation that carries only platform parameters converts the def into a platform and nothing else. The links and parts are appended: applying the operation twice appends them twice.
- The attachment defs themselves are a separate def type. They are not part of the weapon conversion and are not written by the generator.

**Under barrel unit.** A component of its own type on the weapon: a second gun the wielder switches to. It carries two optional labels, two flags (one ammo holder, requires reload), a block of ammo user fields (magazine size, reload time, ammo set), a verb block (the fields of a converted shooting verb, plus an avoid friendly fire radius and a count of rounds one shot uses) and a block of fire modes (burst flag, aim mode, aimed burst size, no single shot). A component with none of these is also used: the unique weapons that carry weapon traits get the bare component as the slot for a trait's unit.

## 2. What the owner's install contains

Counts over the installed Combat Extended and its bundled third party patches (the `ModPatches` folder, which only loads for the matching mods), the official packs and the base game:

| Thing | Count |
|---|---|
| Defs of the attachment type, and conversions that set the platform flag, links or parts | 0 |
| Weapons with an under barrel unit that has data, in the load the harness uses | 3 (two in the Anomaly pack patch, one unique variant in the Odyssey pack patch) |
| Unique weapon variants of the Odyssey pack with the bare component | 10 |
| Weapon trait defs that carry an under barrel block | 3 (one defined by Combat Extended, two added to vanilla traits) |
| Third party patch files with the component | 29 (88 components: 58 with data, 30 bare) |

The third party patches are evidence of the shapes in use, not of this install's load: their mods are not active. Across the 58 components with data: the ammo block names an ammo set in 54 (the 4 without one share the main gun's holder through `oneAmmoHolder`, which 32 components write), a magazine size in 57, a reload time in 57; the verb block always has a verb class, a default projectile, warmup, range and shot sound, and often a tail sound (46), a burst size (23), a count of rounds per shot (22), target rules (20), a minimum range (7), an avoid friendly fire radius (7); the fire modes block names an aim mode in 57, an aimed burst size in 23, the burst flag in 19 and no single shot in 10; labels are written in about two thirds. One component has an ammo block child the model does not type (a one at a time reload). Children the model does not type are carried as written.

The install has no weapon platform at all. The platform half of the work is therefore specified from the documented merge and tested with fictional defs and golden node trees only; the install cannot confirm it. This is the first limit of the evidence.

The under barrel family has one weapon with a twin that the harness can compare: an assault rifle variant whose vanilla definition carries an equippable ability component (a burner unit with charges). A second weapon of the family has a verb that names no projectile (a spray) and is converted by hand with explicit operations; it is a beam style weapon, listed by the scan as unsupported with that reason.

How the real conversion handles the vanilla unit: the vanilla definition lists its own `comps` (inheritance off), with the ability component standing in for the plain equippable component. The conversion converts the gun with the usual operation, then replaces the ability component with the under barrel component and a plain equippable component. The unit's numbers differ from the vanilla ability's (the range, the warmup and the burst size are redesigned), so nothing about the unit can be derived from the vanilla definition.

## 3. Round trip

Harness: `crates/rimstudio-design/tests/real_ce_fidelity.rs`, the platform section. Method as in [the fidelity note](ce-structure-fidelity-0.1.0.md) section 2, with the Combat Extended block read from the real conversion including the unit.

Findings on the weapon family (the one weapon with an under barrel unit and a vanilla twin), by path group:

| Group | Before | After |
|---|---|---|
| Under barrel component fields (labels, ammo block, verb block, fire modes block) | 22 | 0 |
| Fields of the vanilla ability component that the real conversion removes | 7 | 0 |
| Component entries (the plain equippable component and the quality component by position) | 3 | 0 |
| All findings of that weapon, missing or not removed | 32 | 0 on the unit |

Over the whole run: missing elements 142 before and 118 after, not removed 51 before and 43 after, value differences 335 and extras 47 unchanged. Applying the generated patch twice differs from applying it once for none of the weapons. The generated patches use only classes that Combat Extended's own data uses and lint without a platform finding.

Classification changes of the scan in the same run: two weapons that previously failed to generate (a beam weapon and the spray weapon above, both read as melee because their verbs name no projectile) are now listed as an unsupported kind with the reason "no projectile". This is a scan fix, not a conversion.

## 4. Convert flow on the family

For the unconverted vanilla twin, the convert flow lists the weapon as convertible with a reason that names the unit, and asks for the unit's ammo set, magazine size, reload time and range on top of the gun's own questions. The reference shown with the range question is the range of the vanilla ability def. With fewer than two under barrel units in the library no median is offered; with two or more, the median of the library's units is shown as a reference and never written.

## 5. What is not supported or not verified

- Weapon platforms against a real conversion: none exists in the install.
- Attachment defs: not written. A link to an attachment def that the install and the project do not define is a lint warning (CEP045).
- Hand written conversions of the unit that bypass the conversion operation (explicit operations for the verb, as the beam style weapon has): the unit is converted only through the generator's own shape.
- The unit's numbers are never predicted. The two units of the library are of one kind; an estimator needs more examples than that, so there are no ratings for unit numbers.
- Nothing was loaded in RimWorld itself.

## Implications for RimStudio

- The unit is a guarded component operation, replacing the vanilla ability component and restoring the plain equippable component; the unit's numbers are asked, because the vanilla ability does not hold them and the library cannot rate an estimate from two units of one kind.
- A platform only operation is the update form for a def that already carries a conversion: the merge applies each parameter only when given.
- The scan must not read a weapon whose verb names no projectile as a melee weapon.
- Lint gains rules for the incomplete unit, an unresolved ammo set or projectile, a missing equippable component, and the empty or unknown attachment link.

## Open questions

- What a real platform conversion looks like in the community's patches: the install has none, so the order of the platform parameters, the typical stat entries of a link and the slot tag names are unobserved.
- Whether a hand written conversion of the unit (explicit operations for the verb, as one weapon of the install has) should be recognised by the reader as a unit; it is converted by hand today and listed as unsupported.
- Whether the unit needs ratings once the library holds more units of more than one kind.
