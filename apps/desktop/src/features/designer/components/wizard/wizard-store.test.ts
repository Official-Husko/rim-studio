import { describe, expect, it } from 'vitest';
import type { ArchetypeProposalDto, DraftDto, DraftEntryDto } from 'rimstudio-ipc-types';
import { fixture, settle } from '../../testSupport';
import { installWizardTransport, loadedWizard, proposalFor } from './wizardTestSupport';

type Call = { name: string; request: unknown };
const proposeCalls = (calls: readonly Call[]) =>
  calls
    .filter((c) => c.name === 'designer_archetype_propose')
    .map((c) => c.request as Record<string, unknown>);

describe('wizard store', () => {
  it('loads the catalogue once', async () => {
    const transport = installWizardTransport();
    const { store } = await loadedWizard();
    await store.loadCatalog();
    expect(store.catalog.value?.families.length).toBeGreaterThan(5);
    expect(transport.calls.filter((c) => c.name === 'designer_archetype_catalog')).toHaveLength(1);
  });

  it('keeps the error when the catalogue cannot be loaded', async () => {
    installWizardTransport({
      designer_archetype_catalog: () => {
        throw { code: 'designer.archetype-unavailable', message: 'no install', errorId: 'e-1' };
      },
    });
    const { store } = await loadedWizard();
    expect(store.catalog.value).toBeUndefined();
    expect(store.catalogError.value?.code).toBe('designer.archetype-unavailable');
  });

  it('starts from the defaults of a type and proposes after the idle time', async () => {
    const transport = installWizardTransport();
    const { store, timers } = await loadedWizard();
    store.chooseArchetype('rifle/sniper');
    expect(store.proposing.value).toBe(true);
    expect(proposeCalls(transport.calls)).toHaveLength(0);
    timers.runTimers();
    await settle();
    const [request] = proposeCalls(transport.calls);
    expect(request).toMatchObject({
      kind: 'ranged',
      archetype: 'rifle/sniper',
      balanceTarget: 'typical',
      mode: 'vanilla',
      descriptors: {
        action: 'bolt',
        handling: 'standard',
        calibre: 'huge',
        rof: { class: 'medium' },
      },
    });
    expect(store.proposal.value?.verdict).toBe('plausible');
    expect(store.proposing.value).toBe(false);
  });

  it('proposes once for a burst of changes and remembers the previous proposal', async () => {
    const transport = installWizardTransport();
    const { store, timers } = await loadedWizard();
    store.chooseArchetype('rifle/sniper');
    timers.runTimers();
    await settle();
    store.setDescriptors({ rof: { rpm: 22 } });
    store.setDescriptors({ rof: { rpm: 24 } });
    store.setBalance('stronger');
    timers.runTimers();
    await settle();
    const calls = proposeCalls(transport.calls);
    expect(calls).toHaveLength(2);
    expect(calls[1]).toMatchObject({
      balanceTarget: 'stronger',
      descriptors: { rof: { rpm: 24 } },
    });
    expect(store.previous.value?.choice.balance).toBe('typical');
    expect(store.proposal.value?.choice.balance).toBe('stronger');
  });

  it('drops an answer that arrives after a newer one', async () => {
    let release: (() => void) | undefined;
    const slow = new Promise<void>((resolve) => {
      release = resolve;
    });
    let n = 0;
    installWizardTransport({
      designer_archetype_propose: async (request) => {
        n += 1;
        if (n === 1) await slow;
        return n === 1 ? fixture('designer-wizard-propose-sniper-stronger') : proposalFor(request);
      },
    });
    const { store, timers } = await loadedWizard();
    store.chooseArchetype('rifle/sniper');
    timers.runTimers();
    await Promise.resolve();
    store.setBalance('typical');
    timers.runTimers();
    await settle();
    release?.();
    await settle();
    expect(store.proposal.value?.choice.balance).toBe('typical');
  });

  it('removes the descriptors a type does not take and clears the tier', async () => {
    const transport = installWizardTransport();
    const { store, timers } = await loadedWizard();
    store.chooseArchetype('sword/long');
    store.setDescriptors({ tier: 'medieval' });
    timers.runTimers();
    await settle();
    store.setDescriptors({ tier: undefined });
    timers.runTimers();
    await settle();
    const calls = proposeCalls(transport.calls);
    expect(calls[0]).toMatchObject({
      kind: 'melee',
      descriptors: { tier: 'medieval', handling: 'standard' },
    });
    expect(calls[0]?.descriptors).not.toHaveProperty('calibre');
    expect(calls[1]?.descriptors).not.toHaveProperty('tier');
  });

  it('asks in Combat Extended mode with the ammo set instead of the calibre class', async () => {
    const transport = installWizardTransport();
    const { store, timers } = await loadedWizard();
    store.chooseArchetype('rifle/assault');
    store.setCeCalibre(true);
    await settle();
    expect(store.calibres.value?.length).toBeGreaterThan(100);
    store.setDescriptors({ ammoSet: 'AmmoSet_556x45mmNATO' });
    timers.runTimers();
    await settle();
    const last = proposeCalls(transport.calls).at(-1);
    expect(last).toMatchObject({
      mode: 'combat-extended',
      descriptors: { ammoSet: 'AmmoSet_556x45mmNATO' },
    });
    expect(last?.descriptors).not.toHaveProperty('calibre');
  });

  it('suggests the def name from the label until the user types one', async () => {
    installWizardTransport();
    const { store } = await loadedWizard();
    store.setPrefix('RS_');
    store.setLabel('Long Rifle');
    expect(store.defName.value).toBe('RS_LongRifle');
    store.setDefName('RS_Mine');
    store.setLabel('Other');
    expect(store.defName.value).toBe('RS_Mine');
    store.setDefName('');
    expect(store.defName.value).toBe('RS_Other');
  });

  it('creates the draft: a blank draft with the proposal applied, stored and handed over', async () => {
    const transport = installWizardTransport({
      designer_draft_save: () => ({ id: 'd-new', savedAtMs: 5 }),
    });
    const created: DraftEntryDto[] = [];
    const { store, timers } = await loadedWizard({ onCreated: (e) => created.push(e) });
    store.chooseArchetype('rifle/sniper');
    timers.runTimers();
    await settle();
    store.setLabel('Test rifle');
    store.setDefName('RS_Test');
    expect(await store.create()).toBe(true);
    const apply = transport.calls.find((c) => c.name === 'designer_archetype_apply')?.request as {
      draft: DraftDto;
      proposal: ArchetypeProposalDto;
      includeCe: boolean;
    };
    expect(apply.draft.spec.identity).toMatchObject({ defName: 'RS_Test', label: 'Test rifle' });
    expect(apply.includeCe).toBe(false);
    const save = transport.calls.find((c) => c.name === 'designer_draft_save')?.request as {
      draft: DraftDto;
    };
    expect(save.draft.archetype?.archetype).toBe('rifle/sniper');
    expect(created[0]).toMatchObject({ id: 'd-new', kind: 'ranged' });
  });

  it('never fills the Combat Extended block for a new draft, even with an ammo set chosen', async () => {
    const transport = installWizardTransport({
      designer_draft_save: () => ({ id: 'd-ce', savedAtMs: 6 }),
    });
    const { store, timers } = await loadedWizard();
    store.chooseArchetype('rifle/assault');
    store.setCeCalibre(true);
    store.setDescriptors({ ammoSet: 'AmmoSet_556x45mmNATO' });
    timers.runTimers();
    await settle();
    store.setDefName('RS_Ce');
    expect(await store.create()).toBe(true);
    const apply = transport.calls.find((c) => c.name === 'designer_archetype_apply')?.request as {
      includeCe: boolean;
      proposal: ArchetypeProposalDto;
    };
    expect(apply.proposal.choice.mode).toBe('combat-extended');
    expect(apply.includeCe).toBe(false);
  });

  it('does not create without a def name or a project and shows the error of a failed apply', async () => {
    installWizardTransport({
      designer_archetype_apply: () => {
        throw { code: 'designer.apply-failed', message: 'nope', errorId: 'e-2' };
      },
    });
    const { store, timers } = await loadedWizard();
    store.chooseArchetype('rifle/sniper');
    timers.runTimers();
    await settle();
    expect(await store.create()).toBe(false);
    store.setDefName('RS_X');
    expect(await store.create()).toBe(false);
    expect(store.error.value?.code).toBe('designer.apply-failed');
    expect(store.busy.value).toBe(false);
  });

  it('retunes an open draft: restores the choice and passes the draft so typed numbers stay', async () => {
    const transport = installWizardTransport();
    const applied: DraftDto[] = [];
    const { store } = await loadedWizard({ onApplied: (d) => applied.push(d) });
    const draft = fixture<{ draft: DraftDto }>('designer-wizard-apply-sniper').draft;
    store.retune(draft);
    await settle();
    expect(store.step.value).toBe('describe');
    expect(store.choice.value.archetypeId).toBe('rifle/sniper');
    const request = proposeCalls(transport.calls)[0] as { draft?: DraftDto };
    expect(request.draft?.spec.identity.defName).toBe('RS_Test');
    expect(await store.applyToTarget()).toBe(true);
    expect(applied).toHaveLength(1);
  });

  it('steps forward and back and forgets everything on reset', async () => {
    installWizardTransport();
    const { store, timers } = await loadedWizard();
    store.chooseArchetype('rifle/sniper');
    store.move(1);
    expect(store.step.value).toBe('describe');
    store.move(-1);
    expect(store.step.value).toBe('category');
    timers.runTimers();
    await settle();
    store.reset();
    expect(store.choice.value.archetypeId).toBeUndefined();
    expect(store.proposal.value).toBeUndefined();
  });
});
