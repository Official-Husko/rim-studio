import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { CustomAmmoDto, WritePlanDto } from 'rimstudio-ipc-types';
import { createCustomAmmoStore } from './store';
import { customAmmo, customPlan, settle, suggestionFmj } from '../testSupport';

const suggest = vi.fn();

function planWith(diagnostics: WritePlanDto['diagnostics'], files = customPlan().files) {
  return async (): Promise<WritePlanDto> => ({
    planId: 'p',
    files,
    diagnostics,
    hasErrors: diagnostics.some((d) => d.severity === 'error'),
  });
}

describe('custom ammo store', () => {
  beforeEach(() => {
    suggest.mockReset();
    suggest.mockResolvedValue(suggestionFmj());
  });

  it('starts from a copy of the ammunition it is given', () => {
    const initial = customAmmo();
    const store = createCustomAmmoStore({ initial });
    store.patch({ name: 'Other' });
    expect(initial.name).not.toBe('Other');
    expect(store.custom.value.types).toHaveLength(initial.types.length);
  });

  it('removes a member of the caliber for an empty text', () => {
    const store = createCustomAmmoStore({ initial: customAmmo() });
    store.patch({ similarTo: 'AmmoSet_Rifle', setLabel: 'Six' });
    store.patch({ setLabel: '' });
    expect(store.custom.value.similarTo).toBe('AmmoSet_Rifle');
    expect('setLabel' in store.custom.value).toBe(false);
  });

  it('adds a type with a class and fills it from the suggestion, keeping typed numbers', async () => {
    const store = createCustomAmmoStore({ suggest });
    store.patch({ name: 'Six', caliber: '6mm' });
    store.addType('FullMetalJacket');
    await settle();
    const type = store.custom.value.types[0];
    expect(suggest).toHaveBeenCalledWith(
      expect.objectContaining({ class: 'FullMetalJacket', hints: { caliber: '6mm' } }),
    );
    expect(type?.projectile.armorPenetrationSharp?.source).toBe('suggested');
    expect(store.suggestionFor(0, '/projectile/armorPenetrationSharp')?.rating).toBe('unreliable');
    store.setField(0, '/projectile/armorPenetrationSharp', { value: 9, source: 'typed' });
    await store.suggestFor(0);
    expect(store.custom.value.types[0]?.projectile.armorPenetrationSharp).toEqual({
      value: 9,
      source: 'typed',
    });
    expect(suggest).toHaveBeenLastCalledWith(
      expect.objectContaining({ hints: expect.objectContaining({ damage: 10 }) }),
    );
  });

  it('uses a suggestion on request as a suggested value', async () => {
    const store = createCustomAmmoStore({ suggest });
    store.addType('FullMetalJacket');
    await settle();
    store.setField(0, '/item/mass', { value: 1, source: 'typed' });
    store.applySuggestion(0, '/item/mass');
    expect(store.custom.value.types[0]?.item.mass).toEqual({ value: 0.024, source: 'suggested' });
  });

  it('copies a real ammo type and remembers where it came from', async () => {
    const store = createCustomAmmoStore({ suggest });
    store.addType();
    store.setClass(0, 'FullMetalJacket');
    await settle();
    suggest.mockResolvedValue({ ...suggestionFmj(), copiedFrom: 'Ammo_303British_FMJ' });
    await store.suggestFor(0, { copyFrom: 'Ammo_303British_FMJ' });
    expect(suggest).toHaveBeenLastCalledWith(
      expect.objectContaining({ copyFrom: 'Ammo_303British_FMJ' }),
    );
    expect(store.custom.value.types[0]?.copiedFrom).toBe('Ammo_303British_FMJ');
  });

  it('shows a type with no suggestion when there is nothing to suggest from', async () => {
    suggest.mockResolvedValue({ ...suggestionFmj(), available: false, reason: 'none', fields: [] });
    const store = createCustomAmmoStore({ suggest });
    store.addType('Odd');
    await settle();
    expect(store.custom.value.types[0]?.projectile).toEqual({});
    expect(store.suggestions.value[store.ids.value[0] as string]?.available).toBe(false);
  });

  it('keeps the error of a failed suggestion', async () => {
    suggest.mockRejectedValue({ code: 'designer.failed', message: 'no', errorId: 'e-1' });
    const store = createCustomAmmoStore({ suggest });
    store.addType('FullMetalJacket');
    await settle();
    expect(store.suggestError.value?.code).toBe('designer.failed');
    expect(store.suggesting.value).toBeUndefined();
  });

  it('duplicates, moves and removes types and keeps the default type in step', () => {
    const store = createCustomAmmoStore({ initial: { ...customAmmo(), defaultType: 'AP' } });
    const first = store.custom.value.types[0]?.key;
    store.duplicateType(0);
    expect(store.custom.value.types).toHaveLength(3);
    expect(store.custom.value.types[1]?.key).toBe(`${first}2`);
    expect(store.typeIndex.value).toBe(1);
    store.moveType(1, 1);
    expect(store.custom.value.types[2]?.key).toBe(`${first}2`);
    expect(store.typeIndex.value).toBe(2);
    store.moveType(2, 1);
    expect(store.typeIndex.value).toBe(2);
    store.removeType(store.custom.value.types.findIndex((t) => t.key === 'AP'));
    expect(store.custom.value.defaultType).toBeUndefined();
    expect(store.custom.value.types).toHaveLength(2);
  });

  it('plans the ammunition after a pause and keeps the diagnostics that concern it', async () => {
    const plan = vi.fn(
      planWith([
        {
          code: 'ce.ammo-field-required',
          severity: 'error',
          message: 'needs damage',
          field: '/ce/customAmmo/types/1/projectile/damage',
        },
        {
          code: 'design.required-missing',
          severity: 'error',
          message: 'needs bulk',
          field: '/ce/bulk',
        },
      ]),
    );
    const store = createCustomAmmoStore({ initial: customAmmo(), plan, debounceMs: 5 });
    store.setField(0, '/projectile/damage', { value: 12, source: 'typed' });
    store.setField(0, '/projectile/speed', { value: 130, source: 'typed' });
    expect(store.stale.value).toBe(true);
    expect(store.canSave.value).toBe(false);
    await new Promise((r) => setTimeout(r, 30));
    expect(plan).toHaveBeenCalledTimes(1);
    expect(
      (plan.mock.calls[0] as unknown as [CustomAmmoDto])[0].types[0]?.projectile.damage?.value,
    ).toBe(12);
    expect(store.diagnostics.value).toHaveLength(1);
    expect(store.errors.value).toHaveLength(1);
    expect(store.otherErrors.value).toHaveLength(1);
    expect(store.canSave.value).toBe(false);
  });

  it('can save once the plan has no ammo errors, and finds the ammunition file', async () => {
    const store = createCustomAmmoStore({
      initial: customAmmo(),
      plan: planWith([]),
      debounceMs: 0,
    });
    store.check();
    await settle();
    expect(store.canSave.value).toBe(true);
    expect(store.ammoFile.value?.path).toContain('Compat/CombatExtended/Defs/Ammo/');
  });

  it('keeps the error of a plan that failed', async () => {
    const store = createCustomAmmoStore({
      initial: customAmmo(),
      plan: async () => {
        throw { code: 'designer.plan-failed', message: 'no', errorId: 'e-1' };
      },
    });
    store.check();
    await settle();
    expect(store.planError.value?.code).toBe('designer.plan-failed');
    expect(store.stale.value).toBe(false);
  });

  it('goes to the section, the type and the tab of a diagnostic', () => {
    const store = createCustomAmmoStore({ initial: customAmmo() });
    store.goTo('/ce/customAmmo/types/1/recipe/ingredients/0/count');
    expect(store.section.value).toBe('types');
    expect(store.typeIndex.value).toBe(1);
    expect(store.tab.value).toBe('recipe');
    expect(store.focusField.value).toBe('/ce/customAmmo/types/1/recipe/ingredients/0/count');
    store.goTo('/ce/customAmmo/name');
    expect(store.section.value).toBe('identity');
  });

  it('has nothing to check without a plan function', () => {
    const store = createCustomAmmoStore({ initial: customAmmo() });
    store.check();
    expect(store.hasPlan).toBe(false);
    expect(store.canSave.value).toBe(true);
  });
});
