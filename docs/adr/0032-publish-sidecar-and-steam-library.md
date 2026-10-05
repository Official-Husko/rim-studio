# ADR 0032: Publish sidecar protocol and the user's Steam library

Status: accepted | Last updated: 2026-10-04 | Register: D-061, D-062, D-063, D-086

## Context

R9 adds Workshop publishing later, based on the owner's Parallax project. Valve's native library cannot sit in the main process without widening risk, and EResult 9 against app 294100 is unresolved until spike S-04. The owner decided on 2026-10-04 (D-086) that RimStudio uses the Steam client the user already has installed and ships no Valve library.

## Decision

Valve's native library lives only in the sidecar `rimstudio-steam-helper` (steamworks 0.13.1 behind a `SteamBackend` trait), spawned per operation and speaking newline JSON protocol v1: one terminal event, lines of at most 16 KiB, a 120 s watchdog. The path is resolved beside the executable and spawned through `Launcher`, not `tauri-plugin-shell`, so the CLI can use it too. The helper does not ship Valve's redistributable library and RimStudio does not ship its own Steam: detection goes through the user's own Steam files, launching and publishing go through the user's installation and RimWorld's own steam library (loaded at run time, for example via `libloading`), and the app works without any of it. Spike S-04 must still prove that publishing works this way; the fallback is to ask the user for the library path. Upload ignore patterns live in project JSONC key `uploadIgnore` (gitignore syntax via `ignore`), with defaults excluding `.git`, `Source`, `Raw Assets`, `.rimstudio`, `*.pdb` and `.vs`; project metadata defaults to the data root and never sits inside a shipped folder.

## Consequences

- S-04 gates M6 and its only open question is whether publishing works through the user's installation; there is no redistribution question left.
- A user without Steam installed sees the workshop feature as unavailable with an explanation.
- The publisher can move earlier in the roadmap because the architecture supports it (owner decision).
- Staging copies, not the source folder, are uploaded.

## Alternatives rejected

- The shell plugin (wider permission surface).
- In-process libsteam_api.
- Bundling Valve's library under the owner's Steamworks agreement, and shipping an own Steam (declined by D-086).
- `.rimstudioignore`; metadata under `Config/`, which would ship to every subscriber.

## Evidence

- [Workshop publishing research](../research/workshop-publishing-research.md) sections 6.1 and 8 and implication 11
- [Modding toolkit scope](../research/modding-toolkit-scope.md) section 3.3
