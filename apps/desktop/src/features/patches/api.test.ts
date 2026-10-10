import { describe, expect, it } from 'vitest';
import { loadFixture } from 'rimstudio-testkit';
import type { ApplyReportDto, WritePlanDto } from 'rimstudio-ipc-types';
import { applyConversion, applyRemoval, planConversion, planRemoval } from './api';
import { installTransport } from './testSupport';

const REMOVE = { defName: 'OH_G41m', answers: { overrides: { oneHanded: false, beltFed: false } } };

describe('planRemoval', () => {
  it('asks the export command for the removal of one weapon and sends no answers', async () => {
    const transport = installTransport();
    const plan = await planRemoval('p-1', 'ranged', 'OH_G41m');
    expect(plan.files[0]?.action).toBe('update-region');
    expect(transport.calls).toHaveLength(1);
    const call = transport.calls[0];
    expect(call?.name).toBe('designer_export_plan');
    expect(call?.request).toMatchObject({
      projectId: 'p-1',
      draft: { kind: 'ranged', spec: { identity: { defName: 'OH_G41m' } } },
      convert: { ...REMOVE, remove: true },
    });
    const sent = call?.request as { convert?: { groups?: unknown } } | undefined;
    expect(sent?.convert?.groups).toBeUndefined();
  });

  it('builds the placeholder draft of the weapon kind', async () => {
    const transport = installTransport();
    await planRemoval('p-1', 'melee', 'RS_Sword');
    expect(transport.calls[0]?.request).toMatchObject({
      draft: { kind: 'melee', spec: { identity: { defName: 'RS_Sword' } } },
      convert: { defName: 'RS_Sword', remove: true },
    });
  });

  it('is not what a conversion sends: a conversion never carries the remove flag', async () => {
    const transport = installTransport();
    await planConversion('p-1', 'ranged', { defName: 'OH_G41m', answers: REMOVE.answers });
    const sent = transport.calls[0]?.request as { convert?: Record<string, unknown> } | undefined;
    expect(sent?.convert).toBeDefined();
    expect('remove' in (sent?.convert ?? {})).toBe(false);
  });
});

describe('applyRemoval', () => {
  it('applies the reviewed plan of the removal with a backup and without a dry apply', async () => {
    const transport = installTransport();
    const plan = loadFixture<WritePlanDto>('patches-plan-remove');
    const report = await applyRemoval(plan.planId, 'p-1', 'ranged', 'OH_G41m');
    expect(report.written[0]?.action).toBe('update-region');
    const call = transport.calls[0];
    expect(call?.name).toBe('designer_apply_plan');
    expect(call?.request).toMatchObject({
      planId: plan.planId,
      request: { projectId: 'p-1', convert: { ...REMOVE, remove: true } },
      backup: true,
      dryApply: false,
    });
  });

  it('leaves the conversion apply as it was', async () => {
    const transport = installTransport({
      designer_apply_plan: () => loadFixture<ApplyReportDto>('patches-apply-report'),
    });
    await applyConversion(
      'plan',
      'p-1',
      'ranged',
      { defName: 'OH_G41m', answers: REMOVE.answers },
      { backup: false, dryApply: true },
    );
    expect(transport.calls[0]?.request).toMatchObject({ backup: false, dryApply: true });
    const request = transport.calls[0]?.request as {
      request: { convert: Record<string, unknown> };
    };
    expect('remove' in request.request.convert).toBe(false);
  });
});
