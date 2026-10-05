import type {
  AskItemDto,
  ConvertAnswerGroupDto,
  ConvertAnswersDto,
  ConvertCandidateDto,
  ConvertRequestDto,
  ConvertStatusDto,
  DiagnosticDto,
  SeverityDto,
} from 'rimstudio-ipc-types';
import type { CeBlockPatch } from '~/shared/ce';
import type { MessageKey } from '~/shared/i18n';
import { blockMembers, isNestedPointer, overridesOf } from './blockAnswers';

// Display helpers and the request builders of the page. The numbers, the questions and the plan come from
// the backend; nothing here decides what a conversion contains.

/** The text key of each status chip. */
export const STATUS_LABEL: Record<ConvertStatusDto, MessageKey> = {
  'not-converted': 'patches.status.not-converted',
  'already-ce': 'patches.status.already-ce',
  'unsupported-kind': 'patches.status.unsupported-kind',
  'target-not-found': 'patches.status.target-not-found',
};

/** Which badge tone each status gets. */
export const STATUS_TONE: Record<ConvertStatusDto, 'info' | 'success' | 'warning' | 'danger'> = {
  'not-converted': 'info',
  'already-ce': 'success',
  'unsupported-kind': 'warning',
  'target-not-found': 'danger',
};

/** True for a weapon the page can convert. */
export function isConvertible(candidate: ConvertCandidateDto): boolean {
  return candidate.status === 'not-converted';
}

/** The family key of a candidate; empty when it has none. */
export function familyOf(candidate: ConvertCandidateDto): string {
  return candidate.family ?? '';
}

/**
 * The default projectile that the family key ends with (`ranged/<first tag>/<default projectile>`); empty
 * for a melee weapon. The tags themselves come from the scan, not from the key.
 */
export function projectileOf(candidate: ConvertCandidateDto): string {
  return familyOf(candidate).split('/').slice(2).join('/');
}

/** The ask field pointer split into its path segments, without the leading `ce`. */
function segments(field: string): string[] {
  return field.split('/').filter((s) => s !== '' && s !== 'ce');
}

/** What the user answered, kept in one shape for a weapon and for a family of weapons. */
export interface AnswerSet {
  ammoSet?: string;
  weaponTagClass?: string;
  oneHanded?: boolean;
  beltFed?: boolean;
  /** Numeric answers by ask field pointer, for example `/ce/shotSpread`. */
  numbers: Record<string, number>;
  /** Text and flag answers of nested asks by pointer, for example `/ce/underBarrel/ammoSet`. */
  values?: Record<string, string | boolean>;
  /** The optional members of the block (bows, platforms, tool list, extras) set in the options tab. */
  block?: CeBlockPatch;
  /** Leave the under barrel unit of the weapon out of the conversion. */
  skipUnderBarrel?: boolean;
}

/** An empty set. */
export function emptyAnswers(): AnswerSet {
  return { numbers: {} };
}

/** True when the set holds no answer. */
export function isEmptyAnswers(set: AnswerSet | undefined): boolean {
  return (
    !set ||
    (set.ammoSet === undefined &&
      set.weaponTagClass === undefined &&
      set.oneHanded === undefined &&
      set.beltFed === undefined &&
      Object.keys(set.numbers).length === 0 &&
      Object.keys(set.values ?? {}).length === 0 &&
      Object.keys(blockMembers(set.block)).length === 0 &&
      set.skipUnderBarrel !== true)
  );
}

/** The answer of one ask in a set, as text for choices, a flag or a number. */
export function answerOf(
  set: AnswerSet | undefined,
  ask: AskItemDto,
): string | number | boolean | undefined {
  if (!set) return undefined;
  switch (ask.field) {
    case '/ce/ammoSet':
      return set.ammoSet;
    case '/ce/weaponTagClass':
      return set.weaponTagClass;
    case '/ce/oneHanded':
      return set.oneHanded;
    case '/ce/beltFed':
      return set.beltFed;
    default:
      return set.numbers[ask.field] ?? set.values?.[ask.field];
  }
}

/** A copy of the set with one answer changed; undefined removes it. */
export function withAnswer(
  set: AnswerSet | undefined,
  ask: AskItemDto,
  value: string | number | boolean | undefined,
): AnswerSet {
  const next: AnswerSet = { ...(set ?? emptyAnswers()), numbers: { ...(set?.numbers ?? {}) } };
  switch (ask.field) {
    case '/ce/ammoSet':
      if (typeof value === 'string' && value !== '') next.ammoSet = value;
      else delete next.ammoSet;
      break;
    case '/ce/weaponTagClass':
      if (typeof value === 'string' && value !== '') next.weaponTagClass = value;
      else delete next.weaponTagClass;
      break;
    case '/ce/oneHanded':
      if (typeof value === 'boolean') next.oneHanded = value;
      else delete next.oneHanded;
      break;
    case '/ce/beltFed':
      if (typeof value === 'boolean') next.beltFed = value;
      else delete next.beltFed;
      break;
    default:
      if (typeof value === 'string' || typeof value === 'boolean') {
        next.values = { ...(next.values ?? {}), [ask.field]: value };
        delete next.numbers[ask.field];
      } else {
        if (next.values) next.values = withoutKey(next.values, ask.field);
        if (typeof value === 'number' && Number.isFinite(value)) next.numbers[ask.field] = value;
        else delete next.numbers[ask.field];
      }
  }
  return next;
}

function withoutKey<T>(map: Record<string, T>, key: string): Record<string, T> {
  const { [key]: _removed, ...rest } = map;
  return rest;
}

type Sourced = { value: number; source: 'answered' };

function sourced(value: number): Sourced {
  return { value, source: 'answered' };
}

/**
 * The numeric answers as the two members of the answers object: the tool penetration entries (one per
 * tool, from `/ce/toolPenetration/<tool>/<sharp|blunt>`) and the overrides of the other fields (from
 * `/ce/<name>`).
 */
function numberMembers(numbers: Record<string, number>) {
  const tools = new Map<string, { tool: string; sharp?: Sourced; blunt?: Sourced }>();
  const overrides: Record<string, Sourced> = {};
  for (const field of Object.keys(numbers).sort()) {
    const value = numbers[field];
    if (value === undefined) continue;
    const [first = '', tool = '', side = ''] = segments(field);
    if (first === 'toolPenetration' && tool !== '' && (side === 'sharp' || side === 'blunt')) {
      const entry = tools.get(tool) ?? { tool };
      entry[side] = sourced(value);
      tools.set(tool, entry);
    } else if (first !== '' && !isNestedPointer(field)) {
      overrides[first] = sourced(value);
    }
  }
  return { toolPenetration: [...tools.values()], overrides };
}

/** The answers of one weapon as the typed answers of the request. */
export function toAnswersDto(set: AnswerSet | undefined): ConvertAnswersDto {
  const { toolPenetration, overrides } = numberMembers(set?.numbers ?? {});
  return {
    ...(set?.ammoSet !== undefined ? { ammoSet: set.ammoSet } : {}),
    ...(set?.weaponTagClass !== undefined ? { weaponTagClass: set.weaponTagClass } : {}),
    ...(set?.oneHanded !== undefined ? { oneHanded: set.oneHanded } : {}),
    ...(set?.beltFed !== undefined ? { beltFed: set.beltFed } : {}),
    ...(toolPenetration.length > 0 ? { toolPenetration } : {}),
    ...(set?.skipUnderBarrel === true ? { skipUnderBarrel: true } : {}),
    overrides: {
      oneHanded: false,
      beltFed: false,
      ...overrides,
      ...overridesOf(set?.block, set?.numbers ?? {}, set?.values ?? {}),
    },
  };
}

/** The answers of a family as the raw object of an answer group, holding only what was answered. */
export function toGroupAnswers(set: AnswerSet): Record<string, unknown> {
  const { toolPenetration, overrides } = numberMembers(set.numbers);
  const groupOverrides = {
    ...overrides,
    ...overridesOf(set.block, set.numbers, set.values ?? {}),
  };
  return {
    ...(set.ammoSet !== undefined ? { ammoSet: set.ammoSet } : {}),
    ...(set.weaponTagClass !== undefined ? { weaponTagClass: set.weaponTagClass } : {}),
    ...(set.oneHanded !== undefined ? { oneHanded: set.oneHanded } : {}),
    ...(set.beltFed !== undefined ? { beltFed: set.beltFed } : {}),
    ...(toolPenetration.length > 0 ? { toolPenetration } : {}),
    ...(set.skipUnderBarrel === true ? { skipUnderBarrel: true } : {}),
    ...(Object.keys(groupOverrides).length > 0 ? { overrides: groupOverrides } : {}),
  };
}

/** The request of one weapon: its own answers and, when the family has some, the family group. */
export function buildRequest(
  candidate: ConvertCandidateDto,
  own: AnswerSet | undefined,
  family: AnswerSet | undefined,
): ConvertRequestDto {
  const groups: ConvertAnswerGroupDto[] =
    family && !isEmptyAnswers(family) && familyOf(candidate)
      ? [
          {
            family: familyOf(candidate),
            answers: toGroupAnswers(family) as ConvertAnswerGroupDto['answers'],
          },
        ]
      : [];
  return {
    defName: candidate.defName,
    answers: toAnswersDto(own),
    ...(groups.length > 0 ? { groups } : {}),
  };
}

/** Severity order, most serious first. */
export const SEVERITY_ORDER: SeverityDto[] = ['error', 'warning', 'info', 'hint'];

/** The diagnostic code of an open question in a plan. */
export const NEEDS_ANSWER = 'designer.convert-needs-answer';
const DERIVED_CODES = new Set(['designer.convert-derived', 'ce.derived-value']);
const NOT_CHECKED = 'ce.not-checked';

/** A number the conversion filled in, with where it came from. */
export interface DerivedValue {
  field: string;
  value: string;
  how: string;
  predictor?: string;
  rating?: string;
}

/**
 * Read a derived value from its diagnostic. The diagnostic carries the field and a sentence of the form
 * `FIELD = VALUE was HOW`; the predictor and rating are read out of the sentence for display only.
 */
export function parseDerived(d: DiagnosticDto): DerivedValue {
  const match = /^(\S+) = (\S+) was (.*)$/.exec(d.message);
  const how = d.args?.how ?? match?.[3] ?? d.message;
  const predictor = /predicted \((\w+)\)/.exec(how)?.[1];
  const rating = /rated (\w+)/.exec(how)?.[1];
  return {
    field: (d.field ?? match?.[1] ?? '').replace(/^ce\./, ''),
    value: d.args?.value ?? match?.[2] ?? '',
    how,
    ...(predictor ? { predictor } : {}),
    ...(rating ? { rating } : {}),
  };
}

/** A plan's diagnostics sorted into what the page shows where. */
export interface SplitDiagnostics {
  open: DiagnosticDto[];
  derived: DiagnosticDto[];
  notChecked: DiagnosticDto[];
  other: DiagnosticDto[];
}

/** Split diagnostics into open questions, derived values, rules that did not run and the rest. */
export function splitDiagnostics(list: readonly DiagnosticDto[]): SplitDiagnostics {
  const out: SplitDiagnostics = { open: [], derived: [], notChecked: [], other: [] };
  for (const d of list) {
    if (d.code === NEEDS_ANSWER) out.open.push(d);
    else if (DERIVED_CODES.has(d.code)) out.derived.push(d);
    else if (d.code === NOT_CHECKED) out.notChecked.push(d);
    else out.other.push(d);
  }
  out.other.sort((a, b) => SEVERITY_ORDER.indexOf(a.severity) - SEVERITY_ORDER.indexOf(b.severity));
  return out;
}
