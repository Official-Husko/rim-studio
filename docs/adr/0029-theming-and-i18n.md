# ADR 0029: Theming and internationalisation

Status: accepted | Last updated: 2026-10-04 | Register: D-055, D-056

## Context

R8 asks for a modern Blueprint-inspired UI with user-visible theming; R10 bans XML catalogs. Old webviews (WebKitGTK) lack some CSS features and backdrop filters are costly on large panels.

## Decision

All colours are CSS variables mapped through Tailwind `@theme inline`; light, dark and system modes; one accent variable; user themes are JSONC token files validated by valibot; `@layer user` comes last; fonts are self-hosted; no backdrop filters on large panels; a startup feature probe shows a clear page on unsupported webviews. i18n is a thin layer over `intl-messageformat` 12.1.2 with flat semantic-key JSON catalogs (`en.json` is the source, other languages optional and lazy), a generated key union and a CI completeness check. Game text is never translated.

## Consequences

- Tokens live in `styles/tokens.css` and are the interface to the later design prompt (R8).
- Translators work with ICU messages in JSON.
- Catalog completeness gates the build.

## Alternatives rejected

- Runtime CSS in JS.
- i18next, Paraglide or Fluent (not JSON).

## Evidence

- [Frontend stack research](../research/frontend-stack-research.md) implications 3, 4 and 12 and section 3.3
- [Webview and IPC performance](../research/webview-and-ipc-performance.md) implication 9
