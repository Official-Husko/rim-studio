# ADR 0012: Data roots, portable mode, settings files and secrets

Status: proposed | Last updated: 2026-10-04 | Register: D-024, D-030

## Context

The identifier and roots fix where data lives; the research found settings in the reference managers mixed with caches and tokens. R4 needs custom mod folders persisted somewhere safe.

## Decision

Four roots (config, data, cache, logs) are resolved once by the shell and passed in; core crates never discover directories. A marker file `rimstudio.portable` beside the executable moves everything under `./data` (this disables the Windows updater). Settings are split: `settings.jsonc` for app preferences and `workspace.jsonc` for path overrides, mod sources, custom folders, launch arguments and dataset choices (it holds the `paths` override section of the Steam note). Secrets go to the OS credential store behind `CredentialStore` with a permission-restricted fallback file. Status is proposed because the `keyring` 4.2.0 feature set is unverified and the owner must confirm keychain use.

## Consequences

- Custom folders (R4) persist in `workspace.jsonc`, any number, with per-folder flags.
- Changing the app identifier after release moves data directories (see ADR 0031).
- The fallback file must never be world-readable.

## Alternatives rejected

- One settings file.
- Tokens inside settings.
- `directories` inside domain crates.

## Evidence

- [RimSort settings catalogue](../research/rimsort-settings-catalog.md) implications 2 and 3
- [RimCrow analysis](../research/rimcrow-analysis.md) implication 13
- [Data and persistence](../architecture/data-and-persistence.md)
