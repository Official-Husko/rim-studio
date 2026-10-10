# ADR 0060: Removing the generated patch of a weapon

Status: accepted | Last updated: 2026-10-11 | Register: D-213

## Context

A weapon that already carries a Combat Extended conversion was only offered update mode: per field operations in a RimStudio owned override file layered on top of the conversion that is there ([combat extended patching](../features/combat-extended-patching.md) section 6). The owner asked on 2026-10-10 that existing patches can be deleted or overwritten. A conversion made with wrong answers, or one the modder no longer wants, could not be removed or regenerated from the application; the modder had to find the file in a file manager.

## Decision

1. A conversion that RimStudio generated can be removed from the project (D-213). A conversion request with `remove` set plans the removal and the ordinary `designer_apply_plan` job applies it, so the removal is reviewed as a diff, checked against the files on disk (a file changed since the plan is not overwritten) and backed up outside the mod folder like every other write.
2. Only patch files that carry the generator's mark are touched. A section is the comment `====== <defName> ======` and the run of operations after it that mention the definition; an operation a person added after it ends the section and stays.
3. A file that holds no operation after the removal is deleted (a new file action `delete`); any other file keeps every byte outside the removed sections.
4. A hand written patch, a patch of another mod and the definition files are never changed. The scan says where a conversion comes from (`existing.origin`) and which generated files a removal touches (`existing.files`).
5. Replacing a conversion is the removal followed by a new conversion on the same weapon, which is then a not converted weapon again. The Patches page offers Replace only when the conversion itself is RimStudio's.

## Consequences

- The removal needs no game install and no Combat Extended data: it works on the files of the project only.
- Update mode stays the way to change a conversion that RimStudio did not generate and one of another mod.
- The plan file actions gain `delete`; the scan candidates gain `existing`; the conversion request gains `remove`. All three are additive.
- The apply step skips the dry apply of a removal: it has no definitions to run a patch against.

## Alternatives rejected

- Two new commands for planning and applying a removal: the plan and apply pipeline already gives review, stale detection, guarded writes and backups.
- Regenerating a conversion while the old patch is still applied: the resolved definition then already carries the conversion, so the numbers read from it are converted ones and the patch would be built on the wrong base.
- Removing sections from hand written patches: there is no section structure to rely on, and a wrong cut breaks a file the modder owns.

## Evidence

- [Combat Extended patching](../features/combat-extended-patching.md) section 14.18, [decision register](../architecture/decision-register.md) D-213.
- Tests: `crates/rimstudio-toolkit/tests/patch_remove.rs`, the unit tests of `shared::merge`, `tests/e2e/specs/17-patch-remove.spec.ts`.
