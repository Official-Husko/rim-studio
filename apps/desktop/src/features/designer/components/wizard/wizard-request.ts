import type {
  ArchetypeDto,
  DesignerArchetypeProposeRequest,
  DescriptorsDto,
  DraftDto,
} from 'rimstudio-ipc-types';
import { applies, modeOf, type WizardChoice } from './wizard-model';

/**
 * The proposal request for a choice: only the descriptors the type takes, and in Combat Extended
 * mode the ammo set instead of the calibre class. With an open draft the request carries it, so
 * the numbers the user typed are marked and kept.
 */
export function buildRequest(
  archetype: ArchetypeDto | undefined,
  choice: WizardChoice,
  draft: DraftDto | undefined,
): DesignerArchetypeProposeRequest | undefined {
  if (!archetype) return undefined;
  const d = choice.descriptors;
  const mode = modeOf(choice);
  const descriptors: DescriptorsDto = {};
  if (applies(archetype, 'action') && d.action) descriptors.action = d.action;
  if (applies(archetype, 'rof') && d.rof) descriptors.rof = d.rof;
  if (applies(archetype, 'calibre')) {
    if (mode === 'combat-extended' && d.ammoSet) descriptors.ammoSet = d.ammoSet;
    else if (d.calibre) descriptors.calibre = d.calibre;
  }
  if (applies(archetype, 'handling') && d.handling) descriptors.handling = d.handling;
  if (applies(archetype, 'tier') && d.tier) descriptors.tier = d.tier;
  const base: DesignerArchetypeProposeRequest = {
    kind: archetype.kind,
    archetype: archetype.id,
    descriptors,
    balanceTarget: choice.balance,
    mode,
  };
  return draft ? { ...base, draft } : base;
}
