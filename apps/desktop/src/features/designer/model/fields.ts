import type { MessageKey } from '~/shared/i18n';
import type { Pointer } from './pointer';

/** A numeric field of the form: where it lives, its label, its unit and its natural step. */
export interface NumberFieldDef {
  pointer: Pointer;
  label: MessageKey;
  unit?: string;
  step: number;
  /** The stored value is a bare number, not a value with a source (cost list counts). */
  plain?: boolean;
}

/** The numbers of the shooting verb and the accuracy profile of a ranged weapon. */
export const RANGED_FIELDS: readonly NumberFieldDef[] = [
  { pointer: '/ranged/damage', label: 'designer.field.damage', unit: 'damage', step: 1 },
  { pointer: '/ranged/armorPenetration', label: 'designer.field.ap', unit: 'fraction', step: 0.01 },
  { pointer: '/ranged/burstCount', label: 'designer.field.burst', unit: 'shots', step: 1 },
  {
    pointer: '/ranged/ticksBetweenBurstShots',
    label: 'designer.field.ticks',
    unit: 'ticks',
    step: 1,
  },
  { pointer: '/ranged/warmup', label: 'designer.field.warmup', unit: 's', step: 0.05 },
  { pointer: '/ranged/cooldown', label: 'designer.field.cooldown', unit: 's', step: 0.05 },
  { pointer: '/ranged/range', label: 'designer.field.range', unit: 'tiles', step: 0.1 },
];

/** The accuracy profile of a ranged weapon, one number per distance band. */
export const ACCURACY_FIELDS: readonly NumberFieldDef[] = [
  { pointer: '/ranged/accuracy/touch', label: 'designer.field.touch', step: 0.01 },
  { pointer: '/ranged/accuracy/short', label: 'designer.field.short', step: 0.01 },
  { pointer: '/ranged/accuracy/medium', label: 'designer.field.medium', step: 0.01 },
  { pointer: '/ranged/accuracy/long', label: 'designer.field.long', step: 0.01 },
];

/** Numbers shared by every weapon: mass, work to make and the optional market value. */
export const COMMON_FIELDS: readonly NumberFieldDef[] = [
  { pointer: '/mass', label: 'designer.field.mass', unit: 'kg', step: 0.05 },
  { pointer: '/workToMake', label: 'designer.field.work', unit: 'work', step: 100 },
  { pointer: '/marketValue', label: 'designer.field.market', unit: 'silver', step: 1 },
];

interface ToolFieldDef extends Omit<NumberFieldDef, 'pointer'> {
  /** The key inside the tool entry. */
  suffix: string;
}

/** The numbers of one melee tool (and of the gun bash tools). */
export const TOOL_FIELDS: readonly ToolFieldDef[] = [
  { suffix: 'power', label: 'designer.field.power', unit: 'damage', step: 1 },
  { suffix: 'cooldownTime', label: 'designer.field.toolCooldown', unit: 's', step: 0.05 },
  { suffix: 'armorPenetration', label: 'designer.field.ap', unit: 'fraction', step: 0.01 },
  { suffix: 'chanceFactor', label: 'designer.field.chance', unit: 'weight', step: 0.1 },
];

/** The field definitions of tool number `index`. */
export function toolFields(index: number): NumberFieldDef[] {
  return TOOL_FIELDS.map(({ suffix, ...rest }) => ({
    ...rest,
    pointer: `/tools/${index}/${suffix}`,
  }));
}
