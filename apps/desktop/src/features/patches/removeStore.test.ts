import { beforeEach, describe, expect, it } from 'vitest';
import { loadFixture, mockError } from 'rimstudio-testkit';
import type { ConvertScanDto, WritePlanDto } from 'rimstudio-ipc-types';
import { projectRevision } from '~/shared/project';
import { plans } from './planStore';
import {
  closeRemoval,
  confirmRemoval,
  isRemovable,
  openRemoval,
  removal,
  removalTarget,
} from './removeStore';
import { installTransport, openFixtureProject } from './testSupport';

const converted = loadFixture<ConvertScanDto>('patches-scan-converted').candidates;
const [generated] = converted;
if (!generated) throw new Error('fixture');

const PATH = 'Compat/CombatExtended/Patches/gewehr41_Weapons_Ranged.xml';

function setup(extra: Parameters<typeof installTransport>[0] = {}) {
  const transport = installTransport(extra);
  const p = openFixtureProject();
  return { transport, project: { projectId: p.projectId, path: p.path, name: p.name } };
}

beforeEach(() => {
  projectRevision.value = 0;
});

describe('removalTarget', () => {
  it('names the weapon by its label and falls back to the definition name', () => {
    expect(removalTarget(generated, 'remove')).toEqual({
      defName: 'OH_G41m',
      name: 'Gewehr 41 (m)',
      kind: 'ranged',
      mode: 'remove',
      keepsConversion: false,
    });
    const bare = removalTarget({ ...generated, label: '', kind: undefined }, 'replace');
    expect(bare).toMatchObject({ name: 'OH_G41m', kind: 'ranged', mode: 'replace' });
  });
});

describe("removalTarget of a conversion that is not RimStudio's own", () => {
  it('keeps the conversion when only overrides of it were generated', () => {
    const overrides = {
      ...generated,
      existing: {
        origin: 'project' as const,
        file: 'Patches/ce_patch.xml',
        files: [PATH],
        removable: true,
      },
    };
    expect(removalTarget(overrides, 'remove').keepsConversion).toBe(true);
    expect(removalTarget({ ...generated, existing: undefined }, 'remove').keepsConversion).toBe(
      false,
    );
  });
});

describe('openRemoval', () => {
  it('asks for the plan of the removal and holds it for review', async () => {
    const { transport, project } = setup();
    const pending = openRemoval(project, removalTarget(generated, 'remove'));
    expect(removal.value.phase).toBe('planning');
    await pending;
    const state = removal.value;
    expect(state.phase).toBe('review');
    if (state.phase !== 'review') return;
    expect(state.plan.files.map((f) => [f.path, f.action])).toEqual([[PATH, 'update-region']]);
    const call = transport.calls.find((c) => c.name === 'designer_export_plan');
    expect(call?.request).toMatchObject({
      projectId: project.projectId,
      convert: { defName: 'OH_G41m', remove: true },
    });
    const sent = call?.request as { convert?: { groups?: unknown } } | undefined;
    expect(sent?.convert?.groups).toBeUndefined();
  });

  it('keeps a plan with errors apart from one that can be applied', async () => {
    const { project } = setup();
    await openRemoval(project, removalTarget({ ...generated, defName: 'OH_Unknown' }, 'remove'));
    const state = removal.value;
    expect(state.phase === 'review' && state.plan.hasErrors).toBe(true);
    if (state.phase !== 'review') return;
    expect(state.plan.diagnostics[0]?.code).toBe('designer.remove-nothing');
    expect(isRemovable(state.plan)).toBe(false);
  });

  it('reports a failed plan with the error of the backend', async () => {
    const { project } = setup({
      designer_export_plan: () => {
        throw mockError('designer.internal', 'something broke');
      },
    });
    await openRemoval(project, removalTarget(generated, 'remove'));
    const state = removal.value;
    expect(state.phase === 'failed' && state.error.code).toBe('designer.internal');
  });

  it('drops a plan that arrives after the dialog was closed', async () => {
    const { project } = setup();
    const pending = openRemoval(project, removalTarget(generated, 'remove'));
    closeRemoval();
    await pending;
    expect(removal.value.phase).toBe('closed');
  });
});

describe('confirmRemoval', () => {
  it('applies the reviewed plan, keeps the backup, drops the plans and asks for a rescan', async () => {
    const { transport, project } = setup();
    plans.value = { OH_G41w: { key: 'k', phase: 'loading' } };
    await openRemoval(project, removalTarget(generated, 'replace'));
    const reviewed = removal.value;
    if (reviewed.phase !== 'review') throw new Error('review');
    await confirmRemoval(project);
    const state = removal.value;
    expect(state.phase).toBe('done');
    const call = transport.calls.find((c) => c.name === 'designer_apply_plan');
    expect(call?.request).toMatchObject({
      planId: reviewed.plan.planId,
      backup: true,
      dryApply: false,
      request: { convert: { defName: 'OH_G41m', remove: true } },
    });
    expect(state.phase === 'done' && state.report.written[0]?.backupPath).toContain('.bak');
    expect(plans.value).toEqual({});
    expect(projectRevision.value).toBe(1);
  });

  it('does nothing for a plan with errors or when no plan is under review', async () => {
    const { transport, project } = setup();
    await confirmRemoval(project);
    expect(removal.value.phase).toBe('closed');
    await openRemoval(project, removalTarget({ ...generated, defName: 'OH_Unknown' }, 'remove'));
    await confirmRemoval(project);
    expect(removal.value.phase).toBe('review');
    expect(transport.calls.filter((c) => c.name === 'designer_apply_plan')).toHaveLength(0);
    expect(projectRevision.value).toBe(0);
  });

  it('does not start a second write while one is running', async () => {
    let release: () => void = () => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const { transport, project } = setup({
      designer_apply_plan: async () => {
        await gate;
        return loadFixture('patches-apply-remove');
      },
    });
    await openRemoval(project, removalTarget(generated, 'remove'));
    const first = confirmRemoval(project);
    expect(removal.value.phase).toBe('running');
    await confirmRemoval(project);
    release();
    await first;
    expect(transport.calls.filter((c) => c.name === 'designer_apply_plan')).toHaveLength(1);
  });

  it('reports a failed apply and still looks at the disk again', async () => {
    const { project } = setup({
      designer_apply_plan: () => {
        throw mockError('designer.plan-stale', 'the project changed since the plan was made');
      },
    });
    await openRemoval(project, removalTarget(generated, 'remove'));
    await confirmRemoval(project);
    const state = removal.value;
    expect(state.phase === 'failed' && state.error.code).toBe('designer.plan-stale');
    // a failed write can have changed some of the files
    expect(projectRevision.value).toBe(1);
  });

  it('still tells the page to rescan when the dialog was closed during the write', async () => {
    let release: () => void = () => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const { project } = setup({
      designer_apply_plan: async () => {
        await gate;
        return loadFixture('patches-apply-remove');
      },
    });
    await openRemoval(project, removalTarget(generated, 'remove'));
    const run = confirmRemoval(project);
    closeRemoval();
    release();
    await run;
    expect(removal.value.phase).toBe('closed');
    expect(projectRevision.value).toBe(1);
  });
});

describe('isRemovable', () => {
  const plan = loadFixture<WritePlanDto>('patches-plan-remove');

  it('needs files and no error', () => {
    expect(isRemovable(plan)).toBe(true);
    expect(isRemovable({ ...plan, hasErrors: true })).toBe(false);
    expect(isRemovable({ ...plan, files: [] })).toBe(false);
  });
});
