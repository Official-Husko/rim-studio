// Moving values in and out of a custom ammo type by JSON pointer. Presentation only: what a value
// means and whether it is valid is the backend's call.

/** The segments of a pointer such as `/projectile/damage`. */
export function segmentsOf(pointer: string): string[] {
  return pointer.split('/').filter((s) => s !== '');
}

/** The value at a pointer, or undefined when any step is missing. */
export function getAt(root: unknown, pointer: string): unknown {
  let node: unknown = root;
  for (const key of segmentsOf(pointer)) {
    if (node === null || typeof node !== 'object') return undefined;
    node = (node as Record<string, unknown>)[key];
  }
  return node;
}

function put(node: unknown, keys: readonly string[], value: unknown): unknown {
  const [head, ...rest] = keys;
  if (head === undefined) return value;
  const base: Record<string, unknown> =
    node !== null && typeof node === 'object' && !Array.isArray(node)
      ? { ...(node as Record<string, unknown>) }
      : {};
  const next = put(base[head], rest, value);
  if (next === undefined) delete base[head];
  else base[head] = next;
  return base;
}

/** A copy of the object with the value set at the pointer; an undefined value removes the member. */
export function setAt<T extends object>(root: T, pointer: string, value: unknown): T {
  return put(root, segmentsOf(pointer), value) as T;
}
