import type { ReadoutUnitDto } from 'rimstudio-ipc-types';
import { formatNumber } from '~/shared/format';
import { t, type MessageKey } from '~/shared/i18n';

/** The words of the readouts the backend names by key. */
const READOUT_LABELS: Record<string, MessageKey> = {
  'cycle-time': 'designer.readout.cycleTime',
  dps: 'designer.readout.dps',
  'hit-dps-12': 'designer.readout.hitDps12',
  'hit-dps-25': 'designer.readout.hitDps25',
  'implied-ap': 'designer.readout.impliedAp',
  'strength-index': 'designer.readout.strength',
  'melee-swing-damage': 'designer.readout.swingDamage',
  'melee-swing-cooldown': 'designer.readout.swingCooldown',
  'melee-dps': 'designer.readout.meleeDps',
  'melee-ap': 'designer.readout.meleeAp',
  'melee-fight-dps': 'designer.readout.fightDps',
  'market-value': 'designer.readout.price',
  'ce-ammo-damage': 'designer.readout.ceAmmoDamage',
  'ce-bulk': 'designer.readout.ceBulk',
  'ce-cooldown': 'designer.readout.ceCooldown',
  'ce-magazine': 'designer.readout.ceMagazine',
  'ce-reload': 'designer.readout.ceReload',
  'ce-mass': 'designer.readout.ceMass',
  'ce-range': 'designer.readout.ceRange',
  'ce-sustained-dps': 'designer.readout.ceSustainedDps',
  'ce-warmup': 'designer.readout.ceWarmup',
};

/** The label of a readout; a key the UI does not know is shown as it is. */
export function readoutLabel(key: string): string {
  const message = READOUT_LABELS[key];
  return message ? t(message) : key;
}

const UNIT_TEXT: Record<ReadoutUnitDto, string> = {
  number: '',
  'damage-per-second': 'dmg/s',
  seconds: 's',
  tiles: 'tiles',
  silver: 'silver',
  kilograms: 'kg',
  fraction: '',
};

/** The unit symbol of a readout. */
export function unitText(unit: ReadoutUnitDto): string {
  return UNIT_TEXT[unit];
}

/** A readout value for display; undefined (not computable yet) shows as a dash. */
export function readoutValue(value: number | undefined): string {
  return value === undefined ? '-' : formatNumber(value, 3);
}

/** A signed delta such as +1.25 or -0.5. */
export function signed(value: number | undefined): string {
  if (value === undefined) return '-';
  const text = formatNumber(Math.abs(value), 3);
  if (value > 0) return `+${text}`;
  if (value < 0) return `-${text}`;
  return text;
}

const STAT_LABELS: Record<string, MessageKey> = {
  ap: 'designer.stat.ap',
  burst: 'designer.stat.burst',
  cooldown: 'designer.stat.cooldown',
  damage: 'designer.stat.damage',
  dps: 'designer.stat.dps',
  fight_dps: 'designer.stat.fightDps',
  long: 'designer.stat.long',
  market_value: 'designer.stat.price',
  mass: 'designer.stat.mass',
  medium: 'designer.stat.medium',
  range: 'designer.stat.range',
  short: 'designer.stat.short',
  swing_damage: 'designer.stat.swingDamage',
  ticks_between: 'designer.stat.ticks',
  touch: 'designer.stat.touch',
  warmup: 'designer.stat.warmup',
  work: 'designer.stat.work',
  speed: 'designer.stat.speed',
  tools: 'designer.stat.tools',
};

/** The label of a pool or fit stat; an unknown stat is shown with spaces for underscores. */
export function statLabel(stat: string): string {
  const message = STAT_LABELS[stat];
  return message ? t(message) : stat.replace(/_/g, ' ');
}
