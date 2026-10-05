import { beforeEach, describe, expect, it } from 'vitest';
import { loadFixture } from 'rimstudio-testkit';
import type { ProjectLayoutFixPlanDto } from 'rimstudio-ipc-types';
import {
  backToReview,
  closeFix,
  fixFlow,
  goConfirm,
  openFix,
  resetFixFlow,
  reviewAgain,
  runApply,
  selectAllSafe,
  selectNone,
  toggleItem,
  toggleRename,
} from './fixStore';
import { gewehrRef, installWithProject } from './testSupport';
import { loadProject } from './store';

async function start(extra = {}) {
  const transport = installWithProject(extra);
  resetFixFlow();
  await loadProject(gewehrRef());
  return transport;
}

describe('fix flow', () => {
  beforeEach(() => resetFixFlow());

  it('plans and ticks every applicable item without a conflict', async () => {
    await start();
    await openFix();
    expect(fixFlow.value.phase).toBe('review');
    const plan = fixFlow.value.plan as ProjectLayoutFixPlanDto;
    expect(fixFlow.value.selected).toEqual(plan.items.map((i) => i.id));
  });

  it('plans only the codes of one finding and ticks the item of its path', async () => {
    const transport = await start();
    await openFix({ codes: ['layout.ce-outside-gate'], path: 'Patches/ce_patch.xml' });
    const call = transport.calls.find((c) => c.name === 'project_layout_fix_plan');
    expect(call?.request).toMatchObject({ fixes: ['layout.ce-outside-gate'] });
    const plan = fixFlow.value.plan as ProjectLayoutFixPlanDto;
    const move = plan.items.find((i) => i.from === 'Patches/ce_patch.xml');
    // the move brings the item it requires
    expect(fixFlow.value.selected).toContain(move?.id);
    for (const need of move?.requires ?? []) expect(fixFlow.value.selected).toContain(need);
    expect(fixFlow.value.selected.length).toBeLessThan(plan.items.length + 1);
  });

  it('ticking an item brings what it requires and unticking the base drops the dependants', async () => {
    await start();
    await openFix();
    selectNone();
    const plan = fixFlow.value.plan as ProjectLayoutFixPlanDto;
    const move = plan.items.find((i) => i.requires.length > 0);
    if (!move) throw new Error('fixture has a dependent item');
    toggleItem(move.id, true);
    expect(fixFlow.value.selected).toContain(move.requires[0]);
    toggleItem(move.requires[0] as string, false);
    expect(fixFlow.value.selected).not.toContain(move.id);
    selectAllSafe();
    expect(fixFlow.value.selected.length).toBe(plan.items.length);
  });

  it('keeps a conflicting item unticked until the numbered name is chosen', async () => {
    await start({ project_layout_fix_plan: () => loadFixture('layout-fix-plan-conflict') });
    await openFix();
    const plan = fixFlow.value.plan as ProjectLayoutFixPlanDto;
    const move = plan.items.find((i) => i.conflict?.destinationExists);
    if (!move) throw new Error('fixture has a conflict');
    expect(fixFlow.value.selected).not.toContain(move.id);
    toggleRename(move.id, true);
    expect(fixFlow.value.selected).toContain(move.id);
    expect(fixFlow.value.rename).toEqual([move.id]);
    toggleRename(move.id, false);
    expect(fixFlow.value.selected).not.toContain(move.id);
  });

  it('applies the ticked items with the plan id and the rename choice', async () => {
    const transport = await start();
    await openFix();
    goConfirm();
    expect(fixFlow.value.phase).toBe('confirm');
    backToReview();
    goConfirm();
    await runApply();
    expect(fixFlow.value.phase).toBe('result');
    const plan = loadFixture<ProjectLayoutFixPlanDto>('layout-fix-plan-gewehr');
    const apply = transport.calls.find((c) => c.name === 'project_layout_fix_apply');
    expect(apply?.request).toMatchObject({
      planId: plan.planId,
      items: plan.items.map((i) => ({ id: i.id, renameOnConflict: false })),
    });
    // the project is read again afterwards
    expect(transport.calls.filter((c) => c.name === 'project_layout_check').length).toBe(2);
  });

  it('does not go on without a ticked item', async () => {
    await start();
    await openFix();
    selectNone();
    goConfirm();
    expect(fixFlow.value.phase).toBe('review');
    await runApply();
    expect(fixFlow.value.phase).toBe('review');
  });

  it('keeps the error of a stale plan and plans again', async () => {
    let first = true;
    await start({
      project_layout_apply: undefined,
      project_layout_fix_apply: () => {
        if (first) {
          first = false;
          throw { code: 'designer.plan-stale', message: 'the plan changed', errorId: 'e-1' };
        }
        return loadFixture('layout-fix-apply-gewehr');
      },
    });
    await openFix();
    goConfirm();
    await runApply();
    expect(fixFlow.value.phase).toBe('error');
    expect(fixFlow.value.error?.code).toBe('designer.plan-stale');
    await reviewAgain();
    expect(fixFlow.value.phase).toBe('review');
    expect(fixFlow.value.error).toBeUndefined();
  });

  it('reports a plan that cannot be made and closes', async () => {
    await start({
      project_layout_fix_plan: () => {
        throw { code: 'project.not-found', message: 'gone', errorId: 'e-2' };
      },
    });
    await openFix();
    expect(fixFlow.value.phase).toBe('error');
    closeFix();
    expect(fixFlow.value.phase).toBe('closed');
  });
});
