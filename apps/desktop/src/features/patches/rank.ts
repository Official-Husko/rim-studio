import type { AskItemDto, CeChoiceDto } from 'rimstudio-ipc-types';
import type { ComboboxOption } from 'rimstudio-ui';
import { t } from '~/shared/i18n';

/**
 * The options of a choice ask: the candidates the backend ranked first (best first, with how many
 * converted weapons use each), then the rest in the order the backend gave them.
 */
export function rankedOptions(ask: AskItemDto, choice: CeChoiceDto | undefined): ComboboxOption[] {
  const known = new Set(ask.options);
  const ranked = (choice?.candidates ?? []).filter((c) => known.has(c.name));
  const rankedNames = new Set(ranked.map((c) => c.name));
  const head: ComboboxOption[] = ranked.map((c) => ({
    value: c.name,
    label: c.name,
    hint:
      c.firstDamage !== undefined
        ? t('patches.rank.used-damage', { n: c.usedBy, damage: c.firstDamage })
        : t('patches.rank.used', { n: c.usedBy }),
  }));
  const tail: ComboboxOption[] = ask.options
    .filter((name) => !rankedNames.has(name))
    .map((name) => ({ value: name, label: name }));
  return [...head, ...tail];
}

/** The line under a ranked choice: how the candidates were found and why the choice is open. */
export function rankNote(choice: CeChoiceDto | undefined, ranked?: number): string | undefined {
  const n = ranked ?? choice?.candidates.length ?? 0;
  if (!choice || n === 0) return undefined;
  return t('patches.rank.note', { n });
}
