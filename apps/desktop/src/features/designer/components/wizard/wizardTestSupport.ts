import type {
  ArchetypeCatalogDto,
  ArchetypeProposalDto,
  DesignerArchetypeApplyResponse,
  DesignerArchetypeProposeRequest,
} from 'rimstudio-ipc-types';
import { fixture, installTransport, manualScheduler } from '../../testSupport';
import { createWizardStore, type WizardDeps, type WizardStore } from './wizard-store';

/** The catalogue recorded from the real bridge. */
export function catalogFixture(): ArchetypeCatalogDto {
  return fixture<ArchetypeCatalogDto>('designer-wizard-catalog');
}

/** The recorded proposal that fits a request: by type, target and calibre. */
export function proposalFor(request: unknown): ArchetypeProposalDto {
  const r = request as DesignerArchetypeProposeRequest;
  if (r.archetype === 'sword/long') return fixture('designer-wizard-propose-sword');
  if (r.mode === 'combat-extended') return fixture('designer-wizard-propose-ce');
  if (r.descriptors.tier === 'archotech') return fixture('designer-wizard-propose-thin');
  if (r.balanceTarget === 'stronger') return fixture('designer-wizard-propose-sniper-stronger');
  return fixture('designer-wizard-propose-sniper');
}

/** A mock transport that answers the three archetype commands from the recorded fixtures. */
export function installWizardTransport(
  handlers: Record<string, (request: unknown) => unknown> = {},
) {
  return installTransport({
    designer_archetype_catalog: (request) =>
      (request as { includeCalibres: boolean }).includeCalibres
        ? fixture('designer-wizard-catalog-ce')
        : fixture('designer-wizard-catalog'),
    designer_archetype_propose: proposalFor,
    designer_archetype_apply: () =>
      fixture<DesignerArchetypeApplyResponse>('designer-wizard-apply-sniper'),
    ...handlers,
  });
}

/** A wizard store with a manual scheduler and a loaded catalogue. */
export async function loadedWizard(deps: Partial<WizardDeps> = {}): Promise<{
  store: WizardStore;
  timers: ReturnType<typeof manualScheduler>;
}> {
  const timers = manualScheduler();
  const store = createWizardStore({
    projectId: () => 'p-1',
    onCreated: () => undefined,
    scheduler: timers.scheduler,
    ...deps,
  });
  await store.loadCatalog();
  return { store, timers };
}
