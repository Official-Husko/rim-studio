import type { ArchetypeCatalogDto, ArchetypeDto, RateOfFireDto } from 'rimstudio-ipc-types';
import { RulerSlider, SegmentedControl } from 'rimstudio-ui';
import { formatNumber } from '~/shared/format';
import { t } from '~/shared/i18n';
import { rpmOf, rpmRange, secondsBetweenShots } from './wizard-model';

export interface RateOfFireControlProps {
  archetype: ArchetypeDto;
  catalog: ArchetypeCatalogDto;
  value: RateOfFireDto | undefined;
  onChange: (rof: RateOfFireDto) => void;
}

/**
 * The rate of fire. A gun gets a slider in rounds per minute with the three classes marked and the
 * time between shots shown; a melee weapon gets the three classes as swing speed.
 */
export function RateOfFireControl({ archetype, catalog, value, onChange }: RateOfFireControlProps) {
  const classId = value && 'class' in value ? value.class : undefined;
  const classes = archetype.rofClasses.flatMap((id) => {
    const found = catalog.rof.find((r) => r.id === id);
    return found ? [found] : [];
  });
  const range = rpmRange(archetype, catalog);
  const rpm = rpmOf(archetype, catalog, value);
  const picker = (
    <SegmentedControl
      label={t('designer.wizard.rof.class')}
      options={classes.map((c) => {
        const at = rpmOf(archetype, catalog, { class: c.id });
        return {
          value: c.id,
          label: at === undefined ? c.label : `${c.label} ${formatNumber(at, 0)}`,
        };
      })}
      value={classId ?? ''}
      onValueChange={(id) => onChange({ class: id })}
    />
  );
  if (!range || rpm === undefined) {
    return (
      <div class="flex flex-col gap-2">
        {picker}
        <p class="text-small text-muted">{t('designer.wizard.rof.swing')}</p>
      </div>
    );
  }
  const marks = classes.map((c) => {
    const at = rpmOf(archetype, catalog, { class: c.id }) ?? rpm;
    return { value: at, label: c.label };
  });
  return (
    <div class="flex flex-col gap-2">
      {picker}
      <RulerSlider
        label={t('designer.wizard.rof.slider')}
        unit={t('designer.wizard.rof.unit')}
        min={range.min}
        max={range.max}
        step={range.step}
        value={Math.min(range.max, Math.max(range.min, rpm))}
        marks={marks}
        onValueChange={(next) => onChange({ rpm: next })}
      />
      <p class="font-mono text-mono text-fg" role="status">
        {t('designer.wizard.rof.effect', {
          rpm: formatNumber(rpm, 0),
          seconds: formatNumber(secondsBetweenShots(rpm), 2),
        })}
      </p>
    </div>
  );
}
