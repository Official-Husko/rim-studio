import type { JsonValue } from 'rimstudio-ipc-types';

interface TreeNode {
  tag: string;
  attrs: Array<[string, string]>;
  children: Array<TreeNode | string>;
}

function isNode(value: unknown): value is TreeNode {
  return (
    typeof value === 'object' &&
    value !== null &&
    typeof (value as { tag?: unknown }).tag === 'string' &&
    Array.isArray((value as { children?: unknown }).children)
  );
}

function escapeText(text: string): string {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function escapeAttr(text: string): string {
  return escapeText(text).replace(/"/g, '&quot;');
}

function lines(node: TreeNode, depth: number, out: string[]): void {
  const pad = '  '.repeat(depth);
  const attrs = (Array.isArray(node.attrs) ? node.attrs : [])
    .map(([k, v]) => ` ${k}="${escapeAttr(String(v))}"`)
    .join('');
  const open = `${pad}<${node.tag}${attrs}`;
  const children = node.children;
  if (children.length === 0) {
    out.push(`${open} />`);
    return;
  }
  if (children.every((c) => typeof c === 'string')) {
    out.push(`${open}>${escapeText(children.join(''))}</${node.tag}>`);
    return;
  }
  out.push(`${open}>`);
  for (const child of children) {
    if (typeof child === 'string') {
      if (child.trim() !== '') out.push(`${pad}  ${escapeText(child.trim())}`);
    } else if (isNode(child)) {
      lines(child, depth + 1, out);
    }
  }
  out.push(`${pad}</${node.tag}>`);
}

/**
 * Show a resolved definition tree (tag, attrs, children as the engine returns it) as indented
 * XML text. Display only: the text is never parsed or sent back.
 */
export function treeToXml(tree: JsonValue): string {
  if (!isNode(tree)) return '';
  const out: string[] = [];
  lines(tree, 0, out);
  return out.join('\n');
}
