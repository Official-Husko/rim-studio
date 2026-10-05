import type { RawChildDto, RawNodeDto } from 'rimstudio-ipc-types';

/** True when a node holds only text (a value element such as relicChance). */
export function isTextOnly(node: RawNodeDto): boolean {
  return node.children.every((child) => typeof child === 'string');
}

/** The text of a text only node. */
export function textOf(node: RawNodeDto): string {
  return node.children.filter((child): child is string => typeof child === 'string').join('');
}

/** A value element: a tag with a text. An empty text gives an empty element. */
export function leafNode(tag: string, text: string): RawNodeDto {
  return { tag, attrs: [], children: text === '' ? [] : [text] };
}

/** The result of reading the JSON text of a node tree. */
export type TreeReading = { ok: true; node: RawNodeDto } | { ok: false; reason: string };

function isChild(value: unknown): value is RawChildDto {
  return typeof value === 'string' || isNode(value);
}

function isNode(value: unknown): value is RawNodeDto {
  if (typeof value !== 'object' || value === null) return false;
  const { tag, attrs, children } = value as Record<string, unknown>;
  return (
    typeof tag === 'string' &&
    Array.isArray(attrs) &&
    attrs.every(
      (pair) =>
        Array.isArray(pair) &&
        pair.length === 2 &&
        typeof pair[0] === 'string' &&
        typeof pair[1] === 'string',
    ) &&
    Array.isArray(children) &&
    children.every(isChild)
  );
}

/**
 * Read the JSON form of a node tree ({tag, attrs, children}). Only the JSON and the shape are
 * checked here; whether the tree is valid XML is the backend's call and comes back as a diagnostic.
 */
export function readTreeJson(text: string): TreeReading {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    return { ok: false, reason: 'json' };
  }
  return isNode(parsed) ? { ok: true, node: parsed } : { ok: false, reason: 'shape' };
}

/** The JSON text of a node tree, for the tree editor. */
export function treeJson(node: RawNodeDto): string {
  return JSON.stringify(node, null, 2);
}
