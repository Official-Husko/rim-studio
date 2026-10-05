import type {
  ArchetypeProposalDto,
  DesignerArchetypeApplyResponse,
  DraftDto,
  DraftEntryDto,
} from 'rimstudio-ipc-types';
import * as api from '../../api';
import * as wizardApi from './wizard-api';

/**
 * Apply a proposal to a blank draft and store it. The optional Combat Extended block is not
 * filled in: its presence is what switches the patch on, and the wizard never does that (D-085).
 * A Combat Extended ammo set only shapes the numbers and is kept in the choice of the draft.
 */
export async function createEntry(
  projectId: string,
  blank: DraftDto,
  proposal: ArchetypeProposalDto,
): Promise<{ entry: DraftEntryDto; response: DesignerArchetypeApplyResponse }> {
  const response = await wizardApi.archetypeApply(blank, proposal, false, true);
  const saved = await api.draftSave(projectId, response.draft);
  const entry: DraftEntryDto = {
    id: saved.id,
    defName: response.draft.spec.identity.defName,
    label: response.draft.spec.identity.label,
    kind: blank.kind,
    updatedAtMs: saved.savedAtMs,
    draft: response.draft,
  };
  return { entry, response };
}
