import { callCommand } from '~/shared/ipc';
import type {
  ArchetypeCatalogDto,
  ArchetypeProposalDto,
  DesignerArchetypeApplyResponse,
  DesignerArchetypeProposeRequest,
  DraftDto,
} from 'rimstudio-ipc-types';

// One function per command; no state and no rules. The wizard store calls these.

/** The taxonomy of weapon types and the choices of every descriptor. */
export function archetypeCatalog(includeCalibres: boolean): Promise<ArchetypeCatalogDto> {
  return callCommand('designer_archetype_catalog', { includeCalibres });
}

/** Every number of a weapon of the chosen type, with its reason and the verdict. Read only. */
export function archetypePropose(
  request: DesignerArchetypeProposeRequest,
): Promise<ArchetypeProposalDto> {
  return callCommand('designer_archetype_propose', request);
}

/** Write a proposal into a draft: typed values stay, the rest is filled. Not saved. */
export function archetypeApply(
  draft: DraftDto,
  proposal: ArchetypeProposalDto,
  includeCe: boolean,
  refreshStructure: boolean,
): Promise<DesignerArchetypeApplyResponse> {
  return callCommand('designer_archetype_apply', {
    draft,
    proposal,
    includeCe,
    refreshStructure,
    completeStructure: true,
  });
}
