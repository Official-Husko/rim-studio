import { describe, expect, it } from 'vitest';
import type { ProjectWeaponDto } from 'rimstudio-ipc-types';
import { projectWeaponsAnswer } from '../testSupport';
import { detailFacts, keyNumbers, techLevelText, weaponName } from './project-weapons';

function weapon(defName: string): ProjectWeaponDto {
  const found = projectWeaponsAnswer().weapons.find((w) => w.defName === defName);
  if (!found) throw new Error(`no ${defName} in the fixture`);
  return found;
}

describe('weaponName', () => {
  it('prefers the label and falls back to the definition name', () => {
    expect(weaponName(weapon('RS_Gun_Ranger'))).toBe('ranger rifle');
    expect(weaponName({ ...weapon('RS_Gun_Ranger'), label: '' })).toBe('RS_Gun_Ranger');
  });
});

describe('keyNumbers', () => {
  it('shows damage, range and cooldown of a gun', () => {
    expect(keyNumbers(weapon('RS_Gun_Ranger')).map((f) => [f.label, f.value])).toEqual([
      ['Damage', '16'],
      ['Range', '32.9'],
      ['Cooldown', '1.6'],
    ]);
  });

  it('shows damage and cooldown of a melee weapon, without a range', () => {
    expect(keyNumbers(weapon('RS_Melee_Cleaver')).map((f) => f.id)).toEqual(['damage', 'cooldown']);
  });

  it('leaves out a number the definition does not give', () => {
    const bare: ProjectWeaponDto = { ...weapon('RS_Gun_Ranger'), numbers: { damage: 12 } };
    expect(keyNumbers(bare).map((f) => f.id)).toEqual(['damage']);
  });

  it('rounds to two decimals', () => {
    const odd: ProjectWeaponDto = { ...weapon('RS_Gun_Ranger'), numbers: { damage: 12.3456 } };
    expect(keyNumbers(odd)[0]?.value).toBe('12.35');
  });
});

describe('detailFacts', () => {
  it('lists everything a gun has, names and files in mono', () => {
    const facts = detailFacts(weapon('RS_Gun_Ranger'));
    expect(facts.map((f) => f.id)).toEqual([
      'defName',
      'file',
      'parent',
      'techLevel',
      'projectile',
      'tags',
      'weaponClasses',
      'damage',
      'range',
      'cooldown',
      'warmup',
      'burst',
      'mass',
      'marketValue',
    ]);
    const byId = new Map(facts.map((f) => [f.id, f]));
    expect(byId.get('file')).toMatchObject({
      label: 'File',
      value: 'Defs/ThingDefs_Weapons/RS_Weapons.xml',
      mono: true,
    });
    expect(byId.get('techLevel')).toMatchObject({
      label: 'Tier',
      value: 'Industrial',
      mono: false,
    });
    expect(byId.get('weaponClasses')?.value).toBe('Ranged, LongShots');
    expect(byId.get('marketValue')).toMatchObject({ label: 'Market value', value: '410' });
  });

  it('leaves out what the definition does not give', () => {
    const ids = detailFacts(weapon('RS_Melee_Cleaver')).map((f) => f.id);
    expect(ids).not.toContain('projectile');
    expect(ids).not.toContain('tags');
    expect(ids).not.toContain('range');
    expect(ids).not.toContain('warmup');
    expect(ids).not.toContain('burst');
    expect(ids).toContain('weaponClasses');
  });
});

describe('techLevelText', () => {
  it('turns the written tier into the word the editor uses', () => {
    expect(techLevelText('Spacer')).toBe('Spacer');
    expect(techLevelText('neolithic')).toBe('Neolithic');
    expect(techLevelText(' ARCHOTECH ')).toBe('Archotech');
  });

  it('keeps a tier it does not know as it is', () => {
    expect(techLevelText('Animal')).toBe('Animal');
  });
});
