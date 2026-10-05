# Combat Extended bows, 0.1.0

Evidence note. It records how Combat Extended converts bows and crossbows on the owner's install and how the generator of the task `ce-bows` compares with those conversions. Only aggregates are recorded: no element value, def name or number of the install is copied (R11). The structure fidelity note of the earlier task, [CE structure fidelity](ce-structure-fidelity-0.1.0.md), set bows aside as an unsupported kind; this note follows that up.

## 1. Question

Combat Extended converts a bow in a style of its own. Can the generator write the same structure for a bow, for a vanilla bow and for a newly designed one, and what is left that differs?

## 2. The style, as observed

The install holds 4 converted bows in the library of the reader (three vanilla bows and a crossbow like device of the Anomaly pack; a fourth vanilla variant that inherits a bow's conversion is excluded from the library as a unique duplicate). The three vanilla bows are converted by the same gun conversion operation as a gun, with these differences:

| Part | Gun | Bow |
| --- | --- | --- |
| Ammo | a caliber set and a real magazine size and reload time | an arrow set; no magazine size and no reload time; an ammo spawn count (`AmmoGenPerMagOverride`) in all 3 |
| Fire modes | aim mode, burst flag, aimed burst count | an empty element in all 3 |
| Verb properties | recoil, burst, ticks between shots | none of them |
| Tags | a class tag and companions | one bow tag in all 3 |
| Mass | written when the conversion has a number | not written in any of the 3 (the vanilla mass stays) |
| Run and Gun | allowed | the operation sets `AllowWithRunAndGun` to false in all 3 |
| Tools | the gun bash, converted tool by tool | one blunt tool, the same tool in all 3, with no label |

The crossbow like device keeps a magazine of one, a reload time and an aim mode (one fire modes entry) beside the bow tag and a spawn count: a hand converted gun with a bow tag. Its vanilla form carries no bow tag, class or arrow projectile, so it is not a bow shape and the generator converts it as a gun.

Combat Extended has no draw time and no draw strength stat. The Combat Extended source (the read only reference checkout) was searched for any such field and none exists; the pull of a bow is carried by the range, the warmup and the projectile of the ammo set.

Across the install's other mod patches the same style recurs (the earlier survey counts 131 uses of the spawn count in the compatibility patches); that survey is the source of the style, this note only checks the three vanilla bows against the generator.

## 3. Method

The harness is the one of the earlier note (`crates/rimstudio-design/tests/real_ce_fidelity.rs`, ignored, writes nothing), extended by a bow section. A bow that the scan lists as convertible and that has a vanilla twin and no converted parent is read as the block of its twin (typed values), converted by the generator, applied to the real vanilla defs by the def engine, applied twice for the idempotence check, and compared path by path with the real resolved def, with the vanilla def as the base. The variant that inherits a bow's conversion carries no patch of its own; it is compared as a resolved def. The convert flow is also run on the three bows with no answers, with the arrow set answered, and with every open number answered.

## 4. Results

Before the change the scan listed 4 bow defs as unsupported (the 3 and the variant) and the harness set them aside: 36 weapons compared (17 guns, 19 melee weapons), 185 generated operations. After: 39 weapons compared (20 guns of which 3 are bows, 19 melee weapons), 191 generated operations, all applied, none failed, and the class names used are all classes of Combat Extended's own data.

| Set | Missing | Not removed | Value | Extra | Removed extra |
| --- | --- | --- | --- | --- | --- |
| All weapons before | 139 | 45 | 320 | 47 | 6 |
| All weapons after | 142 | 51 | 335 | 47 | 6 |
| The 3 bows alone | 4 | 6 | 15 | 0 | 0 |
| The inheriting variant | 5 | 2 | 5 | 0 | 0 |

Unexplained missing or not removed elements: 0 for all weapons, 0 for the bows and 0 for the variant. Applying the patch twice changed 0 of the 3 bows (0 of 34 generated weapons in all, as before the change).

What the bow differences are, by class:

- **Left to the vanilla design (D-085):** the graphic draw size (2 of 3 bows) and the draw offset of the gun draw extension (2 of 3), which Combat Extended's own conversion adds and the designer keeps in vanilla terms.
- **Tool list restructured by Combat Extended:** the bow's tool keeps its second capacity and its label in the generated result where the real conversion removes both (3 of 3, each for the capacity and the label), and the tool's power and cooldown differ (values). The same class as the gun bash and melee tool restructuring of the earlier note.
- **Economy and values:** the cost of the bow (3 of 3), the range, the warmup, and the tool power and cooldown differ in value (3 of 3 each). The range and the warmup are not predicted, because the library has too few bows to rate an estimate (see below), so the vanilla value is carried over with a derived value record.
- **Variant that resets its components:** the variant lists its own components with `Inherit="False"`, so it does not receive the ammo component and the fire modes that its parent's conversion adds; Combat Extended converts such variants in a separate file. This accounts for 3 of the 5 missing elements of the variant, the other 2 are the art elements above.

The difference between the two all weapons rows is the bow findings of these classes (4 missing, 6 not removed, 15 value) less one missing weapon tag: the crossbow like device now gets its bow tag, because the tag of the block follows the bow tag of a bow shaped conversion.

Lint of the generated bow patches: no finding for the 3 bows. The one finding of the run is the bow rule hint for the gun converted crossbow like device (its operation does not set `AllowWithRunAndGun` to false), which is correct.

The convert flow on the 3 vanilla bows asks the arrow set, bulk, sway, spread and the penetration of the tool (5 questions each). Once the set is answered it asks 4, because the library holds 4 converted bows and a rating needs 5 conversions: every bow number is `Unmeasured` and is asked, with the reason. The default projectile is the first member of the answered set, and the bow tag and spawn count are derived from the bows (3 of 3 agree). With every number answered, the plan has the patch file and `LoadFolders.xml` and no error.

## 5. Limits

- 4 converted bows are too few to estimate a bow number; on an install with more bows (a library of ten or more) ratings can reach reliable or rough and the numbers are then written.
- Bows still count as examples of the gun estimates (D-097 recorded that they lower the gun figures); separating the pools would change the measured gun figures of [the evaluation note](ce-conversion-eval-0.1.0.md) and is left for a later round.
- A hand converted crossbow with a magazine is converted as a gun when its vanilla form has no bow shape.
- Nothing was loaded in RimWorld itself: the check is the def engine, which applies the same documented merge as the game.

## Implications for RimStudio

- Bows and crossbows are converted by the generator in the bow style (D-140, which supersedes D-097), from the convert flow, from a designed weapon and in update mode; the scan lists them as convertible.
- With the library of this install every bow number is asked, so a bow conversion is a short question list (arrow set, three numbers, the tool penetration) and not a silent estimate. This is the rating gate working as designed (D-095), not a gap in the generator.
- The classified remainder is the same as for guns: art and economy stay in the vanilla design (D-085), and the tool list is restructured by Combat Extended.

## Open questions

- Should the gun estimator pool exclude bows once the bow pool is measured on its own? It would change the gun figures of [the evaluation note](ce-conversion-eval-0.1.0.md), so it needs a new measurement.
- Should the bow tool (one blunt tool in every converted bow) be learned from the converted bows as a habit? With 4 bows the habit would be rated unmeasured like the numbers, so it stays a question for now.
- A held out check against hand written bow conversions of other mods was not made in this round; the numbers above compare with Combat Extended's own conversions only.
