# RimStudio design brief

This is the written prompt for the visual design tool: a self-contained, in-depth brief for the RimStudio desktop application (a RimWorld mod manager and modding toolkit) covering product context, the Blueprint visual language with verified colour tokens, the application shell, every screen with its states, the component sheet, realistic sample data and microcopy, and an acceptance checklist for each design round. It is written to be pasted in parts: Parts A and B first, then the screen parts in batches. Everything here traces to the specifications ([UI inventory](ui-inventory.md), [mod manager](../features/mod-manager.md), [item designer](../features/items-toolkit.md) and the others) and to the verified architecture ([frontend architecture](../architecture/frontend-architecture.md)).

Status: draft | Last updated: 2026-10-04

## How to use this brief (for the owner; not part of the prompt)

1. Open a new design project. Paste **Part A** and **Part B** as the first message (they stand alone: nothing in them needs another part). Ask for one deliverable only: the design system sheet in dark (tokens, type scale, the motifs of B5, one mod row in the three densities, icon sample). The shell and the manager need Parts C and D, so they come in message two.
2. Review each round with the checklist in Part H before moving on. Reply with specific changes in the form "Screen id, what is wrong, what you want" so the tool can iterate on one artboard at a time.
3. Message two: paste **Part C** (shell), **Part F** (sample data) and **Part G** (microcopy) once, plus the Round 1 briefs of **Part D**. After that, paste the screen briefs of Part D in batches of three or four, each batch with only the Part F and G pieces it cites. Each brief stands alone and repeats the ids it needs.
4. Ask for two divergent options only for the two hardest screens (the manager workspace and the item designer). Everything else gets one option plus its states as separate artboards.
5. Ask for dark first. Ask for light only after the dark system is approved, because light is a token swap, not a redesign (the parity rules are in B1). Light is requested as "Round 1b": the design system sheet, the shell and the manager workspace in vellum.
6. When a round is approved, export the tokens (names in Part B.10) and the component sheet; they map one to one onto the Tailwind v4 theme described in the [frontend architecture](../architecture/frontend-architecture.md).
7. Do not paste the owner-facing notes (this section and the checklist in Part H) as design instructions.

---

## PART A: MASTER BRIEF (paste first)

### A1. The product in one paragraph

RimStudio is a desktop application for people who modify RimWorld, a colony simulation game with an enormous community mod scene. It is two tools in one shell. The **mod manager** shows every mod from the game folder, the Steam Workshop and any number of custom folders on one screen, lets the user arrange the load order, explains and fixes problems, and launches the game. The **modding toolkit** lets a modder explore and test the game's definitions (called defs), design new weapons and apparel with balance math that fits the vanilla game and the Combat Extended mod, check a mod before release, and publish it to the Steam Workshop. The interface must feel like a precise instrument: fast, calm, dense where it should be, and always able to say why it did what it did.

### A2. Who it is for

| Persona | Situation | Top jobs | What they fear |
|---|---|---|---|
| Mara, the heavy player | Runs 600 mods, not technical, plays on Linux and Windows | Enable and disable mods, sort, fix warnings, save, launch, share a list | Breaking a save; red errors at startup; not knowing which mod is the culprit |
| Jonas, the modder | Makes weapon and armour mods, writes patches for Combat Extended, publishes to the Workshop | Design an item, check what fits, generate a patch, run checks, publish an update | Uploading junk files; numbers that do not fit the game; losing work |
| Ling, the patch writer and translator | Reads and edits XML daily, mod names in several scripts | Find which mod changes a def, test a patch, see the resolved result | Silent overrides; unreadable provenance |

### A3. What makes it different (each point is a design obligation)

1. **It shows what the game will actually see.** RimWorld builds its mod list from exactly three places: the Data folder of the install (Core and the official expansions), the Mods folder of the install, and Steam Workshop subscriptions. Anything else, including a custom mod folder, is invisible to it until RimStudio links the mods in. The game also silently drops an active mod it cannot find and rewrites its list, which is why launching is blocked when a link is broken. "Visible to the game" is therefore a first-class state shown on rows, in detail panels and before launch, drawn with a hatch pattern when a mod is not visible.
2. **It explains every automatic decision.** Every sort move and every warning carries a provenance trail: which rule, from which layer (the mod's own About file, the community rules database, or the user's rules). "Why is X above Y?" has a visible answer in one click.
3. **It never blocks.** Progress lives in a task centre and inline, not in modal dialogs. Modals are only for destructive confirmations.
4. **Everything is undoable and every edit is visible as a diff.** A dirty marker shows how the list differs from what was saved.
5. **It is honest about uncertainty.** Balance suggestions come with P50 and P80 bands and a "fit meter", never with false precision.
6. **It is fast.** Lists hold 5,000 rows, search answers while typing, and the main list appears instantly from a cached snapshot with no spinner.

### A4. Platform and constraints

- Desktop application in a system webview (Windows, macOS, Linux; also Steam Deck in desktop mode at 1280 x 800 with touch). Minimum window 1280 x 800, comfortable at 1440 x 900, scales to 4K. Mouse and keyboard first, touch supported through a larger density mode.
- Lists are virtualised with fixed row heights: design rows at exact heights and never rely on variable-height rows except an expanded row.
- Light and dark themes, three density modes, a user accent colour, user themes. No external fonts or network assets at runtime; everything is self-hosted.
- Performance rules for the visual design: no backdrop blur, no large soft shadows, no per-row filters, no full-bleed images behind lists. Textures are CSS-only (gradients and hairlines).
- App data is JSON and settings are JSONC; there is no XML anywhere in the UI's own data. XML appears only as text the user is reading or editing (a def, a patch, a file preview).

### A5. Design principles

1. **Instrument, not toy.** Precision, restraint, exact alignment. The look is a technical drawing, not a game skin.
2. **Calm density.** Show a lot, but with strong hierarchy, hairline separation and generous alignment; never decorate a row.
3. **Blueprint is texture, not obstacle.** Grid, registration marks, dimension lines and hatch appear on canvases, headers, empty states and annotations. Never behind a list or text that must be read quickly.
4. **Machine values are monospaced.** Versions, package ids, paths, hashes, rule names and every number in the balance tools use the mono face with tabular figures.
5. **Colour carries meaning and never alone.** Every colour-coded state also has an icon, a label or a pattern. Red is reserved for error-severity problems (hard conflicts, things that break loading or block an action) and for destructive confirmations; advisory states such as warnings, unusual balance numbers and stale data use amber, blue or neutral.
6. **Keyboard first.** Every list action has a key; focus is always visible; a command palette reaches every action.
7. **Progressive disclosure.** The first view is simple (a list, a search, a Launch button); expert detail lives one click away (provenance, diffs, raw XML).
8. **Explain, then act.** Before any action that changes files outside RimStudio's own data (linking mods, saving the game's mod list, publishing), show exactly what will change.

### A6. Voice and tone

Calm, exact, never blaming, never cute. Sentence case everywhere. No exclamation marks. Say what happened, why, and what the user can do, in that order. Use the game's own terms (mod, load order, def, patch, Workshop) and spell out unfamiliar ones once.

### A7. Do and do not

| Do | Do not |
|---|---|
| Use the real sample data (Part F, pasted with the screen briefs) | Use lorem ipsum or invented mod names |
| Use hatch for "not visible to the game" | Use red for anything that is not an error-severity problem or a destructive confirmation |
| Keep lists plain, with 1 px hairline row separators | Put the blueprint grid behind a list |
| Express elevation with lines and surface tint | Use drop shadows, glass, gradients on controls, glow |
| Use at most three hues on one screen besides neutrals (accent, one signal colour, one more; the rule-layer colours, the quality ladder and the fit meter states are chip and marker colours that always carry a label and do not count) | Use text below 11 px or low-contrast grey on grey |
| Show provenance chips and diffs | Hide decisions behind icons without labels |
| Reuse the same component for the same job | Invent a new control for a one-off |
| Draw the game's items abstractly (icons, schematics) | Copy the game's art, logo, screenshots or fonts |

### A8. What to deliver, in rounds

| Round | Deliver |
|---|---|
| 1 | Design system sheet (dark first; light in Round 1b), application shell with the shell states SS-01 to SS-10, then SCR-02 manager workspace (two options), SCR-03 detail, SCR-04 explain drawer, SCR-05 sort preview, SCR-06 save preview, SCR-07 issues panel |
| 2 (ask in two batches) | 2a: SCR-01 first run, SCR-21 to SCR-25 settings, SCR-22 sources, SCR-15 game visibility, SCR-14 pre-launch, SCR-16 datasets. 2b: SCR-08 to SCR-10 duplicates and missing, SCR-11 to SCR-13 profiles and import and export, SCR-17 to SCR-20 rules, replacements, bisect, updates |
| 3 | SCR-26 log viewer, SCR-27 Def Explorer, SCR-28 patch tester, SCR-29 validator, SCR-30 to SCR-34 project tools |
| 4 | SCR-36 design form (two options) with fit meter and charts, SCR-38 quiz, SCR-35 item list, SCR-37 matrix, SCR-39 output preview and CE patch view, SCR-40 CE update and lint |
| 5 | SCR-41 to SCR-48 publisher, SCR-49 task centre, SCR-50 command palette, SCR-51 diagnostics and About, SCR-52 update dialog, SCR-53 shortcut map |

Each screen needs its states as separate artboards (empty, loading, populated, error, offline, extreme data) as listed in Part D. For the secondary screens (everything except SCR-02, SCR-03, SCR-36 and the Round 1 panels) the tool may draw the minor states on one "states sheet" per screen at half scale; the populated state and the error state always get their own full artboard. Every artboard is 1280 x 800 unless stated.

---

## PART B: VISUAL LANGUAGE "BLUEPRINT"

### B1. Concept

The application is a drafting table. The dark theme is a **cyanotype night**: deep blue paper, fine light-blue linework, white and pale-blue ink. The light theme is **vellum**: pale blue-white paper with blue ink. The vocabulary comes from engineering drawings: a ruled grid, registration marks at the corners of sheets, dimension lines with measured labels, a title block along the bottom edge, hatch for sections that are cut away or inactive, stamps for revision states, leader lines for callouts. These are used with discipline (see B5): the drawing language frames and annotates the work; it never competes with the lists and numbers the user came to read.

**Dark and light parity.** Light is the same drawing on vellum, not a different design: identical layout, spacing, motif placement and icon set; only the colour tokens change. Texture strength is tuned per theme through `--rs-grid-*` and `--rs-hatch` (already set below), never by adding or removing a motif. Light replaces any glow-like effect by a line. The first-run grid, the hatch and the stamps must look equally quiet in both themes; if a motif is louder in one theme, adjust its token alpha, not its recipe.

### B2. Colour tokens (verified contrast)

All values are final unless the contrast column says otherwise. Contrast is measured against the stated surface (WCAG 2.x ratio). "Text" tokens must reach 4.5:1 on every surface they are allowed on.

**Dark theme (cyanotype night)**

| Token | Hex | Role | Contrast notes |
|---|---|---|---|
| `--rs-bg` | `#081A31` | Application ground | base |
| `--rs-surface` | `#0D2440` | Panels, lists | base |
| `--rs-surface-raised` | `#122E52` | Raised panels, headers, inputs | base |
| `--rs-surface-hover` | `#193B68` | Hover and pressed surfaces | base |
| `--rs-text` | `#E8F2FF` | Primary text | 15.5 on bg, 13.8 on surface, 10.0 on the hover surface |
| `--rs-text-muted` | `#A9C3E3` | Secondary text | 9.7 on bg, 8.6 on surface, 6.2 on the hover surface |
| `--rs-text-faint` | `#7F9FC8` | Tertiary text, units, hints | 6.4 on bg, 5.7 on surface, 5.0 on the raised surface; do not use on the hover surface |
| `--rs-accent` | `#3FC8FF` | Primary action, selection, focus-adjacent | 9.1 on bg, 8.1 on surface |
| `--rs-accent-contrast` | `#04223A` | Text on accent fills | 8.4 on accent |
| `--rs-accent-tint` | accent at 10% on surface | Selected row fill (also bold name and a leading check or marker, never tint alone) | text 11.3, muted 7.0, faint 4.7, danger 4.6, rule-community 5.2 on it |
| `--rs-warning` | `#FFB84D` | Warnings | 9.1 on surface |
| `--rs-danger` | `#FF6B7A` | Error-severity problems and destructive actions only | 5.7 on surface, 5.0 on the raised surface; not on the hover surface |
| `--rs-success` | `#58E0AD` | Success, "visible to game" | 9.4 on surface |
| `--rs-info` | `#56B4E9` | Neutral information | 6.8 on surface |
| `--rs-rule-about` | `#56B4E9` | Rule layer: the mod's own About file | 6.8 on surface |
| `--rs-rule-community` | `#D98DBD` | Rule layer: community database | 6.3 on surface |
| `--rs-rule-user` | `#4FD1A5` | Rule layer: the user's rules | 8.2 on surface |
| `--rs-focus` | `#8BE3FF` | 2 px focus ring, 2 px offset | 12.1 on bg, 9.4 on the raised surface |
| `--rs-border-subtle` | `#78BEFF` at 18% | Row separators, decorative rules | 1.5 on surface (decorative only) |
| `--rs-border` | `#78BEFF` at 32% | Panel and card borders | 2.0 on surface |
| `--rs-border-strong` | `#78BEFF` at 55% | Control borders, active dividers | 3.4 on surface, 3.2 on the raised surface (meets 3:1 for controls); on the hover surface it is 2.8, so a hovered control changes its border to `--rs-accent` instead |
| `--rs-grid-minor` | `#78BEFF` at 6% | Canvas grid, 16 px | decorative |
| `--rs-grid-major` | `#78BEFF` at 11% | Canvas grid, every 80 px | decorative |
| `--rs-hatch` | `#78BEFF` at 22% | 135 degree hatch, 1 px lines every 6 px | decorative |

Quality ladder (item designer previews), all on surface: awful `#8A94A6` (5.1), poor `#A8B3C4` (7.4), normal `#E8F2FF` (13.8), good `#6FE0A8` (9.6), excellent `#5BB8FF` (7.3), masterwork `#C79CFF` (7.2), legendary `#FFC15C` (9.7). Tokens `--rs-q-awful` to `--rs-q-legendary`. The ladder is always paired with the quality name.

**Light theme (vellum)**

| Token | Hex | Role | Contrast notes |
|---|---|---|---|
| `--rs-bg` | `#F2F6FB` | Application ground | base |
| `--rs-surface` | `#FFFFFF` | Panels, lists | base |
| `--rs-surface-raised` | `#E8EFF8` | Raised panels, headers, inputs | base |
| `--rs-surface-hover` | `#DCE7F4` | Hover and pressed | base |
| `--rs-text` | `#0B1E36` | Primary text | 15.4 on bg, 16.8 on surface |
| `--rs-text-muted` | `#38557A` | Secondary text | 7.0 on bg, 7.6 on surface, 6.1 on the hover surface |
| `--rs-text-faint` | `#4B668C` | Tertiary text | 5.4 on bg, 5.9 on surface, 4.7 on the hover surface |
| `--rs-accent` | `#0A63C9` | Primary action, selection | 5.3 on bg, 5.8 on surface |
| `--rs-accent-contrast` | `#FFFFFF` | Text on accent fills | 5.8 on accent |
| `--rs-warning` | `#9A5200` | Warnings | 5.4 on bg |
| `--rs-danger` | `#B91C32` | Error-severity problems and destructive actions only | 5.9 on bg |
| `--rs-success` | `#0A7350` | Success, "visible to game" | 5.4 on bg |
| `--rs-info` | `#1B6699` | Neutral information | 5.7 on bg |
| `--rs-rule-about` / `-community` / `-user` | `#1B6699` / `#A23B86` / `#09694A` | Rule layers | 5.7 / 5.5 / 6.2 on bg |
| `--rs-focus` | `#0A63C9` | Focus ring | 5.3 on bg |
| `--rs-border-subtle` / `--rs-border` / `--rs-border-strong` | `#143C78` at 22% / 38% / 60% | Separators, borders, control borders | 1.5 / 2.1 / 3.5 on surface; strong is 3.3 on the raised surface |
| `--rs-grid-minor` / `--rs-grid-major` / `--rs-hatch` | `#143C78` at 6% / 11% / 20% | Canvas texture | decorative |

**Colour rules.** Red (`--rs-danger`) is used only for error-severity diagnostics (a hard conflict, a missing dependency, an unresolved id, a blocked save or launch), for the insertion line that warns a drop breaks a hard rule, and for destructive confirmations. Amber (`--rs-warning`) is for warnings the user may knowingly accept and for numbers that are unusual. Blue (`--rs-info`) is for information and for "plausible" balance values. Green (`--rs-success`) is for confirmed good states, "visible to the game" and "typical" balance values. The three rule-layer colours appear only on provenance chips, graphs and the rule editor. A user accent replaces `--rs-accent` only; it never replaces warning, danger or success. The fit meter never uses red. Every state keeps its icon and label. Dataset statuses "rejected", "failed" and "stale" use amber, never red, because the last good copy is kept.

**Accessibility rules that complete the table.** (1) Every figure above was recomputed with the WCAG 2.x relative luminance formula; the light selected-row tint (accent at 10% on white) keeps text 14.5, muted 6.6, faint 5.1, accent 5.0, danger 5.5. (2) Non-text contrast: control borders, focus rings, chart markers and icons that carry meaning need 3:1 against their surface. (3) `--rs-text-faint` is never used on the hover surface in dark, and never for anything the user must act on. (4) Under `forced-colors: active` the tints, hatch and grid vanish: every state must still read from its icon, label and border (rows get a 1 px system-colour outline when selected; focus uses the system focus colour). (5) Colour-never-alone pairs: severity = icon shape plus label (circle for error, triangle for warning, square for information); fit meter = icon shape plus word; rule layer = chip with a letter (A, C, U) plus name; visible to game = link glyph versus hatch-link glyph plus word; diff = `+` and `-` characters; quality = name beside the colour.

### B3. Typography

Three families, all open licence, self-hosted, latin subset by default: **Barlow** (interface text), **Barlow Condensed** (labels, section headers, title-block cells, tool rail captions), **Azeret Mono** (every machine value). CJK and other scripts fall back to the system font; designs must be shown with mixed-script names.

| Style | Face and size | Use |
|---|---|---|
| Display | Barlow Condensed 600, 28 px, tracking 0.02em | First-run and empty-state titles |
| Section header | Barlow Condensed 600, 12 px, uppercase, tracking 0.08em, `--rs-text-muted` | Panel headers, drawer sections, title block labels |
| Title | Barlow 600, 16 px | Dialog and drawer titles, mod name in detail |
| Body | Barlow 400, 13 px, line height 1.4 | Default text, row names |
| Body strong | Barlow 600, 13 px | Row names when selected or in focus |
| Small | Barlow 400, 11.5 px | Secondary lines, helper text |
| Mono data | Azeret Mono 400, 12 px, tabular figures | Versions, ids, paths, hashes, numbers, rule names |
| Mono small | Azeret Mono 400, 11 px | Dense tables, chips |
| Mono display | Azeret Mono 500, 20 px | The one big number on a card (for example DPS) |

Truncation: single-line names truncate in the middle for paths and at the end for names, with the full text in a tooltip and in the detail panel. Never wrap a list row. Numbers are right-aligned in tables with units set in `--rs-text-faint`.

### B4. Space, size, density, shape

- Base unit 4 px. Panel padding 12 px. Gutter between panels 1 px hairline (panels are separated by lines, not gaps).
- **Row heights (density, root attribute `data-density`)**: Compact 28 px, Comfortable 34 px (the default from 1920 px wide and the height the owner's earlier designs used), Roomy 40 px (touch, accessibility and the Steam Deck). Row text stays 13 px (14 px in Roomy). Icons 16 px in rows, 20 px in toolbars. Row height never changes with content.
- Control heights: 28 px (Compact), 32 px (Comfortable), 40 px (Roomy). Minimum pointer target: nothing below 24 px in any density, 32 px for rail and toolbar buttons, 40 px for primary controls in Roomy.
- Radii: 2 px for rows, chips and inputs; 4 px for buttons, cards and menus; 0 px for the title block and panels. Nothing is pill-shaped except toggle switches.
- Borders: 1 px everywhere; 2 px for the active tab underline and the focused panel edge. Draw on whole device pixels (no blurry half pixels).
- Elevation: no blur shadows. Raised elements (menus, popovers, drawers) use `--rs-surface-raised`, a `--rs-border-strong` border and a 1 px offset solid shadow line (`0 1px 0 var(--rs-border-subtle)`); the scrim behind modals is `--rs-bg` at 70%.
- Focus: a 2 px `--rs-focus` ring with 2 px offset on every interactive element, always visible on keyboard focus. Inside lists, tables, trees and any scroll container the ring is drawn inset (offset -2 px) because the container would clip an outer ring; on accent-filled buttons the ring keeps its 2 px offset gap in `--rs-bg`, and on notched shapes (B5 section cut) the ring is drawn on an unclipped wrapper as a plain rectangle. Mouse clicks do not show the ring (`:focus-visible` only). Minimum text size anywhere is 11 px, including title block labels and chips.
- Chrome heights at the floor: title bar 32 px, top bar 48 px, tab strip 32 px (only when tabs exist), banner 36 px, status strip 32 px. At 1280 x 800 the workspace therefore has 656 px (624 with tabs), and a list shows about 14 Comfortable rows under a 96 px list header.

### B5. The Blueprint motifs: recipes and where they are allowed

Every motif is cheap CSS (gradients, borders, pseudo-elements, inline SVG). None may require an image file.

| Motif | Recipe | Use | Do not use |
|---|---|---|---|
| Canvas grid | Four layered `linear-gradient` lines: minor 16 px (`--rs-grid-minor`), major 80 px (`--rs-grid-major`) | Behind empty states, the first-run wizard, the item designer chart canvas, the explain graph, panel headers at 40% strength | Behind lists, tables, text editors, forms |
| Registration marks | Four L-shaped ticks, each arm 8 px long and 1 px thick in `--rs-border-strong`, drawn inside the corner and flush with the border (they consume no layout space and never sit outside the sheet) | The workspace sheet, the item designer card, dialogs, mod preview frames (arm 5 px) | On every panel; inside lists |
| Dimension line | A 1 px line with 5 px end ticks and a centred mono label on a `--rs-surface` notch | Annotating a comparison (DPS difference, a range, a width), the fit meter, chart callouts | As decoration |
| Ruler | A 4 px track with minor ticks every 5% of the range (4 px long) and major ticks every 25% (8 px long, mono 11 px labels below the track); reference marks (pool minimum, p10, median, p90, maximum) are taller 12 px ticks with labels above the track; the whole control is 32 px high (input row) plus 28 px (ruler), 60 px in total | Every numeric slider in the item designer; the quiz bins | Plain settings sliders |
| Title block | A strip 32 px high of cells separated by 1 px `--rs-border` lines, no outer radius; each cell a label (Barlow Condensed 11 px uppercase, `--rs-text-muted`) above a value (Azeret Mono 12 px): game version, mods active of total, scan time, datasets age, task count | The application status strip, and the readouts panel of the item designer option "Sheet" (cells in a two-column grid) | Any other footer or panel |
| Hatch | `repeating-linear-gradient(135deg, var(--rs-hatch) 0 1px, transparent 1px 6px)` | Mods not visible to the game, inactive sections, disabled drop zones, "cut away" regions of a diff | Behind readable text without a solid chip over it; across a whole list row |

| Stamp | An unfilled label 20 px high, 0 px radius, 1 px border and text both in one state colour (neutral by default), Barlow Condensed 11 px uppercase, tracking 0.1em, 6 px side padding, placed at the top right of its card aligned with the card title; no rotation in functional UI. A stamp is not a chip: chips are filled with a 2 px radius and live inside rows and fields | Revision and state labels (DRAFT, CE READY, PRIVATE, EXPERIMENTAL), at most one per card | Rotated or repeated as decoration (rotation of 2 degrees only on empty-state illustrations); inside rows |
| Leader line | 1 px elbow line from an annotated element to a callout label | The explain drawer pointing at the two rows being compared; chart annotations | Between unrelated panels |
| Section cut | The top right corner clipped at 45 degrees, 6 px legs (`clip-path` polygon) | Active tab and the primary Launch button | Everywhere else |

**How hatch and the "Not visible to the game" chip combine.** In a list row only the 20 x 20 px source-icon slot gets a hatch tile behind the hatch-link glyph; the row, the name and the other slots stay plain. The chip "Not visible" (in the detail panel, the sources editor and the issues panel) is a 20 px tall chip with a solid `--rs-surface-raised` fill, a 1 px `--rs-border-strong` border, a 16 px hatch tile at its left end and the label on the solid part, so text never sits on hatch. "Visible" is the same chip with a link glyph, a green label and no hatch. A hatched region elsewhere (a disabled drop zone, a missing image) is a 6 px pitch field with a solid chip over any label. Hatch and grid are each one `background-image` gradient on one element per region, never one per row; in a list at most the small source-icon slot or a chip end-cap of a few dozen visible rows carries a hatch fill.

**Provenance chips in a row and in a trail.** A rule-layer chip is 18 px high (inside 28 to 40 px rows only as a single letter chip, 18 x 18 px: A, C or U, in the layer colour with a 1 px border and `--rs-surface` fill); in the explain drawer and the detail panel it widens to letter plus name ("A About", "C Community", "U User", "D Derived" in neutral). A ProvenanceTrail is a vertical list of steps joined by a 1 px line with a 6 px node per step; each step has the layer chip, the rule in mono, and the dataset date in `--rs-text-faint` (never the only carrier of meaning).

### B6. Iconography

Stroke icons on a 16 px and 20 px grid, 1.5 px stroke, square caps and mitred joins (technical drawing feel), single colour from the text tokens or the signal colours. A custom set of about 60 glyphs is needed; the manager needs source marks (game, official content, Workshop, custom folder), runtime kind (code, XML only), tags, groups, link (visible to game), hatch-link (not visible), warning, hard conflict, replacement, update, new, lock, undo, redo, sort, diff, rule layers. Every icon-only button has a tooltip and an accessible name.

### B7. Motion

Short and functional. Hover and press 100 ms ease-out. Drawers and panels 180 ms ease-out. Stepper and tab content 160 ms fade plus 4 px shift. Chart strokes draw on in 200 ms. Row moves after a sort animate over 200 ms only for the visible rows, with a brief tint on moved rows. Respect reduced motion by removing all movement and keeping instant state changes. No looping animation except an indeterminate progress line. Every transition uses only `transform` and `opacity`: drawers and the post-sort row move slide with `transform`, never by animating width, height, top or left, and a row move animates only the rows currently on screen (rows outside the virtual window are not animated).

### B8. Data visualisation language

Charts follow the drafting vocabulary: axes in mono, minor and major grid at canvas strength, data in `--rs-accent` and the signal tokens, reference clouds in `--rs-text-faint` at 55% opacity, the user's item as a larger accent marker with a dimension line to its nearest reference. Error bands (P50 and P80) are drawn as nested translucent bands with hatch on the outer band. The fit meter is a horizontal ruler with the P50 and P80 bands shaded and a caret for the value; the caret and its label read green "typical" inside P50, blue "plausible" inside P80 and amber "unusual" outside, each with its own icon shape, and never red. Always label axes with units. Never rely on colour alone: markers differ by shape. Charts hold at most 200 points (reference pools are 19 to 126 items): draw them as a faint cloud with the user's item as the one larger marker.

### B9. Imagery

Mod preview images are 16:9 thumbnails in a 1 px frame with registration ticks at the corners; a missing image is a hatch field with the mod's initials in Barlow Condensed. Thumbnails in lists and the detail panel are at most 256 px wide and decoded lazily; the frame, the registration ticks and the hatch fallback are drawn with CSS around or instead of the image, never as a raster. Do not use the game's art, logo or screenshots anywhere. Illustrations for empty states are line drawings in the blueprint style (a sheet with a ruler, a folder with a link, a magnifier over a def), 1.5 px stroke, `--rs-text-faint`, drawn in a 160 x 120 px box above the title, with one dimension line and at most one accent-coloured element; they sit on the canvas grid (B5), are centred in the empty region, and are the only place the 2 degree stamp rotation is allowed. Under 700 px of free height the illustration is dropped and the text stays.

### B10. Token names for implementation

Export the design as CSS custom properties with exactly these names (they map to Tailwind v4 utilities through the `@theme` block, so the names are the contract): `--rs-bg`, `--rs-surface`, `--rs-surface-raised`, `--rs-surface-hover`, `--rs-text`, `--rs-text-muted`, `--rs-text-faint`, `--rs-accent`, `--rs-accent-hover`, `--rs-accent-press`, `--rs-accent-contrast`, `--rs-accent-tint`, `--rs-warning`, `--rs-danger`, `--rs-success`, `--rs-info`, `--rs-rule-about`, `--rs-rule-community`, `--rs-rule-user`, `--rs-q-awful`, `--rs-q-poor`, `--rs-q-normal`, `--rs-q-good`, `--rs-q-excellent`, `--rs-q-masterwork`, `--rs-q-legendary`, `--rs-focus`, `--rs-border-subtle`, `--rs-border`, `--rs-border-strong`, `--rs-grid-minor`, `--rs-grid-major`, `--rs-hatch`, `--rs-font-sans`, `--rs-font-display`, `--rs-font-mono`, `--rs-row-h`, `--rs-control-h`, `--rs-radius-sm`, `--rs-radius-md`, `--rs-radius-lg`, `--rs-scrim`, `--rs-diff-added`, `--rs-diff-removed`, `--rs-band-p50`, `--rs-band-p80`, `--rs-source-official`, `--rs-source-install`, `--rs-source-workshop`, `--rs-source-custom`, `--rs-group-1` to `--rs-group-8` (theme-aware swatches for groups and mod colours), `--rs-chart-1` to `--rs-chart-6`, `--rs-dur-fast` (100 ms), `--rs-dur-base` (180 ms), `--rs-dur-slow` (200 ms), `--rs-z-menu`, `--rs-z-drawer`, `--rs-z-dialog`, `--rs-z-toast`. Density switches `--rs-row-h` and `--rs-control-h` through a data attribute on the root; theme switches the colour tokens through another. No colour value may appear in a component outside these tokens. The names `--rs-bg`, `--rs-surface`, `--rs-surface-raised`, `--rs-border`, `--rs-text`, `--rs-text-muted`, `--rs-accent`, `--rs-accent-contrast`, `--rs-danger`, `--rs-warning`, `--rs-success` and `--rs-info` are the base vocabulary the application already uses; every other token above is a Blueprint extension that implementers add to the theme allow list. Accent hover, press and tint are derived in the token file from `--rs-accent` with `color-mix()` and exported as variables with those names, so a user accent works without further tokens: show them in the design as derived values. Border and grid tokens that carry transparency are exported as 8-digit hex or `color-mix()` values. The three font tokens are not themable by users (a family name is neither a colour nor a length).

---

## PART C: APPLICATION SHELL (paste after Parts A and B)

The shell is static and paints before any data arrives. Design it at 1280 x 800 first (the floor), then scale up. Every region below is a slot.

### C1. Regions

| Id | Region | Content and behaviour |
|---|---|---|
| SH-01 | Window frame | Custom title bar where the platform allows it (window controls on the platform's side, product name RimStudio in Barlow Condensed), otherwise the operating system's bar. Thin corner ticks may frame the main sheet, never the window edge. |
| SH-02 | Tool rail (left) | One icon plus label per tool: Mod manager, Workspace (Def Explorer, patch tester, validator), Project, Designer, Workshop, Logs, Settings. Icon mode 56 px, labelled mode about 200 px. A tool whose requirement is missing (no game install, no open project, Combat Extended not installed, no Steam helper, no log file) is shown disabled with the reason as tooltip and a link to the fix, never hidden. The active tool has an accent bar; pages stay mounted so scroll and selection survive switching. |
| SH-03 | Top bar | Installation picker (the active RimWorld install with its full version label, for example "1.6.4871 rev598"), profile switcher with a Modified marker and a diff button, search field and command palette entry (Ctrl+K), status chips (Steam, datasets freshness: Fresh, Updating, Stale, Offline, Error; Game running), and the Launch button (Save and Run, Ctrl+Enter), the single primary button of the bar. Launch explains itself when disabled (game running, blocking diagnostic, no install). Pickers are popovers, not modals. At 1280 px wide the search field is 240 px, the status chips collapse to icon-only with tooltips, the installation picker shows only the version label, and Launch keeps its label. The rail command "Save and Run" is the same command as Launch: it is drawn as an accent-outlined icon button so the bar keeps the one filled primary button. |
| SH-04 | Tab strip | Document tabs for tools that need them (several projects, several Def Explorer queries), closable, with a dirty marker. |
| SH-05 | Workspace area | The active tool's page. Manager: four columns. Toolkit tools: list or tree on the left, main pane, detail pane on the right, diagnostics strip at the bottom. No horizontal page scroll at 1280 px. |
| SH-06 | Status strip (title block) | One fixed-height line styled as a drawing title block: cells separated by 1 px lines, each a tiny condensed label over a mono value. Fixed 32 px high, full window width under the rail and workspace, cells 104 to 200 px wide (eight cells fit at 1280). Cells: GAME (version and detection state), MODS (active of total), SCAN (idle with last duration, or a thin indeterminate line while refreshing), ISSUES (errors and warnings), DUPLICATES (count, opens the resolver), UNSAVED (changes and undo depth), DATASETS (age), TASKS (running count and the newest task's progress). Every cell is a button. Below 700 px window height it folds into the top bar. |
| SH-07 | Task centre drawer | Bottom or right drawer from the status strip: each job with name, phase, determinate or indeterminate bar, elapsed time, Cancel and Details; finished tasks stay for the session with outcome and Retry. Never blocks input. |
| SH-08 | Toasts | Bottom right, at most 3 visible, one action each (Undo, Show, Retry). 5 s default, warnings 8 s, errors persist. |
| SH-09 | Command palette | Centred overlay: fuzzy search over commands, mods and settings, recent first, shortcut hints, parameterised commands (go to mod, switch profile, add folder), unavailable commands shown disabled with the reason. |
| SH-10 | Dialog layer | Native dialogs wrapped in the design system; see the modal policy below. |
| SH-11 | Banner slot | Under the top bar: info, warning and danger banners with one action each ("The game list changed outside RimStudio", "Settings will not be saved"). |

### C2. Modal policy

Modals are only for destructive confirmations: permanent delete when no trash exists, reset to vanilla, remove all links, discard unsaved changes on quit, delete a profile, reset all settings (typed confirmation), restore a backup, "Launch anyway" after a block (typed confirmation), and the publish confirmation. Everything else is a non-modal panel over the relevant column, or a drawer: sort preview, save preview, missing dependencies, duplicates, import and export, diagnostics, and the item designer's quiz (a stepper over the form, not a gate). Progress is inline or in the task centre, never a blocking window. Dialogs trap focus and return it to the opener.

### C3. Density and responsive behaviour

| Width | Layout |
|---|---|
| 1280 to 1439 (the floor) | Tool rail 56 px, detail 280 px, actions rail 48 px (icons; 160 px when labelled), so the two lists share the remaining 896 px (about 446 px each after hairlines; 518 px each at 1440). Fixed right-aligned slots for icons and diagnostics; names truncate. Draw it in Comfortable (34 px); Compact is a variant. Design this first: no clipped controls, no overlapping badges, no horizontal scroll. |

Row slot budget at 446 px (Comfortable, Active list): group bar 3, padding 6, position 32, source icon 22, content icon 22, name flexible, version chip 56 (mono 11 px, truncates), state badges 24 (one icon-only slot showing the highest-priority state, "+n" when more), diagnostics 34, overflow button 24 (visible on hover or focus; its slot is always reserved), gaps 8. That leaves about 215 px for the name, roughly 32 characters; longer names truncate with an ellipsis. The Inactive list has no position slot (about 247 px for the name). Tags appear only from 1920 px wide; Compact hides tags and shows the version chip only on duplicates. Draw one row at every density with the slots outlined to prove nothing collides.
| 1440 to 1919 | Four columns, detail 300 px (resizable 240 to 480). |
| 1920 to 2559 | Rail labelled (about 200 px), Comfortable density, tags and version chips fit on most rows. |
| 2560 and above | Same columns; extra width goes to the lists, never to the detail beyond 480 px; the issues panel may dock on the right. |
| 1100 to 1279 (unsupported, usable) | Detail becomes an overlay drawer. |
| Below 1100 | The lists become tabs (Inactive, Active); a move is a button or key instead of a drag. |
| 200% zoom at 1280 x 800 | Falls back to the drawer layout. |

Steam Deck (1280 x 800, touch): Roomy density with 40 px primary targets, every drag has a keyboard equivalent, native dialogs, no custom title bar in game mode.

### C4. Shell-level states to draw (each a banner or a screen, in both themes)

| Id | State | Presentation |
|---|---|---|
| SS-01 | First run | Full-window wizard replaces the workspace; rail and top bar dimmed and inert |
| SS-02 | Datasets offline | Chip reads "Offline" in neutral styling (never error styling when a cached copy exists); status strip says "using last copy from 2026-10-01" |
| SS-03 | Game running | "Game running" chip; Save, deploy and Launch disabled with the reason "RimWorld is running"; list editing still works |
| SS-04 | Safe graphics (Linux) | Small persistent chip in the status strip (the crash-marker choice that leads here is a native dialog drawn before the web UI exists, so it is not designed) |
| SS-05 | Outdated system webview | Static full-page notice naming the minimum engine and what to update, no shell. It is drawn with plain CSS only (system font, literal hex colours, no custom properties, no `@layer`, no `color-mix()`, no `:has()`), because the engine that shows it may lack those features |
| SS-06 | Crash notice | Banner after restart: "RimStudio closed unexpectedly last time" with Copy diagnostics and Open logs; a render failure in one feature shows an error card in that region only |
| SS-07 | Update available | Quiet chip in the top bar; the update dialog only on click, never over a running task or an unsaved list |
| SS-08 | Backend unavailable | Full-screen banner with Restart |
| SS-09 | Read-only data folders | Banner "Settings will not be saved" with Open data folder |
| SS-10 | Modified, unsaved | Modified marker in the top bar and a dot on Save |

Draw each state as the shell at 1280 x 800 with the manager populated behind it (except SS-01, SS-05 and SS-08, which replace it), one artboard per state, dark first. Banners stack under the top bar (SH-11) at most two at a time; the more severe one is on top.

---

## PART D: SCREEN BRIEFS

Each brief gives: purpose, layout, content (with real data from Part F), the states to draw as separate artboards, and the key interactions. Draw every screen in dark first. The ids (SCR-nn) match the [UI inventory](ui-inventory.md), which lists the requirement ids behind each. The universal state set is: empty, loading (skeletons, never a blocking spinner), partial, populated, error (card with a short cause, one fix button and "Copy diagnostics", never a raw stack trace), offline (neutral styling when a cached copy exists) and extreme data (5,000 rows, 51-character names, mixed Latin and CJK names, 41-character package ids).

### Round 1: the manager core

**SCR-02 Manager workspace (draw two options)**
Purpose: the home screen: arrange which mods are active and in what order, understand problems at a glance, launch. Layout at 1440 x 900 and at 1280 x 800 (widths in C3: detail 300 or 280 px, lists 518 or 446 px, actions rail 48 px): one framed sheet (registration marks at its corners) with four columns: Detail (collapsible with Ctrl+B) | Inactive list | Active list | Actions rail (icons, expands to labels). Each list header is 96 px: row one title and count, row two search field and sort menu, row three filter chips and the clickable counters (counters collapse to icon plus number at the floor). Each list has a header: title, count ("4,312 of 5,000"), search field with filter chips, sort menu, and clickable counters (errors, warnings, new, updated, offline, duplicates). Rows are 34 px with fixed slots from left to right: group bar (3 px, group colour), position number (Active only, mono), source icon (official content, install Mods folder, Workshop, custom folder), content icon (contains code, or XML only), name with tags, version chip, state badges (hatch-link "Not visible to the game", Offline, New, Updated, Pinned), diagnostics icon with count, overflow button on hover or focus. Group headers are collapsible rows with a count and diagnostics totals. The Actions rail holds Save (dot when dirty), Save and Run (primary), Sort, Undo, Redo, Refresh, Clear, Profiles, Import and export, Tools, Palette.
Option A, "Ledger": the two lists side by side as equals with a 1 px centre line (no gap). Option B, "Stack": the detail is an overlay drawer (300 px, opens on selection or Ctrl+B), the Active list is wide on the left (about 700 px at 1280, so tags and a second slot for state badges fit) and the Inactive list is a narrower library pane on the right (about 476 px, collapsible to a 40 px strip with its count); the actions rail stays at the far right. Both must show every row state: default, hover, focus-visible (keyboard cursor), selected, multi-selected, dragging (ghost with the first row and a count badge), drop target with the insertion line in its three variants (allowed, breaks a hard rule in red with a one-line reason, refused with a message for official content), and a moved-by-undo outline.
Content: the real list head from Part F (Prepatcher, Harmony, six official packages, Adaptive Storage Framework, HugsLib, Vanilla Expanded Framework and more). One mod with three diagnostics (error, warning, info) so the badge shows the highest severity with a count.
After a sort is applied, a one-line strip stays at the top of the Active list ("Last sort: 14 mods moved, each names its rule. Why? Undo") until dismissed or the next edit, so every automatic move stays attributable. States: empty with no game ("RimWorld not found", Choose install), empty with no mods, only official content active (drag hint, Import list), empty filter ("No mods match"), loading (cached rows at once plus a thin refreshing line; a cold start fills progressively), partial, populated, error (scan failed with counts per source and Retry; a card when ModsConfig.xml is unreadable, listing the backups with Restore), offline source rows, extreme (5,000 rows, CJK names, 51-character names). Also draw the row sheet: one Active row and one Inactive row at each of the three densities with every slot filled, and the same row with the slots outlined.
Interactions: drag between lists with multi-select; Space and Return move the selection to the other list; Alt+Up and Alt+Down move within the Active list; Esc cancels a drag; rows never reorder by themselves when new data arrives.

**SCR-03 Mod detail panel**
Purpose: everything about the selected mod; a library summary when nothing is selected; a bulk bar for a multi-selection. Layout: 300 px column. Header: preview (16:9, 256 px wide, framed with registration ticks; a missing image is a hatch field with initials), name (wraps to two lines), source and state badges. Fields: package id (mono with copy), authors, version, supported versions as chips with the current game version highlighted, tags, group, colour, notes (editable, debounced autosave), path, links, times, folder size ("Calculating", then a value), dependencies and incompatibilities with satisfied, missing and inactive states and jump links, the rules that apply with layer chips, then a lazily loaded description (sanitised BBCode or plain text). Tabs: Overview, Relations (a lazy graph), Diagnostics, Files. The Files tab starts with an **overlap strip**: six 40 x 16 px cells in a row labelled Defs, Patches, Textures, Assemblies, Languages, Sounds (the mod's top-level content kinds), each filled by how many of its files are also defined or patched by other mods (empty, one to nine, ten or more: three tints plus the number inside), never red. Selecting a cell lists the shared files: file path in mono, the other mod, who loads later (and so wins), and for Defs the defName; an "intentional override" is data, not an error. The strip lives in the detail panel only, never in list rows. (The earlier approved brief showed conflicts as file-level overlap; the specs hold the facts for duplicate defs and patch overrides, see SCR-27. If the data for a cell is unavailable the cell shows a dash and "not indexed".)
States: nothing selected shows library counts per source, dataset freshness and the game version; multi-selection shows a count and bulk actions (enable, disable, tag, colour, group, pin); loading skeleton with the description last; partial (size calculating, image placeholder); error ("Could not read this mod's details", path, Copy diagnostics); extreme (20 authors, 4,000 characters of notes, a description with many images).

**SCR-04 Why-is-it-here drawer**
Purpose: explain a position or why mod A sits above mod B. Layout: a 360 px right drawer over the detail column. Header: the mod and its position ("Vanilla Expanded Framework at 14"), a sentence for the tier and reason, the binding predecessor and successor shown with leader lines to the two rows in the list, and the slack ("could sit anywhere from 11 to 17") drawn as a dimension line. Below, the chain: one step per row, "earlier, later, kind (chain, reverse, tier, dropped, none)", each with a rule-layer chip (About, community, user, derived), the rule text, the comment and the dataset date. Actions per step: Open source (About line, community rule read-only, user rule editable), Ignore this rule, Restore suppressed edge, Refresh when stale.
Example chain: "Harmony before HugsLib: About (force rule)"; "HugsLib before Vanilla Expanded Framework: community, dataset of 2026-09-12". States: no rule ("No rule: this position is your choice"), loading, partial (some sources unresolved), stale (offer Refresh), error, extreme (a chain of 12 steps with long names).

**SCR-05 Sort preview**
Purpose: preview, then apply, a sort. Layout: a panel over the Active column (not a modal). Header: "Sort preview: 14 mods would move", an algorithm switch (Tiers, Game style) and options (community rules, user rules, dependencies imply order, keep pinned, ask about missing dependencies). Body: a diff of moves grouped into blocks, each with before and after positions, names and a short reason chip; a block move reads "moved 40 mods". Below: dropped edges, cycles listed by mod name with rule source, tier conflicts, unmapped mods, remaining diagnostics. Footer: Apply (primary, one undoable command) and Cancel. Every moved row in the diff shows its rule-layer chip so each move is attributable before it is applied.
States: already sorted (a tick, "Nothing would move"), computing (progress and Cancel), partial (cycles shown as a result, never an error), error, extreme (600 of 610 moved: show summary counts first and "Show all").

**SCR-06 Save preview**
Purpose: show exactly what saving will write. A non-modal panel: diff against the saved list, the link plan (links to create or remove), the pre-launch checks that apply, backup status; buttons Confirm, Cancel, Copy diff. States: clean (nothing to save), game running (blocked with the reason), partial warnings, error (write failed, backup untouched, list stays dirty).

**SCR-07 Issues panel and diagnostics popover**
Purpose: all diagnostics for the working list. A row popover shows the first three messages with Mute and Fix. The panel groups by severity then code with counts; each item has the code in mono, the message with mod names in bold, and fix buttons (Enable dependency, Disable incompatible mod, Move after X, Use replacement, Link now, Reconnect and rescan); filters `has:error` and `is:muted`. Blocking codes (not visible to the game, source offline, unresolved id, Core inactive) are visually distinct (a lock icon and "blocks saving"). States: no issues (a tick), loading, partial (some codes muted), populated, error, extreme (400 diagnostics grouped, one mod with 9 messages).

### Round 2: setup, sources, data and housekeeping

**SCR-01 First-run wizard**
Purpose: show that detection already happened; at most three steps (Detected, Sources, Ready). The canvas grid is allowed here. Five cards (RimWorld install, Steam libraries, Workshop content, user data folder with ModsConfig.xml, game version), each with the path in mono, a "found because" line, a confidence chip (high, medium, low), validity marks (exists, readable, writable) and a segmented list of other candidates. Then a Mod folders row (detected sources and an Add folder drop zone) and a dataset opt-in sentence with a toggle; Ready shows counts and Open library.
States: loading (skeleton cards), partial (a card "Not found" that states the consequence: "launching is disabled"), populated (all found: one click), error (detection failed: Retry and Choose folder), offline (works; the dataset line says it will fetch later), extreme (two Steam libraries, native and Proton user folders, a non-ASCII path).

**SCR-22 Mod sources and custom folders editor (Settings)**
Purpose: add any number of custom mod folders. Ordered cards: built-in sources first (greyed handles), then custom folders in priority order with a drag handle. A card shows label, path (mono), kind chip, mod count, last scan, a reachability dot and a visibility chip ("Needs a link" drawn with the hatch, or "Linked (12 mods)"), and a menu. An Add folder panel shows the probe result; a Test result panel shows reachable, mod count by layout, scan depth, parse failures, overlap check, link plan preview and duration; an Advanced expander (layout, scan depth 1 to 4, watch, read-only, link mode, volume hint).
Example: the folder "My mods" on an external drive with 22 mods found at depth 2, and the owner's mod template repository flagged "not a mod". States: empty ("No mods yet": the three source kinds explained, Add a mod folder), offline drive (card greyed, "Find the drive"), overlap error with its code, an empty folder (suggest depth 2), extreme (8 folders, a 240-character Windows path). Remove never deletes files and says how many active mods would become missing.

**SCR-15 Game visibility (link) panel and SCR-14 Pre-launch checks**
Purpose: make custom mods visible to the game, and refuse a launch that would silently drop mods. The panel shows per custom folder the chip "Needs a link" or "Linked (12 mods)", a deploy plan with a dry run (every link to create or remove), an audit table of owned and unowned entries in the game's Mods folder, and actions Make visible, Repair links, Re-point drive, Remove all links (it only unlinks), Audit; an "Experimental" stamp. The pre-launch checklist lists Pass, Warn or Block per check (path valid, game not running, every active id resolves, link integrity, ModsConfig.xml writable and version, diagnostics, backup). The block dialog lists the unresolved ids with reasons and offers Fix automatically, Deactivate unresolved mods and launch, and Launch anyway (typed confirmation). Use messages 1 to 3 of Part G. States: nothing to link, planning progress, partial ("Applied 12 of 14, rolled back"), per-operating-system error messages, offline drive, extreme (22 mods across 3 folders, 8 unresolved ids across two drives).

**SCR-16 Datasets panel**
Purpose: show the five community datasets and their freshness. One card per dataset (Community rules, Steam Workshop database, Use This Instead, No version warning, RimWorld versions): purpose, a status chip (never fetched, ready, stale, updating, rejected, offline, failed, disabled), entry count, source and version, last checked, last changed, licence note. Actions: Refresh now, View changelog (added, removed, changed), Change source, Enable, Revert to previous, Open cache folder. Global switches and a list of what requests are made (nothing identifying). Every status renders a plain sentence and the next action, and never hides the last good copy.
States: never fetched ("Fetch now"), updating with bytes, partial ("3 records skipped"), a rejected download (amber, never red: "Update rejected: entries dropped from 631 to 12. Kept the last good copy."), offline (neutral, with the age), failed, all disabled.

**SCR-09 Duplicates resolver, SCR-08 Missing dependencies, SCR-10 Missing mods**
Duplicates: groups by package id; each copy shows source icon, path, version, modified date, size and whether it is active; the effective copy is marked with why it won ("pinned", "higher source priority", "matches game 1.6", "newer"); actions Pin this copy, Unpin, Open folder, Ignore this group. Never deletes or renames. Example: seven groups, three across roots. Missing dependencies: three groups (satisfied, available locally with the source icon, not installed with a Workshop link) with checkboxes, alternatives as options of one requirement, and Add selected, Add selected and sort, Sort without adding, Open Workshop pages, Ignore. Missing mods: a collapsible section above the Active list (not ghost rows inside the order) with name when known, Workshop id and status; actions Open Workshop page, Subscribe, Search in custom sources, Remove from list, and a "Keep and accept" checkbox that names the consequence (the game will drop it).

**SCR-11 Profiles and history, SCR-12 Import, SCR-13 Export**
Profiles: a top-bar popover (Switch, New from current, Duplicate, Rename, Delete, Compare with current, Set as default). History: a list of snapshots with time, note, counts and the delta against the previous one, plus a diff of a snapshot against the live list (added, removed, moved, newly installed, no longer installed). Import: a drop zone and paste box; the format is detected, never asked; preview with matched, missing, already active, to add, to remove, to move and unmatched lines; modes Replace, Merge, Add as new profile; a game version warning. Export: a side panel with live preview, options (inactive count, links, groups, paths), formats (ModsConfig style, RimStudio JSON, plain or Markdown report, Workshop id list, share code), Copy and Save as file.

**SCR-17 User rule editor, SCR-18 Replacements, SCR-19 Bisect, SCR-20 Updates**
Rule editor: a table per mod with rows Load after, Load before, Incompatible with, Load first, Load last; a layer column and comments; a toggle to show community and About rules read-only; suppressed rules; the ignore list of muted diagnostics; a rejected rule shows the cycle (edges and layers). Replacements: grouped by replaced mod with the replacement and the match basis; Replace (enable new, disable old, keep position) or Ignore; information severity only. Bisect: a setup (what stays fixed, a one-line problem note) and a round view (round number, suspects remaining, rounds left, answer buttons "Problem still happens" and "Problem is gone", Skip, Restore the original list). Updates: a list of Workshop mods with update and new badges, Open Workshop changelog, Update, Select all.

**SCR-21, SCR-23, SCR-24, SCR-25 Settings**
One full page: left navigation (Game and Steam, Mod sources, Datasets, Library and sorting, Appearance, Shortcuts, Launch, Tools, Performance, Diagnostics and data, About and updates, Import) and a content column with a search box on top. A setting row: label, control, one-sentence description, "Reset to default" (only when changed), a source tag ("set by command line"); edits apply immediately with a quiet "Saved" mark; no OK and Cancel pair. Appearance: theme (light, dark, system, user themes), accent (presets plus hex with a contrast check that refuses a failing accent and shows the numbers), density (three live previews), text size 80 to 160%, language, reduced motion, plus a live sample panel (a mod row, a diagnostic chip, a button, a card). Shortcuts: a searchable table with a chord recorder, conflict marks and reset. Advanced: launch method and arguments (live quoting errors), the four developer switches, safe graphics state (Linux), scan workers and watch policy, storage summary, Clear caches (never touches user data), restore from backup with a diff preview, and the Import from RimSort wizard.

### Round 3: toolkit

**SCR-26 Log viewer**
Counters as filter toggles (errors, exceptions, warnings, mod issues, info); identical blocks folded with counts; mod attribution as a link; filters (text, severity, mod, time); Previous and Next (F3, Shift+F3); a live-follow toggle; the raw block with its stack on the right. Actions: Copy, Copy block, Open mod folder, Disable mod (undoable), Bisect from this mod, Open in Def Explorer. States: "Game log not found" (expected path, Choose file), loading (first page under 300 ms, streaming), parse error with a raw view, extreme (100,000 lines, 2,000 identical blocks folded).

**SCR-27 Def Explorer**
Search any def and see parents, provenance and the resolved XML. A search bar with filters (type, mod, load folder, abstract, patched, overridden), a virtualised result list, a switch between Defs and **Overrides** (the conflict view: defs defined or patched by more than one mod, listed with the contenders in load order, the winner marked, and each contender's file path and patch operation index; overlap shown as data, not errors), and four tabs: Tree (parent chain up, children), Provenance (the defining file plus every patch operation in order, each with file and operation index, drawn as a ProvenanceTrail), Resolved (the final XML with a layer slider "as defined, after patches, after inheritance" and changed-field highlighting), References (def and texture edges). Actions: Copy as XML, Open file at line, Copy as patch starter, Compare with. States: empty ("Open a project or choose a reference set"), indexing progress with a "stale" badge, partial (unknown types, custom operations marked "not simulated"), error, extreme (13,212 vanilla defs; a 600-mod session; 12 Combat Extended overrides shown as "overridden" data, not errors). XML is shown in a mono code view; this is the only place raw XML appears besides the patch tester and the file preview.

**SCR-28 Patch tester, SCR-29 Validator**
Patch tester: left an operation editor with XPath highlighting and inline lints; right top the match list; right bottom the diff of the first match with a selector; gutter badges pass, fail, "not simulated"; Run, Run in load order here, Copy as file, "assume CE active". Validator: a left list grouped by severity and check with counts, centre rows with path and message, right detail with the offending XML line highlighted and the game-behaviour explanation; a scope switch (project, project plus conflicts, whole reference set); codes like author.*, defs.*, xpath.*, ce.*; Fix, Suppress per path (kept in reports as suppressed), Copy report.

**SCR-30 to SCR-34 Project tools**
Scaffolder: a three-step panel (what, where, options) with a live file tree preview and an About.xml text preview, ending with a checklist. About editor: form sections (identity, versions, dependencies, ordering, incompatibilities, description) on the left, raw XML on the right with the edited span highlighted, an issues strip, dependency rows with "pick from installed mods", description length counter and BBCode preview; Save shows a diff of exactly what will be written. Version folders and LoadFolders manager: a tree with version badges, a list of conditional blocks with condition chips, and an effective-files pane with active-mod simulation. Dev launcher: a compact panel with list source, four switches (dev mode shows a locked state if the game forbids it), an isolated save folder path with an inline check, and a big Launch that becomes "Running", then "Restore list and show log". Save inspector: a table of saves (name, game version, mod count, size, date) and the selected save's mod list with chips (active and matching, different position, installed but inactive, missing); "Activate exactly these mods" as an undoable manager edit. States for each of the five: empty (no project open, no saves found: say where it looked), loading, error (card with cause and fix), and the one extreme case that matters (a project of 2,000 files; 300 saves; a save with 400 mods).

### Round 4: item designer (the signature feature; give it the most care)

Context: the item designer helps a modder create a weapon or apparel that fits the game. It reads reference items from the user's own game install, computes live readouts with the game's own formulas, shows where the new item sits against reference items with honest bands, and writes a vanilla definition into the mod project; the designer always writes vanilla. A Combat Extended patch is optional: an off by default toggle "Add a Combat Extended patch (optional)" whose helper text says that the patch goes into its own files in a folder that `LoadFolders.xml` loads only when Combat Extended is active, that the vanilla definition is never changed and that nothing is added automatically even when Combat Extended is installed. A quiz is an optional way to get a better-fitting starting point.

**SCR-36 Design form with live readouts, fit meter and charts (draw two options)**
Layout: left, grouped fields (each shows its unit in mono, a source chip: typed, anchor, class median, answer; and the reference pool's p10, median and p90); right, live readouts: cycle time, nominal and hit-adjusted DPS, implied armour penetration, a strength index, armour-adjusted survival; for melee the two DPS values; for apparel coverage, an armour power index and a price breakdown; when the item's optional Combat Extended patch toggle is on, a Combat Extended tab with sustained DPS, mass, Bulk and a "meets armour" table (disabled with the reason when Combat Extended is absent). At the top the **fit meter**, and beside it the toggle "Add a Combat Extended patch (optional)", off by default, with the gated folder explanation. Below or beside, reference comparison charts: a scatter of two stats with reference items labelled and the new item highlighted, distribution strips, DPS against distance, and a rank readout ("heavier than 12 of 19 reference rifles"). Numeric sliders are **ruler sliders**: a text input plus a slider whose track is a ruler with ticks at the pool minimum, p10, median, p90 and maximum, labelled marks for the three nearest reference items, shaded P50 and P80 bands, and distinct markers for the current and suggested values.
Widths at 1280 x 800 (workspace 1224 px wide after the tool rail, 656 px high). Option A, "Cockpit": fit meter head pinned on top (56 px: share inside P80, reference count, typicality, calibration date; the per-stat bars open as a panel beneath it, closed by default), fields left 380 px (scrolls on its own, sticky group headers, about eight ruler sliders visible), right column 844 px with the readouts as a 3 x 2 grid of cards (112 px high) above two charts side by side (410 x 260 px each); the right column scrolls, the head and readouts stay pinned. Option B, "Sheet": fields 320 px left, a drawing-sheet canvas 624 px in the centre (the chart on the grid with dimension-line callouts), and the readouts as a 280 px title block on the right (the one allowed second use of the title block motif); the fit meter head sits on top of the canvas and the per-stat bars are a strip along its bottom edge. Ruler sliders fit the field width (about 356 px in Cockpit): input at the right of the label row, three nearest-reference labels staggered on two levels above the track and hidden (tooltip only) when they would overlap.
Fit meter: a panel head with the share of stats inside P80, the number of reference items behind the bands ("19 reference weapons"), a typicality score 0 to 100 with the sentence "low means unusual, not wrong", and the calibration date with Recalibrate; one bar per stat with the prediction marker, the P50 and P80 bands and the item's value, drawn green "typical", blue "plausible", amber "unusual", never red; notices "bands are rough" and "bands optimistic: many near twins"; unusual stats listed with the nearest reference value and a "set to suggestion" button.
States: empty (choose a kind and parent base), calibrating (progress), partial (rough or optimistic bands), error (a formula domain error inline), extreme (47 materials; a field flagged "ask" with no estimate: show the pool range instead).

**SCR-38 Calibration quiz (a stepper over the form, not a modal)**
An anchor card (name, picture where available, the strength index components with real numbers) with three large answers (weaker, about the same, stronger) or an option set, a typed-value field and "not sure". A header reads "question 3 of about 6" and states the cost up front ("about 6 questions"). A side panel shows the live estimate updating. Buttons: Back, Skip to result, Use what I have. Draw the real questions in Part F. Answers are saved in the draft; the quiz is never a gate.

**SCR-35 Item list, SCR-37 Material and quality matrix, SCR-39 Output preview, SCR-40 CE update and lint**
Item list: a table of drafts and project defs (label, defName, kind, calibration mode, CE patch: off, on, already CE; status: draft, written, out of date; fit summary) with a Reference browser tab; the empty state explains three starting points (new weapon, new apparel, clone or convert). Matrix: rows are the allowed materials (up to 47), columns the seven quality levels, switchable metric, cells highlighted for cap, threshold and band violations with icon and text; an unstuffed ranged weapon shows only the quality ladder. Output preview: tabs Definition, then CE patch and LoadFolders only when the item's CE patch toggle is on, About changes, Files; each a diff against the project files with static validation codes above; the confirm step lists every file with status (create, update region, unchanged) and size; nothing is written before confirmation. CE update and lint: lists convertible defs ("already converted" is never converted again), lint findings (ce.*), and update mode showing the regions it will change.

### Round 5: publisher and shell screens

**SCR-41 to SCR-48 Publisher flow**
A header stepper (Project, Plan, Metadata, Review, Publish, Result). Project picker: a searchable list (name, package id, source, Workshop id or "not published", last published, status chip: Ready, Has blocking findings, Needs recovery, Not uploadable). Plan and ignore editor: a tri-state file tree with sizes and excluding-rule chips, locks on required paths, totals, changes since the last upload, ignore chips (version control, IDE and build, source folders, raw art, junk, archives) and a pattern text area with live matching. Preflight: Blocking, Warnings and Information groups with code, message, file or field and a fix action, live-updating. Metadata and description: title with a 128 counter, description with an 8,000 counter and a live Steam BBCode preview (banner: "Steam renders BBCode, so Markdown shows literally"), tags, visibility, preview image info, a change note with "Fill from changes". Review: a summary of target, account, title, tags, visibility, files and bytes, diff counts, warnings; Publish opens one confirmation with no countdown. Progress: stage labels (Staging, Connecting to Steam, Creating item, Preparing content, Uploading content, Uploading preview, Committing changes), a byte bar, a log drawer, Cancel always enabled and saying what it does. Result variants: Done, Needs agreement ("The item stays hidden until you accept the Workshop agreement"), Failed (code, message, remedy), Cancelled. History: a list of runs with the manifest and a diff against the current plan. Use the Lone Wolf Weapon Package with `.git` and `Raw Assets` excluded by default as the example, sizes left as placeholders. States to draw for the flow: no project (empty picker with Create a project), a project with blocking findings (Publish disabled and saying why), loading (plan scanning with a progress line), offline (Steam unavailable: the stage list shows where it stopped), the four result variants, and extreme (4,000 files in the tree, 30 preflight findings, a 40-run history). The stepper never blocks going back; the steps are not a modal.

**SCR-49 to SCR-53 Task centre, command palette, diagnostics and About, update dialog, shortcut map**
Task centre: a drawer of jobs (name, phase, bar, elapsed, Cancel, Details), finished tasks stay for the session; empty "Nothing is running"; extreme: 40 finished tasks. Command palette: input, grouped results (commands, mods, settings), shortcut hints, parameter steps; empty shows recent items; "No command matches". Diagnostics and About: version, channel, licences, data folders with mode (installed or portable) and sizes, Copy diagnostics (redacted), a plain statement that there is no telemetry and the list of network requests. Update dialog: version, notes, size, Download and install, Later; offline "Needs network"; signature failure message. Shortcut map overlay (opened with `?`): grouped table by context, closed with Esc, with focus returned to the opener. Extreme states to draw: 40 finished tasks in the task centre; a command palette with no match and one with a parameter step (switch profile); the About page with portable data folders and long paths; the update dialog offline and with a signature failure.

---

## PART E: COMPONENT SHEET (request after Round 1 is approved)

Ask for one sheet per theme with a matrix: one row per component, one column per state in the vocabulary **default, hover, active, focus-visible, disabled, loading, error, selected, dragging, drop-target** (omit a state that does not apply). Components: **Actions** Button (primary, secondary, ghost, danger; sm, md), IconButton (ghost, subtle, danger), ToggleButton (single, in group), SegmentedControl (2 to 5 options, icon or text), Kbd (single key, chord). **Inputs** Checkbox (normal, indeterminate), Switch (sm, md), RadioGroup (vertical, horizontal), TextField (single line, multi line, prefix and suffix), NumberField (unit, stepper, range hint, out of range), Slider (single, range, with ticks) and RulerSlider, SearchField and QueryBar (filter chips, inline parse error with position), Select (single, grouped), Combobox (single, multi, async options), TagInput (free, constrained, at limit), ColorPicker (swatch grid, hex input), ShortcutRecorder (idle, recording, conflict). **Containers** Panel (plain, framed with corner ticks, titled; collapsed, focused), Card (flat, raised, selectable), Tabs (line, boxed, overflow), TabStrip (closable document tabs, dirty marker), Splitter (horizontal, vertical, collapsed, keyboard resize), Drawer (right, bottom for the task centre), Dialog (confirm, form, destructive; open, closing, busy), Popover, Menu (dropdown, nested, with checks), ContextMenu (on row, on empty area), FormField (label, help, error, required), FormSection (collapsible, plain). **Feedback** Badge (neutral, info, warning, danger, success), StatusDot (colour plus shape), Chip (removable, selectable), ProgressBar (determinate, indeterminate; running, paused, error), Spinner (sm, md), Skeleton (text, row, block), Tooltip (text, rich), Toast (info, success, warning, error with action), Banner (info, warning, danger, with action), EmptyState (default, compact, with illustration slot), ErrorCard (boundary, inline, with fix action). **Data** Table (simple, sortable, resizable columns), TreeView (single and multi select, lazy children), PropertyGrid (key value editor, nested; dirty, invalid, readonly), Meter (linear, with target band; below, within, above, unknown), DiffView (inline, side by side), ModPreview (image with hatch and initials fallback, sizes 32 to 256 px). These 46 are the architecture's `rimstudio-ui` inventory; do not add a primitive that is not in this list without saying so. **Composites** VirtualList (fixed row height windowing, skeleton rows, sticky group headers, auto-scroll while dragging), ModRow (the three densities, active and inactive), GroupHeader, DiagnosticsBadge and DiagnosticItem, RuleChip (layers About, community, user, derived; kinds load after, load before, incompatible, force top, force bottom; suppressed and dropped), ProvenanceTrail, DiffBlock, DatasetCard, TaskItem, FitMeter, QuizStep, ReferenceChart, MaterialMatrix, DropIndicator (allowed, breaks hard rule, refused), ListHeader, ActionsRail, SourceBadge, SourceCard, DetectionCard, StatusChip, FileTree, LogBlock, StepperHeader, PreflightChecklist, ProfileSwitcher, BBCodePreview. **Blueprint parts** (built from tokens and plain CSS, not in the 46; the implementer adds them to the library inventory) TitleBlock (the status strip), Stamp, RegistrationMarks, DimensionLine, Ruler (tick scale used by RulerSlider and the quiz bins), HatchField, LeaderLine. Every component shows its tokens in use and its keyboard focus appearance. Each row and control component is drawn at Compact, Comfortable and Roomy height, and every state in the matrix is drawn with tokens only: no state may need a blur, a filter, a gradient on a control or a raster image. State tints (selected, hover, drop-target, diff added and removed, fit bands, group and source colour dots) must be named as tokens on the sheet so the implementer can add them to the theme allow list.

---

## PART F: SAMPLE DATA (use this; do not invent)

### F1. A real active list head (owner's 610-mod list)

These rows come from the owner's own list (positions, ids, names, authors and supported versions are as the mods declare them in their About files). A range such as "1.2 to 1.6" is shorthand in this table for the listed entries; draw the chips as individual versions, and draw a gap for any version a mod skips (a real example from the same list, not in the table: Trading Spot lists 1.0 to 1.3, 1.5 and 1.6, no 1.4).

| # | Name | Package id | Authors | Supported versions | Notes for the design |
|---|---|---|---|---|---|
| 1 | Prepatcher | `zetrith.prepatcher` | Zetrith | 1.4, 1.5, 1.6 | |
| 2 | Harmony | `brrainz.harmony` | Andreas Pardeike | 1.2 to 1.6 | short name: rows must not look empty |
| 3 to 8 | Core, Royalty, Ideology, Biotech, Anomaly, Odyssey | `ludeon.rimworld`, `ludeon.rimworld.royalty`, `.ideology`, `.biotech`, `.anomaly`, `.odyssey` | Ludeon Studios | all | official content (source icon, no overflow delete); the version chip shows the game version, not a list |
| 9 | Adaptive Storage Framework | `adaptive.storage.framework` | (none) | 1.4, 1.5, 1.6 | empty author, 3 load-after entries |
| 13 | HugsLib | `unlimitedhugs.hugslib` | UnlimitedHugs | 1.0 to 1.6 | version chip with 7 entries |
| 14 | Vanilla Expanded Framework | `oskarpotocki.vanillafactionsexpanded.core` | Oskar Potocki, XeoNovaDan, Orion, Kikohi, Taranchuk, Sarg Bjornson, Erdelf (7 authors; truncate with a count) | 1.0 to 1.6 | 41-character id, 7 load-after entries |
| 18 | Humanoid Alien Races | `erdelf.humanoidalienraces` | erdelf | 0.19 to 1.6 | |
| 22 | Alpha Animals | `sarg.alphaanimals` | Sarg Bjornson, AFriend | 1.5, 1.6 | 12 load-after entries |
| 25 | Vanilla Fishing Expanded - Fishing Treasures AddOn | `vanillaexpanded.vcefaddon` | Oskar Potocki, Sarg Bjornson | 1.4 to 1.6 (the About file lists them out of order as 1.6, 1.4, 1.5; show them sorted) | long Latin name (50 characters); position 24 is its parent, Vanilla Fishing Expanded (`vanillaexpanded.vcef`) |
| 27 | Combat Extended | `ceteam.combatextended` | CE Team | 1.6 | 48 load-after entries |
| 31 | [JM]Disable Pseudo Translate 禁用泰南语 | `jm.disstynan` | 術滅(Jutsumetsu) | 1.5, 1.6 | mixed Latin and CJK |
| 42 | Expanded Prosthetics and Organ Engineering - Forked | `vat.epoeforked` | Victorique, Aurani, Tarojun | 1.0 to 1.6 | longest name in the sample (51 characters) |
| 55 | [TW1.6]堂丸贴图重置~UI Tang's~Retexture~UI | `tw.tangs.retexture.ui` | TangW | 1.2 to 1.6 | bracket prefix plus CJK |
| 56 | [TW1.6]堂丸贴图重置~制成品 Tang's_Retexture_Manufactured | `tw.tangs.retexture.manufactured` | TangW | 1.2 to 1.6 | |
| 60 | [TW1.6]堂丸贴图重置~武器 Tang's~Retexture~Weapons | `tw.tangs.retexture.weapons` | TangW | 1.2 to 1.6 | |

Other real names to use: Cherry Picker, XML Extensions, Vanilla Backgrounds Expanded, Vehicle Framework, Alpha Memes, Alpha Genes, [K4G] RimWorld War 2, [RH2] BCD: First Aid, [SYR] Harvest Yield (Continued), Quarry. For the "custom folder" source use mods named after the owner's own projects (for example "Gewehr 41", "The Lone Wolf Weapon Package") with the hatch-link "Not visible to the game" badge on those not yet linked.

### F2. Diagnostics (stable codes; the pairings are illustrative)

Codes, severities and message templates are those of the product's validation catalogue; the mod names come from the sample list but every pairing, direction and count is illustrative and must not read as a claim about those mods. Show a mod with several messages at once (an error, a warning and an info) so the badge shows the highest severity with a count.

| Code | Severity | Message | Fix buttons |
|---|---|---|---|
| `list.missing-dependency` | Error | "Vanilla Backgrounds Expanded requires Vanilla Expanded Framework, which is not active" (state: inactive and available) | Enable dependency; Use alternative; Open Workshop page; Mute |
| `list.order-hard` | Error | "Harmony must load before HugsLib (force rule from About)" | Move after or before; Re-sort |
| `list.order-soft` | Warning | "Cherry Picker should load after XML Extensions (community: example comment)" | Move; Sort; Suppress the rule |
| `list.version-mismatch` | Warning | "A fictional mod lists 1.4, 1.5, not 1.6" (the real template is "{mod} lists {versions}, not {gameVersion}"; Core and official expansions never get it) | Look for update; Mute |
| `list.duplicate-id` | Error | "{mod} and {other} share the package id {id}" (not ignorable) | Keep one (opens Duplicates) |
| `list.replacement-available` | Info | "{mod} has a suggested replacement: {replacement}" | Use replacement; Mute |
| `list.unresolved-id` | Error, blocks saving until removed or confirmed | "{id} is in the list but no installed mod has it" | Remove from list; Locate; Subscribe |
| `deploy.not-visible` | Error, blocks save and launch | "{mod} is in {folder}, which the game cannot see until it is linked" (folder: My mods) | Link now; Disable |
| `deploy.source-offline` | Error, blocks save and launch | "{mod}'s folder {source} is not reachable" | Reconnect and rescan; Disable |
| `sort.cycle` | Warning (Error if a hard edge was dropped; preview only) | "Rules {members} contradict each other; {dropped} was ignored" | Open rule editor; Suppress; Delete |
| `list.incompatible` | Error | "{mod} is incompatible with {other} (declared by {declaredBy})", shown on both mods (fictional pair) | Disable one of them; Mute |
| `list.missing-properties` | Warning | "{mod} has no usable package id" or "{mod} declares no supported versions" | Open About.xml; Mute |
| `list.core-inactive` | Error (not ignorable) | "Core is not active" | Enable Core |
| `list.pinned-conflict` | Warning | "{mod} is pinned at {position}, but {other} must load {direction} it" | Release pin; Move |
| `deploy.modsconfig-version` | Warning when reading, Error when writing | "ModsConfig.xml was written for {fileVersion}; the game runs {gameVersion}" (the game would discard the whole list at its next start) | Save (RimStudio rewrites it with the install version after a backup); Check the install path |
| `sort.tier-conflict` | Warning (preview only) | "{mod} is flagged {flag} but must load {direction} {other}" | Change flag; Suppress |
| `sort.unmapped` | Info (preview only) | "{mod} could not be ordered by rules and was placed by name" | Fix About.xml |
| `dataset.rejected` (dataset pipeline, not in the validation catalogue) | Warning | "Update rejected: entries dropped from 631 to 12. Kept the last good copy." | Inspect; Force accept |

### F3. Item designer content

Real reference weapons (vanilla):

| Reference | Damage x burst | Armour penetration | Cycle (s) | Nominal DPS | Hit factor at 3, 12, 25, 40 tiles |
|---|---|---|---|---|---|
| Revolver | 12 x 1 | 0.18 | 1.9 (0.3 warmup, 1.6 cooldown) | 6.32 | 0.80, 0.75, 0.55, 0.40 |
| Assault rifle | 11 x 3 | 0.165 | 3.033 (1.0 warmup, 1.7 cooldown, two gaps of 10/60 s) | 10.88 | 0.60, 0.70, 0.65, 0.55 |

The design example is the owner's Gewehr 41 rifle (package id `oh.weapons.gewehr41`, project "Gewehr 41"), placed between those two anchors in tier Industrial, role rifle. All its numbers are **illustrative** and must be labelled so in the design. Pool: 19 reference ranged weapons. Fit meter bands (real factors): range P50 x1.10, P80 x1.15; mass P50 x1.17, P80 x2.68; with the optional Combat Extended patch on, range x1.14 and x1.33, Bulk x1.25 and x1.46, magazine x1.18 and x2.0. Typicality example (a fictional peer set, scored per stat; the item score is the mean over stats, shown 0 to 100): peers 8, 9, 10, 11, 12, 14 give a stat score of 1.0 for a value of 13 and 0.003 for 30; the sentence is "low means unusual, not wrong". The P80 bands cover about 70 to 84 percent of held-out reference items, so label them "rough" and show the pool size beside them (19 vanilla direct-fire reference weapons; "bands are rough" below about 15 items in the chosen role). Rank readout: "heavier than 12 of 19 reference rifles" (an example sentence; the count is illustrative). Combat Extended fields to show: Bulk (flagged "ask"), magazine size (always a question), ammo set picker, a read-only damage and penetration lookup, the meets-armour table.

Quiz questions (real wording and bins): "Which tech level is it?" (only the levels present in the reference pool are offered: the vanilla direct-fire pool has Neolithic, Industrial and Spacer, so draw those three for the vanilla variant and the full Neolithic to Archotech list as a second artboard); "What is it?" (role list); "Is your item weaker, about the same, or stronger than the assault rifle?" (anchor card with its real numbers); "How does it fire?" (single, short burst 2 to 4, long burst 5 to 9, belt 10 or more); "How far does it shoot?" (under 20, 20 to 27, 27 to 35, over 35 tiles); "Compared with the assault rifle, is its mass lighter, similar or heavier?" (below 85%, similar, above 118% of the anchor's mass). Header: "question 3 of about 6", with "about 6 questions" stated up front. Controls: Back, Skip to result (after every answer), Use what I have, and "not sure" on every question; a side panel shows the live estimate. The Combat Extended variant has its own bins (single, burst or auto 2 to 6, sustained 7 or more; range under 20, 20 to 40, 40 to 60, over 60 cells) and uses no vanilla numbers. Reference weapon values are real vanilla data shown as numbers only; never reproduce the game's weapon art, and do not copy Combat Extended data into the design (show CE fields empty or as labelled placeholders).

### F4. Other content

Datasets: five cards (Community rules, Steam Workshop database, RimWorld versions, Use This Instead, No Version Warning) with status chips drawn from never fetched, ready, stale, updating (bytes shown), rejected (beside the last good copy), offline, failed and disabled, and licence notes ("No licence file found upstream: fetched on your machine only" for community rules, Steam Workshop database and RimWorld versions; "MIT, credit: Mlie" for Use This Instead and No Version Warning). Source cards: the install Mods folder, one Workshop library, and "My mods" on an external drive (22 mods at depth 2). Steam helper states: running, missing. Publisher: The Lone Wolf Weapon Package (its sizes are placeholders: none were measured, so label any byte count "illustrative"; default exclusions such as `.git` and `Raw Assets`). Publisher stepper: Project, Plan, Metadata, Review, Publish, Result. Limits to show on live counters: title 128 characters, description 8000, change note 8000, each tag 255 and the joined tag string 1024; the preview image is checked for real format versus extension, over 1 MB (a soft warning: the limit is a community claim, not a Valve rule) and 16:9 (recommended 640 x 360). Progress stage labels: Staging, Connecting to Steam, Creating item, Preparing configuration, Preparing content, Uploading content, Uploading preview, Committing changes. Result variants: Done, NeedsAgreement (not an error), Failed, Cancelled. Staged soft limits default to 500 MB and 5,000 files (warnings).

### F5. Cardinality and stress cases to draw

0, 1 and many for every list and panel; 5,000 rows in both lists (counts in headers such as "4,312 of 5,000", skeleton rows, scroll position); 0, 1 and 400 diagnostics; 7 duplicate groups; 4 and 60 missing ids; 22 mods in one custom folder; 12 links in a deploy plan; 100 history snapshots; a 40-mod block move; 13,212 defs in the explorer; a 100,000-line log; a 3,000-file publish plan; names up to 51 characters, a 7-author mod (the longest author list in the sample; a synthetic 20-author case is allowed and must be labelled as invented).

---

## PART G: MICROCOPY (tone and key messages)

The rows below are the product's agreed example messages; keep their wording. Slots in braces are filled from the code catalogue and must be drawn with sample values, never left as braces. Offline and rejected-update messages describe a state, not a failure. Do not use dash-like punctuation in copy: use a comma, a colon or a new sentence. Tone: calm, exact, never blaming. State what is true, what the app did or will do, and the one next step. Name the mod, folder or file; give numbers; no exclamation marks, no "oops", no "failed to" without a cause. "RimStudio" is the app, "the game" is RimWorld. Offline is a state, not a failure. Buttons are verbs; destructive buttons name the object ("Delete profile"). Counts precede nouns ("1 mod", "14 mods"). Paths and ids are always mono. Sentence case except product names.

| Moment | Message | Action |
|---|---|---|
| Launch blocked: a link is broken | "RimWorld was not started. The folder for Gewehr 41 moved (it was D:\, now E:\). The game would remove it from your list." | Fix automatically |
| Launch blocked: drive offline | "RimWorld was not started. The drive holding My mods is not connected, so 14 active mods cannot be found." | Reconnect and rescan |
| Launch anyway (typed confirmation) | "Launch anyway starts the game with this list. The game will rewrite ModsConfig.xml without the 14 mods it cannot find. A backup is saved first. Type LAUNCH to confirm." | Launch anyway |
| Dataset download quarantined | "Update rejected: entries dropped from 631 to 12. Kept the last good copy." | Inspect |
| Datasets offline | "Last checked 3 days ago. Sorting and checks use the last copy." | Refresh now |
| Unresolved active ids | "4 mods in the list are not installed. Saving is paused until you remove them, find them, or accept that the game will drop them." | Review missing mods |
| Sort result with reasons | "Sorted 14 mods. Most moved because Harmony loads before HugsLib (About), and 3 moved after Vanilla Expanded Framework (community rule)." | Undo |
| Already sorted | "Already sorted. Nothing would move." | Close |
| Sort cycle | "Two rules contradict each other, so one was ignored. The ignored rule is listed with its source." | Open rule editor |
| Preflight failure | "Publishing is paused: 2 problems need fixing. About.xml has no packageId, and the preview image is not a PNG or JPEG." | Open About editor |
| Steam not running | "Steam is not running or not signed in. Start Steam, sign in, and try again." | Retry |
| Needs agreement | "Uploaded. The item stays hidden until you accept the Workshop agreement on its page." | Open Workshop page |
| CE not installed | "Combat Extended is not in your reference set, so the optional CE patch is off. Vanilla design works fully." | Choose reference set |
| Hard conflict | "Mod A and Mod B are incompatible (declared by Mod A's About.xml). Disable one of them." | Disable Mod B |
| Hard order rule on drop | "Moving here breaks a rule: HugsLib must load after Harmony." | (shown on the insertion line) |
| Official content refused | "Core stays first. The game enforces this order." | |
| Game running | "RimWorld is running. Saving waits until it closes. You can keep editing." | Check again |
| Game changed the list | "The game changed the list while it ran. Review the changes before they replace yours." | Show diff |
| Duplicate packages | "Two copies share the id {id}. RimStudio uses the Workshop copy. Nothing on disk was changed." | Pin the other copy |
| Custom folder rejected | "This folder contains another source, so mods would be counted twice. Remove the other folder or choose a deeper path." | Choose another folder |
| RimWorld not found | "RimWorld was not found. Places checked are listed below. You can still use your own mod folders; launching is off until the game is set." | Choose folder |
| Settings cannot be saved | "Settings will not be saved. The data folder is read-only." | Open data folder |
| Dataset in a newer format | "This data uses a newer format. Update RimStudio to use it. The last good copy is still in use." | Check for updates |
| Publish: item created earlier, never submitted | "An empty Workshop item (id) was created earlier. Publishing will update it." | Continue |
| Lint without Combat Extended | "Not checked: CE not installed." | Choose reference set |
| Core not active | "Core is not active" | Enable Core |
| Version mismatch | "{mod} lists {versions}, not {gameVersion}" | Look for update |
| ModsConfig.xml from another game version | "ModsConfig.xml was written for {fileVersion}; the game runs {gameVersion}" | Save |

---

## PART H: ACCEPTANCE CHECKLIST FOR EACH ROUND (for the owner; not part of the prompt)

Score each artboard pass or fail; send back fails with the screen id. Each check names how to measure it. Use the tool's inspect panel or the exported CSS for tokens and sizes, and a contrast checker for ratios.

| Area | Check (how to measure) |
|---|---|
| Tokens | Exported CSS uses exactly the names of B10; a search of the export for hex, rgb, hsl or oklch literals outside the token block finds none; dark and light share one structure (same layout, only token values differ); accent hover, press and tint are derived with `color-mix()` from `--rs-accent` and are not extra hard-coded colours |
| Token allow list | The export names every state tint, band tint, diff added and removed tint, group and source dot colour and chart colour as a token; font tokens are present but not themable (user themes accept only colour and length values); no token needs a `url()` |
| Contrast | Text roles reach 4.5:1 on every surface they sit on (check text-faint and danger are not placed on the hover surface in dark, where they measure 4.1); control borders reach 3:1 on bg, surface and raised (on the dark hover surface `--rs-border-strong` measures 2.8, so a hovered control must step its border up); the focus ring is 2 px with 2 px offset and reaches 3:1 on its surface; spot check ten random pairs per theme |
| Red discipline | Count red (`--rs-danger`) uses per artboard and list each: allowed only for error-severity diagnostics, the hard-rule insertion line, parse errors and destructive confirmations; zero in the fit meter, in balance bands and in warnings |
| Colour never alone | For each state, name its icon shape or text; diff lines carry + and -; every badge has a letter or icon; fit states have three distinct marker shapes; converting an artboard to greyscale still separates error, warning and info |
| Blueprint restraint | Grid appears only on canvases, empty states, the first-run wizard, the explain graph and chart areas, plus panel headers at 40 percent; zero grid behind any list, table, form or editor; at most one stamp per card; hatch only for not visible to the game, inactive, disabled drop zones and cut-away diff regions, and never behind text without a solid chip; every motif is CSS or inline SVG with no image file |
| Real data | Part F names, ids, diagnostics and numbers are used verbatim (spot check 10 rows of F1 against the table); the 51-character name and 3 mixed-script names appear; 5,000-row counts are shown; every invented number carries the word illustrative; zero lorem ipsum; zero game art, logo or screenshots |
| Density | Rows measure exactly 28, 34 and 40 px in the three density artboards (inspect three rows each); controls 28, 32 and 40 px; no control under 24 px, rail and toolbar buttons 32 px or more; no row has two lines or a wrapped label; right-aligned slots start at the same x on every row; 1280 x 800 has no clipped text and no horizontal scroll |
| Windowing | Every long list is drawn with fixed-height rows (variable height only for an expanded row), a visible scroll position, skeleton rows and a count such as "4,312 of 5,000"; no artboard needs more than 3,000 rows in the DOM to look right |
| States | Each screen has its required states from Part D as separate artboards: empty, loading with skeletons, partial, populated, error, offline, extreme; count them against the screen brief |
| Provenance | Each automatic decision (sort move, warning, balance suggestion) shows its source (rule layer, file, anchor or answer) and a control that opens it; check each in the artboards |
| Undo and diff | Wherever an edit happens, a dirty marker, an undo toast and a diff entry point are visible; undo and redo are shown disabled when empty |
| Modal policy | Only destructive confirmations and the publish confirmation are modal; count modals per artboard (more than one fails); progress is inline or in the task centre; the quiz is a stepper over the form |
| Keyboard | One focus-order artboard per round with numbered stops; every tooltip lists its shortcut when one exists; every drag shows its keyboard alternative (for example Alt plus arrows) |
| Performance | No blur, backdrop blur, large or soft shadow, per-row filter, glow, gradient on a control or raster texture anywhere in the export; shadows are at most the 1 px solid line of B4; animation uses only transform and opacity, with the durations of B7 (100, 180, 160 and 200 ms) and a reduced motion variant; elevation is line and tint; charts hold fewer than 200 points |
| Motion | Looping animation exists only for the indeterminate progress line; every other transition is listed with its duration; reduced motion removes movement and keeps instant state change |
| Components | Part E matrix has one row for each of the 46 library components plus the composites and Blueprint parts; every row shows the states that apply, in both themes and at three heights for row and control components |
| Voice | Copy matches Part G: zero exclamation marks, zero dash-like punctuation, no blame; ids, paths, versions and counts in mono; counts precede nouns; sentence case; no slot braces left in the artboards |
| Honesty | Item designer numbers are labelled illustrative; bands show their pool size and the word rough where the pool is small; typicality says low means unusual, not wrong; Combat Extended fields carry no copied data; invented authors, folders and sizes are labelled |
| Licence and format | Fonts are Barlow, Barlow Condensed and Azeret Mono only (open licence, self-hosted); no UI data is shown as XML except raw def or About text in a code view; no dataset content is copied into the design beyond the sample rows of Part F |

---

## APPENDIX: NOTES FOR IMPLEMENTERS (not part of the prompt)

1. **Density values.** The [frontend architecture](../architecture/frontend-architecture.md) currently proposes two density constants (28 and 36 px) to be tuned in the gallery. This brief supersedes them with Compact 28, Comfortable 34 (the owner's earlier designs) and Roomy 40; when the first design round is approved, update the architecture text and add the third `data-density` value.
2. **Token allow list.** User themes accept only known token names with colour or length values ([frontend architecture](../architecture/frontend-architecture.md) section 9.3). Add every token of B10 that the architecture does not already list; the base vocabulary is `--rs-bg`, `--rs-surface`, `--rs-surface-raised`, `--rs-border`, `--rs-text`, `--rs-text-muted`, `--rs-accent`, `--rs-accent-contrast`, `--rs-danger`, `--rs-warning`, `--rs-success`, `--rs-info`.
3. **Blueprint parts.** TitleBlock, Stamp, RegistrationMarks, DimensionLine, Ruler, HatchField and LeaderLine (Part E) are not in the architecture's inventory of 46 components. Decide whether they join `rimstudio-ui` or stay feature-level compositions; the Blueprint look must stay removable by tokens and component styles alone.
4. **Open design questions the first rounds should settle:** whether file-level overlap (the earlier approved design showed overlap by domain with a per-file resolver) belongs in the specifications (today only duplicate defs and the Def Explorer's overrides view exist); whether the About editor should suggest a version bump (the earlier design did); the 10% selected-row tint (raised only together with a contrast recheck); and whether the vanilla tech-level question shows the three levels present in the pool or all six.
5. **Data limits.** The sample list holds the first 78 mods of the owner's 610-mod list; statements about maximum author counts and name lengths apply to that range.

