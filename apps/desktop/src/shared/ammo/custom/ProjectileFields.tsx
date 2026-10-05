import type {
  CustomFragmentDto,
  CustomProjectileDto,
  CustomSecondaryDamageDto,
} from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';
import { Group } from '~/shared/ce/Group';
import { RawNodeListField } from '~/shared/ce/RawNodeListField';
import { TriState } from '~/shared/ce/TriState';
import { AmmoNumberField, AmmoTextField } from './fields';
import { diagnosticsUnder, typedNumber, typePointer } from './model';
import { PairListField, type PairRow } from './PairListField';
import type { CustomAmmoStore } from './store';

export interface ProjectileFieldsProps {
  store: CustomAmmoStore;
  index: number;
}

/** Every number and choice of the projectile of one ammo type. */
export function ProjectileFields({ store, index }: ProjectileFieldsProps) {
  const projectile: CustomProjectileDto = store.custom.value.types[index]?.projectile ?? {};
  const field = { store, index } as const;
  const flag = (pointer: string, label: string) => (
    <TriState
      label={label}
      value={store.valueAt(index, pointer) as boolean | undefined}
      onChange={(v) => store.setField(index, pointer, v)}
    />
  );
  const secondary: PairRow[] = (projectile.secondaryDamage ?? []).map((s) => ({
    name: s.def,
    amount: s.amount?.value,
  }));
  const fragments: PairRow[] = (projectile.fragments ?? []).map((f) => ({
    name: f.def,
    amount: f.count,
  }));
  const diag = (sub: string) => diagnosticsUnder(store.diagnostics.value, typePointer(index, sub));
  return (
    <div class="flex flex-col gap-3">
      <Group title={t('ammo.projectile.damage')} open summary={t('ammo.projectile.damage-help')}>
        <div class="grid grid-cols-[repeat(auto-fit,minmax(14rem,1fr))] gap-x-4 gap-y-3">
          <AmmoNumberField
            {...field}
            pointer="/projectile/damage"
            label={t('ammo.field.damage')}
            unit={t('ammo.unit.damage')}
            step={0.5}
          />
          <AmmoNumberField
            {...field}
            pointer="/projectile/armorPenetrationSharp"
            label={t('ammo.field.sharp')}
            unit={t('ammo.unit.sharp')}
            step={0.1}
          />
          <AmmoNumberField
            {...field}
            pointer="/projectile/armorPenetrationBlunt"
            label={t('ammo.field.blunt')}
            unit={t('ammo.unit.blunt')}
            step={0.5}
          />
          <AmmoNumberField
            {...field}
            pointer="/projectile/speed"
            label={t('ammo.field.speed')}
            unit={t('ammo.unit.speed')}
            step={1}
          />
          <AmmoNumberField
            {...field}
            pointer="/projectile/pelletCount"
            label={t('ammo.field.pellets')}
            whole
            step={1}
          />
          <AmmoNumberField
            {...field}
            pointer="/projectile/spreadMult"
            label={t('ammo.field.spread')}
            step={0.1}
          />
          <AmmoTextField
            {...field}
            pointer="/projectile/damageDef"
            label={t('ammo.field.damage-def')}
            help={t('ammo.field.damage-def-help')}
          />
        </div>
        <PairListField
          label={t('ammo.field.secondary')}
          nameLabel={t('ammo.field.secondary-def')}
          amountLabel={t('ammo.field.secondary-amount')}
          addLabel={t('ammo.field.secondary-add')}
          rows={secondary}
          pointer={typePointer(index, '/projectile/secondaryDamage')}
          diagnostics={diag('/projectile/secondaryDamage')}
          onChange={(rows) => {
            const list: CustomSecondaryDamageDto[] = rows.map((r) => ({
              def: r.name,
              ...(r.amount !== undefined ? { amount: typedNumber(r.amount) } : {}),
            }));
            store.setField(index, '/projectile/secondaryDamage', list);
          }}
        />
      </Group>
      <Group title={t('ammo.projectile.burst')} summary={t('ammo.projectile.burst-help')}>
        <div class="grid grid-cols-[repeat(auto-fit,minmax(14rem,1fr))] gap-x-4 gap-y-3">
          <AmmoNumberField
            {...field}
            pointer="/projectile/explosionRadius"
            label={t('ammo.field.radius')}
            unit={t('ammo.unit.cells')}
            step={0.1}
          />
          <AmmoNumberField
            {...field}
            pointer="/projectile/suppressionFactor"
            label={t('ammo.field.suppression')}
            step={0.1}
          />
          <AmmoNumberField
            {...field}
            pointer="/projectile/dangerFactor"
            label={t('ammo.field.danger')}
            step={0.1}
          />
        </div>
        {flag('/projectile/explosionNeighbors', t('ammo.field.neighbors'))}
        <PairListField
          label={t('ammo.field.fragments')}
          nameLabel={t('ammo.field.fragment-def')}
          amountLabel={t('ammo.field.fragment-count')}
          addLabel={t('ammo.field.fragment-add')}
          rows={fragments}
          whole
          pointer={typePointer(index, '/projectile/fragments')}
          diagnostics={diag('/projectile/fragments')}
          onChange={(rows) => {
            const list: CustomFragmentDto[] = rows.map((r) => ({
              def: r.name,
              count: r.amount ?? 1,
            }));
            store.setField(index, '/projectile/fragments', list);
          }}
        />
      </Group>
      <Group title={t('ammo.projectile.behaviour')} summary={t('ammo.projectile.behaviour-help')}>
        <div class="grid grid-cols-[repeat(auto-fit,minmax(14rem,1fr))] gap-x-4 gap-y-3">
          <AmmoTextField
            {...field}
            pointer="/projectile/label"
            label={t('ammo.field.projectile-label')}
            help={t('ammo.field.projectile-label-help')}
          />
          <AmmoTextField
            {...field}
            pointer="/projectile/parent"
            label={t('ammo.field.parent')}
            help={t('ammo.field.parent-help')}
          />
          <AmmoTextField
            {...field}
            pointer="/projectile/thingClass"
            label={t('ammo.field.thing-class')}
          />
          <AmmoTextField
            {...field}
            pointer="/projectile/casingMote"
            label={t('ammo.field.casing-mote')}
          />
          <AmmoTextField
            {...field}
            pointer="/projectile/casingFilth"
            label={t('ammo.field.casing-filth')}
          />
        </div>
        <div class="flex flex-wrap gap-4">
          {flag('/projectile/dropsCasings', t('ammo.field.drops-casings'))}
          {flag('/projectile/incendiary', t('ammo.field.incendiary'))}
          {flag('/projectile/flyOverhead', t('ammo.field.fly-overhead'))}
        </div>
      </Group>
      <Group title={t('ammo.raw.title')} summary={t('ammo.raw.summary')}>
        <RawNodeListField
          label={t('ammo.raw.projectile-props')}
          help={t('ammo.raw.help')}
          nodes={projectile.extra ?? []}
          onChange={(nodes) => store.setField(index, '/projectile/extra', nodes)}
        />
        <RawNodeListField
          label={t('ammo.raw.projectile-thing')}
          nodes={projectile.thingExtra ?? []}
          onChange={(nodes) => store.setField(index, '/projectile/thingExtra', nodes)}
        />
      </Group>
    </div>
  );
}
