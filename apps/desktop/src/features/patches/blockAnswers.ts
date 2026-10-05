import type { CePatchSpecDto } from 'rimstudio-ipc-types';
import type { CeBlockPatch } from '~/shared/ce';

// How the answers beyond the plain asks (nested fields such as the under barrel unit, and the optional
// members of the block) become the `overrides` of a conversion request. Only moving values around: what
// they mean and whether they are valid is the backend's call.

type Json = Record<string, unknown>;

function isObject(value: unknown): value is Json {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

/** Merge `from` into a copy of `into`: objects member by member, everything else replaced. */
export function mergeDeep(into: Json, from: Json): Json {
  const out: Json = { ...into };
  for (const [key, value] of Object.entries(from)) {
    const held = out[key];
    out[key] = isObject(held) && isObject(value) ? mergeDeep(held, value) : value;
  }
  return out;
}

/** An object with one value set at a path of keys. */
export function nested(path: readonly string[], value: unknown): Json {
  const [head, ...rest] = path;
  if (head === undefined) return {};
  return { [head]: rest.length === 0 ? value : nested(rest, value) };
}

/** True when a pointer below `/ce` names a member inside another member, such as `/ce/underBarrel/range`. */
export function isNestedPointer(field: string): boolean {
  const parts = field.split('/').filter((s) => s !== '' && s !== 'ce');
  return parts.length >= 2 && parts[0] !== 'toolPenetration';
}

/** True when the pointer is an under barrel member. */
export function isUnderBarrelPointer(field: string): boolean {
  return field.startsWith('/ce/underBarrel');
}

/** The block members the optional editors hold, with the members that are empty removed. */
export function blockMembers(block: CeBlockPatch | undefined): Json {
  const out: Json = {};
  for (const [key, value] of Object.entries(block ?? {})) {
    if (value === undefined || (Array.isArray(value) && value.length === 0)) continue;
    if (value === false && key === 'isWeaponPlatform') continue;
    out[key] = value;
  }
  return out;
}

/** A copy of a block patch with a patch applied: undefined members are removed. */
export function patchBlockAnswers(
  block: CeBlockPatch | undefined,
  patch: CeBlockPatch,
): CeBlockPatch | undefined {
  const next: Record<string, unknown> = { ...(block ?? {}) };
  for (const [key, value] of Object.entries(patch)) {
    if (value === undefined) delete next[key];
    else next[key] = value;
  }
  return Object.keys(blockMembers(next as CeBlockPatch)).length === 0
    ? undefined
    : (next as CeBlockPatch);
}

/** The overrides of the answers: the optional block members, then the nested asks on top. */
export function overridesOf(
  block: CeBlockPatch | undefined,
  numbers: Readonly<Record<string, number>>,
  values: Readonly<Record<string, string | boolean>>,
): Partial<CePatchSpecDto> {
  let out: Json = blockMembers(block);
  const nestedAnswers: Array<[string, unknown]> = [
    ...Object.entries(numbers)
      .filter(([field]) => isNestedPointer(field))
      .map(([field, value]): [string, unknown] => [field, { value, source: 'answered' }]),
    ...Object.entries(values),
  ];
  for (const [field, value] of nestedAnswers.sort(([a], [b]) => a.localeCompare(b))) {
    const path = field.split('/').filter((s) => s !== '' && s !== 'ce');
    out = mergeDeep(out, nested(path, value));
  }
  return out as Partial<CePatchSpecDto>;
}
