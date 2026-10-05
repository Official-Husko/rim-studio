# ADR 0026: Preact 10 now, Preact 11 behind a gate

Status: accepted | Last updated: 2026-10-04 | Register: D-050

## Context

R1 names Preact. Preact 11 is days old, has no patch release and `@tanstack/preact-query` peers `^10`. No 11 feature is needed for the first releases.

## Decision

Start on exact pins: preact 10.29.8, @preact/signals 2.11.3, vite 8.3.2, typescript 6.0.3, tailwindcss 4.3.3, @tauri-apps/api and cli 2.12.1. Write code that is Preact 11 clean and run a CI leg against 11.0.0. Promote to 11.0.x when a patch release exists or 30 days have passed (earliest 2026-10-30), the leg is green and the pinned libraries report no problems. The proposed review date is 2026-11-01 (spike S-13), which the owner may move.

The CI leg on Preact 11 is non-blocking until the gate date; its result is recorded in the repository so the promotion is a documented act and not a hunch. Signals and the own data layer are the only parts of the stack that touch Preact internals, which keeps the surface small.

## Consequences

- The upgrade is a one-line change if the leg stays green.
- The hand-built `shared/lists` hook and own query layer avoid peers that exclude 11.

## Alternatives rejected

- Start on Preact 11 now.
- React: heavier and not the owner's stack.

## Evidence

- [Frontend stack research](../research/frontend-stack-research.md) section 2.2
- [Frontend architecture](../architecture/frontend-architecture.md)
