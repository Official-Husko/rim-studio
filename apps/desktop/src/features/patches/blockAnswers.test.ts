import { describe, expect, it } from 'vitest';
import {
  blockMembers,
  isNestedPointer,
  isUnderBarrelPointer,
  mergeDeep,
  nested,
  overridesOf,
  patchBlockAnswers,
} from './blockAnswers';

describe('pointers', () => {
  it('tells nested members from plain ones and tool penetration entries', () => {
    expect(isNestedPointer('/ce/underBarrel/range')).toBe(true);
    expect(isNestedPointer('/ce/shotSpread')).toBe(false);
    expect(isNestedPointer('/ce/toolPenetration/stock/blunt')).toBe(false);
    expect(isUnderBarrelPointer('/ce/underBarrel/ammoSet')).toBe(true);
    expect(isUnderBarrelPointer('/ce/ammoSet')).toBe(false);
  });
});

describe('merging', () => {
  it('builds a nested object and merges objects member by member', () => {
    expect(nested(['a', 'b'], 1)).toEqual({ a: { b: 1 } });
    expect(mergeDeep({ a: { b: 1, c: 2 }, d: [1] }, { a: { b: 9 }, d: [2] })).toEqual({
      a: { b: 9, c: 2 },
      d: [2],
    });
  });
});

describe('block members', () => {
  it('drops empty members and a platform flag that is off', () => {
    expect(blockMembers({ bow: true, extraTags: [], isWeaponPlatform: false })).toEqual({
      bow: true,
    });
    expect(blockMembers(undefined)).toEqual({});
  });

  it('applies a patch and forgets a block that ends up empty', () => {
    expect(patchBlockAnswers(undefined, { bow: true })).toEqual({ bow: true });
    expect(patchBlockAnswers({ bow: true }, { bow: undefined })).toBeUndefined();
    expect(patchBlockAnswers({ bow: true }, { recoilPattern: 'R' })).toEqual({
      bow: true,
      recoilPattern: 'R',
    });
  });
});

describe('overridesOf', () => {
  it('puts nested asks on top of the under barrel block, as answered numbers and plain texts', () => {
    const out = overridesOf(
      { underBarrel: { oneAmmoHolder: false, requiresReload: true, ammoSet: 'Old' }, bow: true },
      { '/ce/underBarrel/range': 25, '/ce/shotSpread': 0.1 },
      { '/ce/underBarrel/ammoSet': 'AmmoSet_Grenade' },
    );
    expect(out).toEqual({
      bow: true,
      underBarrel: {
        oneAmmoHolder: false,
        requiresReload: true,
        ammoSet: 'AmmoSet_Grenade',
        range: { value: 25, source: 'answered' },
      },
    });
  });
});
