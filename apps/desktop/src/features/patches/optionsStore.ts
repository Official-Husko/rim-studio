import { signal } from '@preact/signals';
import type { CeOptionDto, ConvertCandidateDto } from 'rimstudio-ipc-types';
import { suggestWithBlock } from './api';
import { familyOf } from './model';
import type { AnswerSet } from './model';

/** The optional additions the backend suggests, by weapon definition name. Empty while loading or unknown. */
export const optionSuggestions = signal<Readonly<Record<string, readonly CeOptionDto[]>>>({});

const asked = new Map<string, string>();

/** The key a suggestion depends on: the family and the answered weapon tag class. */
function keyOf(candidate: ConvertCandidateDto, tagClass: string | undefined): string {
  return `${familyOf(candidate) || candidate.defName}|${tagClass ?? ''}`;
}

/**
 * Ask the backend which optional additions the user's conversions suggest for a weapon. The weapon tag class
 * answered so far is sent with the request, because the habits are those of that class. A failure leaves
 * the list empty: the suggestions are a help, not a requirement.
 */
export async function loadOptions(
  candidate: ConvertCandidateDto,
  answers: AnswerSet | undefined,
): Promise<void> {
  if (candidate.kind === undefined) return;
  const key = keyOf(candidate, answers?.weaponTagClass);
  if (asked.get(candidate.defName) === key) return;
  asked.set(candidate.defName, key);
  try {
    const result = await suggestWithBlock(candidate.kind, candidate.defName, candidate.tags ?? [], {
      oneHanded: false,
      beltFed: false,
      ...(answers?.weaponTagClass ? { weaponTagClass: answers.weaponTagClass } : {}),
    });
    optionSuggestions.value = {
      ...optionSuggestions.peek(),
      [candidate.defName]: result.options ?? [],
    };
  } catch {
    optionSuggestions.value = { ...optionSuggestions.peek(), [candidate.defName]: [] };
  }
}

/** Forget the loaded suggestions. */
export function resetOptions(): void {
  asked.clear();
  optionSuggestions.value = {};
}
