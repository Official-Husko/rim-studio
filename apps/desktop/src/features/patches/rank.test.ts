import type { AskItemDto, CeChoiceDto } from 'rimstudio-ipc-types';
import { describe, expect, it } from 'vitest';
import { rankNote, rankedOptions } from './rank';

const ask: AskItemDto = {
  field: '/ce/ammoSet',
  label: 'q',
  kind: 'choice',
  options: ['A', 'B', 'C'],
};
const choice: CeChoiceDto = {
  field: '/ce/ammoSet',
  label: 'q',
  kind: 'choice',
  required: true,
  status: 'ask',
  candidates: [
    { name: 'C', usedBy: 3, score: 1.5, firstDamage: 12 },
    { name: 'Z', usedBy: 1, score: 1 },
    { name: 'B', usedBy: 1, score: 0.5 },
  ],
};

describe('rankedOptions', () => {
  it('puts the ranked options first, skips ones the ask does not offer and keeps the rest in order', () => {
    const options = rankedOptions(ask, choice);
    expect(options.map((o) => o.value)).toEqual(['C', 'B', 'A']);
    expect(options[0]?.hint).toBe('used by 3 converted weapons, first damage 12');
    expect(options[1]?.hint).toBe('used by 1 converted weapons');
    expect(options[2]?.hint).toBeUndefined();
  });

  it('keeps the plain order without a ranking', () => {
    expect(rankedOptions(ask, undefined).map((o) => o.value)).toEqual(['A', 'B', 'C']);
    expect(rankNote(undefined)).toBeUndefined();
    expect(rankNote(choice)).toContain('3 choices');
  });
});
