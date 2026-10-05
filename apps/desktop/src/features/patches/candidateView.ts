import type { AskItemDto, ConvertCandidateDto, ConvertStatusDto } from 'rimstudio-ipc-types';
import type { SortDirection } from 'rimstudio-ui';
import { answerOf, type AnswerSet, isConvertible, NEEDS_ANSWER } from './model';
import type { PlanEntry } from './planStore';

// How the table orders and narrows the rows it was given. This only arranges what the backend sent; it
// decides nothing about a conversion.

/** The columns the table can be sorted by. */
export type SortKey =
  'weapon' | 'status' | 'damage' | 'range' | 'cooldown' | 'warmup' | 'mass' | 'open';

/** The numeric columns, in display order, with the field of the vanilla block each one reads. */
export const NUMBER_COLUMNS = ['damage', 'range', 'cooldown', 'warmup', 'mass'] as const;
export type NumberColumn = (typeof NUMBER_COLUMNS)[number];

/** What the filter bar holds; an empty text means no filter. */
export interface CandidateFilter {
  status: ConvertStatusDto | '';
  tag: string;
}

export const NO_FILTER: CandidateFilter = { status: '', tag: '' };

/** The tag and class chips of a candidate: tags first, then classes, each once. */
export function chipsOf(candidate: ConvertCandidateDto): string[] {
  return [...new Set([...(candidate.tags ?? []), ...(candidate.weaponClasses ?? [])])];
}

/** Every tag or class that appears in the list, sorted, for the filter choices. */
export function allTags(list: readonly ConvertCandidateDto[]): string[] {
  const found = new Set<string>();
  for (const c of list) for (const chip of chipsOf(c)) found.add(chip);
  return [...found].sort((a, b) => a.localeCompare(b));
}

/** The candidates that pass the filter. */
export function filterCandidates(
  list: readonly ConvertCandidateDto[],
  filter: CandidateFilter,
): ConvertCandidateDto[] {
  return list.filter(
    (c) =>
      (filter.status === '' || c.status === filter.status) &&
      (filter.tag === '' || chipsOf(c).includes(filter.tag)),
  );
}

/** How many questions of a weapon are still open for the table (see {@link openCount}). */
export interface OpenInput {
  candidate: ConvertCandidateDto;
  plan?: PlanEntry;
  own?: AnswerSet;
  family?: AnswerSet;
}

function isAnswered(ask: AskItemDto, own?: AnswerSet, family?: AnswerSet): boolean {
  return answerOf(own, ask) !== undefined || answerOf(family, ask) !== undefined;
}

/**
 * The open questions of a weapon. A plan that is ready for the current answers is the authority and its
 * own open diagnostics are counted. Without one, the asks of the scan that nobody answered yet are.
 */
export function openCount({ candidate, plan, own, family }: OpenInput): number | undefined {
  if (!isConvertible(candidate)) return undefined;
  if (plan?.phase === 'ready' && plan.plan) {
    return plan.plan.diagnostics.filter((d) => d.code === NEEDS_ANSWER).length;
  }
  return candidate.asks.filter((ask) => !isAnswered(ask, own, family)).length;
}

function numberOf(
  c: ConvertCandidateDto,
  key: SortKey,
  open: number | undefined,
): number | undefined {
  switch (key) {
    case 'open':
      return open;
    case 'damage':
    case 'range':
    case 'cooldown':
    case 'warmup':
    case 'mass':
      return c.vanilla?.[key];
    default:
      return undefined;
  }
}

/**
 * A sorted copy. A row without a value goes last in either direction, and ties keep the scan order, so
 * the result is stable.
 */
export function sortCandidates(
  list: readonly ConvertCandidateDto[],
  key: SortKey,
  direction: SortDirection,
  openOf: (c: ConvertCandidateDto) => number | undefined,
): ConvertCandidateDto[] {
  const sign = direction === 'asc' ? 1 : -1;
  const indexed = list.map((c, index) => ({ c, index }));
  indexed.sort((a, b) => {
    let order: number;
    if (key === 'weapon') {
      order = (a.c.label || a.c.defName).localeCompare(b.c.label || b.c.defName);
    } else if (key === 'status') {
      order = a.c.status.localeCompare(b.c.status);
    } else {
      const left = numberOf(a.c, key, openOf(a.c));
      const right = numberOf(b.c, key, openOf(b.c));
      if (left === undefined || right === undefined) {
        if (left === right) return a.index - b.index;
        return left === undefined ? 1 : -1;
      }
      order = left - right;
    }
    return order === 0 ? a.index - b.index : order * sign;
  });
  return indexed.map((entry) => entry.c);
}
