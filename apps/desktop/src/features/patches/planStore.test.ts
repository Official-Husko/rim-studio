import { beforeEach, describe, expect, it } from 'vitest';
import { loadFixture } from 'rimstudio-testkit';
import type { ConvertScanDto } from 'rimstudio-ipc-types';
import { setAnswer } from './answerStore';
import { loadPlan, plans, requestFor } from './planStore';
import { installTransport, openFixtureProject } from './testSupport';

const candidate = loadFixture<ConvertScanDto>('designer_convert_scan').candidates[0];
if (!candidate) throw new Error('fixture');
const ammo = candidate.asks.find((a) => a.field === '/ce/ammoSet');
if (!ammo) throw new Error('fixture');

beforeEach(() => {
  installTransport();
});

describe('planStore', () => {
  it('plans with the answers and reuses a plan for the same request', async () => {
    const transport = installTransport();
    const p = openFixtureProject();
    const project = { projectId: p.projectId, path: p.path, name: p.name };
    await loadPlan(project, candidate);
    await loadPlan(project, candidate);
    expect(transport.calls.filter((c) => c.name === 'designer_export_plan')).toHaveLength(1);
    setAnswer(candidate, 'weapon', ammo, 'AmmoSet_303British');
    const entry = await loadPlan(project, candidate);
    expect(entry.plan?.files).toHaveLength(2);
    expect(transport.calls.filter((c) => c.name === 'designer_export_plan')).toHaveLength(2);
    expect(plans.value['OH_G41m']?.phase).toBe('ready');
  });

  it('builds the request from the weapon answers and its family group', () => {
    const req = requestFor(candidate);
    expect(req.defName).toBe('OH_G41m');
    expect(req.groups).toBeUndefined();
    setAnswer(candidate, 'family', ammo, 'AmmoSet_45ACP');
    expect(requestFor(candidate).groups?.[0]?.family).toBe(candidate.family);
  });

  it('keeps the error of a failing plan', async () => {
    installTransport({
      designer_export_plan: () => {
        throw { code: 'designer.internal', message: 'broke', errorId: 'e' };
      },
    });
    const p = openFixtureProject();
    const entry = await loadPlan({ projectId: p.projectId, path: p.path, name: p.name }, candidate);
    expect(entry.phase).toBe('error');
    expect(entry.error?.code).toBe('designer.internal');
  });
});
