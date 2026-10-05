import { afterEach, describe, expect, it } from 'vitest';
import type {
  CeSuggestionDto,
  DesignerApplyPlanRequest,
  DesignerExportPlanRequest,
} from 'rimstudio-ipc-types';
import { mockError } from 'rimstudio-testkit';
import { projectRevision, resetProjectStore } from '~/shared/project';
import { outputEntry, setupOutput, callsOf, recordedPlan } from './output-testSupport';
import { fixture, settle } from './testSupport';

let stop: (() => void) | undefined;
afterEach(() => {
  stop?.();
  stop = undefined;
  resetProjectStore();
});

async function opened(name: 'vanilla' | 'ce-on' | 'ce-answered' = 'vanilla') {
  const setup = setupOutput();
  stop = setup.output.start();
  setup.editor.open(outputEntry(name));
  await setup.flush();
  return setup;
}

const lastRequest = (
  setup: ReturnType<typeof setupOutput>,
  name: string,
): DesignerExportPlanRequest | undefined =>
  [...setup.transport.calls].reverse().find((c) => c.name === name)?.request as
    DesignerExportPlanRequest | undefined;

describe('output store: the vanilla plan', () => {
  it('plans a vanilla clone with one file and no Combat Extended option', async () => {
    const { output, transport } = await opened();
    const plan = output.plan.value;
    expect(plan?.files.map((f) => f.path)).toEqual([
      'Defs/ThingDefs_Misc/Weapons/RangedIndustrial/Gun_OutRifle.xml',
    ]);
    expect(plan?.files[0]?.action).toBe('create');
    expect(output.ceOn.value).toBe(false);
    expect(output.selectedPath.value).toBe(plan?.files[0]?.path);
    const sent = transport.calls.find((c) => c.name === 'designer_export_plan')
      ?.request as DesignerExportPlanRequest;
    expect(sent.acceptSuggestions).toBeUndefined();
    expect(output.canApply.value).toBe(true);
  });

  it('asks about Combat Extended once while the switch is off, to learn whether it exists', async () => {
    const setup = await opened();
    expect(callsOf(setup.transport, 'designer_ce_suggest')).toBe(1);
    expect(setup.output.suggestion.value?.available).toBe(true);
    setup.editor.setField('/mass', { value: 4, source: 'typed' });
    await setup.flush();
    expect(callsOf(setup.transport, 'designer_ce_suggest')).toBe(1);
    expect(callsOf(setup.transport, 'designer_export_plan')).toBe(2);
  });

  it('plans again after an edit, one call for several edits', async () => {
    const setup = await opened();
    setup.editor.setField('/mass', { value: 4, source: 'typed' });
    setup.editor.setField('/mass', { value: 5, source: 'typed' });
    expect(setup.output.stale.value).toBe(true);
    expect(setup.output.canApply.value).toBe(false);
    expect(setup.output.blockedBy.value).toBe('designer.output.blocked.planning');
    await setup.flush();
    expect(callsOf(setup.transport, 'designer_export_plan')).toBe(2);
    expect(setup.output.stale.value).toBe(false);
  });

  it('drops an answer that arrives after a newer one', async () => {
    let release: (() => void) | undefined;
    const setup = setupOutput({
      designer_export_plan: (r) => {
        const request = r as DesignerExportPlanRequest;
        const plan = recordedPlan(request);
        if (request.draft.spec.mass?.value === 4) {
          return new Promise((resolve) => {
            release = () => resolve({ ...plan, planId: 'old' });
          });
        }
        return { ...plan, planId: 'new' };
      },
    });
    stop = setup.output.start();
    setup.editor.open(outputEntry());
    await setup.flush();
    setup.editor.setField('/mass', { value: 4, source: 'typed' });
    setup.clock.runTimers();
    await settle();
    setup.editor.setField('/mass', { value: 5, source: 'typed' });
    await setup.flush();
    release?.();
    await settle();
    expect(setup.output.plan.value?.planId).toBe('new');
  });

  it('shows the error of a failed plan call and blocks Apply', async () => {
    const setup = setupOutput({
      designer_export_plan: () => {
        throw mockError('designer.plan-failed', 'The plan failed.');
      },
    });
    stop = setup.output.start();
    setup.editor.open(outputEntry());
    await setup.flush();
    expect(setup.output.planError.value?.code).toBe('designer.plan-failed');
    expect(setup.output.blockedBy.value).toBe('designer.output.blocked.failed');
  });

  it('forgets everything when another draft opens', async () => {
    const setup = await opened();
    setup.editor.open({ ...outputEntry(), id: 'd-other' });
    expect(setup.output.plan.value).toBeUndefined();
    expect(setup.output.suggestion.value).toBeUndefined();
    await setup.flush();
    expect(setup.output.plan.value).toBeDefined();
  });
});

describe('output store: Combat Extended', () => {
  it('writes the block only when the switch goes on and removes it when it goes off', async () => {
    const setup = await opened();
    expect(setup.editor.draft.value?.spec.ce).toBeUndefined();
    setup.output.setCeEnabled(true);
    expect(setup.editor.draft.value?.spec.ce).toEqual({ oneHanded: false, beltFed: false });
    await setup.flush();
    expect(setup.output.ceOn.value).toBe(true);
    expect(callsOf(setup.transport, 'designer_ce_suggest')).toBe(2);
    setup.output.setCeEnabled(false);
    expect(setup.editor.draft.value?.spec.ce).toBeUndefined();
    await setup.flush();
    expect(setup.output.plan.value?.files).toHaveLength(1);
  });

  it('brings the answers back when the switch goes on again', async () => {
    const setup = await opened();
    setup.output.setCeEnabled(true);
    setup.output.answer('/ce/ammoSet', 'AmmoSet_303British_SB');
    setup.output.setCeEnabled(false);
    setup.output.setCeEnabled(true);
    expect(setup.editor.draft.value?.spec.ce?.ammoSet).toBe('AmmoSet_303British_SB');
  });

  it('patches the block as one edit and ignores a patch while the switch is off', async () => {
    const setup = await opened();
    setup.output.patchBlock({ bow: true });
    expect(setup.editor.draft.value?.spec.ce).toBeUndefined();
    setup.output.setCeEnabled(true);
    setup.output.patchBlock({ bow: true, extraTags: ['A'], reloadOneAtATime: true });
    expect(setup.editor.draft.value?.spec.ce).toEqual({
      oneHanded: false,
      beltFed: false,
      bow: true,
      extraTags: ['A'],
      reloadOneAtATime: true,
    });
    setup.output.patchBlock({ bow: undefined, extraTags: [] });
    expect(setup.editor.draft.value?.spec.ce).toEqual({
      oneHanded: false,
      beltFed: false,
      reloadOneAtATime: true,
    });
  });

  it('sends no accepted values until the user takes some', async () => {
    const setup = await opened('ce-on');
    expect(lastRequest(setup, 'designer_export_plan')?.acceptSuggestions).toBeUndefined();
    expect(setup.output.plan.value?.hasErrors).toBe(true);
    expect(setup.output.blockedBy.value).toBe('designer.output.blocked.errors');
    setup.output.setAcceptMode('all');
    await setup.flush();
    expect(lastRequest(setup, 'designer_export_plan')?.acceptSuggestions).toEqual({ fields: [] });
    expect(callsOf(setup.transport, 'designer_ce_suggest')).toBe(1);
  });

  it('lists the answers the plan waits for', async () => {
    const setup = await opened('ce-on');
    setup.output.setAcceptMode('all');
    await setup.flush();
    expect(setup.output.pending.value.map((d) => d.field)).toEqual([
      '/ce/ammoSet',
      '/ce/weaponTagClass',
      '/ce/defaultProjectile',
      '/ce/magazineSize',
      '/ce/shotSpread',
    ]);
    expect(setup.output.canApply.value).toBe(false);
  });

  it('takes the reliable values only, by pointer', async () => {
    const setup = setupOutput({
      designer_ce_suggest: () => {
        const s = fixture<CeSuggestionDto>('designer-output-suggest-on');
        return {
          ...s,
          fields: s.fields.map((f) => (f.field === '/ce/bulk' ? { ...f, rating: 'reliable' } : f)),
        };
      },
    });
    stop = setup.output.start();
    setup.editor.open(outputEntry('ce-on'));
    await setup.flush();
    setup.output.setAcceptMode('reliable');
    await setup.flush();
    expect(lastRequest(setup, 'designer_export_plan')?.acceptSuggestions).toEqual({
      fields: ['/ce/bulk'],
    });
    expect(setup.output.accepted.value).toEqual(['/ce/bulk']);
  });

  it('turns one derived value on and off as a custom choice', async () => {
    const setup = await opened('ce-on');
    setup.output.setAccepted('/ce/bulk', true);
    expect(setup.output.acceptMode.value).toBe('custom');
    await setup.flush();
    expect(lastRequest(setup, 'designer_export_plan')?.acceptSuggestions).toEqual({
      fields: ['/ce/bulk'],
    });
    setup.output.setAccepted('/ce/bulk', false);
    await setup.flush();
    expect(lastRequest(setup, 'designer_export_plan')?.acceptSuggestions).toBeUndefined();
  });

  it('writes the patch files into the plan once everything is answered', async () => {
    const setup = await opened('ce-answered');
    setup.output.setAcceptMode('all');
    await setup.flush();
    const plan = setup.output.plan.value;
    expect(plan?.files.map((f) => f.kind)).toEqual(['vanilla-defs', 'ce-patch', 'load-folders']);
    expect(plan?.hasErrors).toBe(false);
    expect(setup.output.pending.value).toEqual([]);
    expect(setup.output.canApply.value).toBe(true);
  });

  it('answers a tool penetration by the name of the tool', async () => {
    const setup = await opened('ce-on');
    setup.output.answer('/ce/toolPenetration/stock/blunt', { value: 2.5, source: 'answered' });
    expect(setup.editor.draft.value?.spec.ce?.toolPenetration).toEqual([
      { tool: 'stock', blunt: { value: 2.5, source: 'answered' } },
    ]);
  });
});

describe('output store: apply', () => {
  it('writes the reviewed plan with the choices of the dialog and refreshes the tree', async () => {
    const setup = await opened('ce-answered');
    setup.output.setAcceptMode('all');
    await setup.flush();
    const before = projectRevision.value;
    setup.output.openApply();
    expect(setup.output.apply.value.phase).toBe('confirm');
    await setup.output.confirmApply({ backup: true, dryApply: true });
    const sent = lastRequest(setup, 'designer_apply_plan') as unknown as DesignerApplyPlanRequest;
    expect(sent.planId).toBe(setup.output.plan.value?.planId);
    expect(sent.backup).toBe(true);
    expect(sent.dryApply).toBe(true);
    expect(sent.request.acceptSuggestions).toEqual({ fields: [] });
    expect(setup.output.apply.value.phase).toBe('done');
    expect(setup.output.apply.value.report?.written).toHaveLength(3);
    expect(projectRevision.value).toBe(before + 1);
  });

  it('never opens the dialog while the plan has errors', async () => {
    const setup = await opened('ce-on');
    setup.output.openApply();
    expect(setup.output.apply.value.phase).toBe('idle');
    await setup.output.confirmApply({ backup: true, dryApply: true });
    expect(callsOf(setup.transport, 'designer_apply_plan')).toBe(0);
  });

  it('keeps the error of a refused apply and plans again', async () => {
    const setup = setupOutput({
      designer_apply_plan: () => {
        throw mockError('designer.plan-stale', 'The plan changed since you reviewed it.');
      },
    });
    stop = setup.output.start();
    setup.editor.open(outputEntry());
    await setup.flush();
    setup.output.openApply();
    await setup.output.confirmApply({ backup: false, dryApply: false });
    expect(setup.output.apply.value.phase).toBe('failed');
    expect(setup.output.apply.value.error?.code).toBe('designer.plan-stale');
    expect(setup.output.stale.value).toBe(true);
  });

  it('cannot be closed while it writes', async () => {
    let release: (() => void) | undefined;
    const setup = setupOutput({
      designer_apply_plan: () =>
        new Promise((resolve) => {
          release = () => resolve({ planId: 'x', written: [], unchanged: [], diagnostics: [] });
        }),
    });
    stop = setup.output.start();
    setup.editor.open(outputEntry());
    await setup.flush();
    setup.output.openApply();
    const done = setup.output.confirmApply({ backup: true, dryApply: false });
    setup.output.closeApply();
    expect(setup.output.apply.value.phase).toBe('running');
    release?.();
    await done;
    setup.output.closeApply();
    expect(setup.output.apply.value.phase).toBe('idle');
  });
});
