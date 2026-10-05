import type { CeUnderBarrelDto } from 'rimstudio-ipc-types';
import { Combobox, FormField, Switch, TextField, type ComboboxOption } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import type { CeBlock, CeBlockPatch } from './blockModel';
import { Group } from './Group';
import { PlainNumberField } from './PlainNumberField';
import { RawNodeListField } from './RawNodeListField';
import { SourcedNumberField } from './SourcedNumberField';
import { UnderBarrelFireModes } from './UnderBarrelFireModes';

export interface CeUnderBarrelFieldsProps {
  block: CeBlock;
  onChange: (patch: CeBlockPatch) => void;
  /** Ammo sets to pick from; any other name can still be typed in the box. */
  ammoSets?: ComboboxOption[];
}

type Unit = CeUnderBarrelDto;

/** A unit with nothing set: the two flags are always present in the block. */
const EMPTY_UNIT: Unit = { oneAmmoHolder: false, requiresReload: false };

function Text({
  label,
  help,
  value,
  onChange,
}: {
  label: string;
  help?: string;
  value: string | undefined;
  onChange: (v: string | undefined) => void;
}) {
  return (
    <FormField label={label} {...(help ? { help } : {})}>
      <TextField
        aria-label={label}
        value={value ?? ''}
        onValueChange={(v) => onChange(v === '' ? undefined : v)}
      />
    </FormField>
  );
}

/** The under barrel unit of a weapon: its ammo, numbers, fire modes and the other elements it carries. */
export function CeUnderBarrelFields({ block, onChange, ammoSets }: CeUnderBarrelFieldsProps) {
  const unit = block.underBarrel;
  const set = (patch: Partial<Unit>): void => {
    const next = { ...(unit ?? {}), ...patch } as Record<string, unknown>;
    for (const [key, value] of Object.entries(next)) {
      if (value === undefined || (Array.isArray(value) && value.length === 0)) delete next[key];
    }
    onChange({ underBarrel: next as Unit });
  };
  return (
    <Group
      title={t('ceblock.ub.title')}
      summary={unit ? (unit.ammoSet ?? t('ceblock.ub.noAmmo')) : t('ceblock.off')}
    >
      <p class="m-0 text-small text-muted">{t('ceblock.ub.help')}</p>
      <Switch
        checked={unit !== undefined}
        onCheckedChange={(on) => onChange({ underBarrel: on ? EMPTY_UNIT : undefined })}
      >
        {t('ceblock.ub.flag')}
      </Switch>
      {unit ? (
        <div class="flex flex-col gap-3">
          <div class="grid grid-cols-2 gap-3">
            <Text
              label={t('ceblock.ub.standardLabel')}
              value={unit.standardLabel}
              onChange={(standardLabel) => set({ standardLabel })}
            />
            <Text
              label={t('ceblock.ub.underLabel')}
              value={unit.underBarrelLabel}
              onChange={(underBarrelLabel) => set({ underBarrelLabel })}
            />
          </div>
          <div class="flex flex-wrap gap-4">
            <Switch
              checked={unit.oneAmmoHolder}
              onCheckedChange={(oneAmmoHolder) => set({ oneAmmoHolder })}
            >
              {t('ceblock.ub.oneHolder')}
            </Switch>
            <Switch
              checked={unit.requiresReload}
              onCheckedChange={(requiresReload) => set({ requiresReload })}
            >
              {t('ceblock.ub.reloads')}
            </Switch>
          </div>
          <FormField label={t('ceblock.ub.ammoSet')} help={t('ceblock.ub.ammoSetHelp')}>
            {ammoSets && ammoSets.length > 0 ? (
              <Combobox
                aria-label={t('ceblock.ub.ammoSet')}
                options={ammoSets}
                value={unit.ammoSet}
                onValueChange={(ammoSet) => set({ ammoSet })}
                placeholder={t('ceblock.ub.choose')}
                emptyText={t('ceblock.ub.noMatch')}
              />
            ) : (
              <TextField
                aria-label={t('ceblock.ub.ammoSet')}
                value={unit.ammoSet ?? ''}
                onValueChange={(v) => set({ ammoSet: v === '' ? undefined : v })}
              />
            )}
          </FormField>
          <Text
            label={t('ceblock.ub.projectile')}
            help={t('ceblock.ub.projectileHelp')}
            value={unit.defaultProjectile}
            onChange={(defaultProjectile) => set({ defaultProjectile })}
          />
          <div class="grid grid-cols-2 gap-3 sm:grid-cols-3">
            <SourcedNumberField
              label={t('ceblock.ub.magazine')}
              whole
              value={unit.magazineSize}
              onChange={(magazineSize) => set({ magazineSize })}
            />
            <SourcedNumberField
              label={t('ceblock.ub.reloadTime')}
              unit="s"
              value={unit.reloadTime}
              onChange={(reloadTime) => set({ reloadTime })}
            />
            <SourcedNumberField
              label={t('ceblock.ub.range')}
              unit="tiles"
              value={unit.range}
              onChange={(range) => set({ range })}
            />
            <SourcedNumberField
              label={t('ceblock.ub.warmup')}
              unit="s"
              value={unit.warmupTime}
              onChange={(warmupTime) => set({ warmupTime })}
            />
            <SourcedNumberField
              label={t('ceblock.ub.recoil')}
              value={unit.recoilAmount}
              onChange={(recoilAmount) => set({ recoilAmount })}
            />
            <PlainNumberField
              label={t('ceblock.ub.minRange')}
              value={unit.minRange}
              onChange={(minRange) => set({ minRange })}
            />
            <PlainNumberField
              label={t('ceblock.ub.burst')}
              whole
              value={unit.burstShotCount}
              onChange={(burstShotCount) => set({ burstShotCount })}
            />
            <PlainNumberField
              label={t('ceblock.ub.ticks')}
              whole
              value={unit.ticksBetweenBurstShots}
              onChange={(ticksBetweenBurstShots) => set({ ticksBetweenBurstShots })}
            />
            <PlainNumberField
              label={t('ceblock.ub.perShot')}
              whole
              value={unit.ammoConsumedPerShot}
              onChange={(ammoConsumedPerShot) => set({ ammoConsumedPerShot })}
            />
            <PlainNumberField
              label={t('ceblock.ub.friendly')}
              value={unit.avoidFriendlyFireRadius}
              onChange={(avoidFriendlyFireRadius) => set({ avoidFriendlyFireRadius })}
            />
            <PlainNumberField
              label={t('ceblock.ub.flash')}
              value={unit.muzzleFlashScale}
              onChange={(muzzleFlashScale) => set({ muzzleFlashScale })}
            />
          </div>
          <div class="grid grid-cols-2 gap-3">
            <Text
              label={t('ceblock.ub.sound')}
              value={unit.soundCast}
              onChange={(soundCast) => set({ soundCast })}
            />
            <Text
              label={t('ceblock.ub.replaces')}
              help={t('ceblock.ub.replacesHelp')}
              value={unit.replacesComp}
              onChange={(replacesComp) => set({ replacesComp })}
            />
          </div>
          <UnderBarrelFireModes
            modes={unit.fireModes ?? { noSingleShot: false }}
            onChange={(fireModes) => set({ fireModes })}
          />
          <RawNodeListField
            label={t('ceblock.ub.verbExtra')}
            nodes={unit.verbExtra ?? []}
            onChange={(verbExtra) => set({ verbExtra })}
          />
          <RawNodeListField
            label={t('ceblock.ub.propsExtra')}
            nodes={unit.propsExtra ?? []}
            onChange={(propsExtra) => set({ propsExtra })}
          />
          <RawNodeListField
            label={t('ceblock.ub.extra')}
            nodes={unit.extra ?? []}
            onChange={(extra) => set({ extra })}
          />
        </div>
      ) : null}
    </Group>
  );
}
