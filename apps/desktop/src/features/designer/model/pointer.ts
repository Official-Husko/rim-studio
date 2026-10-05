/** A JSON pointer such as /ranged/damage or /tools/0/power. */
export type Pointer = string;

function segments(pointer: Pointer): string[] {
  return pointer
    .split('/')
    .slice(1)
    .map((s) => s.replace(/~1/g, '/').replace(/~0/g, '~'));
}

/** Read the value a pointer names; undefined when any step is missing. */
export function getAt(root: unknown, pointer: Pointer): unknown {
  let node: unknown = root;
  for (const key of segments(pointer)) {
    if (node === null || typeof node !== 'object') return undefined;
    node = (node as Record<string, unknown>)[key];
  }
  return node;
}

function writeInto(node: unknown, keys: string[], value: unknown): unknown {
  const [head, ...rest] = keys;
  if (head === undefined) return value;
  if (Array.isArray(node)) {
    const copy = [...node] as unknown[];
    const index = Number(head);
    const next = rest.length === 0 ? value : writeInto(copy[index], rest, value);
    if (next === undefined) copy.splice(index, 1);
    else copy[index] = next;
    return copy;
  }
  if (node === undefined || node === null) {
    // a missing container: numeric keys build an array, anything else an object
    if (/^\d+$/.test(head)) {
      const created: unknown[] = [];
      const next = rest.length === 0 ? value : writeInto(undefined, rest, value);
      if (next !== undefined) created[Number(head)] = next;
      return created;
    }
  }
  const base: Record<string, unknown> =
    node !== null && typeof node === 'object' ? { ...(node as Record<string, unknown>) } : {};
  const next = rest.length === 0 ? value : writeInto(base[head], rest, value);
  if (next === undefined) delete base[head];
  else base[head] = next;
  return base;
}

/**
 * A copy of root with the value written at the pointer, creating objects on the way. Writing
 * undefined removes the key (or the array entry). The input is never changed.
 */
export function setAt<T>(root: T, pointer: Pointer, value: unknown): T {
  return writeInto(root, segments(pointer), value) as T;
}

/** The pointers that lead to a pointer, nearest first: /a/b/c gives /a/b/c, /a/b, /a. */
export function ancestors(pointer: Pointer): Pointer[] {
  const out: Pointer[] = [];
  let current = pointer;
  while (current.length > 0) {
    out.push(current);
    current = current.slice(0, current.lastIndexOf('/'));
  }
  return out;
}
