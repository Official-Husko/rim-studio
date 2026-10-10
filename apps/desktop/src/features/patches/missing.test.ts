import { loadFixture } from 'rimstudio-testkit';
import type { ConvertScanDto, WritePlanDto } from 'rimstudio-ipc-types';
import { describe, expect, it } from 'vitest';
import { missingOf } from './missing';

const candidate = loadFixture<ConvertScanDto>('designer_convert_scan').candidates[1];
const open = loadFixture<WritePlanDto>('patches-plan-open');
const ready = loadFixture<WritePlanDto>('patches-plan-ready');

describe('missingOf', () => {
  it('names every question of the scan before a plan exists', () => {
    if (!candidate) throw new Error('fixture');
    const items = missingOf(candidate, undefined);
    expect(items.map((i) => i.key)).toEqual(candidate.asks.map((a) => a.field));
    expect(items[0]?.label).toBe('Which caliber (ammo set) does the weapon use?');
    // the ammo set and the class tag say why they are never chosen
    expect(items[0]?.why).toMatch(/never chosen for you/);
    expect(items[1]?.why).toMatch(/never chosen for you/);
  });

  it('follows the plan: only the questions it reports as open, in the order of the scan', () => {
    if (!candidate) throw new Error('fixture');
    const items = missingOf(candidate, open.diagnostics);
    expect(items.map((i) => i.key)).toEqual(candidate.asks.map((a) => a.field));
    expect(missingOf(candidate, ready.diagnostics)).toEqual([]);
  });

  it('uses the words of the plan for a question the scan did not list', () => {
    if (!candidate) throw new Error('fixture');
    const items = missingOf(candidate, [
      {
        code: 'designer.convert-needs-answer',
        severity: 'error',
        message: 'A new question?',
        field: '/ce/extra',
      },
    ]);
    expect(items).toEqual([{ key: '/ce/extra', label: 'A new question?', why: undefined }]);
  });
});
