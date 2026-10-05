import { describe, expect, it } from 'vitest';
import { ancestors, getAt, setAt } from './pointer';

describe('pointer', () => {
  const spec = { ranged: { damage: { value: 18 } }, tools: [{ label: 'a' }, { label: 'b' }] };

  it('reads nested values and array entries', () => {
    expect(getAt(spec, '/ranged/damage/value')).toBe(18);
    expect(getAt(spec, '/tools/1/label')).toBe('b');
    expect(getAt(spec, '/nothing/here')).toBeUndefined();
  });

  it('writes a copy and leaves the input alone', () => {
    const next = setAt(spec, '/ranged/damage', { value: 22 });
    expect(getAt(next, '/ranged/damage/value')).toBe(22);
    expect(spec.ranged.damage.value).toBe(18);
  });

  it('creates missing objects and arrays', () => {
    expect(setAt({}, '/ranged/range', 5)).toEqual({ ranged: { range: 5 } });
    expect(setAt({}, '/tools/0/power', 9)).toEqual({ tools: [{ power: 9 }] });
  });

  it('removes a key or an array entry when the value is undefined', () => {
    expect(setAt(spec, '/ranged/damage', undefined)).toEqual({ ranged: {}, tools: spec.tools });
    expect(getAt(setAt(spec, '/tools/0', undefined), '/tools/0/label')).toBe('b');
  });

  it('lists the ancestors of a pointer, nearest first', () => {
    expect(ancestors('/a/b/c')).toEqual(['/a/b/c', '/a/b', '/a']);
  });
});
