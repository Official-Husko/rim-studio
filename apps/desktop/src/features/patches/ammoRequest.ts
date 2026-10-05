import type { ConvertCandidateDto, ConvertRequestDto, CustomAmmoDto } from 'rimstudio-ipc-types';
import { familyAnswers, ownAnswers, type AnswerScope } from './answerStore';
import { buildRequest, emptyAnswers, familyOf, type AnswerSet } from './model';

function withCustom(set: AnswerSet | undefined, custom: CustomAmmoDto): AnswerSet {
  const next: AnswerSet = {
    ...(set ?? emptyAnswers()),
    block: { ...set?.block, customAmmo: custom },
  };
  delete next.ammoSet;
  return next;
}

/**
 * The conversion request of a weapon as it would be with a custom caliber in its answers: the same request
 * the page sends, with the custom ammunition in the answers of the scope that is being edited. It is used
 * only to plan, so the backend can check the custom ammunition before it is saved.
 */
export function requestWithCustomAmmo(
  candidate: ConvertCandidateDto,
  scope: AnswerScope,
  custom: CustomAmmoDto,
): ConvertRequestDto {
  const own = ownAnswers.peek()[candidate.defName];
  const key = familyOf(candidate);
  const family = key ? familyAnswers.peek()[key] : undefined;
  if (scope === 'family' && key) {
    return buildRequest(candidate, own, withCustom(family, custom));
  }
  return buildRequest(candidate, withCustom(own, custom), family);
}
