import { useMemo } from 'preact/hooks';
import { Combobox, FormField, Select } from 'rimstudio-ui';
import { RawNodeListField } from '~/shared/ce/RawNodeListField';
import { t } from '~/shared/i18n';
import type { CatalogStore } from '../catalogStore';
import { AMMO_POINTER } from './model';
import { SetTextField } from './SetTextField';
import type { CustomAmmoStore } from './store';

export interface SetSectionProps {
  store: CustomAmmoStore;
  pool: CatalogStore;
}

/** How the caliber sits among the install's ammo sets: the set it is similar to, its category and its default type. */
export function SetSection({ store, pool }: SetSectionProps) {
  const custom = store.custom.value;
  const entries = pool.entries.value;
  const similar = useMemo(
    () =>
      [...entries]
        .sort((a, b) => Number(b.generic) - Number(a.generic) || a.label.localeCompare(b.label))
        .map((e) => ({
          value: e.defName,
          label: e.label,
          hint: e.generic ? t('ammo.set.generic-hint', { def: e.defName }) : e.defName,
        })),
    [entries],
  );
  const keys = custom.types.map((type) => type.key.trim()).filter((k) => k !== '');
  const similarError = store.diagnostics.value.find(
    (d) => d.field === `${AMMO_POINTER}/similarTo` && d.severity === 'error',
  );
  return (
    <div class="flex max-w-xl flex-col gap-4">
      <p class="m-0 text-small text-muted">{t('ammo.set.intro')}</p>
      <div data-ammo-field={`${AMMO_POINTER}/similarTo`}>
        <FormField
          label={t('ammo.field.similar')}
          help={t('ammo.field.similar-help')}
          {...(similarError ? { error: similarError.message } : {})}
        >
          <Combobox
            options={similar}
            value={custom.similarTo}
            placeholder={t('ammo.field.similar-choose')}
            emptyText={t('ammo.copy.empty')}
            onValueChange={(v) => store.patch({ similarTo: v })}
          />
        </FormField>
      </div>
      <div data-ammo-field={`${AMMO_POINTER}/defaultType`}>
        <FormField label={t('ammo.field.default-type')} help={t('ammo.field.default-type-help')}>
          <Select
            value={custom.defaultType ?? ''}
            options={[
              { value: '', label: t('ammo.field.default-first') },
              ...keys.map((k) => ({ value: k, label: k })),
            ]}
            onValueChange={(v) => store.patch({ defaultType: v === '' ? undefined : v })}
          />
        </FormField>
      </div>
      <SetTextField
        store={store}
        member="categoryParent"
        label={t('ammo.field.category-parent')}
        help={t('ammo.field.category-parent-help')}
      />
      <SetTextField
        store={store}
        member="categoryIcon"
        label={t('ammo.field.category-icon')}
        help={t('ammo.field.category-icon-help')}
      />
      <RawNodeListField
        label={t('ammo.raw.set')}
        help={t('ammo.raw.help')}
        nodes={custom.setExtra ?? []}
        onChange={(nodes) => store.patch({ setExtra: nodes })}
      />
    </div>
  );
}
