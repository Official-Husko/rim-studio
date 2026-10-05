# ADR 0005: Ports in core and a cfg quarantine in rimstudio-platform

Status: accepted | Last updated: 2026-10-04 | Register: D-009, D-010

## Context

Detection, link deployment and launching differ per OS (registry, junctions versus symlinks, process names, credential stores). Both reference managers scatter OS checks across the code. CI must test detection and deployment on machines without Steam.

## Decision

Port traits (Clock, EnvProbe, RegistryProbe, FsProbe, ProcessProbe, LinkBackend, Launcher, CredentialStore, SandboxProbe, InstallSourceProbe, DetectEnv) live in `rimstudio-core::ports`. `rimstudio-platform` implements all of them and is the only crate allowed `cfg(target_os)` and OS dependencies; the other permitted places are the shell window code, the helper's library path lookup and xtask packaging. Consumers receive `&dyn` ports through `AppContext`. Behaviour that is data (default paths, executable names) uses `Os::current()`. `cargo xtask check-cfg` fails the build on any other use.

## Consequences

- Three-OS parity is testable with fake ports from `rimstudio-testing`.
- Dynamic dispatch cost is negligible at these call rates.
- Platform code cannot depend upward on consumers, which is why traits are not placed in consumer crates.

## Alternatives rejected

- Traits per consumer crate.
- Generics everywhere (compile time and signature noise).
- Scattered cfg blocks.

## Evidence

- [Steam and game detection](../research/steam-and-game-detection.md) section 7.2
- [Cross-platform packaging](../research/cross-platform-packaging-research.md) section 6
- [Cross-platform architecture](../architecture/cross-platform.md)
