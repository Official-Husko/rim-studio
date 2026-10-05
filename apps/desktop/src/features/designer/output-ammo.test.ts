import { afterEach, describe, expect, it } from 'vitest';
import type { DesignerExportPlanRequest } from 'rimstudio-ipc-types';
import { customAmmo } from '~/shared/ammo/testSupport';
import { outputEntry, setupOutput } from './output-testSupport';

let stop: (() => void) | undefined;
afterEach(() => {
  stop?.();
  stop = undefined;
});

describe('ammo planner of the output store', () => {
  it('plans the open draft with the custom caliber in place of the chosen set, and changes nothing', async () => {
    const setup = setupOutput();
    stop = setup.output.start();
    setup.editor.open(outputEntry('ce-answered'));
    await setup.flush();
    const before = setup.editor.draft.value;
    expect(before?.spec.ce?.ammoSet).toBeDefined();
    await setup.output.planWithAmmo(customAmmo());
    const request = setup.transport.calls.filter((c) => c.name === 'designer_export_plan').at(-1)
      ?.request as DesignerExportPlanRequest;
    expect(request.projectId).toBe('p-out');
    expect(request.draft.spec.ce?.customAmmo?.name).toBe(customAmmo().name);
    expect(request.draft.spec.ce?.ammoSet).toBeUndefined();
    expect(request.draft.spec.ce?.defaultProjectile).toBeUndefined();
    expect(setup.editor.draft.value).toBe(before);
  });

  it('sends the derived values the plan takes now', async () => {
    const setup = setupOutput();
    stop = setup.output.start();
    setup.editor.open(outputEntry('ce-answered'));
    await setup.flush();
    setup.output.setAcceptMode('all');
    await setup.output.planWithAmmo(customAmmo());
    const request = setup.transport.calls.filter((c) => c.name === 'designer_export_plan').at(-1)
      ?.request as DesignerExportPlanRequest;
    expect(request.acceptSuggestions).toEqual({ fields: [] });
  });

  it('knows whether a project is open and refuses to plan without a draft or a project', async () => {
    const none = setupOutput({}, () => undefined);
    expect(none.output.hasProject()).toBe(false);
    await expect(none.output.planWithAmmo(customAmmo())).rejects.toThrow('no draft or project');
    const open = setupOutput();
    expect(open.output.hasProject()).toBe(true);
    expect(open.output.currentDraft()).toBeUndefined();
  });
});
