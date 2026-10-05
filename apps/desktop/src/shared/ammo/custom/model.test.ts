import { describe, expect, it } from 'vitest';
import {
  chipKindOf,
  cleaned,
  diagnosticsAt,
  diagnosticsUnder,
  duplicateOf,
  emptyType,
  isAmmoDiagnostic,
  overlay,
  placeOf,
  typePointer,
  typeTitle,
  worst,
} from './model';

const diag = (code: string, severity: 'error' | 'warning' | 'info', field?: string) => ({
  code,
  severity,
  message: code,
  ...(field ? { field } : {}),
});

describe('custom ammo model', () => {
  it('names a type by its label, key, class or position', () => {
    expect(typeTitle({ ...emptyType('FullMetalJacket'), label: 'Ball' }, 0)).toBe('Ball');
    expect(typeTitle({ ...emptyType('FullMetalJacket'), key: 'FMJ' }, 0)).toBe('FMJ');
    expect(typeTitle(emptyType('FullMetalJacket'), 0)).toBe('FullMetalJacket');
    expect(typeTitle(emptyType(), 1)).toBe('Type 2');
  });

  it('maps a source to a chip kind', () => {
    expect(chipKindOf('typed')).toBe('typed');
    expect(chipKindOf('suggested')).toBe('suggested');
    expect(chipKindOf('anchor')).toBe('anchor');
    expect(chipKindOf('answered')).toBe('derived');
  });

  it('knows the diagnostics of the ammunition', () => {
    expect(isAmmoDiagnostic(diag('x.y', 'error', '/ce/customAmmo/name'))).toBe(true);
    expect(isAmmoDiagnostic(diag('ce.ammo-class-unknown', 'error'))).toBe(true);
    expect(isAmmoDiagnostic(diag('ce.cep053-recipe-broken', 'error'))).toBe(true);
    expect(isAmmoDiagnostic(diag('design.required-missing', 'error', '/ce/bulk'))).toBe(false);
  });

  it('sorts diagnostics to a field and below it, and finds the worst severity', () => {
    const list = [
      diag('a', 'error', typePointer(0, '/projectile/damage')),
      diag('b', 'warning', typePointer(0, '/projectile/speed')),
      diag('c', 'info', typePointer(1, '/projectile/speed')),
    ];
    expect(diagnosticsAt(list, typePointer(0, '/projectile/damage'))).toHaveLength(1);
    expect(diagnosticsUnder(list, typePointer(0))).toHaveLength(2);
    expect(worst(diagnosticsUnder(list, typePointer(0)))).toBe('error');
    expect(worst(diagnosticsUnder(list, typePointer(1)))).toBeUndefined();
    expect(worst([list[1] as (typeof list)[number]])).toBe('warning');
  });

  it('finds the section, type and tab of a pointer', () => {
    expect(placeOf('/ce/customAmmo/name')).toEqual({ section: 'identity' });
    expect(placeOf('/ce/customAmmo/similarTo')).toEqual({ section: 'set' });
    expect(placeOf('/ce/customAmmo/types/1/recipe/ingredients/0/count')).toEqual({
      section: 'types',
      typeIndex: 1,
      tab: 'recipe',
    });
    expect(placeOf('/ce/customAmmo/types/0/item/texPath')).toEqual({
      section: 'art',
      typeIndex: 0,
    });
    expect(placeOf('/ce/customAmmo/types/2/ammoClass')).toEqual({
      section: 'types',
      typeIndex: 2,
      tab: 'projectile',
    });
    expect(placeOf('/ce/bulk')).toEqual({ section: 'review' });
    expect(placeOf(undefined)).toEqual({ section: 'review' });
  });

  it('duplicates a type with a new key', () => {
    const type = { ...emptyType('ArmorPiercing'), key: 'AP', label: 'AP round' };
    const copy = duplicateOf(type, ['AP']);
    expect(copy.key).toBe('AP2');
    expect(copy.label).toBe('AP round 2');
    expect(duplicateOf(type, ['AP', 'AP2']).key).toBe('AP3');
    expect(type.key).toBe('AP');
  });

  it('keeps typed numbers over a new suggestion and replaces the suggested ones', () => {
    const suggested = {
      projectile: {
        damage: { value: 10, source: 'suggested' },
        speed: { value: 140, source: 'suggested' },
        parent: 'BaseBullet',
      },
    };
    const current = {
      projectile: {
        damage: { value: 12, source: 'typed' },
        speed: { value: 100, source: 'suggested' },
        parent: '',
      },
    };
    expect(overlay(suggested, current)).toEqual({
      projectile: {
        damage: { value: 12, source: 'typed' },
        speed: { value: 140, source: 'suggested' },
        parent: 'BaseBullet',
      },
    });
  });

  it('removes a member for an empty text or list', () => {
    expect(cleaned({ label: 'x' }, '/label', '')).toEqual({});
    expect(cleaned({}, '/item/tradeTags', [])).toEqual({ item: {} });
    expect(cleaned({}, '/item/tradeTags', ['a'])).toEqual({ item: { tradeTags: ['a'] } });
  });
});
