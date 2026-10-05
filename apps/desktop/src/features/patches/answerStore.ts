import { signal } from '@preact/signals';
import type { AskItemDto, ConvertCandidateDto } from 'rimstudio-ipc-types';
import { answerOf, familyOf, withAnswer, type AnswerSet } from './model';

/** The answers of each weapon by definition name. */
export const ownAnswers = signal<Readonly<Record<string, AnswerSet>>>({});

/** The answers shared by a family of weapons, by family key. */
export const familyAnswers = signal<Readonly<Record<string, AnswerSet>>>({});

/** Where the form writes: one weapon or its whole family. */
export type AnswerScope = 'weapon' | 'family';

/** Change one answer. A family answer needs a family key. */
export function setAnswer(
  candidate: ConvertCandidateDto,
  scope: AnswerScope,
  ask: AskItemDto,
  value: string | number | boolean | undefined,
): void {
  if (scope === 'family' && familyOf(candidate)) {
    const key = familyOf(candidate);
    familyAnswers.value = {
      ...familyAnswers.peek(),
      [key]: withAnswer(familyAnswers.peek()[key], ask, value),
    };
  } else {
    const key = candidate.defName;
    ownAnswers.value = {
      ...ownAnswers.peek(),
      [key]: withAnswer(ownAnswers.peek()[key], ask, value),
    };
  }
}

/** The answer in force for an ask: the weapon's own first, then the family's. */
export function effectiveAnswer(
  candidate: ConvertCandidateDto,
  ask: AskItemDto,
  own: Readonly<Record<string, AnswerSet>> = ownAnswers.value,
  family: Readonly<Record<string, AnswerSet>> = familyAnswers.value,
): { value: string | number | boolean | undefined; from: AnswerScope | undefined } {
  const mine = answerOf(own[candidate.defName], ask);
  if (mine !== undefined) return { value: mine, from: 'weapon' };
  const key = familyOf(candidate);
  const shared = key ? answerOf(family[key], ask) : undefined;
  return shared !== undefined
    ? { value: shared, from: 'family' }
    : { value: undefined, from: undefined };
}

/** Forget every answer. */
export function resetAnswers(): void {
  ownAnswers.value = {};
  familyAnswers.value = {};
}
