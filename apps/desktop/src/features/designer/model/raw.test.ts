import { describe, expect, it } from 'vitest';
import { isTextOnly, leafNode, readTreeJson, textOf, treeJson } from './raw';

describe('raw nodes', () => {
  it('builds a value element and reads its text back', () => {
    const node = leafNode('relicChance', '2');
    expect(node).toEqual({ tag: 'relicChance', attrs: [], children: ['2'] });
    expect(isTextOnly(node)).toBe(true);
    expect(textOf(node)).toBe('2');
    expect(leafNode('empty', '').children).toEqual([]);
  });

  it('knows an element with child elements is not text only', () => {
    expect(
      isTextOnly({ tag: 'a', attrs: [], children: [{ tag: 'li', attrs: [], children: [] }] }),
    ).toBe(false);
  });

  it('reads the JSON of a tree and says what is wrong when it is not one', () => {
    const node = {
      tag: 'a',
      attrs: [['k', 'v']],
      children: ['x', { tag: 'b', attrs: [], children: [] }],
    };
    expect(readTreeJson(treeJson(node as never))).toEqual({ ok: true, node });
    expect(readTreeJson('{')).toEqual({ ok: false, reason: 'json' });
    expect(readTreeJson('{"tag":"a","attrs":[["k"]],"children":[]}')).toEqual({
      ok: false,
      reason: 'shape',
    });
    expect(readTreeJson('[]')).toEqual({ ok: false, reason: 'shape' });
  });
});
