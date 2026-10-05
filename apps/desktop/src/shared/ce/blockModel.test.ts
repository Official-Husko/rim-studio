import { describe, expect, it } from 'vitest';
import {
  applyPatch,
  leafNode,
  moveBy,
  nodeSummary,
  optionPatch,
  optionTaken,
  parseTree,
  replaceAt,
  treeProblem,
  typedInt,
} from './blockModel';

const BASE = { oneHanded: false, beltFed: false };

describe('applyPatch', () => {
  it('sets members and removes undefined, empty lists and false', () => {
    const start = { ...BASE, bow: true, extraTags: ['A'], isWeaponPlatform: true };
    const next = applyPatch(start, {
      bow: undefined,
      extraTags: [],
      isWeaponPlatform: false,
      recoilPattern: 'R',
    });
    expect(next).toEqual({ ...BASE, recoilPattern: 'R' });
  });

  it('keeps the two plain flags even when false', () => {
    expect(applyPatch({ ...BASE, oneHanded: true }, { oneHanded: false })).toEqual(BASE);
  });
});

describe('list helpers', () => {
  it('replaces, removes and moves entries', () => {
    expect(replaceAt([1, 2, 3], 1, 9)).toEqual([1, 9, 3]);
    expect(replaceAt([1, 2, 3], 1, undefined)).toEqual([1, 3]);
    expect(moveBy([1, 2, 3], 2, -1)).toEqual([1, 3, 2]);
    expect(moveBy([1, 2, 3], 0, -1)).toEqual([1, 2, 3]);
  });

  it('rounds a typed whole number and never goes below zero', () => {
    expect(typedInt(2.6)).toEqual({ value: 3, source: 'typed' });
    expect(typedInt(-4).value).toBe(0);
  });
});

describe('raw trees', () => {
  it('judges text as a tree and summarises elements', () => {
    expect(treeProblem('{"tag":"li"}')).toBeUndefined();
    expect(treeProblem('{"tag"')).toBe('json');
    expect(treeProblem('[]')).toBe('shape');
    expect(treeProblem('{"tag":""}')).toBe('shape');
    expect(parseTree('{"tag":"li","children":["x"]}')).toEqual({
      tag: 'li',
      attrs: [],
      children: ['x'],
    });
    expect(nodeSummary(leafNode('li', 'x'))).toBe('li: x');
    expect(nodeSummary(leafNode('li', ''))).toBe('li');
  });
});

describe('options', () => {
  const tool = {
    id: 'tool-plan',
    field: '/ce/toolPlan',
    label: 'Tool list',
    value: { kind: 'tool-plan' as const, value: [{ label: 'muzzle' }] },
    examples: 1,
    of: 1,
    why: '',
  };

  it('maps a tool list suggestion to a patch and knows when it is taken', () => {
    expect(optionPatch(tool, BASE)).toEqual({ toolPlan: [{ label: 'muzzle' }] });
    expect(optionTaken(tool, BASE)).toBe(false);
    expect(optionTaken(tool, { ...BASE, toolPlan: [{ label: 'x' }] })).toBe(true);
  });
});
