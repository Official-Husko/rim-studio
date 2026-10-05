# Combat Extended combat and equipment model

Scope: what Combat Extended (CE) changes in RimWorld 1.6 combat, the meaning and units of each CE stat and def field for ranged weapons, melee weapons, ammunition and apparel, the formulas the CE code applies, and real value distributions measured on a resolved vanilla plus CE load. The aim is to let RimStudio's item designer (requirement R7) compute CE-compatible stats for a new weapon or apparel. CE is CC BY-NC-SA 4.0: RimStudio must not copy CE data files or code; it reads CE values from the user's installed copy at runtime and derives its own heuristics. This note describes behaviour in its own words and ships only derived numbers for analysis (a research dataset, not product data).

Status: research note | Last verified: 2026-10-04

## 1. Method and evidence

Everything below is either read from the CE source tree, computed from a resolved def load, or marked as unverified.

| Source | What it gave | Pointer |
| --- | --- | --- |
| CE README and supported-mod list | feature overview; 760 per-mod folders under `ModPatches/` selected by `LoadFolders.xml` | `CombatExtended-Development/README.md`, `CombatExtended-Development/SupportedThirdPartyMods.md` |
| CE source (C#) | formulas, constants, field meanings | `CombatExtended-Development/Source/CombatExtended/CombatExtended/` (Verbs/Verb_LaunchProjectileCE.cs, Verbs/Verb_ShootCE.cs, Verbs/Verb_MeleeAttackCE.cs, ShiftVecReport.cs, ArmorUtilityCE.cs, Projectiles/ProjectileCE.cs, Projectiles/ProjectilePropertiesCE.cs, Comps/CompAmmoUser.cs, Comps/CompFireModes.cs, Comps/CompSuppressable.cs, MassBulkUtility.cs, GunPatcherUtil.cs, PatchOperationMakeGunCECompatible.cs) |
| CE defs and patches | the values | `CombatExtended-Development/Defs/`, `Patches/Core/`, `LoadFolders.xml` (v1.6 block: core folder, then the five DLC folders when active) |
| RimWorld 1.6.4871 install | vanilla values and the base for patches | `/home/pawbeans/.steam/steam/steamapps/common/RimWorld/Data/{Core,Royalty,Ideology,Biotech,Anomaly,Odyssey}` |
| def engine | merge, patch and inheritance pipeline | `docs/research/data/def-engine/` and `docs/research/def-engine-semantics.md` (used unmodified) |
| CE wiki | intended meaning of fields, balance spreadsheets | https://github.com/CombatExtended-Continued/CombatExtended/wiki/Compatibility-Patch-Guide and https://github.com/CombatExtended-Continued/CombatExtended/wiki/Stat-&-Balance-Spreadsheets (page names as fetched; text kept in the scratch folder; accessed 2026-10-04). The spreadsheets themselves (Google Sheets, linked from the second page) were not fetched. |

Resolution procedure (`docs/research/data/ce-dataset/`):

1. `ce_load.py` loads Core and the five DLCs, then CE as one more mod with CE's own load folders, using the def engine. The engine runs 2849 patch operations (vanilla DLC patches included) and applies all of them. It resolves 17080 defs from 1924 files. The only diagnostic is 14 `inherit_duplicate_node_name` errors.
2. The engine needs a def class table. CE ships no compiled assembly in the source tree (only a loader DLL), so `ce_load.py` builds the table from the C# sources: every class deriving from `Def` or `ThingDef` becomes an entry (AmmoDef, AmmoSetDef, GunPatcherPresetDef and others). This is a workaround that works for the data used here.
3. Vanilla guns are converted to CE by a runtime patch class (`CombatExtended.PatchOperationMakeGunCECompatible`) that the engine cannot run. `ce_ops.py` re-implements its documented behaviour (statBases merge with vanilla Accuracy* removal, costList replacement, VerbPropertiesCE verb, AmmoUser and FireModes comps, tags and classes). The patched revolver in the dataset matches the numbers written in `Patches/Core/ThingDefs_Misc/Weapons_Guns.xml` (Mass 1.39, Bulk 2.79, ShotSpread 0.16, SwayFactor 1.39, recoilAmount 2.96, magazine 6, reload 4.6 s).
4. `build_dataset.py` loads vanilla alone and vanilla plus CE, and writes `ce-ranged.json`, `ce-melee.json`, `ce-apparel.json`, `ce-ammo.json` and `ce-presets.json`, with vanilla values beside the CE values for the same defName. `analyze.py` produces the CSV files, the tables in this note and four figures.

Caveats: no third-party mod patches are active, CE mod settings are the defaults the engine plugin uses (generic ammo off), numbers are the 1.6 snapshot of the development branch in this repository, and the vanilla comparison uses the resolved vanilla projectile (for example the vanilla sniper bullet has damage 25 and speed 100). Runtime-only effects (Harmony patches, attachments, weapon quality, pawn skill) are not in the data.

## 2. CE in one page

CE replaces RimWorld's hit-chance model with simulated projectiles, replaces the 0 to 2 fractional armor model with two physical units, and adds mass, bulk and ammunition as first-class constraints.

| Topic | Vanilla | Combat Extended |
| --- | --- | --- |
| Hit resolution | accuracy by distance band (AccuracyTouch/Short/Medium/Long), then a roll | a projectile with speed, height and gravity flies a ballistic path; hit depends on aim error (sway, spread, recoil, range error, lighting) and on what the path collides with |
| Damage per shot | projectile damage, armor reduces by a random fraction | damage and penetration are separate; armor compares penetration with its rating layer by layer |
| Armor units | ArmorRating_Sharp and _Blunt as 0 to 2 fractions | sharp in mm of rolled homogeneous armor (mm RHA), blunt in MPa; plus heat and electric as fractions |
| Stopped sharp attacks | partial damage reduction | the stopped part decays into a blunt attack that continues through the armor |
| Ammo | none | every gun has an ammo set, a magazine and a reload time; ammo is a carried item with Mass and Bulk |
| Carrying | no limit besides mass for caravans | Mass (kg) and Bulk (volume) capacities slow movement, work and melee dodge |
| Fire control | burst or single | aimed, snapshot and suppress modes, burst and auto modes, recoil that builds up over a burst |
| Suppression | none | bullets passing near a pawn add suppression, which can force the pawn to take cover or hunker |
| Melee | tool damage, random armor reduction | tool power and two penetration values, parry, dodge and crit as chance offsets |

Counts in this load: 83 ranged weapon defs (64 carry a CE ammo component; 70 are not Odyssey `_Unique` variants and 51 of those carry one, the rest are grenades, turret guns and special items), 930 AmmoDefs in 307 AmmoSets, 126 apparel defs, 63 stuffs and 46 melee tool rows on 20 melee weapon defs (24 rows on Core and CE weapons). The CE README groups the mod's features as shooting, melee, armor, inventory and medical; this note covers the first four. Evidence: `docs/research/data/ce-dataset/*.json`.

## 3. Ranged combat

### 3.1 Ballistics and projectile flight

Projectiles are real objects (`ProjectileCE`, `BulletCE`). A shot is launched with a horizontal rotation, a vertical angle, a start height (the shooter's shot height), and a speed. The flight is a point-mass trajectory with gravity: height at time t is the start height plus speed times the sine of the angle times t, minus half gravity times t squared (evaluated per tick, see the comment above the height function in `ProjectileCE.cs`). Constants in `CE_Utility.cs`: gravity 9.8, one cell is 5 m wide and 1.75 m high, so the projectile `speed` field is in cells per second and a value of 168 (5.56 NATO) corresponds to about 840 m/s (inference from the cell width, not stated in the code). `gravityFactor` scales gravity per projectile (default 1). `ballisticCoefficient`, `mass` and `diameter` are ranges rolled per shot and used by a few projectiles for energy loss (5 projectile defs set them; the loss model was not traced).

Trajectory workers: the default is a lerped (straight-line interpolated) path for bullets, with `lerpPosition false` or a `trajectoryWorker` class selecting the true ballistic arc (which defs use which was not surveyed). `isInstant` projectiles (beams, lasers) use a ray cast and damage falloff (`damageFalloff`). A projectile hits what its height intersects: cover and walls have collision heights, pawns have height regions (head, torso, legs), so shooting over low cover and aiming at a body region are physical.

The damage of a hit is the projectile's `damageAmountBase` (CE applies weapon quality and stuff multipliers through `GetDamageAmount`) and its penetration is `armorPenetrationSharp` or `armorPenetrationBlunt` depending on the damage def's armor category, multiplied by the weapon's `RangedWeapon_ArmorPenetrationMultiplier` (`ProjectileCE.cs`, PenetrationAmount). Explosions use `explosionRadius`, `secondaryDamage` (extra damage instances with their own chance and penetration, used for HE, incendiary and EMP parts) and fragment spawns.

### 3.2 Accuracy components

Aim error is built from independent terms, evaluated by `ShiftVecReport` and `Verb_LaunchProjectileCE`:

| Term | Formula in words | Where it comes from |
| --- | --- | --- |
| Shooter skill | `ShootingAccuracyPawn` (turret: `ShootingAccuracyTurret`), capped at 4.5 | pawn stat |
| Sway amplitude (degrees) | max(0, 4.5 minus shooting accuracy) times the weapon `SwayFactor` | `SwayAmplitude`; applied as a slow sine wobble in rotation and elevation (the time argument includes the shooter's thing id so pawns are out of phase) |
| Sway in aimed mode | sway times max(0, 1 minus AimingAccuracy) divided by max(1, SightsEfficiency) | `Verb_ShootCE.SwayAmplitudeFor` |
| Sway when suppressed | sway times 1.5, and aimed mode is not available | `SuppressionSwayFactor` |
| Inherent spread (degrees) | `ShotSpread` times the projectile `spreadMult`; a random cone, not reduced by skill | `ShiftVecReport.spreadDegrees`, `Verb_LaunchProjectileCE` |
| Accuracy factor | (1.5 minus AimingAccuracy) divided by `SightsEfficiency` (AimingAccuracy capped at 1.5) | `ShiftVecReport.accuracyFactor` |
| Range error | distance times (distance divided by max(effective range, 20)) times min(0.5 times accuracy factor, 0.8) | `ShiftVecReport.distShift` |
| Visibility error | (lighting times 7 plus weather times 1.5) times a range multiplier, times distance over 50 over sights, times (2 minus AimingAccuracy), plus smoke | `environmentShift`, `visibilityShift` |
| Lead error (moving target) | target speed times flight time, times min(0.25 times accuracy factor, 2.5), plus lighting and smoke terms | `leadShift` |
| Effective range | verb `range` times (1 plus the weapon range multiplier offset plus the projectile `effectiveRangeMultiplier` offset) plus `effectiveRangeOffset` | `EffectiveRange` |

So `SightsEfficiency` divides the range, visibility and lead errors and shortens aimed sway; it does not change spread or recoil. `ShotSpread` is the floor on accuracy that no skill removes. `SwayFactor` scales the skill-dependent wobble. The in-game hit chance readout calls `CE_Math.CalculateHitPercent` with distance, target bounds, speed, angle, sway, spread and the combined shift, so RimStudio can show a hit chance only by re-implementing that function (not read in this pass).

### 3.3 Recoil, sway, aim and fire modes

Recoil: each shot of a burst adds a random kick. The horizontal part is uniform in plus or minus half the recoil amount, the vertical part in minus a third to plus the full recoil amount (degrees), multiplied by a growth factor equal to (5 minus ShootingAccuracy) raised to min(10, shots fired) divided by 6.25. The recoil amount is the verb's `recoilAmount`, scaled by the weapon stat `CE_RangedWeapon_RecoilMultiplier` and the projectile's `recoilMultiplier` and `recoilOffset`. A weapon with `useEquipmentStatValues` reads the `Recoil` stat instead.

Recovery: the warm-up of the next shot in a burst is cut to a fraction of the full warm-up: at least 0.5 for aimed shots, 0.25 for snapshots, 0.1 for suppressive fire, and more if the line of fire moved (angle change plus the last recoil kick, divided by 45 degrees, is the fraction). So a gun with low recoil and a short `warmupTime` can re-aim quickly and keep a tight burst. This is the only "recovery speed" in CE; there is no separate recovery field.

Timing: `warmupTime` (seconds, scaled by the weapon warm-up multiplier and the projectile's warm-up terms) is the first-shot aim time; `ticksBetweenBurstShots` is the interval inside a burst (rate of fire = 3600 divided by ticks, per minute); `burstShotCount` is the verb's native burst length; `RangedWeapon_Cooldown` (seconds) is the wait after a burst or shot.

Aim modes (CompFireModes, enum AimMode): `AimedShot` (full warm-up and the reduced sway above), `Snapshot` (quick, sway at full value), `SuppressFire` (long bursts at the area, lowest per-shot warm-up). Snapshot and SuppressFire are offered unless `noSnapshot` is set. Fire modes: single shot unless the weapon has a burst; `aimedBurstShotCount` is the burst length in aimed mode; `noSingleShot` removes the single mode; for fully automatic guns the verb burst is the automatic mode. The AI uses `aiAimMode` and `aiUseBurstMode`. Weapon tags `CE_AI_*` select AI behaviour classes. Measured combinations among personal firearms:

| class | verb burst | aimedBurstShotCount | snapshot | aiAimMode | n |
|---|---|---|---|---|---|
| machine gun | burst>1 | aimedBurst | - | AimedShot | 2 |
| machine gun | burst>1 | aimedBurst | - | Snapshot | 2 |
| machine gun | burst>1 | aimedBurst | - | SuppressFire | 1 |
| pistol | single | - | - | Snapshot | 2 |
| rifle | burst>1 | aimedBurst | - | AimedShot | 3 |
| rifle | single | - | - | AimedShot | 1 |
| rifle | single | - | - | Snapshot | 1 |
| shotgun | single | - | - | Snapshot | 3 |
| smg | burst>1 | aimedBurst | - | Snapshot | 2 |
| sniper | burst>1 | aimedBurst | - | AimedShot | 1 |
| sniper | single | - | - | AimedShot | 3 |

### 3.4 Suppression

Projectiles that pass within 3 cells of a pawn of another faction than the shooter add suppression to that pawn through `CompSuppressable`. The amount is the projectile's damage times `suppressionFactor`, times `airborneSuppressionFactor` while it is still flying; explosions use the explosion damage and radius. The pawn multiplies it by its `Suppressability` stat and by 2, adds it to a pool capped at 1050 that decays by 4 per tick after 30 ticks without new input. The pawn counts as suppressed when the pool exceeds a threshold that depends on its mood and mental-break threshold: square root of max(0, mood minus hard break threshold) times 1050 times 0.125. A suppressed pawn loses aimed fire (sway times 1.5), may hunker for at least 240 ticks, and the code has constants for a mental break (600 ticks minimum, 0.1 percent chance per tick above the threshold; the exact condition was not traced). `dangerFactor` (default 1) tells the AI's danger map how much a bullet path matters. Weapon designers influence suppression only through projectile damage and the two factors; they appear on 268 and 10 of the 1017 CE projectile defs respectively.

### 3.5 Ammunition (AmmoSet, AmmoDef, projectile properties)

Chain of defs: weapon `CompProperties_AmmoUser.ammoSet` points to an `AmmoSetDef`; the set lists pairs of (AmmoDef, projectile ThingDef) in `ammoTypes`; the weapon's verb `defaultProjectile` is used when the gun has no ammo component; `similarTo` links a set to a generic set for loadout and trade logic. An `AmmoDef` is an item (class `CombatExtended.AmmoDef`) with `ammoClass` (FullMetalJacket, ArmorPiercing, HollowPoint, ...; 74 distinct values in this load), Mass, Bulk, a `cookOffProjectile` and trade tags. Its caliber is the set it belongs to, shown through thing categories such as `Ammo556x45mmNATO`.

Projectile properties (`ProjectilePropertiesCE`, resolved defs): the 1017 CE projectiles use these most often: `damageDef` (1014), `speed` (1005), `damageAmountBase` (978), `armorPenetrationBlunt` (828), `armorPenetrationSharp` (825), `dropsCasings` (806), `secondaryDamage` (224), `explosionRadius` (203), `pelletCount` (23), `spreadMult` (31), `suppressionFactor` (268). Units: damage is hit points of damage per projectile; `armorPenetrationSharp` is mm RHA; `armorPenetrationBlunt` is MPa; speed is cells per second; `pelletCount` projectiles are fired per trigger pull, each with its own spread cone (`ShotSpread` times `spreadMult`).

Relation between rounds of one caliber (median over ammo sets where a `_FMJ` round and a bullet-damage variant both exist; our computation from `ce-ammo.json`):

| ammoClass (bullet damage only) | n sets | damage / FMJ | AP sharp / FMJ | AP blunt / FMJ | speed / FMJ |
|---|---|---|---|---|---|
| ArmorPiercing | 137 | 0.62 | 2 | 1 | 1 |
| HollowPoint | 121 | 1.26 | 0.5 | 1 | 1 |
| ExplosiveAP | 72 | 1 | 1 | 1 | 1 |
| IncendiaryAP | 72 | 0.62 | 2 | 1 | 1 |
| Sabot | 72 | 0.52 | 3.5 | 1.28 | 1.36 |
| BuckShot | 5 | 0.58 | 0.6 | 0.28 | 0.96 |
| Slug | 5 | 0.92 | 0.91 | 0.94 | 1.28 |

Reading: armor-piercing rounds roughly double sharp penetration at about 62 percent of the damage, hollow points trade half the penetration for a quarter more damage, sabot rounds triple penetration at about half the damage and higher speed. Blunt penetration and speed stay the same for AP and HP, which means blunt force after a deflection is a property of the caliber, not of the round type.

Correlations across the 211 bullet ammo sets that define damage, speed and both penetrations (Spearman rank): damage with sharp penetration 0.83, and the product damage times speed with blunt penetration 0.91, but the ratio of blunt penetration to damage times speed ranges from 0.0008 to 1.47, so no single constant gives the blunt value (CE's own projectile spreadsheet presumably derives it from mass and speed; unverified).

### 3.6 Reload, magazine, bulk and mass

`CompProperties_AmmoUser`: `magazineSize` (shots), `reloadTime` (seconds), `ammoSet`, `reloadOneAtATime` (tube magazines and gate-loaded revolvers; reloads one round per cycle), `AmmoGenPerMagOverride` (pawn generation only). Effective reload time is `reloadTime` times the projectile's `reloadTimeMultiplier`, times the weapon stat `CE_RangedWeapon_ReloadFactor`, divided by the pawn's `ReloadSpeed` (seconds converted to ticks in `Toils_CombatCE`). `ammoConsumedPerShotCount` covers multi-barrel guns.

Mass (kg) and Bulk (a volume unit): both are item stats. A pawn's carry limits come from `MassBulkUtility`: 35 kg and 20 bulk per unit of body size as constants, with the stat definitions `CarryWeight` (default 40) and `CarryBulk` (default 20) in `Defs/Stats/Stats_Pawns_Inventory.xml`; which of the two feeds the final capacity was not traced. Penalties: movement speed is full until carried mass reaches 25 percent of capacity, then falls linearly toward 0.75; work speed is full up to 35 percent of bulk capacity and falls to 0.75; melee dodge starts to suffer above 50 percent bulk (down to 0.87 at full); hit chance factor starts above 25 percent (down to 0.75). Apparel adds `WornBulk` (what wearing it costs, usually far below its carried `Bulk`) and `CarryWeight`/`CarryBulk` offsets for packs and rigs. Ammo items are light: a 5.56 NATO FMJ round has Mass 0.013 and Bulk 0.02 with a stack limit of 5000.

### 3.7 Ranged weapon stat catalogue

Weapon stat bases and verb fields as they appear on CE guns (typical range = the vanilla and CE personal firearms plus launchers in this load; `n` is how many of 83 ranged defs set the field).

| Field | Meaning | Unit | Used in | Typical range (observed) |
| --- | --- | --- | --- | --- |
| `Mass` stat | weight carried | kg | MassBulkUtility, CompInventory | 1.11 (autopistol) to 35 (heavy blaster) |
| `Bulk` stat | volume carried | bulk | CompInventory, work and dodge penalties | 2.1 to 15 for personal guns (47 of 64 CE guns set it) |
| `RangedWeapon_Cooldown` | rest after a shot or burst | s | vanilla verb cycle | 0.36 (rifle, SMG) to 1.36 (sniper) |
| `SightsEfficiency` | quality of sights | factor, 1 = iron sights | accuracy factor, visibility, aimed sway | 0.7 pistols, 1.0 default, 2.24 (needle gun) and 2.6 (sniper rifle) |
| `ShotSpread` | inherent cone | degrees | spread term | 0.01 (needle, lance) to 0.17 (pistol); flamers 3 to 5, bows 0.5 to 1.5 |
| `SwayFactor` | wobble scale | factor | sway amplitude | 0.5 to 1.9 typical, 3.2 minigun |
| `Recoil` and `CE_RangedWeapon_RecoilMultiplier` | override and scale of recoil | degrees and factor | RecoilAmount | rarely set |
| `BurstShotCount`, `TicksBetweenBurstShots` | stat-level overrides of burst length and interval | shots, ticks | Verb_LaunchProjectileCE | rarely set |
| `CE_RangedWeapon_ReloadFactor` | scale of reload time | factor | CompAmmoUser job time | rarely set |
| `OneHandedness` | usable with a shield | flag | shield logic | pistols, small guns |
| `NightVisionEfficiency` and `MuzzleFlash` | scope night vision and flash | factor | lighting | display |
| verb `range` | maximum range | cells | EffectiveRange | pistols 12, rifles 35 to 55, snipers 62 to 75, MG 40 to 75 |
| verb `warmupTime` | first-shot aim time | s | verb warm-up | 0.6 pistol/SMG, 1.0 to 1.3 rifle, 1.3 to 1.8 sniper, 2.0 to 2.3 MG |
| verb `recoilAmount` | kick per shot | degrees | GetRecoilVec | about 0.5 to 3.0 (55 of 64 set it) |
| verb `burstShotCount`, `ticksBetweenBurstShots` | burst length, interval | shots, ticks | burst loop | 6 shots at 600 to 1200 rpm for rifles and SMGs |
| verb `muzzleFlashScale`, `soundCast` | flash and sound | factor, def | lighting, audio | 9 for most |
| verb `minRange`, `forceNormalTimeSpeed`, `circularError`, `indirectFirePenalty` | indirect fire weapons | cells, flag, cells | ShiftVecReport | mortars and launchers only |
| `magazineSize` | rounds per magazine | rounds | CompAmmoUser | 5 to 30 (rifles, 8 for pistols), 50 to 300 for MG |
| `reloadTime` | magazine swap time | s | CompAmmoUser | 4 standard, 4.6 to 4.9 revolver, LMG, 9.2 for belts |
| `ammoSet` | caliber | def | CompAmmoUser | 34 sets used by weapons in this load |
| `aimedBurstShotCount`, `noSnapshot`, `noSingleShot`, `aiAimMode`, `aiUseBurstMode` | fire mode set | shots, flags | CompFireModes | see table in 3.3 |
| `weaponTags` (`CE_Sidearm`, `CE_SMG`, `CE_AI_AR`, `Bipod_LMG`, ...) | AI class, bipod class | tags | loadouts, bipods | per class |

## 4. Melee combat

Tools are `CombatExtended.ToolCE` entries (vanilla `Tool` plus `armorPenetrationSharp` and `armorPenetrationBlunt`; optional `restrictedGender` and `requiredAttachment`). Each tool has `power` (damage before skill and stuff factors), `cooldownTime` (s), `capacities` (Cut, Stab, Blunt, Poke, Demolish), `chanceFactor` (selection weight among tools), `linkedBodyPartsGroup`, and penetration values. A tool's penetration is a base number: sharp tools carry mm RHA, blunt tools carry MPa.

Penetration in melee (`Verb_MeleeAttackCE`, `StatWorker_MeleeArmorPenetration`): tool penetration times (the damage factor of the attack raised to 0.75) times (1 plus 25/19/100 per melee skill level above 1) times the weapon's `MeleePenetrationFactor` (stuff factors such as plasteel's 1.25 apply here). A critical hit forces the maximum of the damage variation roll, doubles sharp penetration (not for animals), and stuns on blunt attacks.

Chances: crit, dodge and parry are chance offsets. The base values in code are crit 0.1, dodge 0.1 and parry 0.2; the chance of an event is clamp(base plus attacker's stat minus defender's stat), where for a parry the attacker's own parry stat, scaled by 1 plus the `MeleeCounterParryBonus` of the attacker's weapon, is subtracted from the defender's parry chance. Weapons contribute through `equippedStatOffsets` (`MeleeCritChance`, `MeleeParryChance`, `MeleeDodgeChance`), so a longsword adds 0.5, 0.6 and 0.4. A successful parry damages the weapon (reduced by `ToughnessRating`, which is in mm RHA, multiplied by 1.5 and treated as blunt rating against blunt attacks) and a critical parry ripostes. Dodging is reduced by carried bulk.

Weapon def fields and measured values (vanilla weapons patched by CE, plus CE-native ones):

| weapon | best tool | capacity | power (CE) | power (vanilla) | cooldown s (CE) | cooldown (vanilla) | AP sharp mm | AP blunt MPa | Mass | Bulk | crit | parry | dodge |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| LongSword | edge | Cut | 40 | 23 | 1.64 | 2.6 | 0.72 | 3.24 | 2 | 8 | 0.5 | 0.6 | 0.4 |
| Gladius | point | Stab | 27 | 16 | 1.44 | 2 | 0.48 | 0.42 | 0.85 | 3.5 | 0.2 | 0.35 | 0.2 |
| Ikwa | head | Stab | 22 | 15 | 1.54 | 2 | 0.46 | 0.55 | 1.1 | 4 | 0.11 | 0.23 | 0.27 |
| Spear | head | Stab | 18 | - | 1.19 | - | 2.25 | 2.25 | 2 | 10 | 0.17 | 1.45 | 0.9 |
| BreachAxe | head | Demolish | 17 | 7.5 | 2.65 | 1 | - | 3.38 | 1.1 | 9.5 | 0.04 | 0.15 | 0.3 |
| Mace | head | Blunt | 15 | 15.7 | 1.9 | 2 | - | 5.62 | 1.25 | 3.5 | 0.42 | 0.24 | 0.2 |
| Club | head | Blunt | 11 | 14 | 3.48 | 2 | - | 4 | 2 | 4 | 0.67 | 0.15 | 0.2 |
| Knife | point | Stab | 11 | 13 | 1.2 | 2 | 0.42 | 0.25 | 0.5 | 1 | 0.5 | 0.15 | 0.05 |

Shape of the data: for Cut and Stab tools with penetration, sharp penetration is about 0.02 to 0.12 mm RHA per point of power (median 0.03) at power 10 to 40 and cooldown about 1.4 s; blunt tools have blunt penetration 0.14 to 0.38 MPa per point of power (median 0.32) at power 2 to 15 and cooldown about 1.6 s:

| tool group | n | AP sharp per power | AP blunt per power | power | cooldown s |
|---|---|---|---|---|---|
| Cut/Stab with AP sharp | 8 | 0.03 (0.02-0.12) | 0.04 (0.02-0.12) | 19 (10-40) | 1.39 (1.18-1.78) |
| Blunt | 6 | - | 0.32 (0.14-0.38) | 5 (2-15) | 1.63 (0.99-3.48) |

Consequence: blades in CE cannot penetrate even an 8 mm vest (sharp penetration under 1 mm RHA); melee against armor is a blunt contest, where heavy blunt weapons (mace 5.6 MPa) also fall short of 12 MPa steel-vest blunt rating, so a modded melee weapon needs a deliberate penetration choice or it will do nothing against armor. Vanilla `power` is generally raised for blades (long sword 23 to 40) and lowered for improvised blunt items (club 14 to 11, bottle 9 to 2), and cooldowns move toward 1.2 to 1.6 s.

## 5. Armor model

Units and sources. Worn armor stats are `ArmorRating_Sharp` (mm RHA), `ArmorRating_Blunt` (MPa), `ArmorRating_Heat` and `ArmorRating_Electric` (fractions of damage removed). Stuffed apparel gets its rating at runtime: thickness (`StuffEffectMultiplierArmor`, in mm) times the material's `StuffPower_Armor_Sharp` or `_Blunt`. Apparel that is not made from stuff carries fixed ratings in its stat bases. Both forms exist in the data: of 126 apparel defs, 57 are stuffed with a thickness, 37 have fixed ratings, 2 have both and 26 carry no armor.

Resolution of one attack (`ArmorUtilityCE.GetAfterArmorDamage` and `TryPenetrateArmor`):

1. Order: shield first (ranged attacks that hit a covered part), then worn apparel from the outer layer (Shell) inward to OnSkin, then the pawn's natural armor (`BodyPartSharpArmor`, `BodyPartBluntArmor`, and body part density).
2. For a sharp attack against a layer with rating R and remaining penetration P: if R is greater than P the attack is deflected (no sharp damage). Otherwise the damage is multiplied by (P minus R) divided by P and P becomes P minus R. For blunt attacks the same multiplication applies (clamped between 0 and 1) but no deflection concept: the attack simply dies when R reaches P.
3. A deflected sharp attack becomes a blunt attack at the same layer: blunt penetration is the projectile's `armorPenetrationBlunt` (for a melee tool the tool's blunt value) times the fraction of sharp penetration that remained, and blunt damage is the cube root of (blunt penetration times 10000) divided by 10, scaled by the damage instance's share of the projectile's base damage (this matters for fragments). Bullet impact against shields is further scaled by 0.2.
4. A sharp attack that penetrates only partly also inflicts a separate blunt hit from the stopped portion.
5. Penetration left after armor also has to defeat the body part density before reaching inner organs.
6. Fire, electric and similar ambient damage uses no penetration: the multiplier is 1 plus the damage def's default penetration minus the sum of the ratings of every covering layer (and natural armor), clamped to 0 through 1.
7. Armor takes damage too: soft armor (stuff categories SoftArmor, or the def flag) takes at least 20 percent of a sharp attack's damage and nothing from blunt; hard armor takes a formula weighted by how close penetration is to the rating, multiplied by 0.5.

Worked examples from the resolved data (our implementation of rules 2 and 3 for the main hit only; health damage after the layers):

| round | damage, AP sharp, AP blunt | synthread flak jacket (0.8 mm / 0.2 MPa) | steel vest 8 mm / 12 MPa | plasteel vest 16 mm / 24 MPa | plasteel vest over synthread jacket |
|---|---|---|---|---|---|
| 45 ACP FMJ | 12 dmg, 3.5 mm, 10.9 MPa | 9.3 (sharp) | 0 (blunt) | 0 (blunt) | 0 (blunt) |
| 5.56 NATO FMJ | 14 dmg, 6 mm, 34.2 MPa | 12.1 (sharp) | 4.5 (blunt) | 2.1 (blunt) | 1.1 (blunt) |
| 5.56 NATO AP | 9 dmg, 12 mm, 34.2 MPa | 8.4 (sharp) | 3 (sharp) | 2.1 (blunt) | 1.6 (blunt) |
| 7.62 NATO FMJ | 20 dmg, 7 mm, 66.7 MPa | 17.7 (sharp) | 7.2 (blunt) | 5.6 (blunt) | 4.4 (blunt) |
| 7.62 NATO AP | 12 dmg, 14 mm, 66.7 MPa | 11.3 (sharp) | 5.1 (sharp) | 5.6 (blunt) | 5 (blunt) |
| 14.5x114 FMJ | 53 dmg, 18 mm, 634 MPa | 50.6 (sharp) | 29.4 (sharp) | 5.9 (sharp) | 3.5 (sharp) |

Example reading: a 5.56 NATO FMJ round (14 damage, 6 mm, 34.2 MPa) hits a steel armor vest (8 mm / 12 MPa): 6 is less than 8, so it deflects; the blunt remnant has about 7.0 damage at 34.2 MPa, loses 35 percent to the 12 MPa rating, and delivers about 4.5 blunt damage. The armor-piercing 5.56 round (12 mm) keeps 33 percent of its damage after the vest and delivers sharp damage. A .45 ACP bullet (3.5 mm, 10.9 MPa) is completely stopped by the same vest, because both of its penetrations are under the ratings.

Layers and coverage: `apparel.layers` (OnSkin, Middle, Shell, Overhead, ...) and `bodyPartGroups` decide which parts a piece covers. `PartialArmorExt` multiplies the rating on named parts (28 of 126 apparel defs use it; for example the plated vest gives its neck 0.6 of the rating, a helmet gives eyes 0.7 and nose and jaw 0.5), unless `useStatic` makes the value absolute. Stat parts in `Stats_Apparel.xml` use `StuffEffectMultiplierArmor` as the multiplier stat.

Materials (CE values against the vanilla fractions; power per mm of thickness):

| material | stuff categories | CE armor power sharp / blunt / heat | vanilla power | melee dmg mult sharp / blunt | Mass per unit | Bulk per unit |
|---|---|---|---|---|---|---|
| Steel | Metallic/Metallic_Weapon/Steeled | 1 / 1.5 / 0 | 0.9 / 0.45 / 0.6 | 1 / 1 | 0.5 | 0.03 |
| Plasteel | Metallic/Metallic_Weapon/Steeled | 2 / 3 / 0 | 1.14 / 0.55 / 0.65 | 1.1 / 0.9 | 0.25 | 0.03 |
| Cloth | Fabric | 0.01 / 0.01 / 0.04 | 0.36 / 0 / 0.18 | - / - | 0.03 | 0.05 |
| Synthread | Fabric/SoftArmor | 0.2 / 0.05 / 0.01 | 0.94 / 0.26 / 0.9 | - / - | 0.03 | 0.05 |
| Hyperweave | Fabric/SoftArmor | 1 / 1.5 / 0.2 | 2 / 0.54 / 2.88 | - / - | 0.04 | 0.1 |
| DevilstrandCloth | Fabric/SoftArmor | 0.5 / 0.1 / 0.05 | 1.4 / 0.36 / 3 | - / - | 0.03 | 0.05 |
| Leather_Plain | Leathery | 0.05 / 0.04 / 0.01 | 0.81 / 0.24 / 1.5 | - / - | 0.03 | 0.05 |
| Leather_Heavy | Leathery | 0.09 / 0.06 / 0.01 | 1.24 / 0.24 / 1.5 | - / - | 0.03 | 0.05 |
| Leather_Thrumbo | Leathery/SoftArmor | 0.75 / 0.1 / 0.01 | 2.08 / 0.36 / 1.5 | - / - | 0.03 | 0.05 |
| Leather_Bear | Leathery | 0.08 / 0.06 / 0.01 | 1.12 / 0.24 / 1.5 | - / - | 0.03 | 0.05 |
| WoodLog | Woody | 0.1 / 0.2 / 0.03 | 0.54 / 0.54 / 0.4 | 0.4 / 0.9 | 0.4 | 0.07 |
| Gold | Metallic | 0.3 / 0.5 / 0 | 0.72 / 0.36 / 0.36 | 0.75 / 1 | 0.01 | 0.01 |
| Silver | Metallic | 0.4 / 0.6 / 0 | 0.72 / 0.36 / 0.36 | 0.85 / 1 | 0.01 | 0.007 |
| Jade | Stony | 0.6 / 1 / 0 | 0.9 / 0.45 / 0.54 | - / 1.5 | 0.5 | 0.3 |
| Uranium | Metallic/Metallic_Weapon | 1.2 / 1.8 / 0 | 1.08 / 0.54 / 0.65 | 1.1 / 1.5 | 1 | 0.05 |
| Bioferrite | Metallic/Bioferrite/Metallic_Weapon/Steeled | 1.8 / 2.7 / 0 | 1.1 / 0.5 / 0.5 | 1.3 / 0.9 | 0.25 | 0.02 |
| Obsidian | Stony/Metallic/Metallic_Weapon | 0.9 / 0.5 / 0.2 | 0.85 / 0.4 / 0.7 | 1.4 / - | 0.5 | 0.02 |

Observations: CE metal blunt power is 1.5 times its sharp power for steel and plasteel (1 and 1.5, 2 and 3); fabric and leather give almost no blunt protection; woven armor fabrics reach 0.2 (synthread) to 1.0 (hyperweave) mm per mm of thickness, so even a 4 mm jacket in synthread is under 1 mm RHA. Figure: `docs/research/data/ce-dataset/figures/stuff_sharp_power.png`.

Typical pieces and what they compute to (a few of the stuffed apparel defs with the thickest base values; the full list is in `apparel.csv`; mm and MPa after multiplying by the stuff power):

| apparel | stuff categories | thickness mm | Mass | Bulk | WornBulk | steel sharp/blunt | plasteel | synthread | plainleather |
|---|---|---|---|---|---|---|---|---|---|
| FlakVest | Steeled | 8 | 11 | 5 | 3 | 8 / 12 | 16 / 24 | 1.6 / 0.4 | 0.4 / 0.32 |
| Duster | Fabric/Leathery | 5 | 2 | 7.5 | 2 | 5 / 7.5 | 10 / 15 | 1 / 0.25 | 0.25 / 0.2 |
| Jacket | Fabric/Leathery | 5 | 1.7 | 5 | 1 | 5 / 7.5 | 10 / 15 | 1 / 0.25 | 0.25 / 0.2 |
| FlakJacket | Fabric | 4 | 4 | 5 | 2.5 | 4 / 6 | 8 / 12 | 0.8 / 0.2 | 0.2 / 0.16 |

Distribution by kind (our grouping by body part groups and layers):

| kind | rating source | n | Mass kg | Bulk | WornBulk | thickness mm (stuffed) or sharp mm RHA (fixed) | blunt MPa (fixed) |
|---|---|---|---|---|---|---|---|
| headgear | fixed rating | 18 | 2.25 (0.08-6.6) | 4 (1-6) | 1 (1-1) | 8 (0-22) | 14 (0-50) |
| headgear | stuff-based | 28 | 0.12 (0.07-3) | 3 (1-6) | 0.88 (0-2.5) | 2.75 (0.15-10) | - |
| torso | fixed rating | 5 | 3 (0.25-15) | 5 (5-35) | 3 (2.5-10) | 0.24 (0.07-14) | 1.03 (0.06-21) |
| torso | stuff-based | 16 | 1.25 (0.15-11) | 5 (0.75-10) | 2.5 (0.5-5) | 2 (0.8-8) | - |
| legs | stuff-based | 3 | 0.5 (0.25-2.5) | 2.38 (0.75-4) | 2.5 (2.5-2.5) | 1 (0.8-4) | - |
| full body | fixed rating | 13 | 40 (0.5-80) | 95 (40-110) | 15 (6-18) | 16 (0.07-28) | 34 (0.06-60) |
| full body | stuff-based | 8 | 0.75 (0.18-15) | 5 (0.75-100) | 2 (1.5-10) | 3 (1-5) | - |
| other | fixed rating | 27 | 3 (0.2-7) | 8 (1-10) | 2 (0-5) | 0.02 (0.01-0.04) | 0.01 (0.01-0.01) |
| other | stuff-based | 8 | 0.75 (0.04-13) | 3 (0.25-15) | 5 (0-10) | 2 (1.5-8) | - |

Bulk and WornBulk: heavy full-body suits are very bulky (median Bulk 95, WornBulk 15, Mass 40 kg), vests around Bulk 5 and WornBulk 3, helmets Bulk 4 and WornBulk 1, light clothing Bulk 1 or less. CE's own apparel auto-patcher presets (`Defs/ApparelAutoPatcherPresets/*.xml`, class `ApparelPatcherPresetDef`) map a vanilla rating or thickness to CE sharp and blunt values with curves, and assign Bulk, WornBulk and Mass per preset. The presets in this load:

| preset | Bulk | WornBulk | Mass | vanilla rating range matched | sharp (vanilla, mm) points | blunt points | needed layers |
|---|---|---|---|---|---|---|---|
| Helmet | 4 | 1 | 1 | 0.6~0.9 | [0.6, 9, 0.7, 10, 0.8, 11, 0.9, 12] | [0.6, 9, 0.7, 10, 0.8, 11, 0.9, 12] | Overhead |
| LightHelmet | 4 | 1 | 1 | 0.35~0.59 | [0.35, 1, 0.45, 2, 0.55, 4, 0.59, 6] | [0.35, 1.1, 0.45, 3.2, 0.55, 6.4, 0.59, 8.65] | Overhead |
| Hat | 1 | 1 | 1 | 0.00~0.35 | [0, 0, 0.35, 0.25] | [0, 0, 0.35, 0.5] | Overhead |
| ArmorVestJacket | 5 | 5 | 4 | 0.4~1.5 | [0.40, 4, 0.80, 8] | [0.08, 1.5, 0.16, 3] | Shell |
| LeatherJacket | 5 | 1 | - | 0.20~0.45 | [0.20, 0.08, 0.45, 0.60] | [0.08, 0.14, 0.16, 0.85] | Shell |
| ShellClothing | 1 | 1 | - | 0.20~0.45 | [0.20, 0.08, 0.45, 0.60] | [0.08, 0.14, 0.16, 0.85] | Shell |
| MiddleClothing | 1 | 1 | - | 0.08~0.28 | [0.08, 0.04, 0.28, 0.15] | [0, 0, 0.08, 0.08, 0.28, 0.25] | Middle |
| SkinClothing | 1 | 1 | - | 0.10~0.35 | [0.10, 0.05, 0.35, 0.25] | [0.10, 0.18, 0.35, 0.45] | OnSkin |
| PantsClothing | 1 | 1 | - | 0.10~0.35 | [0.10, 0.05, 0.35, 0.25] | [0.10, 0.18, 0.35, 0.45] | OnSkin |
| ArmorVest | 5 | 3 | 13 | 1~2 | [1, 8, 2, 16] | [1, 12, 2, 24] | Middle |
| LightArmorVest | 5 | 1 | 3 | 0.55~0.99 | [0.55, 1.5, 0.99, 8] | [0.55, 2.65, 0.99, 14] | Middle |

## 6. Datasets

Files in `docs/research/data/ce-dataset/` (all under 1.5 MB; derived analysis data, not for the product):

| File | Content | Records |
| --- | --- | --- |
| `ce-ranged.json` | ranged weapon defs with CE stats, verb, ammo and fire mode comps, default projectile properties, and the vanilla values beside them | 83 |
| `ce-melee.json` | weapons with tools, CE tool fields, equipped stat offsets, vanilla tools | 85 |
| `ce-apparel.json` | apparel with stats, layers, groups, partial armor, plus a `stuffs` array with stuff powers and factors, CE and vanilla | 126 apparel, 63 stuffs |
| `ce-ammo.json` | AmmoDefs with their projectile properties and an `ammoSets` array | 930 ammo, 307 sets |
| `ce-presets.json` | CE's gun and apparel auto-patcher presets (parameters only) | 6 and 11 |
| `ranged.csv`, `melee.csv`, `apparel.csv`, `ammo_calibers.csv` | flat versions with our class labels | 83, 46, 126, 307 |
| `figures/*.png` | four figures | 4 |
| `ce_load.py`, `ce_ops.py`, `build_dataset.py`, `analyze.py` | reproduction scripts (paths from arguments or `RIMWORLD_DIR`, `CE_DIR`) | |

Reproduce: `RIMWORLD_DIR=<game> CE_DIR=<CE repo> python3 build_dataset.py` then `python3 analyze.py`. Both set no bytecode and take about 25 seconds.

Ammo census:

| measure | value |
|---|---|
| AmmoDef count | 930 |
| AmmoSetDef count | 307 |
| ammo sets by source | {'CombatExtended': 307} |
| AmmoDef by source | {'CombatExtended': 926, 'RimWorld': 3, 'Biotech': 1} |
| distinct ammoClass values | 74 |
| ammo sets used by a vanilla-or-CE personal weapon in this load | 34 |
| ammo types per set (median, max) | 3, 13 |

Calibers referenced by a weapon in this load (default round of the weapon; AP and blunt values are mm RHA and MPa; "weapons" counts weapon defs including turrets):

| ammoSet | ammo types | default round: damage | AP sharp | AP blunt | speed | pellets | weapons |
|---|---|---|---|---|---|---|---|
| Flamethrower | 3 | 5 PrometheumFlame | - | - | 20 | - | 2 |
| 66mmThermalBolt | 1 | 6 PrometheumFlame | - | - | 0 | - | 2 |
| Arrow | 5 | 6 Arrow | 0.2 | 1.4 | 31 | - | 1 |
| 12Gauge | 4 | 8  | 4 | 4.5 | 83 | 9 | 2 |
| StreamlinedArrow | 5 | 8 ArrowHighVelocity | 0.5 | 2.8 | 40 | - | 1 |
| 410Bore | 4 | 9  | 3 | 6.3 | 85 | 4 | 1 |
| GreatArrow | 5 | 9 Arrow | 1 | 1.5 | 21 | - | 1 |
| 30x64mmFuel | 3 | 10 PrometheumFlame | - | - | 54 | - | 1 |
| 5x50mmCaseless | 1 | 10  | 21 | 73.2 | 206 | - | 1 |
| 5x50mmCaselessToxic | 1 | 11 BulletToxic | 26 | 112.5 | 242 | - | 1 |
| 80x256mmFuel | 1 | 11 PrometheumFlame | - | - | 73 | - | 2 |
| NerveSpikerBolt | 1 | 11 NerveBioferrite | 5 | 10.8 | 34 | - | 1 |
| 45ACP | 3 | 12  | 3.5 | 10.9 | 67 | - | 3 |
| 556x45mmNATO_SB | 6 | 12  | 5 | 22 | 142 | - | 2 |
| BeamRepeater | 2 | 12 BeamBypassShields | - | - | - | - | 1 |
| 5x35mmCharged | 1 | 13  | 33 | 60 | 178 | - | 1 |
| 6x22mmCharged | 1 | 13  | 15 | 25.6 | 151 | - | 1 |
| 6x24mmCharged | 3 | 13  | 15 | 25.6 | 151 | - | 1 |
| 556x45mmNATO | 6 | 14  | 6 | 34.2 | 168 | - | 4 |
| 44Magnum | 3 | 15  | 6 | 23.3 | 88 | - | 1 |
| 8x35mmCharged | 3 | 19  | 16 | 57.6 | 151 | - | 1 |
| 303British | 6 | 20  | 6.5 | 67 | 147 | - | 2 |
| 762x51mmNATO | 6 | 20  | 7 | 66.7 | 156 | - | 5 |
| 30x29mmGrenade | 5 | 22 Bomb | - | - | 51 | - | 1 |
| 40x46mmGrenade | 6 | 22 Bomb | - | - | 27 | - | 1 |
| 12x64mmCharged | 1 | 39  | 30 | 324 | 165 | - | 2 |
| 164x284mmDemo | 1 | 40 Thump | 0 | 0 | 61 | - | 1 |
| 20x102mmNATO | 4 | 44  | 30 | 1029.1 | 182 | - | 1 |
| 145x114mm | 5 | 53  | 18 | 634 | 178 | - | 2 |
| 40x311mmR | 4 | 105  | 70 | 6502.5 | 158 | - | 1 |
| 50mmRocket | 1 | 147 Bomb | - | - | 0 | - | 1 |
| 81mmMortarShell | 9 | 156 Bomb | - | - | 0 | - | 1 |
| 90mmCannonShell | 6 | 304  | 500 | 38 | 151 | - | 1 |

Personal firearms: CE values beside vanilla (resolved vanilla projectile and verb; arrows show the shape of the change: CE guns get much longer range except sidearms, a longer warm-up for pistols, a much shorter cooldown because burst timing moved into `ticksBetweenBurstShots`, and a higher bullet speed):

| gun | range (vanilla -> CE) | warmup s | cooldown s | damage | speed | Mass kg | AP sharp | AP blunt |
|---|---|---|---|---|---|---|---|---|
| AssaultRifle | 30.9 -> 55 | 1 -> 1.1 | 1.7 -> 0.36 | 11 -> 14 | 70 -> 168 | 3.5 -> 3.26 | 6 | 34.2 |
| Autopistol | 25.9 -> 12 | 0.3 -> 0.6 | 1 -> 0.38 | 10 -> 12 | 55 -> 67 | 1.2 -> 1.11 | 3.5 | 10.9 |
| BeamRepeater | 21.9 -> 40 | 4 -> 2 | 3 -> 0.1 | 5 -> 12 | - -> - | 10 -> 10 | - | - |
| BoltActionRifle | 36.9 -> 55 | 1.7 -> 1.1 | 1.5 -> 1.17 | 18 -> 20 | 70 -> 147 | 3.5 -> 4.19 | 6.5 | 67 |
| ChainShotgun | 12.9 -> 16 | 1.2 -> 0.6 | 1.35 -> 0.39 | 18 -> 8 | 55 -> 83 | 4.5 -> 3.5 | 4 | 4.5 |
| ChargeBlasterHeavy | 26.9 -> 75 | 1.25 -> 1.3 | 7.4 -> 0.36 | 15 -> 39 | 90 -> 165 | 22 -> 35 | 30 | 324 |
| ChargeLance | 32.9 -> 62 | 1.7 -> 1.3 | 2.7 -> 0.36 | 30 -> 13 | 120 -> 178 | 8 -> 8 | 33 | 60 |
| ChargeRifle | 27.9 -> 55 | 1 -> 1 | 2 -> 0.36 | 16 -> 13 | 70 -> 151 | 4.6 -> 3 | 15 | 25.6 |
| HeavySMG | 22.9 -> 25 | 0.9 -> 0.6 | 1.65 -> 0.37 | 12 -> 12 | 48 -> 67 | 3.5 -> 2.5 | 3.5 | 10.9 |
| HellcatRifle | 26.9 -> 48 | 1.1 -> 1.1 | 1.7 -> 0.36 | 10 -> 12 | 70 -> 142 | 3.5 -> 4.35 | 5 | 22 |
| LMG | 25.9 -> 62 | 1.8 -> 1.3 | 1.6 -> 0.56 | 12 -> 20 | 46 -> 147 | 8.5 -> 8.7 | 6.5 | 67 |
| MachinePistol | 19.9 -> 12 | 0.5 -> 0.6 | 0.9 -> 0.36 | 6 -> 12 | 55 -> 67 | 2.5 -> 2.84 | 3.5 | 10.9 |
| MiniShotgun | 12.9 -> 14 | 1.2 -> 0.8 | 1.7 -> 0.6 | 10 -> 9 | 55 -> 85 | 1.5 -> 1.5 | 3 | 6.3 |
| Minigun | 30.9 -> 62 | 2.5 -> 2.1 | 1.5 -> 0.35 | 10 -> 20 | 70 -> 156 | 10 -> 20 | 7 | 66.7 |
| Needle | 44.9 -> 75 | 2.5 -> 1.5 | 2.1 -> 0.38 | 15 -> 10 | 90 -> 206 | 2.6 -> 2.6 | 21 | 73.2 |
| PumpShotgun | 15.9 -> 16 | 0.9 -> 0.6 | 1.25 -> 0.99 | 18 -> 8 | 55 -> 83 | 3.4 -> 3.05 | 4 | 4.5 |
| Revolver | 25.9 -> 12 | 0.3 -> 0.6 | 1.6 -> 0.49 | 12 -> 15 | 55 -> 88 | 1.4 -> 1.39 | 6 | 23.3 |
| Slugthrower | 19.9 -> 35 | 0.3 -> 1.3 | 4 -> 0.37 | 12 -> 14 | 70 -> 168 | 1.5 -> 1.5 | 6 | 34.2 |
| SniperRifle | 44.9 -> 75 | 3.5 -> 1.8 | 1.5 -> 1.36 | 25 -> 20 | 100 -> 156 | 4 -> 7.3 | 7 | 66.7 |
| ToxicNeedle | 44.9 -> 75 | 2.35 -> 1.5 | 2.1 -> 0.38 | 25 -> 11 | 90 -> 242 | 2.6 -> 2.6 | 26 | 112.5 |

Figure: `docs/research/data/ce-dataset/figures/ce_vs_vanilla_guns.png`.

## 7. Distributions and archetypes

By class (our grouping of 83 defs, `_Unique` Odyssey variants excluded; median with min to max). `ranged_class_a` is handling, `ranged_class_b` is performance:

| class | n | Mass kg | Bulk | cooldown s | ShotSpread | SwayFactor | SightsEff | recoilAmount |
|---|---|---|---|---|---|---|---|---|
| pistol | 2 | 1.25 (1.11-1.39) | 2.45 (2.1-2.79) | 0.43 (0.38-0.49) | 0.17 (0.16-0.17) | 1.23 (1.07-1.39) | 0.7 (0.7-0.7) | 2.84 (2.72-2.96) |
| smg | 2 | 2.67 (2.5-2.84) | 3.73 (2.95-4.5) | 0.36 (0.36-0.37) | 0.15 (0.14-0.16) | 1.44 (0.94-1.93) | 0.85 (0.7-1) | 1.75 (1.71-1.79) |
| shotgun | 3 | 3.05 (1.5-3.5) | 7 (6.7-10) | 0.6 (0.39-0.99) | 0.15 (0.14-0.15) | 1.26 (0.53-1.31) | 1 (1-1) | 2.54 (2.42-2.72) |
| rifle | 5 | 3.26 (1.5-4.35) | 9.5 (6-12.6) | 0.36 (0.36-1.17) | 0.07 (0.02-0.14) | 1.2 (0.53-1.68) | 1 (1-1.1) | 1.5 (1.16-2.04) |
| sniper | 4 | 4.95 (2.6-8) | 14 (11.92-15) | 0.38 (0.36-1.36) | 0.01 (0.01-0.05) | 0.86 (0.85-1.35) | 2.24 (1-2.6) | 1.56 (0.92-1.8) |
| machine gun | 5 | 20 (8.7-35) | 12 (9.5-13) | 0.35 (0.1-0.56) | 0.05 (0.01-0.1) | 1.37 (1.25-3.22) | 1 (1-1.1) | 0.97 (0.01-1.38) |
| launcher | 8 | 11 (2.5-50) | 11.5 (6.3-20) | 1.5 (0.43-2.59) | 0.16 (0.01-0.2) | 1.12 (0.14-3.24) | 1 (1-2.24) | 3.23 (0.1-3.87) |
| flamer | 2 | 4.25 (1.5-7) | 8 (6-10) | 0.38 (0.37-0.39) | 4 (3-5) | 0.95 (0.53-1.38) | 1 (1-1) | 0.34 (0.33-0.35) |
| bow | 5 | 2 (0.8-3.5) | 4.25 (3-6) | 1 (1-1) | 1 (0.5-1.5) | 2 (1.2-2.5) | 0.8 (0.8-1) | - |
| grenade | 9 | 0.5 (0.24-1.5) | 1.05 (0.61-4.72) | 1 (1-1) | - | - | 1 (1-1) | - |
| turret | 19 | 12 (2.6-500) | 6.5 (6-7) | 0.36 (0.1-3.5) | 0.05 (0.01-0.2) | 0.86 (0.56-1.89) | 1 (0.5-2.36) | 1.43 (0.85-4.74) |

| class | n | range cells | warmup s | mag | reload s | rpm (burst) | damage | AP sharp mm | speed |
|---|---|---|---|---|---|---|---|---|---|
| pistol | 2 | 12 (12-12) | 0.6 (0.6-0.6) | 6 (6-7) | 4.3 (4-4.6) | - | 13.5 (12-15) | 4.8 (3.5-6) | 78 (67-88) |
| smg | 2 | 18.5 (12-25) | 0.6 (0.6-0.6) | 28 (25-30) | 4 (4-4) | 900 (600-1200) | 12 (12-12) | 3.5 (3.5-3.5) | 67 (67-67) |
| shotgun | 3 | 16 (14-16) | 0.6 (0.6-0.8) | 6 (5-8) | 4 (0.85-4.9) | 240 (240-240) | 8 (8-9) | 4 (3-4) | 83 (83-85) |
| rifle | 5 | 55 (35-55) | 1.1 (1-1.3) | 30 (10-30) | 4 (4-4.3) | 720 (720-900) | 14 (12-20) | 6 (5-15) | 151 (142-168) |
| sniper | 4 | 75 (62-75) | 1.5 (1.3-1.8) | 10 (5-20) | 4 (4-4) | 300 (300-300) | 12 (10-20) | 23.5 (7-33) | 192 (156-242) |
| machine gun | 5 | 62 (40-75) | 2 (1.3-2.3) | 250 (50-300) | 9.2 (4-9.2) | 900 (514-1800) | 20 (12-39) | 7 (6.5-30) | 156 (147-165) |
| launcher | 8 | 43 (31-86) | 2 (1-4.3) | 1 (1-5) | 1.6 (0.85-9.8) | 180 (180-180) | 31 (6-250) | 150 (0-300) | 58 (0-100) |
| flamer | 2 | 10.5 (10-11) | 0.85 (0.6-1.1) | 45 (30-60) | 4.5 (4-5) | 960 (720-1200) | 5 (5-5) | - | 20 (20-20) |
| bow | 5 | 22 (9-30) | 1 (0.8-1.3) | 1 (1-1) | 4 (4-4) | - | 9 (6-14) | 1 (0.2-5) | 31 (14-40) |
| grenade | 9 | 10 (7-10) | 0.8 (0.8-1.6) | - | - | - | 56 (10-234) | - | 12 (10-12) |
| turret | 19 | 62 (19.9-700) | 1.37 (0-11) | 50 (1-200) | 7.8 (5-9.8) | 600 (24-1800) | 22 (6-9999) | 16 (5-500) | 151 (0-182) |

Catalogue of the personal firearms (the data a "what fits" check compares against):

| defName | class | Mass | Bulk | cooldown | spread | sway | sights | recoil | range | warmup | mag | reload | burst/rpm | caliber (ammoSet) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Autopistol | pistol | 1.11 | 2.1 | 0.38 | 0.17 | 1.07 | 0.7 | 2.72 | 12 | 0.6 | 7 | 4 | -/- | 45ACP |
| Revolver | pistol | 1.39 | 2.79 | 0.49 | 0.16 | 1.39 | 0.7 | 2.96 | 12 | 0.6 | 6 | 4.6 | -/- | 44Magnum |
| HeavySMG | smg | 2.5 | 4.5 | 0.37 | 0.14 | 0.94 | 1 | 1.79 | 25 | 0.6 | 25 | 4 | 6/600 | 45ACP |
| MachinePistol | smg | 2.84 | 2.95 | 0.36 | 0.16 | 1.93 | 0.7 | 1.71 | 12 | 0.6 | 30 | 4 | 6/1200 | 45ACP |
| MiniShotgun | shotgun | 1.5 | 7 | 0.6 | 0.15 | 0.53 | 1 | 2.42 | 14 | 0.8 | 6 | 4.9 | -/- | 410Bore |
| PumpShotgun | shotgun | 3.05 | 10 | 0.99 | 0.14 | 1.31 | 1 | 2.72 | 16 | 0.6 | 5 | 0.85 | -/- | 12Gauge |
| ChainShotgun | shotgun | 3.5 | 6.7 | 0.39 | 0.15 | 1.26 | 1 | 2.54 | 16 | 0.6 | 8 | 4 | -/240 | 12Gauge |
| Slugthrower | rifle | 1.5 | 6 | 0.37 | 0.07 | 0.53 | 1 | 1.66 | 35 | 1.3 | 10 | 4 | -/- | 556x45mmNATO |
| ChargeRifle | rifle | 3 | 7 | 0.36 | 0.08 | 1.2 | 1.1 | 1.46 | 55 | 1 | 30 | 4 | 6/720 | 6x24mmCharged |
| AssaultRifle | rifle | 3.26 | 10.03 | 0.36 | 0.07 | 1.33 | 1 | 1.5 | 55 | 1.1 | 30 | 4 | 6/900 | 556x45mmNATO |
| BoltActionRifle | rifle | 4.19 | 12.6 | 1.17 | 0.02 | 1.68 | 1 | 2.04 | 55 | 1.1 | 10 | 4.3 | -/- | 303British |
| HellcatRifle | rifle | 4.35 | 9.5 | 0.36 | 0.14 | 1.17 | 1 | 1.16 | 48 | 1.1 | 30 | 4 | 6/720 | 556x45mmNATO_SB |
| Needle | sniper | 2.6 | 15 | 0.38 | 0.01 | 0.85 | 2.24 | 1.62 | 75 | 1.5 | 10 | 4 | -/- | 5x50mmCaseless |
| ToxicNeedle | sniper | 2.6 | 15 | 0.38 | 0.01 | 0.85 | 2.24 | 1.8 | 75 | 1.5 | 20 | 4 | 4/300 | 5x50mmCaselessToxic |
| SniperRifle | sniper | 7.3 | 11.92 | 1.36 | 0.05 | 1.35 | 2.6 | 1.5 | 75 | 1.8 | 5 | 4 | -/- | 762x51mmNATO |
| ChargeLance | sniper | 8 | 13 | 0.36 | 0.01 | 0.88 | 1 | 0.92 | 62 | 1.3 | 10 | 4 | -/- | 5x35mmCharged |
| LMG | machine gun | 8.7 | 12.9 | 0.56 | 0.05 | 1.37 | 1 | 1.38 | 62 | 1.3 | 50 | 4.9 | 10/514 | 303British |
| BeamRepeater | machine gun | 10 | 9.5 | 0.1 | 0.1 | 1.95 | 1.1 | 0.01 | 40 | 2 | 300 | 4 | 25/900 | BeamRepeater |
| Minigun | machine gun | 20 | 10 | 0.35 | 0.06 | 3.22 | 1 | 0.97 | 62 | 2.1 | 250 | 9.2 | 50/1800 | 762x51mmNATO |
| CE_MechanoidMinigun | machine gun | 30 | 12 | 0.35 | 0.01 | 1.25 | 1 | 0.55 | 68 | 2.3 | 250 | 9.2 | 50/1200 | 762x51mmNATO |
| ChargeBlasterHeavy | machine gun | 35 | 13 | 0.36 | 0.01 | 1.33 | 1 | 1.08 | 75 | 1.3 | 100 | 9.2 | 10/600 | 12x64mmCharged |

Figures: `docs/research/data/ce-dataset/figures/ranged_mass_bulk_accuracy.png` (Mass against Bulk, and sway against spread) and `docs/research/data/ce-dataset/figures/ammo_damage_vs_penetration.png`.

Relationships measured on the 21 personal firearms (Spearman rank correlation): Mass with Bulk 0.56, Mass with sway 0.50, Mass with recoil minus 0.79 (heavier guns kick less), range with warm-up 0.81 (longer range, longer aim time), range with sights efficiency 0.70, bullet speed with sharp penetration 0.84 (20 guns). Mass and Bulk are only moderately tied, so Bulk must be judged against class, not derived from Mass.

CE tiers against vanilla tech levels. Vanilla tech level is a def field (`techLevel`); CE keeps it and layers damage, penetration and armor on top, so the tier is the combination of tech level and the research prerequisite in `recipeMaker`:

| tech level | firearms+bows n | damage | AP sharp mm | range cells | Mass kg | helmet sharp mm RHA (fixed) | torso/body sharp (fixed, >1) | thickness mm of stuffed apparel |
|---|---|---|---|---|---|---|---|---|
| Neolithic | 4 | 8.5 (6-14) | 0.8 (0.2-3) | 18 (9-30) | 1.65 (0.8-3) | - | - | 3 (1-10) |
| Medieval | 1 | 11 (11-11) | 5 (5-5) | 28 (28-28) | 3.5 (3.5-3.5) | 0.1 (0.1-0.1) | - | 2 (0.1-5) |
| Industrial | 13 | 14 (8-20) | 6 (3.5-7) | 35 (12-75) | 3.26 (1.11-20) | 4 (0-8) | 14 (14-14) | 4 (1-8) |
| Spacer | 8 | 12.5 (9-39) | 21 (3-33) | 65 (14-75) | 5.5 (1.5-35) | 12 (0-22) | 17 (2.6-28) | - |

Reading: Neolithic bows and thrown weapons have a median of 0.8 mm RHA penetration (pila 3) and are stopped by almost any armor; Industrial firearms cluster at 3.5 to 7 mm sharp penetration (against the 8 mm of a steel vest), which is why they deflect from a good vest; Spacer weapons have a median of 21 mm. Helmets and body armor scale the same way: Industrial helmets about 4 mm RHA median, Spacer helmets 12 mm, Industrial body armor 14 mm, Spacer 17 mm (fixed-rating pieces only).

Caliber to role (derived from the calibers table above, with the weapons that use them):

| Role | Calibers (default round damage, sharp penetration) | Weapon examples |
| --- | --- | --- |
| Sidearm and SMG | .45 ACP (12, 3.5 mm), .44 Magnum (15, 6 mm) | autopistol, revolver, machine pistol, heavy SMG |
| Shotgun | 12 gauge (8 per pellet, 9 pellets, 4 mm), .410 bore (9, 4 pellets) | pump and chain shotgun |
| Battle and assault rifle | 5.56 NATO (14, 6 mm), 5.56 NATO SB (12, 5 mm), .303 British (20, 6.5 mm) | assault rifle, bolt-action rifle, light machine gun |
| Marksman and sniper | 7.62 NATO (20, 7 mm), 5x50 caseless (10, 21 mm) | sniper rifle, spacer needle gun |
| Spacer charged | 6x24 charged (13, 15 mm), 5x35 charged (13, 33 mm), 12x64 charged (39, 30 mm) | charge rifle, lance, heavy blaster |
| Heavy and vehicle | 14.5x114 (53, 18 mm), 20x102 (44, 30 mm), 40x311R (105, 70 mm), 90 mm shell | heavy and autocannon turrets |
| Area | 40x46 and 30x29 grenades (22 explosive), rockets, mortar shells (156 and up) | launchers, mortars |

CE's own conversion heuristics (`GunPatcherPresetDef`): CE converts unpatched modded guns by matching them to one of six presets (assault rifle, machine gun, pistol, revolver, SMG, sniper) using name fragments, weapon tags, and the vanilla range, warm-up, projectile damage and speed ranges; it then sets Mass, Bulk, spread, sway, sights, magazine, reload and AI fields from the preset and uses curves to map vanilla range, warm-up, cooldown and mass onto CE values, and picks a caliber by damage and speed ranges. This is the closest thing to a published "formula mode" and shows which quantities CE considers class-defining. The presets in this load:

| preset | Mass | Bulk | Spread | Sway | Sights | mag | reload | matches vanilla range | vanilla warmup | vanilla dmg | vanilla proj speed | caliber rules | special guns |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| AssaultRifle | 3.25 | 10 | 0.07 | 1.3 | - | 30 | 4 | 22~34 | 0.8~1.6 | 8~18 | 60~80 | 5 | 0 |
| MachineGun | 9 | 14 | 0.05 | 1.53 | - | 50 | 4.9 | 24~34 | 0.4~0.81 | 8~17 | 48~72 | 5 | 0 |
| Pistol | 1.2 | 2.25 | 0.17 | 1.1 | 0.7 | 8 | 4 | 20~25 | 0.4~0.81 | 6~15 | 54~70 | 3 | 1 |
| Revolver | 1.4 | 2.5 | 0.2 | 1.3 | 0.7 | 6 | 4.6 | 10~28 | 0.2~0.6 | 6~15 | 50~68 | 3 | 0 |
| SMGs | 2.5 | 4.5 | 0.14 | 0.93 | - | 30 | 4 | 20~25 | 0.4~0.81 | 6~15 | 54~70 | 4 | 1 |
| SniperRifle | 7.35 | 12 | 0.05 | 1.35 | 2.3 | 6 | 4 | 68~90 | 2~4.9 | 18~32 | 84~120 | 4 | 0 |

## 8. Glossary

| CE field | Meaning | Unit | Where used in code | Typical range |
| --- | --- | --- | --- | --- |
| `damageAmountBase` | damage per projectile | hit points | ProjectileCE.DamageAmount, suppression | 8 to 20 small arms, 39 to 105 heavy |
| `armorPenetrationSharp` | sharp penetration | mm RHA | ProjectileCE.PenetrationAmount, ArmorUtilityCE | 3.5 to 7 industrial rifles, 15 to 33 spacer |
| `armorPenetrationBlunt` | blunt penetration (and the blunt remnant after deflection) | MPa | GetDeflectDamageInfo | 10 to 70 small arms, hundreds for heavy |
| `speed` | muzzle speed | cells per second (5 m per cell) | trajectory | 67 pistol, 142 to 168 rifle, 156 to 242 sniper |
| `pelletCount` | projectiles per shot | count | Verb_LaunchProjectileCE | 1; shotgun 4 to 9 |
| `spreadMult` | projectile scale of weapon spread | factor | spread term | 1; pellets higher |
| `secondaryDamage` | extra damage instances | damage def, amount, chance | armor and explosion code | HE, incendiary, EMP |
| `suppressionFactor`, `airborneSuppressionFactor`, `dangerFactor` | suppression scale | factor | ProjectileCE, CompSuppressable | default 1 |
| `ShotSpread` | inherent cone | degrees | accuracy | 0.01 to 0.17 |
| `SwayFactor` | wobble scale | factor | SwayAmplitude | 0.5 to 3.2 |
| `SightsEfficiency` | sights quality | factor | accuracy factor | 0.7 to 2.6 |
| `recoilAmount` | kick per shot | degrees | GetRecoilVec | 0.5 to 3.0 |
| `warmupTime` | aim time | s | verb | 0.6 to 2.3 |
| `ticksBetweenBurstShots`, `burstShotCount` | fire rate and burst length | ticks, shots | burst loop | 6 to 50 shots |
| `range` | maximum range | cells | EffectiveRange | 12 to 75 |
| `magazineSize`, `reloadTime`, `reloadOneAtATime` | magazine and reload | rounds, s, flag | CompAmmoUser | 5 to 300; 4 to 9.2 |
| `ammoSet` | caliber | AmmoSetDef | CompAmmoUser | 307 sets exist |
| `ammoClass` | round type | enum-like def | AmmoDef | 74 values |
| `Mass`, `Bulk`, `WornBulk` | kg, volume, worn volume | kg, bulk | MassBulkUtility, CompInventory | see tables |
| `StuffEffectMultiplierArmor` | apparel thickness | mm | stat part | 0.15 to 10 |
| `StuffPower_Armor_Sharp`, `_Blunt` | material armor per mm | mm RHA per mm, MPa per mm | stat part | steel 1 and 1.5, plasteel 2 and 3 |
| `ArmorRating_Sharp`, `_Blunt` | armor of a piece or part | mm RHA, MPa | ArmorUtilityCE | 0 to 28 sharp on fixed suits |
| `PartialArmorExt` | per body part multiplier or absolute rating | factor | PartialStat | 0.5 to 0.9 |
| `power`, `cooldownTime`, `armorPenetrationSharp/Blunt` (tool) | melee tool numbers | damage, s, mm RHA, MPa | Verb_MeleeAttackCE | power 2 to 40 |
| `MeleeCritChance`, `MeleeParryChance`, `MeleeDodgeChance` | weapon offsets | chance | Verb_MeleeAttackCE | 0.04 to 1.45 |
| `MeleePenetrationFactor`, `ToughnessRating`, `MeleeCounterParryBonus` | weapon level melee modifiers | factor, mm RHA, factor | melee code | 1 to 1.25 for stuff |

## Implications for RimStudio

1. The item designer must read CE data from the user's own installation at runtime through the shared def pipeline (merge, patch, inheritance, `LoadFolders`), never from files in the repository. A resolved-def reader that handles `PatchOperationMakeGunCECompatible` is required: either implement its documented merge (about 100 lines of logic, `ce_ops.py` is a working reference) or read the weapon after the game's own loader has run. Test: the patched revolver resolves to Mass 1.39, Bulk 2.79, ShotSpread 0.16, SwayFactor 1.39, recoilAmount 2.96, magazine 6, reload 4.6.
2. Model the designer's inputs as a small set of CE quantities with units: Mass (kg), Bulk, Cooldown (s), SightsEfficiency, ShotSpread (degrees), SwayFactor, recoilAmount (degrees), range (cells), warmupTime (s), burst length and ticks between shots, magazine size, reload time, caliber (ammo set), and for armor thickness (mm) or fixed sharp (mm RHA) and blunt (MPa) ratings plus WornBulk. Validate units in the UI (mm RHA, MPa, kg).
3. Judge "what fits" per class, not globally: compute the distribution of each field for the user's own installed CE (class by weapon tags and ammo set), then score a proposed weapon by robust z-score or percentile against its class and flag fields outside the observed min to max. Mass and Bulk are only moderately correlated (rank correlation 0.56), so Bulk gets its own check; range and warm-up correlate at 0.81, so a range that jumps without a longer warm-up is a useful warning.
4. Offer a formula mode that mirrors CE's auto-patcher structure (class preset plus curves from the vanilla-style input to CE values) but with RimStudio's own constants derived from the class medians in section 7, recomputed from the user's install; do not store CE preset numbers in the repository. Test: for the six presets' classes, the formula mode's output for a vanilla-style rifle (range 31, warm-up 1.0, damage 11) lands inside the CE assault rifle range, spread and sway observed here.
5. Penetration needs a "meets armor" preview: implement the layer rules of section 5 (deflect when penetration is below the rating, damage times remaining penetration over penetration, blunt remnant with cube-root damage) and display, for the proposed round, the damage after typical armor tiers (synthread jacket 0.8 mm, steel vest 8 mm, plasteel vest 16 mm). Test vector: 5.56 NATO FMJ against 8 mm / 12 MPa delivers about 4.5 blunt damage; .45 ACP against the same vest delivers 0.
6. Treat projectile damage and penetration as a pair tied to the caliber: expose ammo type multipliers measured here (AP: 0.62 damage and 2.0 sharp penetration; hollow point: 1.26 and 0.5; sabot: 0.52, 3.5 and speed 1.36) as defaults when generating an ammo set, and keep blunt penetration and speed unchanged for AP and HP.
7. CE patch generation should emit a `PatchOperationMakeGunCECompatible` block when the target is a gun (statBases, costList, Properties, AmmoUser, FireModes, weaponTags) because it is the form CE's own patches use; validation inside RimStudio needs the same re-implementation as item 1, because a generic patch engine treats the class as unknown.
8. For apparel emit either stuffed form (StuffEffectMultiplierArmor in mm, categories such as Steeled for metals, SoftArmor for soft stuff) or fixed ratings; show the computed rating per material using the material table (steel 1 and 1.5, plasteel 2 and 3, synthread 0.2 and 0.05) read from the user's install. Add Bulk, WornBulk and layers as checked fields with class ranges (vest Bulk about 5 and WornBulk 3; helmet Bulk 4 and WornBulk 1).
9. Melee designer: warn when sharp penetration is under the lowest common armor rating (blades in CE sit under 1 mm RHA); require choosing crit, parry and dodge offsets from the class ranges; use the vanilla to CE power shift (long sword 23 to 40, club 14 to 11) as the sanity check.
10. Calibration quiz (optional mode): the questions should adjust only the class medians the designer scores against (for example "how deadly should a rifle round be against a steel vest": choose between deflecting and partially penetrating), because the measured distribution is tight inside a class and wide between tech tiers.

## Open questions

1. `CE_Math.CalculateHitPercent` and the lead and cover geometry were not read; reproducing the in-game hit chance in RimStudio needs that function (or a decision to show only the error components).
2. The origin of the blunt penetration values (no constant relation to damage times speed in the data) is probably in CE's projectile spreadsheet, which was not fetched; whether the designer can derive blunt penetration from bullet mass and speed is open.
3. Which capacity (35 kg per body size in code, or the `CarryWeight` stat default of 40) is effective for a standard colonist was not traced; it matters only for a Mass and Bulk "carry burden" readout.
4. The stat parts that turn stuff power into ArmorRating for apparel (multiplier stat `StuffEffectMultiplierArmor`) were read in the stat XML, not in game code; the product form is confirmed by the CE wiki and by the vest values (8 mm times steel 1 and 1.5 equals 8 and 12).
5. Weapon quality, attachments (`WeaponPlatformDef`), bipod stats and `CompUniqueWeapon` traits modify these numbers at runtime and are not modelled here; the designer's v1 should state that its numbers are base values.
6. 760 ModPatches folders were not analysed beyond counting; owner-made CE patches (the external drive mods) are the right second dataset for how modders deviate from the class medians.
7. The engine run uses default CE settings; users who enable generic ammo or change `PartialStat` get different resolved values (the settings class is read by `PatchOperationSettingsConditional`).
8. Only 24 vanilla or CE melee tool rows exist (Royalty persona weapons and others have no CE tools in this load), so melee distributions are thin; more samples must come from mod patches.
