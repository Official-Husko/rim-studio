# ADR 0004: No feature-to-feature edges and a compile-time toolkit registry

Status: accepted | Last updated: 2026-10-04 | Register: D-008, D-066

Amended by [ADR 0038](0038-toolkit-shared-module-and-project-backups.md): the toolkit also has a neutral `shared` module that every tool may use.

## Context

The toolkit holds many tools (Def Explorer, patch tester, project tools, item designer) and the manager is a sibling feature. The toolkit research proposed one crate per tool, which adds eight or more crates, and runtime plugins were considered for extensibility.

## Decision

Feature crates (manager, toolkit, publish) never depend on each other; shared logic lives in services or engines below them. The toolkit is one crate with a module and a Cargo feature per tool (`tool-defs`, `tool-project`, `tool-designer`); a tool becomes its own crate only when it brings a heavy dependency. The toolkit registry is a compile-time table of `ToolDescriptor`s with required capabilities in `rimstudio-app::tools`, mirrored by `apps/desktop/src/app/tools.ts`; `xtask check-tools` compares the ids. Tools whose capabilities are unmet (for example the designer when Combat Extended is absent) are hidden or degrade with an explanation. No dynamic plugin loading in the first releases.

## Consequences

- Module isolation inside the toolkit is enforced by an xtask grep, not by the compiler.
- Adding a tool is a descriptor, a module, a frontend feature folder and a `new-tool` scaffold.
- Third-party extension is out of scope until a stable need appears.

## Alternatives rejected

- One crate per tool.
- Runtime plugin loading (large security and ABI surface).
- Features calling each other through traits.

## Evidence

- [Modding toolkit scope](../research/modding-toolkit-scope.md) sections 5.1 and 5.3
- [Reference architectures](../research/reference-architectures.md) enforceable rules
- [Crate catalog](../architecture/crate-catalog.md)
