# RimStudio frontend stack research

Scope: the verified, current technology brief for RimStudio's frontend (Preact, TypeScript, Vite, Tailwind CSS, state and data patterns, heavy UI components, modularity) with a concrete recommended stack and exact version pins. It builds on the owner's earlier Preact projects and uses RimCrow's Vue frontend only as a real-world comparator. The desktop shell and IPC side is in the companion note `docs/research/webview-and-ipc-performance.md`.

Status: research note | Last verified: 2026-10-04

Method: every version, date, licence, peer range and size below comes from `npm view` registry metadata collected on 2026-10-04 (harvest tables kept outside the repo; the numbers are repeated in the tables here), from the official docs and source of the named projects, or from the measured lab described in the companion note. Items resting on memory only are marked "(unverified)". Nothing was installed or built for this note except the tiny Tauri lab used for the webview measurements.

## 1. Summary of decisions

| Area | Decision | One-line reason |
| --- | --- | --- |
| UI runtime | Preact 10.29.8 now, gated upgrade to 11.0.x | Preact 11.0.0 is 4 days old (2026-09-30), has no patch release yet, and TanStack's Preact query package still declares `preact@^10` only |
| Language | TypeScript 6.0.3 for editor and lint tooling, optional TS 7.0.2 `tsc` in CI | TS 7.0 ships no programmatic API; typescript-eslint and similar tools need the 6.0 API |
| Bundler | Vite 8.3.2 with `@preact/preset-vite` 2.10.6 | Same as the owner's projects; preset declares Vite 8 support; Vite 8 default target (Safari 16.4) equals Tailwind 4's floor |
| Styling | Tailwind CSS 4.3.3 with `@tailwindcss/vite`, CSS-first `@theme` tokens, `tailwind-variants` 3.3.1 plus `tailwind-merge` 3.7.0 | Owner requirement; tokens as CSS variables make light, dark, user accent and user themes cheap |
| Lint and format | oxlint 1.86.0 plus Prettier 3.9.9 with `prettier-plugin-tailwindcss` 0.8.1 | oxlint matches owner convention; Prettier is the only formatter here with a documented Tailwind class sorter |
| Tests | Vitest 5.0.3, `@testing-library/preact` 3.2.4, Playwright 1.63.0 | Vitest 5 declares Vite 8 support; Playwright drives the web layer against mocked IPC |
| State | `@preact/signals` 2.11.3 plus a small own IPC data layer | Snapshot plus delta streams do not fit a request cache model |
| Navigation | Tab shell driven by a signal, no URL router | A desktop app has no URLs to share |
| Validation | valibot 1.5.0 at file and dataset boundaries only | Modular, small bundle cost, Standard Schema compatible |
| i18n | Own thin layer over `intl-messageformat` 12.1.2, flat JSON catalogs | R10: no XML catalogs; ICU plurals are what translators know |
| Lists | `@tanstack/virtual-core` 3.17.11 wrapped in an own Preact hook | No official Preact adapter exists (registry 404 for `@tanstack/preact-virtual`) |
| Drag and drop | `@dnd-kit/dom` 0.5.0 (pointer and keyboard sensors) behind an own facade, spike first | Pointer based, so it does not collide with Tauri's file drop handler on Windows |
| Primitives | `@zag-js/preact` 1.44.0 with individual machines | The only headless set with a first-party Preact binding |
| Charts | Chart.js 4.5.1 with `chartjs-plugin-annotation` 3.1.0, validated by a 5k point spike | Canvas 2D, framework agnostic, covers scatter, radar and bar |
| Graph | Cytoscape 3.34.3 (canvas) loaded lazily | Canvas 2D avoids the WebGL slow path risk on WebKitGTK |
| Editor | CodeMirror 6 (`codemirror` 6.0.2, `@codemirror/view` 6.43.13) | Small, modular, same family RimCrow uses; Monaco is 99 MB unpacked |
| Rich text | AST to vnode renderer, `@bbob/core` 4.4.1 for BBCode, `marked` 18.0.14 for markdown, DOMPurify 3.4.16 as a backstop | No `innerHTML` for untrusted text on the main path |
| Icons | `lucide-preact` 1.52.0 | Preact peer range includes 11; per-icon imports |
| Fonts | Barlow, Barlow Condensed, Azeret Mono via Fontsource (all OFL-1.1) | Owner's previous choice, blueprint feel, self hosted |
| Modularity | One Vite app with feature-sliced folders, plus 2 to 3 workspace packages for genuinely shared code | Boundary rules enforced by oxlint and dependency-cruiser |

## 2. Core versions and decisions

### 2.1 Registry snapshot (2026-10-04)

| Package | Latest | Published | Licence | Peer or engine notes |
| --- | --- | --- | --- | --- |
| preact | 11.0.0 | 2026-09-30 | MIT | dist-tags: latest 11.0.0, rc 11.0.0-rc.2; last 10.x is 10.29.8 |
| @preact/signals | 2.11.3 | 2026-09-30 | MIT | `preact >=10.25.0 \|\| >=11.0.0-0` |
| @preact/signals-core | 1.14.4 | 2026-07-07 | MIT | no peers |
| @preact/preset-vite | 2.10.6 | 2026-07-18 | MIT | `vite 2.x ... 8.x`, `@babel/core 7.x` |
| @preact/compat | 18.3.2 | 2026-02-26 | MIT | `preact@*` |
| preact-iso | 2.12.2 | 2026-08-11 | MIT | `preact >=10 \|\| >=11.0.0-0` |
| wouter-preact | 3.13.0 | 2026-09-30 | Unlicense | `preact ^10 \|\| ^11.0.0-0` |
| @tanstack/preact-query | 5.104.1 | 2026-10-02 | MIT | `preact ^10.0.0` (excludes 11) |
| @testing-library/preact | 3.2.4 | 2024-05-27 | MIT | `preact >=10`; registry modified 2026-08-07 |
| typescript | 7.0.2 | 2026-07-08 | Apache-2.0 | 6.0.3 is the last 6.x |
| vite | 8.3.2 | 2026-10-01 | MIT | Node `^20.19.0 \|\| >=22.12.0` |
| tailwindcss, @tailwindcss/vite | 4.3.3 | 2026-07-16 | MIT | plugin peer `vite ^5.2 \|\| ^6 \|\| ^7 \|\| ^8` |
| vitest | 5.0.3 | 2026-09-30 | MIT | `vite ^6.4 \|\| ^7 \|\| ^8`, `@types/node ^22 \|\| >=24` |
| playwright, @playwright/test | 1.63.0 | 2026-09-04 | Apache-2.0 | none |
| oxlint | 1.86.0 | 2026-09-28 | MIT | optional peer `oxlint-tsgolint >=7.0.2003` |
| prettier | 3.9.9 | 2026-09-23 | MIT | none |
| @biomejs/biome | 2.5.15 | 2026-09-30 | MIT or Apache-2.0 | none |
| oxfmt | 0.71.0 | 2026-09-28 | MIT | 0.x version |
| pnpm | 12.9.1 | 2026-10-03 | MIT | the task context lists pnpm 11 on the machine; 12.x is on the registry |

### 2.2 Preact 10.x versus 11.0.0

What 11 changes (from the official release post and upgrade guide, fetched 2026-10-04 from the Preact site's guide sources): hydration 2.0, refs forwarded as a normal prop (so `ref` on a function component reaches the DOM element without `forwardRef`), `Object.is` equality in hook arguments (NaN safe), `createPortal` exported from core, `use()` and `useEffectEvent()` in compat, ESM-only distribution (`.mjs`), and a raised browser floor (Chrome 71, Safari 12.1, Firefox 69, Edge 79), irrelevant to our webviews. Breaking items that matter for code style: automatic `px` suffixing and `defaultProps` moved into `preact/compat`, `replaceNode` removed from `render()`, `Component.base` removed, `SuspenseList` removed from compat, TypeScript 5.1 minimum. The release post states all first-party packages (signals, render-to-string, iso, prefresh, preset-vite) have supported 11 since the prereleases.

Evidence for the decision:

| Fact | Source | Effect |
| --- | --- | --- |
| 11.0.0 became npm `latest` on 2026-09-30, four days before this note; no 11.0.1 exists in the version list | `npm view preact versions` | No field time for a patch cycle |
| `@tanstack/preact-query` and `@tanstack/preact-store` peers are `preact ^10.0.0` | registry metadata | Warns or fails strict peers on 11 |
| `@preact/signals`, `preact-iso`, `wouter-preact`, `lucide-preact`, `@zag-js/preact` (`>=10`) accept 11 | registry metadata | Core ecosystem is ready |
| `@testing-library/preact` 3.2.4 has not been published since 2024-05 but peers `preact >=10` | registry metadata | Works by range, maintenance risk is independent of 10 vs 11 |
| The owner's projects pin `^10.24.3` and `^10.29.7` | `/run/media/pawbeans/project_drive/pawbeans/Projects/preact/*/package.json` | Known territory |

Recommendation: start on exact `preact@10.29.8` and write 11-clean code from day one (no `defaultProps`, no `Component.base`, no `replaceNode`, no reliance on implicit `px`, refs as props, `createPortal` imported from `preact/compat` so it works in both). Keep a CI job that runs the unit test suite against 11 (`pnpm.overrides` in a matrix leg). Promote to 11.0.x when all of these hold: an 11.0.x patch release exists or 30 days have passed, the 11 test leg is green, and none of the pinned UI libraries report an 11 problem. The upgrade is then a one-line change. Reason to move sooner: none of the 11 features is needed by RimStudio, so waiting costs nothing.

### 2.3 TypeScript 6.x versus 7.x

TypeScript 7.0 (announced 2026-07-08) is a native port that the team describes as typically 8x to 12x faster on full builds. The announcement is explicit that 7.0 ships no programmatic API (expected in 7.1), that tools such as typescript-eslint need the 6.0 API, and that a side by side package `@typescript/typescript6` (executable `tsc6`) exists for this. The language server is new and Volar-style plugins cannot embed it yet. TS 7 turns TS 6 deprecations into hard errors: `baseUrl` removed, `moduleResolution` node10 and classic removed, `target: es5` removed, `downlevelIteration` removed, `strict` on by default, and command line builds refuse file arguments when a `tsconfig.json` is present without `--ignoreConfig`.

Decision: pin `typescript@6.0.3` (the owner's `~6.0.2` line, latest patch) as the project compiler for editors, `vite-plugin-checker` and any API-using tool. Write a tsconfig that is TS 7 clean (no `baseUrl`, `moduleResolution: bundler`, explicit `rootDir`, `noEmit`). Add an optional CI step that installs TS 7.0.2 only to run `tsc --noEmit` for speed and as an early warning. Move the main pin to 7 when 7.1's API lands and typescript-eslint-style consumers support it. The oxlint type-aware option (`oxlint-tsgolint` 7.0.2003) is separate Go tooling and may be added later (not evaluated, unverified).

### 2.4 Vite 8.3.2 and @preact/preset-vite 2.10.6

Vite 8.3.2 requires Node `^20.19.0 || >=22.12.0` (machine has Node 26). The Vite build guide gives the default production target as the Baseline Widely Available set, with Safari 16.4 as the Safari floor, which coincides with Tailwind 4's stated floor (Chrome 111, Safari 16.4, Firefox 128). `@preact/preset-vite` 2.10.6 declares Vite up to 8.x and pulls `@prefresh/vite` for hot reload and Babel plugins for hook names in dev. Tauri-specific settings: fixed dev port with `strictPort`, `clearScreen: false`, and `build.target` left at the default (do not lower it: old targets add transforms for webviews we do not support). Do not add `rolldown-vite`: Vite 8.3.2 itself depends on `rolldown ~1.2.11` and `lightningcss ^1.33.0` (registry dependencies), while the `rolldown-vite` package's latest is 7.3.1.

### 2.5 Tailwind CSS 4.3.3

Facts: CSS-first configuration (`@import "tailwindcss"`, `@theme`), no `tailwind.config.js` required, the Vite plugin `@tailwindcss/vite` 4.3.3, core features depend on Chrome 111, Safari 16.4 and Firefox 128 (docs, `compatibility.mdx`). Two documented patterns matter here: `@custom-variant dark (&:where([data-theme=dark], [data-theme=dark] *));` for attribute driven dark mode (dark-mode docs) and theme variable namespaces (`--color-*`, `--font-*`, `--spacing`, and others) that generate utilities from CSS variables.

Tokens for a themeable blueprint look. Use two layers of variables: semantic runtime variables (set by theme and accent code) and `@theme inline` entries that map utility names to those variables, so a theme switch needs no rebuild and no class change:

```css
@import "tailwindcss";
@custom-variant dark (&:where([data-theme=dark], [data-theme=dark] *));

@layer base {
  :root, [data-theme=light] {
    --rs-paper: oklch(0.97 0.012 250);   /* drawing sheet */
    --rs-ink: oklch(0.25 0.04 255);
    --rs-grid: oklch(0.85 0.03 250);
    --rs-accent: oklch(0.62 0.17 245);   /* user accent overrides this */
  }
  [data-theme=dark] {
    --rs-paper: oklch(0.2 0.03 255);
    --rs-ink: oklch(0.93 0.01 250);
    --rs-grid: oklch(0.32 0.04 255);
  }
}
@theme inline {
  --color-paper: var(--rs-paper);
  --color-ink: var(--rs-ink);
  --color-grid: var(--rs-grid);
  --color-accent: var(--rs-accent);
  --color-accent-soft: color-mix(in oklab, var(--rs-accent) 18%, var(--rs-paper));
  --font-sans: "Barlow", system-ui, sans-serif;
  --font-mono: "Azeret Mono Variable", ui-monospace, monospace;
}
```

`oklch()`, `color-mix()`, `@layer`, `@property` and container queries all returned true in the lab on WebKitGTK 2.52.6 (see the companion note). Layering: Tailwind's own layers (`theme`, `base`, `components`, `utilities`) plus one declared last, `@layer user`, into which user CSS is injected so it wins over components without `!important`.

User themes (R10): a theme is a JSONC file with `{ name, mode, tokens: { "--rs-paper": "oklch(...)" , ... } }`, validated by valibot against an allow list of token names and a value grammar (colour, length, number), applied with `document.documentElement.style.setProperty` or one generated `<style>` element. An optional advanced `custom.css` from the config folder is read through the backend and injected into `@layer user` (CSS is not XML, so R10 allows it, but JSON tokens are the supported contract). Never evaluate theme values as anything but CSS custom property values.

Variant helpers: `tailwind-variants` 3.3.1 (peer `tailwind-merge >=3`, `tailwindcss *`) covers variants, slots and compound variants and merges classes through `tailwind-merge` 3.7.0 (the v3 line is the Tailwind 4 line). `class-variance-authority` 0.7.1 was last published 2024-11-26, has no slots feature and no merge integration, so it is the fallback only. `clsx` is not needed because tailwind-variants accepts conditional values. `tw-animate-css` 1.4.0 (used by RimCrow) is optional and not recommended: write the handful of keyframes needed. Editor support: `@tailwindcss/language-server` 0.16.0 and `prettier-plugin-tailwindcss` 0.8.1 (peer `prettier ^3`).

Performance notes: Tailwind 4 emits only used utilities; keep `content` automatic detection and never build class names dynamically (`bg-${x}`), because the scanner reads source text. Prefer `data-*` attribute variants (`data-[selected=true]:bg-accent-soft`) over class toggling in list rows.

### 2.6 Lint, format, tests

- oxlint 1.86.0 (owner convention, configuration file `.oxlintrc.json` with a `$schema` entry as in color-mixer). The owner enables plugins `react`, `typescript`, `oxc` and the hook rule; keep that and add `import` (cycles) and `jsx-a11y` if present in the installed version (plugin list unverified, check with `oxlint --rules`). For import boundaries use the core `no-restricted-imports` rule: its source in the oxc repository accepts a `patterns` list with `group` entries (checked in `crates/oxc_linter/src/rules/eslint/no_restricted_imports.rs`), applied per folder through config `overrides` (override syntax from memory, unverified).
- Formatter: Prettier 3.9.9 plus `prettier-plugin-tailwindcss` 0.8.1 is recommended because class sorting is the documented plugin feature and prevents noisy diffs. `oxfmt` 0.71.0 is faster but still 0.x and its readme does not mention Tailwind class sorting (searched 2026-10-04), so it is the fallback if formatting speed ever matters. Biome 2.5.15 would replace both oxlint and a formatter, but contradicts the owner's oxlint convention and its Tailwind sorting support was not verified.
- Vitest 5.0.3: the owner's color-mixer uses `^3.0.5`, but 5.0.3 declares `vite ^8`. Use `jsdom` 30.1.2 (or `happy-dom` 20.14.5 for speed, with less fidelity) and `@testing-library/preact` 3.2.4, `@testing-library/dom` 10.4.2, `@testing-library/user-event` 14.6.7, `@testing-library/jest-dom` 7.0.1 (peer `@testing-library/dom >=10 <11`). `@vitest/browser-playwright` 5.0.3 can run component tests in a real engine (Playwright's WebKit is not identical to WebKitGTK or WKWebView, so treat as a smoke test only).
- Playwright 1.63.0 against `vite preview` with `@tauri-apps/api/mocks` (`mockIPC` ships in `@tauri-apps/api` 2.12.1, confirmed in the package files). This tests the whole web layer with a scripted backend and gives screenshot tests of the component gallery. Tests against the real shell use the WebDriver route (`tauri-driver` 2.1.0 on crates.io; macOS has no WKWebView driver, so that path is Linux and Windows only, see the companion note).
- Dev quality gates: `vite-plugin-checker` 0.14.5 (supports oxlint and `typescript`), `knip` 6.39.0 for unused exports and dependencies.

### 2.7 Comparison with the owner's projects and RimCrow

| Aspect | color-mixer / steamworkshop-downloader | RimCrow frontend | RimStudio |
| --- | --- | --- | --- |
| UI | preact `^10.24.3` / `^10.29.7` | Vue 3.5 | preact 10.29.8 (11 gated) |
| Build | vite `^8.2.0`, preset-vite `^2.9.3` / `^2.10.6` | vite `^7.2.4` | vite 8.3.2 |
| TS | `~6.0.2` | JavaScript (no TS in `package.json`) | 6.0.3 |
| Lint / test | oxlint `^1.75.0`, vitest `^3.0.5` | none listed | oxlint 1.86.0, vitest 5.0.3 |
| CSS | none | Tailwind `^4.1.17`, typography plugin, tw-animate-css, sass | Tailwind 4.3.3 only |
| Editor | n/a | CodeMirror 6 via `vue-codemirror6`, lang-json, lang-xml, csharp | CodeMirror 6 directly |
| Lists | n/a | `@tanstack/vue-virtual`, `vue-virtual-scroller` (two virtualisers) | one virtualiser |
| Rich text | n/a | markdown-it, DOMPurify, highlight.js | marked or AST renderer, DOMPurify |
| i18n | n/a | `vue-i18n` with extraction script run before build | thin layer, JSON catalogs, key check script |
| Misc | n/a | driver.js tour, viewerjs, motion-v, colour picker | none by default |

Source: `RimCrow-main/frontend/package.json` and `/run/media/pawbeans/project_drive/pawbeans/Projects/preact/*/package.json`. Lessons: RimCrow carries two virtual list libraries and a heavy set of widgets, and its build is gated on an i18n key check (`prebuild` runs `i18n:check`), which is a pattern worth copying: fail the build when a key is missing from the English catalog.

## 3. State and data

### 3.1 Signals versus alternatives

| Option | Version | Preact fit | Verdict |
| --- | --- | --- | --- |
| `@preact/signals` | 2.11.3 | first party, fine grained updates bind to text nodes and attributes | choose |
| `nanostores` + `@nanostores/preact` | 1.5.4 / 1.1.0 | peer `preact >=10` | fallback; atom model, less ergonomic derived state |
| `@tanstack/preact-store` | 0.13.4 | peer `^10` only | no, 0.x and excludes 11 |
| zustand, jotai, valtio | 5.0.15, 3.0.1, 2.3.2 | React peers, via compat only | no |

Signals matter for this app because a 3000 row table can bind a row cell to a signal and update one text node without re-rendering the list. Rules: one store module per feature exposing signals and pure action functions; derived data with `computed`; never put whole record arrays into a single signal that changes on every delta (use a `Map<id, ReadonlySignal<Row>>` or a version counter plus `peek()` reads in virtualised rows).

### 3.2 Pattern for IPC data

TanStack Query for Preact exists (`@tanstack/preact-query` 5.104.1, created 2026-02-09, 1.7 MB unpacked) but is a cache of request and response pairs with refetch policies. RimStudio's main data (mod list, scan progress, sort results) is a snapshot followed by pushed deltas, where the backend is the source of truth and a refetch would be wasteful. Recommended is a small own layer (about 150 lines, `shared/ipc`):

1. `query(key, fetcher)` returns `{ data, error, loading }` signals; results cached by key in a `Map`; `invalidate(key)` re-runs.
2. `stream(channelCommand, reducer)` opens a Tauri Channel (see companion note), applies batched messages in a reducer inside `requestAnimationFrame`, and exposes a signal updated at most once per frame.
3. `snapshot+delta` model: `loadSnapshot()` returns rows plus a revision number; deltas carry `{rev, upserts, removes}`; a gap in `rev` triggers a snapshot reload.
4. Cancellation: each stream and query holds an `AbortController`-like token mapped to a backend job id; unmount cancels.

Use `@tanstack/preact-query` only if request style data (Workshop API pages, dataset fetches with stale-while-revalidate) grows beyond what this layer handles; it is the fallback, and note the peer range problem if Preact 11 is adopted first.

### 3.3 Navigation shell, validation, i18n, shortcuts

Navigation: a left rail or top tab strip driven by `route = signal<RouteId>`, with each feature page lazily imported (`lazy` from `preact/compat`, or a dynamic `import()` wrapper) and kept mounted but hidden when expensive to rebuild (mod list, editor). Deep links are not needed. Fallbacks: `wouter-preact` 3.13.0 (Unlicense, small) or `preact-iso` 2.12.2.

Validation: valibot 1.5.0 (modular, 1.8 MB unpacked but tree shaken) versus zod 4.6.5 (6 MB unpacked). Both implement Standard Schema (`@standard-schema/spec` 1.1.0). Use valibot for JSONC settings, theme files, imported RimSort user rules and fetched dataset payloads, in the frontend only where the frontend reads files itself. Do not validate the typed IPC payloads at runtime in release builds (types come from generated bindings, see the companion note); validate them in dev and tests only.

i18n (R10: JSON catalogs, never XML): candidates.

| Option | Version | Catalog format | Notes |
| --- | --- | --- | --- |
| thin own layer on `intl-messageformat` | 12.1.2 (BSD-3-Clause) | flat JSON, ICU messages | choose; reactive `locale` signal; plural and select rules |
| i18next | 26.4.2 | JSON | most common, plugins for ICU (`i18next-icu` 2.4.4), larger API, fallback |
| Lingui | 6.9.0 | PO by default, other formats via plugins | macro and Babel path adds build complexity |
| Paraglide | `@inlang/paraglide-js` 2.25.4 | inlang JSON | compiles to tree shaken functions, strongest typing, heavier toolchain |
| Fluent | `@fluent/bundle` 0.19.1 | FTL | not JSON, rejected by R10 |

Catalog rules: `locales/en.json` is the source, flat keys (`modlist.empty.title`), other languages optional and lazily imported; a script generates the key union type and a CI check fails on missing English keys and on unused keys (the pattern RimCrow's `i18n:check` follows). Game text (mod names, descriptions) is never translated by this layer.

Shortcuts and command palette: a registry of commands `{id, titleKey, defaultKeys, run, when}` stored as JSON (user overrides in JSONC settings), `tinykeys` 4.0.1 (76 kB unpacked) for key chord parsing, and an own palette built on the Zag combobox machine or a plain input and filtered list (fuzzy match in about 40 lines). `cmdk` 1.1.1 and `kbar` 1.0.0 are React only; `@tanstack/preact-hotkeys` 0.13.0 exists (peer `preact >=10`) but is 0.x and optional.

## 4. Heavy UI needs

Compatibility column means: native Preact (no React), usable through `preact/compat` (React API alias, unverified unless stated), or React only.

### 4.1 Virtualised lists (thousands of variable height rows)

| Candidate | Version | Preact | Notes |
| --- | --- | --- | --- |
| `@tanstack/virtual-core` | 3.17.11 | framework agnostic core, no Preact adapter in the registry | dynamic measurement, scroll to index, horizontal and grid; choose and wrap in a hook |
| virtua | 0.52.10 | peers react, vue, svelte, solid, angular; Preact only via compat (unverified) | variable size without measuring config; check compat before relying on it |
| react-virtuoso | 4.18.16 | React peers, compat unverified | feature rich (grouping, sticky), larger |
| react-window | 2.3.3 | React peers | needs row size knowledge, weaker for variable rows |
| own windowing | n/a | n/a | fixed height rows are about 80 lines; variable rows need a measured offset cache |

Decision: `@tanstack/virtual-core` with an own `useVirtualList` hook that returns the visible slice and spacer sizes, with these rules: fixed row height by default (compact and comfortable densities are two constants), measured heights only for expanded rows, overscan of 8 rows, row components memoised and keyed by stable mod id, scroll position retained per list across tab switches. The lab (companion note) measured plain DOM layout of 27 000 nodes (3000 rows of 9 nodes) at about 876 ms and 54 000 nodes at 1611 ms versus 196 ms and 494 ms with `content-visibility: auto`, so even the fallback of CSS containment is much better than a full DOM list, but only windowing keeps the DOM under a few thousand nodes.

### 4.2 Drag and drop between two virtualised lists

Requirements: multi select, drag between active and inactive lists, keyboard reordering, rows unmount while dragging (so the drag payload must be data, not DOM), auto scroll at edges.

| Candidate | Version | Mechanism | Fit |
| --- | --- | --- | --- |
| `@dnd-kit/dom` (with `@dnd-kit/abstract`) | 0.5.0 (beta tag 0.5.1-beta) | pointer and keyboard sensors (README), framework agnostic core, sortable entry point | best match; 0.x so API may move; no official Preact adapter, use the vanilla manager from hooks |
| `@atlaskit/pragmatic-drag-and-drop` | 4.0.0 (Apache-2.0) | the browser's built-in HTML5 drag and drop (README), any view layer | solid and tiny, but see the Windows problem below |
| sortablejs | 1.15.7 | HTML5 DnD with fallback | mutates DOM order, conflicts with virtualised rows |
| own pointer implementation | n/a | `pointerdown/move/up`, `setPointerCapture`, rAF auto scroll | most control, about 400 lines plus keyboard layer |
| @dnd-kit/core, @hello-pangea/dnd | 6.3.1, 18.0.1 | React only | no |

Windows problem (verified in Tauri source): Tauri's webview setting `dragDropEnabled` (default true) installs a native drop handler to deliver file drops, and its documentation comment says disabling it is required to use HTML5 drag and drop on the frontend on Windows. The same setting is what lets the app receive dropped folders and files with real paths. RimStudio wants both (drop a mod folder from the OS onto the window; reorder rows). Therefore in-app reordering must not depend on HTML5 drag and drop: choose a pointer based engine (`@dnd-kit/dom` or own), and keep `dragDropEnabled` true.

Keyboard reordering is a first class feature, not an afterthought: commands "move selection up/down by 1", "to top", "to bottom", "to position N" bound to Alt+Arrow and available in the context menu and the command palette; this also covers screen reader users and is simpler to test than pointer gestures. Decision: spike `@dnd-kit/dom` 0.5.0 behind a facade (`shared/dnd`: `useDragSource`, `useDropZone`, `autoScroll`), with the own pointer engine as fallback if the spike fails the two checks (drag across two windowed lists with unmounting rows, and 5 selected rows dragged at once at under 16 ms per frame).

### 4.3 Headless primitives, panels, menus, tooltips, toasts

| Need | Candidates | Choice |
| --- | --- | --- |
| Primitive set | `@zag-js/*` 1.44.0 with `@zag-js/preact` 1.44.0 (peer `preact >=10`, MIT, 40 kB); Radix (`radix-ui` 1.6.7, React peers); React Aria Components 1.21.1 (React peers); Base UI `@base-ui/react` 1.8.0 (React); Headless UI 2.2.10 (React); Ark UI 5.39.2 (React, built on Zag); Ariakit 0.4.40 (React) | Zag: the only candidate with a first-party Preact binding and per-component packages (`menu` 160 kB, `splitter` 203 kB, `toast` 116 kB, `tooltip`, `dialog`, `popover`, `combobox`, `select`, `tabs`, `tree-view`) |
| Resizable panels | `@zag-js/splitter` 1.44.0; `react-resizable-panels` 4.14.2 and `allotment` 1.20.5 (React); `split.js` 1.6.5 (vanilla, last release 2022-01) | Zag splitter; fallback own 80 line pointer splitter with persisted sizes |
| Context menus | `@zag-js/menu` (menu with context trigger, unverified for the exact API), Radix context menu (React), Tauri native menus through the backend | Zag menu; native menus as an option for the desktop feel |
| Tooltips and popovers | `@zag-js/tooltip` and `popover`, `@floating-ui/dom` 1.8.0, native `popover` attribute and CSS anchor positioning (both true in the lab on WebKitGTK 2.52.6) | Zag with Floating UI positioning; native popover only if the minimum WebKit allows |
| Toasts | `@zag-js/toast` 1.44.0, sonner 2.0.8 (React peers), own 60 line store | Zag toast or own store; no React library |
| Dialogs, selects, tabs | Zag machines or the native `<dialog>` element (true in lab) | native `<dialog>` for modal confirms; Zag for select and combobox |

Preact compat alias route (Radix, React Aria through `@preact/compat`) is possible in principle and works for many libraries, but it was not tested here (unverified) and drags in React sized dependency trees; the Zag route avoids that. All primitives are wrapped once in `rimstudio-ui` so the underlying library can be swapped.

### 4.4 Charts (scatter with reference clouds, radar, histograms, up to 5k points)

| Candidate | Version | Rendering | Scatter 5k with highlights | Radar | Histogram | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| Chart.js + annotation | 4.5.1 + 3.1.0 | Canvas 2D | yes (datasets, per point styling) | built in | bar chart or custom bins | framework agnostic; 6 MB unpacked but tree shakeable; one system for all three charts |
| uPlot | 1.6.32 | Canvas 2D | very fast for large series | no | bars via paths | tiny (533 kB unpacked), excellent for scatter, no radar |
| ECharts | 6.1.0 | Canvas or SVG | yes | built in | yes | 58 MB unpacked, modular import possible, large API |
| Observable Plot | 0.6.17 | SVG | 5k SVG nodes is heavy | no | yes | good for static analysis views |
| D3 modules (`d3-scale`, `d3-quadtree`, `d3-shape`) | 4.0.2, 3.0.1, 3.2.0 | own Canvas 2D | full control, quadtree hit testing | own | own | most work, smallest and most tailored |
| regl-scatterplot | 1.16.0 | WebGL | fastest | no | no | avoid: Tauri docs warn that WebGL on WebKitGTK can silently use a slow path and the renderer string is masked |
| visx 4.0.0, recharts 3.10.1, react-chartjs-2 | | SVG, React | | | | React only |

Decision: Chart.js 4.5.1 with `chartjs-plugin-annotation` 3.1.0 for the three chart types, wrapped in one `<Chart>` component that owns the canvas, resize observer and destroy. Reference clouds are a low alpha dataset drawn first, the candidate item is a separate highlighted dataset. This choice is not measured: require a spike that draws 5000 points plus a 5000 point reference cloud and redraws on hover under 16 ms on the oldest supported webview, with uPlot (scatter) and own Canvas 2D (d3-scale plus d3-quadtree) as fallbacks. Radar needs a legible blueprint style, which Chart.js provides through scale options.

### 4.5 Dependency graph visualisation

| Candidate | Version | Rendering | Notes |
| --- | --- | --- | --- |
| Cytoscape.js | 3.34.3 (MIT) | Canvas 2D | layouts via `cytoscape-dagre` 4.0.1, `cytoscape-fcose` 2.2.0 (last release 2023), `cytoscape-elk` 2.3.0; 5.5 MB unpacked, lazy load |
| `@dagrejs/dagre` + own SVG/Canvas | 3.1.1 | any | layered layout only, simple to control, good for load order DAGs |
| d3-force + Canvas | 3.0.0 | Canvas 2D | force layout for exploratory graphs, own rendering |
| sigma 3.0.3 + graphology 0.26.0 | | WebGL | rejected for WebKitGTK reasons above |
| `@xyflow/react` 12.12.0 | | React DOM nodes | React only; node editor style, not for 1000 nodes |
| elkjs 0.12.0 | | layout only | licence EPL-2.0 or GPL-3.0-or-later, compatible choice is EPL, heavy (7.9 MB unpacked) |
| mermaid 12.1.0 | | SVG | for docs, not for interactive graphs |

Decision: Cytoscape.js (Canvas 2D, built in pan, zoom, selection, hit testing) as a lazily loaded chunk, dagre layout by default for "why is this mod before that one" views, fcose or d3-force for overview. A mod list has hundreds to about 1000 nodes (the owner's install has 615 active mods), which Canvas 2D handles; the performance budget is an interaction, not a number measured here.

### 4.6 Code and XML editor

| Candidate | Version | Size and shape | Notes |
| --- | --- | --- | --- |
| CodeMirror 6 | `codemirror` 6.0.2, `@codemirror/state` 6.7.6, `view` 6.43.13, `lang-xml` 6.1.0, `lang-json` 6.0.2, `merge` 6.12.2, `lint` 6.9.7, `autocomplete` 6.20.3, `search` 6.7.2, `commands` 6.11.1 | modular, view is 1.2 MB unpacked, no workers needed | framework agnostic, mount into a ref; same family as RimCrow (it also adds C# via `@replit/codemirror-lang-csharp` 6.2.0) |
| Monaco | `monaco-editor` 0.57.0 | 99 MB unpacked, web workers, AMD or ESM worker plumbing | IntelliSense for JS and JSON, but heavy for XML work and CSP plus worker setup under Tauri; `@monaco-editor/react` is React only |
| Ace | `ace-builds` 1.44.0 | older architecture | no |

Decision: CodeMirror 6. Needs mapped to packages: XML syntax and tag matching (`lang-xml`), schema aware tag and attribute completion fed by the def explorer data through `@codemirror/autocomplete` (lang-xml accepts element and attribute descriptions; unverified, fall back to a custom completion source), diagnostics from the Rust XML boundary crate through `@codemirror/lint`, side by side patch preview through `@codemirror/merge`, JSONC editing for settings (`lang-json` may flag comments as errors, unverified: use a small stream parser or a JSON mode with a comment-tolerant lint source). XML parsing in the frontend is limited to syntax highlighting (the Lezer grammar); all semantic XML work stays in the backend boundary crate (R10). Fast-XML-parser 5.11.2 and similar JS XML libraries must not be added to the frontend.

### 4.7 BBCode, markdown and sanitising

Observed inputs: Steam workshop descriptions use Steam BBCode; RimWorld `About.xml` descriptions in the corpus include Unity style rich text such as `<size=20>` (visible in the dataset built from the owner's workshop folder, `docs/research/data/frontend-stack/gen_mods_dataset.py`), and changelogs are often markdown.

| Task | Candidates | Choice |
| --- | --- | --- |
| BBCode to tree | `@bbob/core` 4.4.1 + `@bbob/preset-html5` + `@bbob/html` (no Preact package: `@bbob/preact` is a 404), `bbcode-parser` 1.0.10 (2015), own parser | `@bbob/core` with a custom Steam tag preset producing an AST; own renderer maps the AST to Preact vnodes |
| Unity rich text | none | tiny own tokenizer for `<size>`, `<color>`, `<b>`, `<i>` |
| Markdown | `marked` 18.0.14, `markdown-it` 15.0.2, `micromark` 4.0.3, `snarkdown` 2.0.0 (2020) | `marked` for tokens, rendered to vnodes; `markdown-it` is the fallback (RimCrow uses it) |
| Sanitising | DOMPurify 3.4.16 (MPL-2.0 or Apache-2.0), `sanitize-html` 2.18.0, native Sanitizer API (false in the lab) | rendering to vnodes needs no sanitiser; run DOMPurify only where raw HTML must be inserted |
| Images and links in text | n/a | allow list schemes (`https`, `steam`), `rel=noopener`, images through a proxy command or the image protocol, never remote `http` fetched directly by the webview (CSP) |

Using a tree to vnode renderer means untrusted text never reaches `innerHTML`. The Tauri CSP (companion note) is the second line of defence.

### 4.8 Icons and fonts

Icons: `lucide-preact` 1.52.0 (ISC, peer `preact ^10.27.2 || ^11.0.0-0`), each icon a separate module so unused icons are dropped; fallbacks `@tabler/icons-preact` 3.48.0 (MIT, peer `^10.5.13`) and the CSS-only route `@iconify/tailwind4` 1.2.3 with `@iconify-json/lucide` 1.2.139 (masks, zero JS, monochrome only). Draw a small custom SVG set for RimWorld concepts (weapon, apparel, def) in the same stroke style.

Fonts (all self hosted through Fontsource, SIL OFL 1.1, version 5.3.0 published 2026-07-19 unless noted):

| Font | Package | Unpacked | Role and note |
| --- | --- | --- | --- |
| Barlow | `@fontsource/barlow` | 1579 kB (all subsets and weights; import only latin 400, 500, 600) | UI text; owner's earlier choice; no variable build |
| Barlow Condensed | `@fontsource/barlow-condensed` | 1540 kB | headings, labels, blueprint annotations |
| Azeret Mono | `@fontsource-variable/azeret-mono` | 102 kB | numbers, ids, code; variable, owner's earlier choice |
| IBM Plex Sans Condensed + Plex Mono | `@fontsource/ibm-plex-sans-condensed`, `ibm-plex-mono` | 1269 kB (condensed) | alternative technical look |
| Chakra Petch, Share Tech Mono, Oxanium | `@fontsource/chakra-petch` (774 kB), `share-tech-mono` (40 kB), `@fontsource-variable/oxanium` (39 kB) | | more "instrument panel", weaker long text legibility |
| JetBrains Mono, Geist Mono | `@fontsource-variable/jetbrains-mono` (199 kB), `geist-mono` (168 kB) | | editor fonts; offer as an editor font choice |

Decision: Barlow plus Barlow Condensed plus Azeret Mono, with `font-display: swap`, latin subset only by default and extra subsets (cyrillic, latin-ext, and so on) loaded when the locale needs them (Fontsource ships per subset CSS entries; exact entry names to be confirmed at install time, unverified). CJK mod names fall back to system fonts through the font stack; do not bundle CJK fonts.

## 5. Frontend modularity

### 5.1 One app or workspace packages

| Option | Pros | Cons |
| --- | --- | --- |
| Single Vite app, feature-sliced folders | one build graph, fastest dev loop, simplest Tauri wiring | boundaries are conventions unless enforced |
| pnpm workspace with a package per feature | hard boundaries by package.json | many build configs, duplicated tooling, slower HMR, little reuse in a single product |
| Hybrid (recommended) | feature folders inside one app, plus few packages where code is truly shared or has a different test and release profile | needs discipline about what becomes a package |

Recommended layout (names use the required prefix; folders are illustrative):

```
apps/desktop/src/
  app/            shell, providers, routing signal, theme application
  features/
    mod-manager/  modlist, sorting, conflicts, profiles
    workspace/    mod project workspace, def explorer, xml and patch tools
    designer/     item designer (weapons, apparel), calibration quiz
    workshop/     upload and update tool
    settings/     paths, custom mod folders, datasets, appearance
  shared/         ipc, i18n, dnd, charts, lists, rich-text (no feature imports)
packages/
  rimstudio-ui/        design system components, tokens, gallery stories (no app imports)
  rimstudio-ipc-types/ generated bindings (output of the backend generator)
  rimstudio-testkit/   mock IPC, fixtures, render helpers
```

```mermaid
flowchart TD
  app --> features
  features --> shared
  features --> ui[rimstudio-ui]
  shared --> ui
  shared --> types[rimstudio-ipc-types]
  features -.x.-> features
```

Rules: a feature may import `shared` and `rimstudio-ui`, never another feature; cross feature needs go through `shared` or through an event registered in `app`; `rimstudio-ui` imports nothing from the app; a feature exposes only its `index.ts`.

### 5.2 Enforcing boundaries

| Tool | Version | What it does | Use |
| --- | --- | --- | --- |
| oxlint `no-restricted-imports` with `patterns` | 1.86.0 | fast per file import ban, editor feedback | primary: ban `features/*/` deep imports and cross feature imports through overrides |
| dependency-cruiser | 18.5.0 | graph rules, cycles, orphans, forbidden folder edges, report output | CI gate: `forbidden` rules mirroring the diagram, `no-circular`, and an HTML or dot graph artifact |
| knip | 6.39.0 | unused files, exports, dependencies | CI warning |
| eslint-plugin-boundaries 7.2.0, `@softarc/eslint-plugin-sheriff` 0.20.0 | | element type rules | not recommended: needs ESLint, which the owner replaced with oxlint |
| madge 8.0.0 | | cycles | stale (2024), superseded by dependency-cruiser |
| Nx 23.2.1 | | project graph and tags | too heavy for one app |

### 5.3 Component gallery

Storybook has a Preact framework package (`@storybook/preact-vite` resolves to 10.6.1 on the registry) but brings a large dependency tree and its own Vite pipeline. Histoire's Vite plugin is not on the registry under `@histoire/plugin-vite` (404). Recommended approach: a dev-only route `/gallery` inside the app (excluded from production by `import.meta.env.DEV` and a dynamic import) that renders each `rimstudio-ui` component in all variants and states from a typed story list, with a theme, accent, density and locale switcher, and Playwright screenshot tests over it for both themes. This doubles as the visual regression suite and costs one folder.

## 6. Recommended stack

### 6.1 Table

| Package | Exact pin | Role | Why | Risk | Fallback |
| --- | --- | --- | --- | --- | --- |
| preact | 10.29.8 | UI runtime | owner's line, stable, 11 ready code | low | 11.0.x after the gate in 2.2 |
| @preact/signals | 2.11.3 | reactive state | first party, supports 10 and 11 | low | nanostores 1.5.4 |
| @preact/preset-vite | 2.10.6 | JSX, HMR | owner's line, Vite 8 declared | low | plain esbuild or oxc JSX transform |
| vite | 8.3.2 | dev server, bundler | owner's line | low | none needed |
| typescript | 6.0.3 | type checking | API needed by tooling | low | 7.0.2 when 7.1 API exists |
| tailwindcss, @tailwindcss/vite | 4.3.3 | styling | owner requirement | low | none |
| tailwind-variants | 3.3.1 | variants and slots | Tailwind 4 aware | low | class-variance-authority 0.7.1 |
| tailwind-merge | 3.7.0 | class conflict merge | v3 line is Tailwind 4 | low | drop merging, rely on variants |
| oxlint | 1.86.0 | lint, boundaries | owner convention | medium (fast moving) | Biome 2.5.15 |
| prettier, prettier-plugin-tailwindcss | 3.9.9, 0.8.1 | format, class sort | documented sorter | low | oxfmt 0.71.0 |
| vitest, jsdom | 5.0.3, 30.1.2 | unit tests | declares Vite 8 | low | happy-dom 20.14.5 |
| @testing-library/preact (+ dom 10.4.2, user-event 14.6.7, jest-dom 7.0.1) | 3.2.4 | component tests | only maintained RTL adapter | medium (last release 2024-05) | own tiny render helper |
| @playwright/test | 1.63.0 | web layer e2e, screenshots | mocks IPC | low | Vitest browser mode |
| valibot | 1.5.0 | file validation | modular | low | zod 4.6.5 |
| intl-messageformat | 12.1.2 | ICU messages | translators know ICU | low | i18next 26.4.2 |
| tinykeys | 4.0.1 | key chords | tiny | low | own parser |
| @tanstack/virtual-core | 3.17.11 | list windowing | agnostic core | medium (own hook) | own windowing, virtua |
| @dnd-kit/dom (+ abstract) | 0.5.0 | drag and drop | pointer based | high (0.x, spike) | own pointer engine |
| @zag-js/preact and machines | 1.44.0 | headless components | Preact binding | medium | own components, compat route |
| @floating-ui/dom | 1.8.0 | positioning | standard | low | native anchor positioning |
| chart.js, chartjs-plugin-annotation | 4.5.1, 3.1.0 | charts | one lib for 3 types | medium (5k spike) | uPlot 1.6.32, own Canvas |
| cytoscape (+ cytoscape-dagre 4.0.1) | 3.34.3 | graphs | Canvas 2D | medium | d3-force 3.0.0 + dagre 3.1.1 |
| codemirror and `@codemirror/*` | 6.0.2, state 6.7.6, view 6.43.13, lang-xml 6.1.0, lang-json 6.0.2, merge 6.12.2, lint 6.9.7, autocomplete 6.20.3 | editors | small, modular | low | none |
| @bbob/core | 4.4.1 | BBCode parse | extensible AST | low | own parser |
| marked | 18.0.14 | markdown | fast | low | markdown-it 15.0.2 |
| dompurify | 3.4.16 | sanitiser backstop | standard | low | sanitize-html 2.18.0 |
| lucide-preact | 1.52.0 | icons | Preact peer incl. 11 | low | @tabler/icons-preact 3.48.0 |
| @fontsource/barlow, @fontsource/barlow-condensed, @fontsource-variable/azeret-mono | 5.3.0 | fonts | owner's fonts, OFL | low | system fonts |
| @tauri-apps/api, @tauri-apps/cli | 2.12.1 | IPC client, tooling | matches `tauri` crate 2.12.1 | low | none |
| dependency-cruiser, knip | 18.5.0, 6.39.0 | boundaries, dead code | CI | low | none |

Pin exact versions in `package.json` for the app (no carets) and use the pnpm lockfile; update on a schedule, not on every install. Tauri plugin packages (dialog 2.8.1, fs 2.6.0, store 2.5.0, opener 2.7.0, window-state 2.5.0, updater 2.13.1, log 2.10.0, os 2.4.0, clipboard-manager 2.4.1) are listed in the companion note.

### 6.2 Do not use

| Item | Reason |
| --- | --- |
| Monaco editor | 99 MB unpacked, workers, React wrapper only, overkill for XML |
| Any WebGL chart or graph library as the only path (sigma, regl-scatterplot, deck.gl) | WebKitGTK can fall back to a slow path silently (Tauri Linux graphics page) |
| React plus `preact/compat` as the default | doubles the dependency tree; use native Preact libraries first |
| `react-beautiful-dnd` 13.1.1 | deprecated; the maintained fork and dnd-kit core are React only |
| Pragmatic drag and drop or SortableJS for in-app reordering | HTML5 DnD collides with Tauri's file drop handler on Windows |
| Two virtualisers (RimCrow carries two) | one windowing hook, one set of bugs |
| Any XML parser, XML catalog or XML config in the frontend (fast-xml-parser, xmldom, xmlbuilder2, xmllint-wasm) | R10: XML only in the boundary crate |
| `localStorage` for data that matters | per viewer and fragile; use backend JSON settings |
| Runtime CSS in JS (styled components style libraries) | Tailwind plus variables covers it |
| `tw-animate-css`, `@tailwindcss/forms` by default | pull styles that fight the blueprint tokens |
| Copying RimSort, RimCrow or CE UI code or data | R11 |

### 6.3 Minimal configuration snippets

`package.json` scripts (all tools run from any cwd with pnpm):

```json
{
  "name": "rimstudio-desktop",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "lint": "oxlint && depcruise src --config .dependency-cruiser.cjs",
    "format": "prettier --write .",
    "test": "vitest run",
    "e2e": "playwright test"
  }
}
```

`vite.config.ts`:

```ts
import { defineConfig } from 'vite'
import preact from '@preact/preset-vite'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [preact(), tailwindcss()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { sourcemap: false } // keep the default target (Safari 16.4 floor)
})
```

`tsconfig.app.json` (TS 7 clean):

```json
{
  "compilerOptions": {
    "target": "ES2022", "module": "ESNext", "moduleResolution": "bundler",
    "jsx": "react-jsx", "jsxImportSource": "preact",
    "strict": true, "noEmit": true, "skipLibCheck": true,
    "verbatimModuleSyntax": true, "isolatedModules": true,
    "types": ["vite/client"], "paths": { "@/*": ["./src/*"] }
  },
  "include": ["src"]
}
```

`.oxlintrc.json` (extends the owner's file):

```jsonc
{
  "$schema": "./node_modules/oxlint/configuration_schema.json",
  "plugins": ["react", "typescript", "oxc", "import"],
  "rules": {
    "react/rules-of-hooks": "error",
    "import/no-cycle": "error",
    "no-restricted-imports": ["error", { "patterns": [
      { "group": ["**/features/*/*"], "message": "import a feature through its index only" }
    ] }]
  }
}
```

(The `patterns` shape is taken from the rule source; the `import` plugin name and rule key are unverified until run.)

Preact 11 test leg: a second Vitest run with `pnpm.overrides` `{ "preact": "11.0.0" }` in a CI matrix.

## 7. Risk register

| # | Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- | --- |
| 1 | Preact 11.0.0 early bugs or library lag | medium | medium | stay on 10.29.8, CI leg on 11, gate in 2.2 |
| 2 | `@dnd-kit/dom` 0.x API churn or no virtualised list support | medium | high | facade plus spike; own pointer engine ready; keyboard commands independent of the engine |
| 3 | Zag Preact binding gaps for a needed component | low | medium | wrap behind `rimstudio-ui`; own component fallback |
| 4 | Chart.js misses the 5k point budget on the oldest webview | medium | medium | spike with the budget; uPlot or own Canvas ready |
| 5 | `@testing-library/preact` unmaintained | medium | low | tiny own helper is about 30 lines over `preact` render |
| 6 | TS 7 tooling mismatch (no API until 7.1) | medium | low | stay on 6.0.3; TS 7 only for `tsc --noEmit` |
| 7 | Old WebKitGTK on LTS distros lacks CSS Tailwind 4 needs | medium | high | startup feature probe and a clear unsupported message (companion note, section on webview realities) |
| 8 | oxlint config features (overrides, import plugin) differ from assumptions | medium | low | verify at setup; dependency-cruiser carries the hard gate |
| 9 | Supply chain: many small packages and fast moving 0.x tools | medium | medium | exact pins, lockfile, scheduled updates, `pnpm` audit in CI, minimal dependency count |
| 10 | User supplied themes or BBCode inject unwanted styles or content | low | medium | token allow list and value grammar; vnode renderer; CSP |
| 11 | Font payload bloat (Fontsource packages include all subsets) | low | low | import only latin entries; check dist size in CI |
| 12 | Peer range warnings with Preact 11 (TanStack Preact packages) | medium | low | avoid those packages in the base stack |

## Implications for RimStudio

1. Start the frontend on `preact@10.29.8`, `@preact/signals@2.11.3`, Vite 8.3.2, TypeScript 6.0.3, Tailwind 4.3.3 with exact pins; add a CI test leg that runs the unit suite on Preact 11.0.0 and adopt 11.0.x only after the gate in section 2.2.
2. Write all components 11 clean: no `defaultProps`, no `Component.base`, no `replaceNode`, no implicit `px` styles, `ref` as an ordinary prop.
3. Define the whole visual system as CSS variables mapped through `@theme inline`; light, dark, user accent (one variable) and user themes (JSONC token files validated by valibot) must work without a rebuild; the production CSS contains no hard coded colours outside the token file.
4. Declare `@layer user` last and inject optional user CSS only there.
5. Build one IPC data module (`shared/ipc`) with `query`, `stream` and snapshot plus delta primitives, rAF batched signal updates and cancellation tied to component lifetime; no component calls `invoke` directly.
6. Use one windowing hook over `@tanstack/virtual-core` for every long list; mod lists of 5000 rows must keep the DOM node count of the list under 3000.
7. Do not use HTML5 drag and drop for in-app reordering; keep Tauri `dragDropEnabled` true for OS file drops; deliver pointer based drag and drop behind `shared/dnd` plus keyboard move commands (Alt+Up, Alt+Down, to top, to bottom) and a context menu entry for each.
8. Wrap every third party widget (Zag machines, Chart.js, Cytoscape, CodeMirror) once inside `rimstudio-ui` or `shared`, so that the library choice stays swappable and boundary rules can ban direct imports from features.
9. Enforce architecture with oxlint `no-restricted-imports` per folder plus a dependency-cruiser CI gate (no cycles, no feature to feature imports, `rimstudio-ui` imports nothing from the app).
10. Keep XML out of the frontend dependency tree: no JS XML parser or serializer, no XML catalogs; i18n catalogs are flat JSON, validated by a script that fails the build on missing English keys.
11. Render all untrusted descriptions (BBCode, Unity rich text, markdown) to vnodes from an AST; DOMPurify only as a backstop; remote images only through the backend or the image protocol.
12. Ship fonts self hosted (Barlow, Barlow Condensed, Azeret Mono, latin subset by default); the app must work offline with no external font request.
13. Provide a dev-only `/gallery` route and Playwright screenshot tests in both themes for every `rimstudio-ui` component.
14. Gate the first release candidates on three spikes recorded in the repository: drag between two virtualised lists (5 selected rows, 16 ms frame), Chart.js with 5000 plus 5000 points (16 ms hover redraw), Cytoscape with 1000 nodes (smooth pan and zoom).

## Open questions

1. Does `@dnd-kit/dom` 0.5.0 support auto scroll inside a custom virtualised scroller and drags that start in one list and end in another with unmounting source rows? (spike required)
2. Which exact `@zag-js` machine and API provides a context menu trigger in the Preact binding? (the package list shows `menu`; the trigger API was not read)
3. Does the installed oxlint 1.86.0 support config `overrides` with `no-restricted-imports` patterns and the `import` plugin cycle rule exactly as written? (verify when the repository is created)
4. Does `prettier-plugin-tailwindcss` 0.8.1 fully support Tailwind 4 CSS-first configuration (entry point option)? (documented as supported in the plugin, not run here)
5. Does `@codemirror/lang-json` accept comments for JSONC editing, or is a custom parser needed?
6. Should Preact 11 become the default at a fixed date (for example 2026-11-01) or only when ecosystem packages such as `@tanstack/preact-query` widen their peer range?
7. Is a Vitest browser mode run in Playwright WebKit representative enough of WebKitGTK to be worth the CI time?
8. Fontsource subset entry file names and total shipped font bytes for latin only (to confirm at install time).
