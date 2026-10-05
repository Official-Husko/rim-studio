import { beforeEach, describe, expect, it } from 'vitest';
import { loadFixture } from 'rimstudio-testkit';
import type { ConvertScanDto } from 'rimstudio-ipc-types';
import { effectiveAnswer, resetAnswers, setAnswer } from './answerStore';

const candidate = loadFixture<ConvertScanDto>('designer_convert_scan').candidates[0];
if (!candidate) throw new Error('fixture');
const flag = candidate.asks.find((a) => a.field === '/ce/beltFed');
if (!flag) throw new Error('fixture');

beforeEach(resetAnswers);

describe('answerStore', () => {
  it('lets the weapon answer win over the family answer', () => {
    setAnswer(candidate, 'family', flag, true);
    expect(effectiveAnswer(candidate, flag)).toEqual({ value: true, from: 'family' });
    setAnswer(candidate, 'weapon', flag, false);
    expect(effectiveAnswer(candidate, flag)).toEqual({ value: false, from: 'weapon' });
    setAnswer(candidate, 'weapon', flag, undefined);
    expect(effectiveAnswer(candidate, flag).from).toBe('family');
  });

  it('writes to the weapon when it has no family', () => {
    const lone = { ...candidate, family: '' };
    setAnswer(lone, 'family', flag, true);
    expect(effectiveAnswer(lone, flag)).toEqual({ value: true, from: 'weapon' });
  });
});
