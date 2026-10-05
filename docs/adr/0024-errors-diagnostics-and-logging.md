# ADR 0024: Error envelope, diagnostic codes and logging

Status: accepted | Last updated: 2026-10-04 | Register: D-046, D-060

## Context

A mod manager reports many content problems that are not program failures: a broken About.xml, a missing dependency. Mixing them with errors loses information. Logs must help users report bugs without leaking Steam ids, and R12 style bans telemetry-like surprises.

## Decision

Each crate has one `thiserror` enum named `<Crate>Error` with a stable `code()`. The IPC envelope is `{code, message, errorId, details}` with redacted details. Content problems are `Diagnostic {code, severity, mod, file, message}` values, never `Err`. Codes are lowercase `<area>.<kebab-name>` with areas xml, defs, xpath, scan, rules, sort, list, author, deploy, steam, dataset, publish, ce, design and log; the golden snake_case def codes map by rule to `defs.*`; CE lint ids CEP001 to CEP022 stay stable. The UI translates codes by key. Logging uses `tracing` as rolling JSON lines in the logs root with no telemetry and redaction of Steam ids and tokens; `tauri-plugin-log` and sentry are rejected. A diagnostics page shows engine and session facts.

## Consequences

- Translators and tests key on codes, so renames are breaking changes.
- `anyhow` stays inside binaries and tests, never across the boundary.

## Alternatives rejected

- String errors.
- `anyhow` across the boundary.
- Log plugin or sentry.

## Evidence

- [RimCrow analysis](../research/rimcrow-analysis.md) implication 12
- [Def engine semantics](../research/def-engine-semantics.md) implication 3
- [Error handling and logging](../architecture/error-handling-and-logging.md)
