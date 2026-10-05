import { describe, expect, it } from 'vitest';
import { getAt, segmentsOf, setAt } from './pointer';

describe('pointer', () => {
  it('splits a pointer into its segments', () => {
    expect(segmentsOf('/projectile/damage')).toEqual(['projectile', 'damage']);
    expect(segmentsOf('')).toEqual([]);
  });

  it('reads a value and answers undefined for a missing step', () => {
    const type = { projectile: { damage: { value: 7 } } };
    expect(getAt(type, '/projectile/damage')).toEqual({ value: 7 });
    expect(getAt(type, '/item/mass')).toBeUndefined();
    expect(getAt(undefined, '/a')).toBeUndefined();
  });

  it('sets a value without changing the original and removes a member with undefined', () => {
    const type = { projectile: { damage: 7, speed: 100 } };
    const next = setAt(type, '/projectile/damage', 9);
    expect(next.projectile.damage).toBe(9);
    expect(type.projectile.damage).toBe(7);
    const removed = setAt(type, '/projectile/speed', undefined) as typeof type;
    expect('speed' in removed.projectile).toBe(false);
    expect(setAt({}, '/item/mass', 1)).toEqual({ item: { mass: 1 } });
  });
});
