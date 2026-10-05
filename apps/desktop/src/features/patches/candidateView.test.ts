import type { ConvertCandidateDto } from 'rimstudio-ipc-types';
import { describe, expect, it } from 'vitest';
import { allTags, chipsOf, filterCandidates, openCount, sortCandidates } from './candidateView';

const base: ConvertCandidateDto = {
  defName: 'A',
  label: 'A',
  kind: 'ranged',
  status: 'not-converted',
  reason: '',
  asks: [
    { field: '/ce/ammoSet', label: 'Ammo', kind: 'choice', options: [] },
    { field: '/ce/shotSpread', label: 'Spread', kind: 'number', options: [] },
  ] as ConvertCandidateDto['asks'],
  tags: ['Gun'],
  weaponClasses: ['Ranged', 'Gun'],
};

describe('candidateView', () => {
  it('lists each tag and class once', () => {
    expect(chipsOf(base)).toEqual(['Gun', 'Ranged']);
    expect(allTags([base, { ...base, tags: ['Pistol'] }])).toEqual(['Gun', 'Pistol', 'Ranged']);
  });

  it('filters by status and tag together', () => {
    const other = {
      ...base,
      defName: 'B',
      status: 'already-ce' as const,
      tags: ['Pistol'],
      weaponClasses: [],
    };
    expect(filterCandidates([base, other], { status: 'already-ce', tag: '' })).toEqual([other]);
    expect(filterCandidates([base, other], { status: '', tag: 'Pistol' })).toEqual([other]);
    expect(filterCandidates([base, other], { status: 'already-ce', tag: 'Gun' })).toEqual([]);
  });

  it('counts the plan open questions when a plan is ready, else the unanswered asks', () => {
    expect(openCount({ candidate: base })).toBe(2);
    expect(openCount({ candidate: base, own: { numbers: { '/ce/shotSpread': 1 } } })).toBe(1);
    const plan = {
      key: 'k',
      phase: 'ready' as const,
      plan: {
        planId: 'p',
        files: [],
        hasErrors: false,
        diagnostics: [
          { code: 'designer.convert-needs-answer', severity: 'info' as const, message: 'x' },
        ],
      },
    };
    expect(openCount({ candidate: base, plan })).toBe(1);
    expect(openCount({ candidate: { ...base, status: 'already-ce' } })).toBeUndefined();
  });

  it('sorts missing values last in both directions and keeps ties in scan order', () => {
    const list = [
      { ...base, defName: 'X' },
      { ...base, defName: 'Y', vanilla: { mass: 2 } },
      { ...base, defName: 'Z', vanilla: { mass: 1 } },
      { ...base, defName: 'W', vanilla: { mass: 2 } },
    ];
    const none = () => undefined;
    expect(sortCandidates(list, 'mass', 'asc', none).map((c) => c.defName)).toEqual([
      'Z',
      'Y',
      'W',
      'X',
    ]);
    expect(sortCandidates(list, 'mass', 'desc', none).map((c) => c.defName)).toEqual([
      'Y',
      'W',
      'Z',
      'X',
    ]);
  });
});
