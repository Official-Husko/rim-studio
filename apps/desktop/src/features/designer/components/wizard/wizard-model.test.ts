import { describe, expect, it } from 'vitest';
import { catalogFixture } from './wizardTestSupport';
import {
  applies,
  defaultDescriptors,
  describeResolved,
  familiesOf,
  findArchetype,
  groupOfField,
  mergeDescriptors,
  modeOf,
  emptyChoice,
  rpmOf,
  rpmRange,
  secondsBetweenShots,
  suggestDefName,
  tierCount,
} from './wizard-model';

describe('wizard model', () => {
  const catalog = catalogFixture();

  it('finds an archetype in its family and lists the families of a kind', () => {
    expect(findArchetype(catalog, 'rifle/sniper')?.label).toBe('Sniper rifle');
    expect(findArchetype(catalog, 'nothing/here')).toBeUndefined();
    expect(familiesOf(catalog, 'melee').map((f) => f.id)).toContain('sword');
    expect(familiesOf(catalog, 'ranged').map((f) => f.id)).toContain('rifle');
  });

  it('starts from the defaults of the archetype and knows which descriptors apply', () => {
    const sniper = findArchetype(catalog, 'rifle/sniper');
    const knife = findArchetype(catalog, 'knife/knife');
    expect(sniper && defaultDescriptors(sniper)).toMatchObject({
      action: 'bolt',
      calibre: 'huge',
      handling: 'standard',
      rof: { class: 'medium' },
    });
    expect(knife && defaultDescriptors(knife)).toEqual({
      rof: { class: 'medium' },
      handling: 'standard',
    });
    expect(applies(sniper, 'calibre')).toBe(true);
    expect(applies(knife, 'calibre')).toBe(false);
    expect(applies(undefined, 'rof')).toBe(false);
  });

  it('turns a rate of fire into rounds per minute and seconds between shots', () => {
    const rifle = findArchetype(catalog, 'rifle/assault');
    expect(rifle).toBeDefined();
    if (!rifle) return;
    expect(rpmOf(rifle, catalog, { class: 'medium' })).toBe(650);
    expect(rpmOf(rifle, catalog, { class: 'fast' })).toBeCloseTo(877.5);
    expect(rpmOf(rifle, catalog, { rpm: 700 })).toBe(700);
    expect(secondsBetweenShots(600)).toBeCloseTo(0.1);
    const range = rpmRange(rifle, catalog);
    expect(range && range.min < 650 && range.max > 877).toBe(true);
    const knife = findArchetype(catalog, 'knife/knife');
    expect(knife && rpmOf(knife, catalog, { class: 'fast' })).toBeUndefined();
  });

  it('groups the fields of a proposal', () => {
    expect(groupOfField('/ranged/damage')).toBe('damage');
    expect(groupOfField('/ranged/accuracy/long')).toBe('range');
    expect(groupOfField('/ranged/warmup')).toBe('firing');
    expect(groupOfField('/tools/1/power')).toBe('attacks');
    expect(groupOfField('/mass')).toBe('weight');
  });

  it('suggests a def name from the prefix and the label', () => {
    expect(suggestDefName('RS_', 'my long rifle')).toBe('RS_MyLongRifle');
    expect(suggestDefName('', 'Eagle Mk 2')).toBe('EagleMk2');
    expect(suggestDefName('RS_', '  ')).toBe('');
  });

  it('merges descriptor patches and drops the cleared ones', () => {
    expect(
      mergeDescriptors({ tier: 'spacer', handling: 'heavy' }, { tier: undefined, action: 'bolt' }),
    ).toEqual({
      handling: 'heavy',
      action: 'bolt',
    });
  });

  it('uses Combat Extended mode only with an ammo set', () => {
    expect(modeOf({ ...emptyChoice(), ceCalibre: true })).toBe('vanilla');
    expect(
      modeOf({ ...emptyChoice(), ceCalibre: true, descriptors: { ammoSet: 'AmmoSet_X' } }),
    ).toBe('combat-extended');
  });

  it('counts the reference weapons of a tier and names the resolved choices', () => {
    expect(tierCount(catalog, 'industrial', 'ranged')).toBe(12);
    expect(tierCount(catalog, 'ultra', 'ranged')).toBe(0);
    const sniper = findArchetype(catalog, 'rifle/sniper');
    if (!sniper) throw new Error('missing archetype');
    const rows = describeResolved(catalog, sniper, {
      action: 'bolt',
      rof: 'medium',
      calibre: 'large',
      handling: 'standard',
      tier: 'industrial',
    });
    expect(rows.map((r) => r.text)).toEqual([
      'Bolt action',
      'Medium',
      'Large',
      'Standard',
      'Industrial',
    ]);
  });
});
