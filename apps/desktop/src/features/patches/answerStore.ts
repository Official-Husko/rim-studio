import { signal } from '@preact/signals';
import type { AskItemDto, ConvertCandidateDto } from 'rimstudio-ipc-types';
import type { CeBlockPatch } from '~/shared/ce';
import { patchBlockAnswers } from './blockAnswers';
import { answerOf, emptyAnswers, familyOf, withAnswer, type AnswerSet } from './model';

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

/** Change the answer set of the weapon or of its family with a function of the set held now. */
function updateSet(
  candidate: ConvertCandidateDto,
  scope: AnswerScope,
  change: (set: AnswerSet) => AnswerSet,
): void {
  if (scope === 'family' && familyOf(candidate)) {
    const key = familyOf(candidate);
    familyAnswers.value = {
      ...familyAnswers.peek(),
      [key]: change(familyAnswers.peek()[key] ?? emptyAnswers()),
    };
  } else {
    const key = candidate.defName;
    ownAnswers.value = {
      ...ownAnswers.peek(),
      [key]: change(ownAnswers.peek()[key] ?? emptyAnswers()),
    };
  }
}

/** Change members of the optional block of the weapon or of its family; an undefined member is removed. */
export function setBlock(
  candidate: ConvertCandidateDto,
  scope: AnswerScope,
  patch: CeBlockPatch,
): void {
  updateSet(candidate, scope, (set) => {
    const block = patchBlockAnswers(set.block, patch);
    const next = { ...set };
    if (block) next.block = block;
    else delete next.block;
    return next;
  });
}

/** The optional block in force for a weapon: the family's members with the weapon's own on top. */
export function effectiveBlock(
  candidate: ConvertCandidateDto,
  own: Readonly<Record<string, AnswerSet>> = ownAnswers.value,
  family: Readonly<Record<string, AnswerSet>> = familyAnswers.value,
): CeBlockPatch {
  const key = familyOf(candidate);
  return { ...(key ? family[key]?.block : undefined), ...own[candidate.defName]?.block };
}

/** Leave the under barrel unit out of the conversion, or put it back. */
export function setSkipUnderBarrel(
  candidate: ConvertCandidateDto,
  scope: AnswerScope,
  skip: boolean,
): void {
  updateSet(candidate, scope, (set) => {
    const next = { ...set };
    if (skip) next.skipUnderBarrel = true;
    else delete next.skipUnderBarrel;
    return next;
  });
}

/** True when the weapon's own answers or its family's leave the under barrel unit out. */
export function skipsUnderBarrel(
  candidate: ConvertCandidateDto,
  own: Readonly<Record<string, AnswerSet>> = ownAnswers.value,
  family: Readonly<Record<string, AnswerSet>> = familyAnswers.value,
): boolean {
  const key = familyOf(candidate);
  return (
    own[candidate.defName]?.skipUnderBarrel === true ||
    (key !== '' && family[key]?.skipUnderBarrel === true)
  );
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

/** The custom ammunition of a weapon, from its own answers or its family's; undefined when it has none. */
export function ammoBlock(
  candidate: ConvertCandidateDto,
  own: Readonly<Record<string, AnswerSet>> = ownAnswers.value,
  family: Readonly<Record<string, AnswerSet>> = familyAnswers.value,
): CeBlockPatch | undefined {
  const block = effectiveBlock(candidate, own, family);
  return block.customAmmo ? block : undefined;
}
