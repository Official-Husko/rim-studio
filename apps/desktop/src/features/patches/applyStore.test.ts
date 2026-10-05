import { beforeEach, describe, expect, it } from 'vitest';
import { loadFixture } from 'rimstudio-testkit';
import type { ConvertScanDto, WritePlanDto } from 'rimstudio-ipc-types';
import { projectRevision } from '~/shared/project';
import { applyState, openReview, runApply } from './applyStore';
import { installTransport, openFixtureProject } from './testSupport';

const candidates = loadFixture<ConvertScanDto>('designer_convert_scan').candidates;

beforeEach(() => {
  projectRevision.value = 0;
});

describe('applyStore', () => {
  it('plans every weapon again right before it applies it and bumps the project revision', async () => {
    const transport = installTransport({
      designer_export_plan: () => loadFixture<WritePlanDto>('patches-plan-ready'),
    });
    const p = openFixtureProject();
    const project = { projectId: p.projectId, path: p.path, name: p.name };
    const picked = candidates.slice(0, 2);
    await openReview(project, picked);
    const state = applyState.value;
    expect(state.phase).toBe('review');
    if (state.phase !== 'review') return;
    const plansBefore = transport.calls.filter((c) => c.name === 'designer_export_plan').length;
    await runApply(project, state.items, { backup: true, dryApply: false });
    const plansAfter = transport.calls.filter((c) => c.name === 'designer_export_plan').length;
    expect(plansAfter - plansBefore).toBe(2);
    const finished = applyState.value;
    expect(finished.phase === 'done' && finished.results.map((r) => r.defName)).toEqual([
      'OH_G41m',
      'OH_G41w',
    ]);
    expect(transport.calls.filter((c) => c.name === 'designer_apply_plan')).toHaveLength(2);
    expect(projectRevision.value).toBe(1);
  });

  it('stops at the first failing apply and reports it', async () => {
    let n = 0;
    installTransport({
      designer_export_plan: () => loadFixture<WritePlanDto>('patches-plan-ready'),
      designer_apply_plan: () => {
        n += 1;
        throw { code: 'designer.apply-failed', message: 'disk full', errorId: 'e' };
      },
    });
    const p = openFixtureProject();
    const project = { projectId: p.projectId, path: p.path, name: p.name };
    await openReview(project, candidates.slice(0, 3));
    const state = applyState.value;
    if (state.phase !== 'review') throw new Error('review');
    await runApply(project, state.items, { backup: true, dryApply: true });
    expect(n).toBe(1);
    const done = applyState.value;
    expect(done.phase === 'done' && done.results[0]?.error?.code).toBe('designer.apply-failed');
    expect(projectRevision.value).toBe(0);
  });
});
