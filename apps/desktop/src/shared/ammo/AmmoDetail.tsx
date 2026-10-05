import type { CeAmmoEntryDto } from 'rimstudio-ipc-types';
import { Banner, Button, KeyValueList } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import { AmmoTypeTable } from './AmmoTypeTable';

export interface AmmoDetailProps {
  entry: CeAmmoEntryDto;
  /** True when this set is the one the weapon uses now. */
  chosen: boolean;
  onSelect: (defName: string) => void;
}

/** The details of one ammo set: its numbers per ammo type, the weapons that use it and the Select button. */
export function AmmoDetail({ entry, chosen, onSelect }: AmmoDetailProps) {
  const facts = [
    { key: t('ammo.detail.def'), value: entry.defName, mono: true },
    { key: t('ammo.detail.caliber'), value: entry.caliber },
    ...(entry.family ? [{ key: t('ammo.detail.family'), value: entry.family }] : []),
    ...(entry.similarTo
      ? [{ key: t('ammo.detail.similar'), value: entry.similarTo, mono: true }]
      : []),
    ...(entry.generic
      ? [
          {
            key: t('ammo.detail.generic'),
            value: tn('ammo.detail.generic-count', entry.similarSets),
          },
        ]
      : []),
  ];
  return (
    <div class="flex min-h-0 flex-col gap-3 overflow-auto p-4" data-ammo-detail={entry.defName}>
      <header class="flex flex-wrap items-center gap-2">
        <h3 class="m-0 min-w-0 flex-1 truncate font-display text-heading font-semibold">
          {entry.label}
        </h3>
        <Button
          variant={chosen ? 'secondary' : 'primary'}
          icon="check"
          disabled={chosen}
          onClick={() => onSelect(entry.defName)}
        >
          {chosen ? t('ammo.detail.chosen') : t('ammo.detail.select')}
        </Button>
      </header>
      <KeyValueList label={t('ammo.detail.facts')} items={facts} />
      {entry.suggested ? <Banner tone="info">{t('ammo.detail.suggested')}</Banner> : null}
      <section class="flex flex-col gap-1" aria-label={t('ammo.detail.types')}>
        <h4 class="m-0 font-display text-label font-semibold tracking-label text-muted uppercase">
          {t('ammo.detail.types')}
        </h4>
        <AmmoTypeTable types={entry.types} />
      </section>
      <section class="flex flex-col gap-1" aria-label={t('ammo.detail.weapons')}>
        <h4 class="m-0 font-display text-label font-semibold tracking-label text-muted uppercase">
          {t('ammo.detail.weapons')}
        </h4>
        {entry.weaponCount === 0 ? (
          <p class="m-0 text-small text-muted">{t('ammo.detail.no-weapons')}</p>
        ) : (
          <>
            <p class="m-0 text-small text-muted">
              {tn('ammo.detail.weapon-count', entry.weaponCount)}
            </p>
            <ul class="m-0 flex list-none flex-wrap gap-x-3 gap-y-1 p-0 text-small">
              {entry.exampleWeapons.map((name) => (
                <li key={name}>{name}</li>
              ))}
            </ul>
          </>
        )}
      </section>
    </div>
  );
}
