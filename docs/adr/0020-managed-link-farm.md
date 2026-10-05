# ADR 0020: Managed link farm for custom mod folders

Status: accepted | Last updated: 2026-10-04 | Register: D-039

## Context

R4 lets users add any number of custom mod folders, for example on an external drive. The game builds its list only from the install Data folder, the install Mods folder and Steam-subscribed items, so a custom folder is invisible to it. Mono tests showed that links are listed and read.

## Decision

Custom folders reach the game through a managed link farm in `<install>/Mods`: a junction on Windows, a symlink elsewhere, with a copy fallback. An ownership manifest records every entry the app created and cleanup is unlink-only. Plans are dry-run first and a pre-launch resolve check confirms every active id will be found. The planner and manifest live in `rimstudio-library::deploy` as portable code; link primitives sit behind the `LinkBackend` port implemented in `rimstudio-platform`. The default deployment mode (link, copy or ask) is an owner decision. Spike S-03 (in-game test per OS, Steam verify behaviour, Flatpak grants) must pass before any release ships the feature.

## Consequences

- Without a link farm the game silently deactivates mods it cannot find.
- Mod folders of the user are never modified.
- Flatpak sandboxes may block links; copy fallback and a clear explanation are needed.

## Alternatives rejected

- Copy only: wasteful and stale.
- Move mods into Mods: destroys user layout.
- Replace the Mods folder with a link.
- `-savedatafolder`: moves user data only.

## Evidence

- [Steam and game detection](../research/steam-and-game-detection.md) section 8.6
- [Mod format and corpus](../research/rimworld-mod-format-and-corpus.md) implication 1
- [Mod manager spec](../features/mod-manager.md)
