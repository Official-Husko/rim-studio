# ADR 0027: Frontend layout, state layer and enforcement

Status: accepted | Last updated: 2026-10-04 | Register: D-051, D-052, D-057

## Context

R2 demands modular components on the frontend as well. In the references, separate packages appear only when a second consumer exists. Snapshot plus delta streams do not fit a request cache, and TanStack Query peers exclude Preact 11.

## Decision

One Vite app `apps/desktop` with feature folders (manager, settings, logs, workspace, project, designer, workshop, onboarding), `shared` and `app`; three packages `rimstudio-ui`, `rimstudio-ipc-types` and `rimstudio-testkit`. A feature imports only shared, `rimstudio-ui` and the contract, never another feature; `shared/platform` is the only module importing `@tauri-apps/*`. State uses `@preact/signals` plus an own `shared/ipc` layer (query, stream, snapshot plus delta, cancellation; about 150 lines) with rows kept in a map of signals; TanStack Query is a fallback only. Enforcement: oxlint `no-restricted-imports` per folder (spike S-12 on oxlint 1.86.0), dependency-cruiser as the CI graph gate, knip as a warning, and a dev-only `/gallery` with Playwright screenshots in both themes.

## Consequences

- Folder rules are enforced by tooling, not review.
- The own data layer is maintained code.
- Packages exist only where there is a second consumer (`rimstudio-ui` is used by the gallery).

## Alternatives rejected

- A package per feature.
- Formal feature-sliced layers: no evidence.
- zustand or jotai through compat.
- ESLint boundary plugins and Storybook.

## Evidence

- [Frontend stack research](../research/frontend-stack-research.md) sections 3, 5.1 to 5.3
- [Frontend architecture](../architecture/frontend-architecture.md)
