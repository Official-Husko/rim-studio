import type { ConvertVanillaDto, ItemKindDto, ProjectWeaponDto } from 'rimstudio-ipc-types';
import { formatNumber } from '~/shared/format';
import { t, type MessageKey } from '~/shared/i18n';

/** One fact of a weapon of the project, ready to show. */
export interface WeaponFact {
  /** Stable id, for keys and tests. */
  id: string;
  label: string;
  value: string;
  /** Machine values (names, paths, numbers) use the mono face. */
  mono: boolean;
}

type NumberKey = keyof ConvertVanillaDto;

const NUMBER_LABELS: Record<NumberKey, MessageKey> = {
  damage: 'designer.field.damage',
  range: 'designer.field.range',
  cooldown: 'designer.field.cooldown',
  warmup: 'designer.field.warmup',
  burst: 'designer.field.burst',
  mass: 'designer.field.mass',
  marketValue: 'designer.field.market',
};

/** The numbers that ride along as chips in the row of a weapon, in reading order. */
const CHIP_NUMBERS: Record<ItemKindDto, readonly NumberKey[]> = {
  ranged: ['damage', 'range', 'cooldown'],
  melee: ['damage', 'cooldown'],
};

/** Every number the definition can give, in the order of the details. */
const DETAIL_NUMBERS: readonly NumberKey[] = [
  'damage',
  'range',
  'cooldown',
  'warmup',
  'burst',
  'mass',
  'marketValue',
];

const TIERS: Record<string, MessageKey> = {
  neolithic: 'designer.tier.neolithic',
  medieval: 'designer.tier.medieval',
  industrial: 'designer.tier.industrial',
  spacer: 'designer.tier.spacer',
  ultra: 'designer.tier.ultra',
  archotech: 'designer.tier.archotech',
};

/** The name of a weapon in lists: its label, or its definition name when it has none. */
export function weaponName(weapon: Pick<ProjectWeaponDto, 'label' | 'defName'>): string {
  return weapon.label === '' ? weapon.defName : weapon.label;
}

/** The tier as the definition writes it, in words; a value the UI does not know is kept as it is. */
export function techLevelText(value: string): string {
  const key = TIERS[value.trim().toLowerCase()];
  return key ? t(key) : value;
}

function numberFact(weapon: ProjectWeaponDto, key: NumberKey): WeaponFact | undefined {
  const value = weapon.numbers[key];
  if (value === undefined) return undefined;
  return { id: key, label: t(NUMBER_LABELS[key]), value: formatNumber(value, 2), mono: true };
}

function present(fact: WeaponFact | undefined): fact is WeaponFact {
  return fact !== undefined;
}

/** The few numbers the row of a weapon shows as chips; a number the definition lacks is left out. */
export function keyNumbers(weapon: ProjectWeaponDto): WeaponFact[] {
  return CHIP_NUMBERS[weapon.kind].map((key) => numberFact(weapon, key)).filter(present);
}

function textFact(id: string, label: string, value: string | undefined): WeaponFact | undefined {
  return value === undefined || value === '' ? undefined : { id, label, value, mono: true };
}

function listFact(
  id: string,
  label: string,
  values: readonly string[] | undefined,
): WeaponFact | undefined {
  return values === undefined || values.length === 0
    ? undefined
    : { id, label, value: values.join(', '), mono: true };
}

/** Everything the details of a weapon list. What the definition does not give is left out. */
export function detailFacts(weapon: ProjectWeaponDto): WeaponFact[] {
  const tier =
    weapon.techLevel === undefined || weapon.techLevel === ''
      ? undefined
      : {
          id: 'techLevel',
          label: t('designer.field.tier'),
          value: techLevelText(weapon.techLevel),
          mono: false,
        };
  return [
    textFact('defName', t('designer.field.defName'), weapon.defName),
    textFact('file', t('designer.projectWeapons.file'), weapon.file),
    textFact('parent', t('designer.field.parent'), weapon.parent),
    tier,
    textFact('projectile', t('designer.field.projectileDef'), weapon.projectile),
    listFact('tags', t('designer.field.weaponTags'), weapon.tags),
    listFact('weaponClasses', t('designer.field.weaponClasses'), weapon.weaponClasses),
    ...DETAIL_NUMBERS.map((key) => numberFact(weapon, key)),
  ].filter(present);
}
