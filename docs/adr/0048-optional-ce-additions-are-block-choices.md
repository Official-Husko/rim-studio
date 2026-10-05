# ADR 0048: Optional Combat Extended additions are choices of the block

Status: accepted | Last updated: 2026-10-05 | Register: D-146, D-085, D-095

## Context

The round trip harness compares the generated patch with Combat Extended's own conversions of the owner's install. After the structure fidelity work a classified remainder was left: ammo comp extras, a verb recoil pattern, restructured tool lists, a tool child that conversions drop, weapon tags beyond the class tag, art nodes, and economy and art changes. The muzzle habit was applied silently, which turned a tool list of the design into a different one without the user asking, and 47 extras of the harness were that muzzle tool.

## Decision

Each remaining difference is a field of the optional Combat Extended block, or a policy:

- The fields `reloadOneAtATime`, `ammoGenPerMag`, `recoilPattern`, `toolPlan`, `keepToolFields`, `extraTags` and `rawExtras` are written only when the block holds them. The generator never fills one in on its own.
- A restructured tool list is a decision: the patch keeps the tools of the design, lists what converted weapons do as hints, and the suggestion `tool-plan` turns it into an explicit plan when the user accepts it.
- A tool child that at least 75 percent of the converted weapons drop (at least 2 twins carry it) is removed and reported as a derived value; `keepToolFields` keeps it.
- Habits (companion tags, recoil pattern, one at a time reload) are offered by `suggest_options` from agreeing examples (2 guns, 60 percent of the class) and written by `accept_options`; a field the block holds is never overwritten.
- Raw nodes are validated for their shape and merged with guards, so art and platform specific additions are written without being modelled.
- Costs, stuff categories and the draw size stay what the design says; every new conversion carries one info diagnostic that says so.

## Consequences

- The plain conversion has fewer extras (47 to 12 on the owner's install) and the user decides about the muzzle tool.
- With the additions stated explicitly the harness finds only policy, one tag the conversion removes, and habit extras.
- The block grows seven optional fields; the TypeScript bindings and the IPC DTO of the spec do not carry them yet.

## Alternatives rejected

- Keep the muzzle habit silent: it changes what the design says.
- Suggest every tag that two guns carry: the tags follow the weapon, not the class, so such a suggestion would mostly be wrong.
- Write costs, stuff categories and draw size to match Combat Extended: against D-085.

## Evidence

- [Remaining differences, measured](../research/ce-remaining-0.1.0.md)
- [Combat Extended patching](../features/combat-extended-patching.md) section 14.16
- [ADR 0037](0037-vanilla-default-optional-ce-patch.md)
