import { signal } from '@preact/signals';
import type { CeChoiceDto, ConvertCandidateDto } from 'rimstudio-ipc-types';
import { suggestChoices } from './api';
import { familyOf } from './model';

/** The ranked choices known for a family: absent while loading, empty when the backend has none. */
export type Choices = Readonly<Record<string, CeChoiceDto>>;

/** The ranked ammo set and weapon class choices by family key. */
export const suggestions = signal<Readonly<Record<string, Choices>>>({});

const asked = new Set<string>();

/**
 * Load the ranked choices of a candidate's family once. A failure leaves the plain option list in place:
 * the ranking is a help, not a requirement.
 */
export async function loadChoices(candidate: ConvertCandidateDto): Promise<void> {
  const key = familyOf(candidate) || candidate.defName;
  if (asked.has(key) || candidate.kind === undefined) return;
  asked.add(key);
  try {
    const result = await suggestChoices(candidate.kind, candidate.defName, candidate.tags ?? []);
    const byField: Record<string, CeChoiceDto> = {};
    for (const choice of result.choices) byField[choice.field] = choice;
    suggestions.value = { ...suggestions.peek(), [key]: byField };
  } catch {
    suggestions.value = { ...suggestions.peek(), [key]: {} };
  }
}

/** Forget the loaded choices. */
export function resetChoices(): void {
  asked.clear();
  suggestions.value = {};
}
