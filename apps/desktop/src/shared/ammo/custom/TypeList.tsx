import { Badge, Button } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { diagnosticsUnder, typePointer, typeTitle, worst } from './model';
import type { CustomAmmoStore } from './store';

export interface TypeListProps {
  store: CustomAmmoStore;
}

/** The ammo types of the caliber: pick one to edit it, add one, duplicate, reorder or remove. */
export function TypeList({ store }: TypeListProps) {
  const types = store.custom.value.types;
  const current = store.typeIndex.value;
  return (
    <div class="flex min-h-0 flex-col gap-2">
      <ul class="m-0 flex list-none flex-col gap-1 p-0" aria-label={t('ammo.types.list')}>
        {types.map((type, index) => {
          const level = worst(diagnosticsUnder(store.diagnostics.value, typePointer(index)));
          const active = index === current;
          return (
            <li key={store.ids.value[index] ?? index}>
              <div
                class={
                  active
                    ? 'flex items-center gap-1 border border-accent bg-accent-tint px-1'
                    : 'flex items-center gap-1 border border-line px-1'
                }
              >
                <button
                  type="button"
                  aria-current={active ? 'true' : undefined}
                  onClick={() => store.selectType(index)}
                  class="flex min-w-0 flex-1 flex-col items-start gap-0.5 px-1 py-1.5 text-left"
                >
                  <span class="max-w-full truncate text-body font-semibold">
                    {typeTitle(type, index)}
                  </span>
                  <span class="max-w-full truncate font-mono text-mono-small text-muted">
                    {type.ammoClass || t('ammo.types.no-class')}
                  </span>
                </button>
                {level ? (
                  <Badge tone={level === 'error' ? 'danger' : 'warning'}>
                    {level === 'error' ? t('ammo.badge.error') : t('ammo.badge.warning')}
                  </Badge>
                ) : null}
              </div>
            </li>
          );
        })}
      </ul>
      {types.length === 0 ? <p class="m-0 text-small text-muted">{t('ammo.types.none')}</p> : null}
      <Button icon="plus" onClick={() => store.addType()}>
        {t('ammo.types.add')}
      </Button>
      {types.length > 0 ? (
        <div class="flex flex-wrap gap-1" role="group" aria-label={t('ammo.types.actions')}>
          <Button size="sm" disabled={current <= 0} onClick={() => store.moveType(current, -1)}>
            {t('ammo.types.up')}
          </Button>
          <Button
            size="sm"
            disabled={current >= types.length - 1}
            onClick={() => store.moveType(current, 1)}
          >
            {t('ammo.types.down')}
          </Button>
          <Button size="sm" icon="copy" onClick={() => store.duplicateType(current)}>
            {t('ammo.types.duplicate')}
          </Button>
          <Button size="sm" icon="trash" variant="danger" onClick={() => store.removeType(current)}>
            {t('ammo.types.remove')}
          </Button>
        </div>
      ) : null}
    </div>
  );
}
