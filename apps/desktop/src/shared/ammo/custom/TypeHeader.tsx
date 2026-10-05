import { useMemo } from 'preact/hooks';
import { Banner, Button, Combobox, FormField, Select, Spinner, TextField } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import type { CatalogStore } from '../catalogStore';
import { diagnosticsAt, typePointer } from './model';
import type { CustomAmmoStore } from './store';

export interface TypeHeaderProps {
  store: CustomAmmoStore;
  index: number;
  /** The catalogue of the install: the ammo classes and the ammunition a type can be copied from. */
  pool: CatalogStore;
}

/** The ammo class and key of a type, the suggestion button and the picker that starts a type from real ammunition. */
export function TypeHeader({ store, index, pool }: TypeHeaderProps) {
  const type = store.custom.value.types[index];
  const entries = pool.entries.value;
  const classes = pool.classes.value;
  const copyOptions = useMemo(() => {
    // an ammo def can belong to several sets; it is listed once, under the first
    const seen = new Set<string>();
    const out: Array<{ value: string; label: string; hint: string }> = [];
    for (const entry of entries) {
      for (const a of entry.types) {
        if (seen.has(a.ammoDef)) continue;
        seen.add(a.ammoDef);
        out.push({ value: a.ammoDef, label: a.ammoLabel, hint: entry.defName });
      }
    }
    return out;
  }, [entries]);
  if (!type) return null;
  const id = store.ids.value[index];
  const suggestion = id === undefined ? undefined : store.suggestions.value[id];
  const busy = store.suggesting.value === id;
  const classError = diagnosticsAt(store.diagnostics.value, typePointer(index, '/ammoClass')).find(
    (d) => d.severity === 'error',
  );
  const keyError = diagnosticsAt(store.diagnostics.value, typePointer(index, '/key')).find(
    (d) => d.severity === 'error',
  );
  const options = classes.map((c) => ({ value: c.name, label: `${c.label} (${c.name})` }));
  if (type.ammoClass !== '' && !options.some((o) => o.value === type.ammoClass)) {
    options.unshift({ value: type.ammoClass, label: type.ammoClass });
  }
  return (
    <div class="flex flex-col gap-3">
      <div class="grid grid-cols-[repeat(auto-fit,minmax(14rem,1fr))] gap-x-4 gap-y-3">
        <div data-ammo-field={typePointer(index, '/ammoClass')}>
          <FormField
            label={t('ammo.field.class')}
            help={t('ammo.field.class-help')}
            {...(classError ? { error: classError.message } : {})}
          >
            <Select
              value={type.ammoClass === '' ? undefined : type.ammoClass}
              placeholder={t('ammo.field.class-choose')}
              options={options}
              onValueChange={(v) => store.setClass(index, v)}
            />
          </FormField>
        </div>
        <div data-ammo-field={typePointer(index, '/key')}>
          <FormField
            label={t('ammo.field.key')}
            help={t('ammo.field.key-help')}
            {...(keyError ? { error: keyError.message } : {})}
          >
            <TextField
              value={type.key}
              onValueChange={(key) => store.setField(index, '/key', key)}
            />
          </FormField>
        </div>
      </div>
      <div class="flex flex-wrap items-end gap-3">
        <Button
          icon="refresh"
          disabled={type.ammoClass === '' || busy}
          onClick={() => void store.suggestFor(index)}
        >
          {t('ammo.suggest.run')}
        </Button>
        <div class="min-w-64 flex-1">
          <FormField label={t('ammo.copy.label')} help={t('ammo.copy.help')}>
            <Combobox
              options={copyOptions}
              value={type.copiedFrom}
              placeholder={
                pool.loading.value && copyOptions.length === 0
                  ? t('ammo.copy.loading')
                  : t('ammo.copy.placeholder')
              }
              emptyText={t('ammo.copy.empty')}
              disabled={type.ammoClass === '' || copyOptions.length === 0}
              onValueChange={(def) => void store.suggestFor(index, { copyFrom: def })}
            />
          </FormField>
        </div>
        {busy ? <Spinner size="sm" label={t('ammo.suggest.busy')} /> : null}
      </div>
      {store.suggestError.value ? (
        <Banner tone="error" title={store.suggestError.value.code}>
          {store.suggestError.value.message}
        </Banner>
      ) : null}
      {suggestion && !suggestion.available ? (
        <Banner tone="warning" title={t('ammo.suggest.none')}>
          {suggestion.reason ?? t('ammo.suggest.none-reason')}
        </Banner>
      ) : null}
      {suggestion?.available ? (
        <p class="m-0 text-small text-muted" data-ammo-basis>
          {suggestion.copiedFrom
            ? t('ammo.suggest.copied', { from: suggestion.copiedFrom })
            : t('ammo.suggest.basis', {
                n: suggestion.nearest.length,
                class: suggestion.classLabel,
              })}
        </p>
      ) : null}
      {suggestion?.notes.map((note) => (
        <p key={note} class="m-0 text-small text-muted">
          {note}
        </p>
      ))}
    </div>
  );
}
