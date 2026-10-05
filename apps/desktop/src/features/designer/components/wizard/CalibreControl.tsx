import type { ArchetypeCatalogDto, ArchetypeDto, CeCalibreDto } from 'rimstudio-ipc-types';
import { Banner, Combobox, SegmentedControl, Spinner, Switch } from 'rimstudio-ui';
import { formatNumber } from '~/shared/format';
import { t } from '~/shared/i18n';

export interface CalibreControlProps {
  archetype: ArchetypeDto;
  catalog: ArchetypeCatalogDto;
  calibre: string | undefined;
  ammoSet: string | undefined;
  ceCalibre: boolean;
  /** The ammo sets of the Combat Extended install; undefined while they load. */
  ceCalibres: CeCalibreDto[] | undefined;
  onCalibre: (id: string) => void;
  onAmmoSet: (set: string) => void;
  onUseCe: (on: boolean) => void;
}

/**
 * The calibre: a class from tiny to huge, or, when the user chooses so and Combat Extended is
 * loaded, one of the real ammo sets of that install.
 */
export function CalibreControl({
  archetype,
  catalog,
  calibre,
  ammoSet,
  ceCalibre,
  ceCalibres,
  onCalibre,
  onAmmoSet,
  onUseCe,
}: CalibreControlProps) {
  const classes = archetype.calibres.flatMap((id) => {
    const found = catalog.calibres.find((c) => c.id === id);
    return found ? [found] : [];
  });
  const current = classes.find((c) => c.id === calibre);
  // every ammo set can be picked; the ones of the families that suit the type come first
  const suits = (c: CeCalibreDto): boolean =>
    c.family !== undefined && archetype.ammoFamilies.includes(c.family);
  const sets = [
    ...(ceCalibres ?? []).filter(suits),
    ...(ceCalibres ?? []).filter((c) => !suits(c)),
  ];
  const ce = catalog.ce;
  return (
    <div class="flex flex-col gap-3">
      <SegmentedControl
        label={t('designer.wizard.calibre.class')}
        options={classes.map((c) => ({ value: c.id, label: c.label }))}
        value={ceCalibre ? '' : (calibre ?? '')}
        onValueChange={(id) => {
          if (ceCalibre) onUseCe(false);
          onCalibre(id);
        }}
      />
      {current && !ceCalibre ? <p class="text-small text-muted">{current.summary}</p> : null}
      <Switch checked={ceCalibre} onCheckedChange={onUseCe} disabled={!ce.available}>
        {t('designer.wizard.calibre.useCe')}
      </Switch>
      {!ce.available ? (
        <p class="text-small text-muted">{ce.reason ?? t('designer.wizard.calibre.ceMissing')}</p>
      ) : null}
      {ceCalibre ? (
        ceCalibres === undefined ? (
          <div class="flex items-center gap-2 text-muted">
            <Spinner label={t('designer.wizard.loading')} />
            <span>{t('designer.wizard.loading')}</span>
          </div>
        ) : (
          <div class="flex flex-col gap-2">
            <Combobox
              aria-label={t('designer.wizard.calibre.ammoSet')}
              placeholder={t('designer.wizard.calibre.pick')}
              emptyText={t('designer.wizard.calibre.none')}
              value={ammoSet}
              onValueChange={onAmmoSet}
              options={sets.map((c) => ({
                value: c.set,
                label: c.label,
                hint: t('designer.wizard.calibre.hint', {
                  set: c.set,
                  damage: formatNumber(c.damage, 1),
                  weapons: c.weaponCount,
                }),
              }))}
            />
            {ammoSet === undefined ? (
              <p class="text-small text-warning" role="status">
                {t('designer.wizard.calibre.needed')}
              </p>
            ) : null}
            <Banner tone="info">{t('designer.wizard.calibre.ceNote')}</Banner>
          </div>
        )
      ) : null}
    </div>
  );
}
