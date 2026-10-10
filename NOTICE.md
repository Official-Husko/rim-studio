# Notices

RimStudio is an independent tool for RimWorld modders. It is not made, endorsed or supported by Ludeon Studios Inc. or by the Combat Extended team.

Data of two projects is included in this repository and in the application: definitions, values, names and tables of RimWorld and of Combat Extended. Their rights holders allow this on condition that the source is named and the official project is linked (decision D-211, [ADR 0059](docs/adr/0059-attributed-game-and-combat-extended-data.md)); this file does that. Art, audio and code of the two projects are not included.

## RimWorld

RimWorld is made by Ludeon Studios Inc. Official project: https://rimworldgame.com/

Portions of the materials used to create this content/mod are trademarks and/or copyrighted works of Ludeon Studios Inc. All rights reserved by Ludeon. This content/mod is not official and is not endorsed by Ludeon.

RimWorld and the Ludeon name and logos are marks of Ludeon Studios Inc. The end user licence agreement of the game is the file EULA.txt in the game folder.

## Combat Extended

Combat Extended is made by the CE Team. Official project: https://github.com/CombatExtended-Continued/CombatExtended (Steam Workshop: https://steamcommunity.com/sharedfiles/filedetails/?id=2890901044).

Combat Extended is used under the Creative Commons Attribution-NonCommercial-ShareAlike 4.0 International licence, https://creativecommons.org/licenses/by-nc-sa/4.0/, as the README of the project states. The data included here was read from the project and summarised in tables, fixtures and test outputs; values may be rounded, grouped or recomputed, so they are not the project's own files. The ShareAlike and NonCommercial terms of the licence apply to the Combat Extended data included here.

## Included data sets

Each set below is declared in `xtask/third-party.jsonc`, and `cargo xtask check-licences` fails when a data file that names a game or Combat Extended definition is outside a set, when a set matches no file, or when this notice lacks the name, holder, link or required text of a source.

| Set | From | Where | What |
|---|---|---|---|
| ui-fixtures | RimWorld, Combat Extended | `packages/testkit/fixtures/**` | Answers of the development bridge recorded from a real install: weapon, ammunition and class data used by the UI tests, the component gallery and the offline mock mode |
| research-data | RimWorld, Combat Extended | `docs/research/data/**` | Tables, figures and scripts behind the research notes, with statistics computed from definitions and values read from the two projects; not part of any build |
| engine-tests | RimWorld, Combat Extended | `crates/*/tests/**` | Test vectors and golden outputs of the engines and the designer; patch outputs name definitions and classes of the two projects |
| ui-strings | RimWorld, Combat Extended | `apps/desktop/src/locales/**` | Interface text that names definitions and classes of the two projects |
| documents | RimWorld, Combat Extended | `docs/**`, `README.md`, `NOTICE.md` | Specifications, design documents and research notes that quote names, values and tables (D-100) |

At run time the application reads the values it works with from the installation of the person who uses it, so they match that person's game and mod versions.

## Reference projects

RimSort (GPL-3.0, https://github.com/RimSort/RimSort) and RimCrow (MIT, https://github.com/Inky-Feather/RimCrow) were read for ideas and file formats, described in our own words. No code or data of either project is included. Community rules datasets are fetched at run time and are never bundled.
