# ADR 0019: Own VDF reader and the DetectionReport

Status: accepted | Last updated: 2026-10-04 | Register: D-037, D-038

## Context

R3 needs the install, Steam libraries and workshop content found through VDF and ACF files and other means. The steamlocate crate misses HKCU keys, workshop ACF files, Proton prefixes and provenance; keyvalues-parser fails on a BOM and on `#include`. Our own parser matched keyvalues on 28 of 28 real files.

## Decision

`rimstudio-steam` contains an own VDF/ACF reader, typed views, a `GameLocator` and a `DetectionReport`. All IO goes through `DetectEnv` ports. `steamlocate` 2.1.1 and `keyvalues-parser` 0.2.4 are dev-dependencies only, checked by xtask, used as test oracles. The report lists every candidate with a closed `how` string, a confidence and warnings; overrides persist in `workspace.jsonc` and are never silently dropped. A valid install needs `Data/Core/About/About.xml`, and the game version comes from `Version.txt`, not ModsConfig.xml.

## Consequences

- The UI can say why a path was chosen (found because).
- If M1 runs out of time, steamlocate may be a stopgap behind the trait (never in the release path by default).
- Flatpak Steam and a second library are in the manual test matrix.

## Alternatives rejected

- steamlocate in the release path.
- First hit wins detection.
- Game version from ModsConfig.xml.

## Evidence

- [Steam and game detection](../research/steam-and-game-detection.md) section 6 and implications 2 to 8
- [Mod format and corpus](../research/rimworld-mod-format-and-corpus.md)
