import { Button, Checkbox, Select, TextField } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import type { CatalogStore } from './catalogStore';

export interface AmmoFiltersProps {
  store: CatalogStore;
  onCreateCustom?: (() => void) | undefined;
  /** Why creating custom ammunition is not offered; shown instead of the button. */
  createHint?: string | undefined;
}

const ALL = '';

/** The search box, the caliber and ammo class filters, the weapons filter and the Create custom ammo button. */
export function AmmoFilters({ store, onCreateCustom, createHint }: AmmoFiltersProps) {
  const calibers = store.calibers.value;
  const classes = store.classes.value;
  return (
    <div class="flex flex-wrap items-end gap-3" role="search" aria-label={t('ammo.filters.label')}>
      <div class="min-w-48 flex-1">
        <TextField
          type="search"
          aria-label={t('ammo.filters.search')}
          placeholder={t('ammo.filters.search-placeholder')}
          value={store.query.value}
          onValueChange={store.setQuery}
        />
      </div>
      <div class="w-48">
        <Select
          aria-label={t('ammo.filters.caliber')}
          value={store.caliber.value ?? ALL}
          onValueChange={store.setCaliber}
          options={[
            { value: ALL, label: t('ammo.filters.all-calibers') },
            ...calibers.map((c) => ({ value: c.name, label: `${c.label} (${c.count})` })),
          ]}
        />
      </div>
      <div class="w-44">
        <Select
          aria-label={t('ammo.filters.class')}
          value={store.ammoClass.value ?? ALL}
          onValueChange={store.setAmmoClass}
          options={[
            { value: ALL, label: t('ammo.filters.all-classes') },
            ...classes.map((c) => ({ value: c.name, label: `${c.label} (${c.count})` })),
          ]}
        />
      </div>
      <Checkbox checked={store.usedOnly.value} onCheckedChange={store.setUsedOnly}>
        {t('ammo.filters.used')}
      </Checkbox>
      {onCreateCustom ? (
        <Button variant="primary" icon="plus" onClick={onCreateCustom}>
          {t('ammo.create')}
        </Button>
      ) : createHint ? (
        <span class="text-small text-muted">{createHint}</span>
      ) : null}
    </div>
  );
}
