import { describe, expect, it } from 'vitest';
import type { ArchetypeProposalDto, DraftDto } from 'rimstudio-ipc-types';
import { fixture } from '../../testSupport';
import {
  allValues,
  describeField,
  groupRows,
  groupTitle,
  headlineStats,
  reasonText,
} from './wizard-rows';

const sniper = () => fixture<ArchetypeProposalDto>('designer-wizard-propose-sniper');

describe('wizard rows', () => {
  it('groups a gun proposal into damage, range, firing, bash attacks and weight', () => {
    const groups = groupRows(sniper(), undefined);
    expect(groups.map((g) => g.id)).toEqual(['damage', 'range', 'firing', 'attacks', 'weight']);
    expect(groups[0]?.rows.map((r) => r.label)).toEqual(['Damage', 'Armor penetration']);
    expect(groups[1]?.rows.map((r) => r.label)).toContain('Range');
    expect(groupTitle('attacks', false)).toBe('Bash attacks');
    expect(groupTitle('attacks', true)).toBe('Attacks');
  });

  it('names the tool numbers by the tool and gives units where they matter', () => {
    const proposal = sniper();
    expect(describeField(proposal, '/tools/1/power').label).toBe('Barrel damage');
    expect(describeField(proposal, '/ranged/range')).toEqual({ label: 'Range', unit: 'tiles' });
    expect(describeField(proposal, '/ranged/damage').unit).toBeUndefined();
    expect(describeField(proposal, '/unknown').label).toBe('/unknown');
  });

  it('shows how a number moved since the last proposal', () => {
    const now = fixture<ArchetypeProposalDto>('designer-wizard-propose-sniper-stronger');
    const rows = groupRows(now, sniper()).flatMap((g) => g.rows);
    const damage = rows.find((r) => r.key === '/ranged/damage');
    expect(damage?.was).toBe(28);
    expect(damage?.value.value).toBeGreaterThan(28);
    const none = groupRows(sniper(), sniper()).flatMap((g) => g.rows);
    expect(none.every((r) => r.was === undefined)).toBe(true);
  });

  it('lists every proposed number and picks the headline stats of a kind', () => {
    expect(allValues(sniper()).length).toBe(sniper().values.length + 4);
    expect(allValues(undefined)).toEqual([]);
    expect(headlineStats(sniper()).map((s) => s.stat)).toEqual([
      'damage',
      'range',
      'dps',
      'cooldown',
      'mass',
    ]);
    const sword = fixture<ArchetypeProposalDto>('designer-wizard-propose-sword');
    expect(headlineStats(sword).map((s) => s.stat)).toEqual([
      'swing_damage',
      'dps',
      'fight_dps',
      'mass',
    ]);
    expect(groupRows(sword, undefined).map((g) => g.id)).toEqual(['attacks', 'weight']);
  });

  it('drops the lead in of a reason that repeats the row', () => {
    expect(reasonText('damage 28: the install median is 12.')).toBe('The install median is 12.');
    expect(reasonText('armor penetration 0.385: left to the game.')).toBe('Left to the game.');
    expect(reasonText('stock power 9: the median of the 15 tools.')).toBe(
      'The median of the 15 tools.',
    );
    expect(reasonText('Something else entirely.')).toBe('Something else entirely.');
  });
});

describe('wizard rows of a retune', () => {
  it('carries the number the user typed for a locked field', () => {
    const proposal = sniper();
    const locked = {
      ...proposal,
      values: proposal.values.map((v) =>
        v.field === '/ranged/damage' ? { ...v, locked: true } : v,
      ),
    };
    const draft = fixture<{ draft: DraftDto }>('designer-wizard-apply-sniper').draft;
    const typedDraft = {
      ...draft,
      spec: {
        ...draft.spec,
        ranged: { ...draft.spec.ranged, damage: { value: 41, source: 'typed' as const } },
      },
    } as DraftDto;
    const rows = groupRows(locked, undefined, typedDraft).flatMap((g) => g.rows);
    expect(rows.find((r) => r.key === '/ranged/damage')?.yours).toBe(41);
    expect(rows.find((r) => r.key === '/ranged/range')?.yours).toBeUndefined();
  });
});
