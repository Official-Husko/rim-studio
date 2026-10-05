import type {
  CustomAmmoDto,
  CustomAmmoTypeDto,
  DiagnosticDto,
  SourcedDto,
  ValueSourceDto,
} from 'rimstudio-ipc-types';
import type { ChipKind } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { setAt } from '../pointer';

// Presentation helpers of the custom ammo window. They move values between the form and the spec and sort
// the backend's diagnostics to the field they concern; whether a value is valid is the backend's call.

/** The sections of the window, in order. */
export type Section = 'identity' | 'types' | 'set' | 'art' | 'review';

/** The tabs of one ammo type. */
export type TypeTab = 'projectile' | 'item' | 'recipe';

/** The pointer of the custom ammo inside the draft. */
export const AMMO_POINTER = '/ce/customAmmo';

/** A custom caliber with nothing in it yet. */
export function emptyCustom(): CustomAmmoDto {
  return { name: '', caliber: '', types: [] };
}

/** A new ammo type of a class, with no values. */
export function emptyType(ammoClass = ''): CustomAmmoTypeDto {
  return { key: '', ammoClass, projectile: {}, item: {}, recipe: {} };
}

/** A typed number. */
export function typedNumber(value: number): SourcedDto<number> {
  return { value, source: 'typed' };
}

/** The pointer of a field of one type inside the draft. */
export function typePointer(index: number, sub = ''): string {
  return `${AMMO_POINTER}/types/${index}${sub}`;
}

/** What a type is called in the list: its label, else its key, else its class. */
export function typeTitle(type: CustomAmmoTypeDto, index: number): string {
  return (
    type.label?.trim() ||
    type.key.trim() ||
    type.ammoClass.trim() ||
    t('ammo.custom.type-n', { n: index + 1 })
  );
}

/** The chip kind of a number's source. */
export function chipKindOf(source: ValueSourceDto): ChipKind {
  switch (source) {
    case 'typed':
      return 'typed';
    case 'suggested':
      return 'suggested';
    case 'anchor':
      return 'anchor';
    default:
      return 'derived';
  }
}

/** True when a diagnostic concerns the custom ammunition. */
export function isAmmoDiagnostic(d: DiagnosticDto): boolean {
  return (
    (d.field ?? '').startsWith(AMMO_POINTER) ||
    d.code.startsWith('ce.ammo-') ||
    /^ce\.cep05[0-7]-/.test(d.code)
  );
}

/** The diagnostics of exactly one field. */
export function diagnosticsAt(list: readonly DiagnosticDto[], pointer: string): DiagnosticDto[] {
  return list.filter((d) => d.field === pointer);
}

/** The diagnostics of a field and everything below it. */
export function diagnosticsUnder(list: readonly DiagnosticDto[], prefix: string): DiagnosticDto[] {
  return list.filter((d) => d.field === prefix || (d.field ?? '').startsWith(`${prefix}/`));
}

/** The worst severity among diagnostics, for a badge. */
export function worst(list: readonly DiagnosticDto[]): 'error' | 'warning' | undefined {
  if (list.some((d) => d.severity === 'error')) return 'error';
  if (list.some((d) => d.severity === 'warning')) return 'warning';
  return undefined;
}

/** Where a pointer lives in the window. */
export interface Place {
  section: Section;
  typeIndex?: number;
  tab?: TypeTab;
}

const SET_FIELDS = new Set([
  'similarTo',
  'categoryParent',
  'categoryIcon',
  'defaultType',
  'setExtra',
  'setLabel',
]);
const ART_FIELDS = /\/(texPath|graphicClass|drawSize|graphicExtra|sound\w*)$/;

/** The section, type and tab a diagnostic pointer belongs to; the review section when it has no place. */
export function placeOf(pointer: string | undefined): Place {
  if (!pointer?.startsWith(AMMO_POINTER)) return { section: 'review' };
  const rest = pointer
    .slice(AMMO_POINTER.length)
    .split('/')
    .filter((s) => s !== '');
  const [head, index, tab] = rest;
  if (head === 'types' && index !== undefined && /^\d+$/.test(index)) {
    const typeIndex = Number(index);
    if (ART_FIELDS.test(pointer)) return { section: 'art', typeIndex };
    if (tab === 'projectile' || tab === 'item' || tab === 'recipe') {
      return { section: 'types', typeIndex, tab };
    }
    return { section: 'types', typeIndex, tab: 'projectile' };
  }
  if (head === 'types') return { section: 'types' };
  if (head !== undefined && SET_FIELDS.has(head)) return { section: 'set' };
  return { section: 'identity' };
}

/** A copy of a type for Duplicate: the key gets a number so the def names stay different. */
export function duplicateOf(type: CustomAmmoTypeDto, keys: readonly string[]): CustomAmmoTypeDto {
  const base = (type.key.trim() || type.ammoClass.trim() || 'Type').replace(/\d+$/, '');
  let n = 2;
  while (keys.includes(`${base}${n}`)) n += 1;
  const copy = structuredClone(type);
  copy.key = `${base}${n}`;
  if (copy.label) copy.label = `${copy.label} ${n}`;
  return copy;
}

function isPlain(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function isSourced(value: unknown): value is SourcedDto<unknown> {
  return isPlain(value) && 'value' in value && 'source' in value;
}

function isEmpty(value: unknown): boolean {
  return value === undefined || value === '' || (Array.isArray(value) && value.length === 0);
}

/**
 * The suggested type with what the user has already set laid over it: a value the user typed stays, a value
 * that was only suggested before is replaced by the new suggestion, and lists the user filled stay.
 */
export function overlay(base: unknown, over: unknown): unknown {
  if (isEmpty(over)) return base;
  if (isSourced(over)) {
    return over.source === 'suggested' && base !== undefined ? base : over;
  }
  if (isPlain(over) && isPlain(base)) {
    const keys = new Set([...Object.keys(base), ...Object.keys(over)]);
    const out: Record<string, unknown> = {};
    for (const key of keys) {
      const next = overlay(base[key], over[key]);
      if (next !== undefined) out[key] = next;
    }
    return out;
  }
  return over;
}

let nextId = 0;

/** A fresh id for a type, so its suggestion stays with it when the list is reordered. */
export const newId = (): string => `t${(nextId += 1)}`;

/** A copy of the object with the value set at the pointer; an empty text or list removes the member. */
export function cleaned<T extends object>(root: T, pointer: string, value: unknown): T {
  const empty = value === '' || (Array.isArray(value) && value.length === 0);
  return setAt(root, pointer, empty ? undefined : value);
}
