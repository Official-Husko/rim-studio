import type { DraftDto, WritePlanDto, CustomAmmoDto } from 'rimstudio-ipc-types';
import type { EditorStore } from './editor-store';
import * as api from './output-api';
import { emptyCeBlock } from './output-model';

export interface AmmoPlannerDeps {
  editor: EditorStore;
  /** The id of the current project; undefined when none is open. */
  projectId: () => string | undefined;
  /** The derived values the plan takes now, as the output store sends them. */
  accepted: () => string[] | undefined;
}

/**
 * What the custom ammo window needs from the output panel: the open draft, whether a project is open and a
 * plan of the draft with a custom caliber in its block. Nothing is written and the draft is not changed.
 */
export function createAmmoPlanner(deps: AmmoPlannerDeps) {
  function currentDraft(): DraftDto | undefined {
    return deps.editor.draft.peek();
  }

  /** Plan the open draft as if its block held this custom ammunition. Rejects when no draft or project is open. */
  function planWithAmmo(custom: CustomAmmoDto): Promise<WritePlanDto> {
    const draft = currentDraft();
    const projectId = deps.projectId();
    if (!draft || !projectId) return Promise.reject(new Error('no draft or project is open'));
    const block = draft.spec.ce ?? emptyCeBlock();
    // the custom caliber stands in for the chosen set and projectile
    const { ammoSet: _set, defaultProjectile: _projectile, ...rest } = block;
    const withAmmo: DraftDto = {
      ...draft,
      spec: { ...draft.spec, ce: { ...rest, customAmmo: custom } },
    };
    const fields = deps.accepted();
    return api.exportPlan({
      projectId,
      draft: withAmmo,
      ...(fields ? { acceptSuggestions: { fields } } : {}),
    });
  }

  return { currentDraft, planWithAmmo, hasProject: () => deps.projectId() !== undefined };
}
