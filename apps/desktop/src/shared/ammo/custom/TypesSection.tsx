import { EmptyState } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import type { CatalogStore } from '../catalogStore';
import type { CustomAmmoStore } from './store';
import { TypeEditor } from './TypeEditor';
import { TypeList } from './TypeList';

export interface TypesSectionProps {
  store: CustomAmmoStore;
  pool: CatalogStore;
}

/** The ammo types: the list on the left and the editor of the chosen type on the right. */
export function TypesSection({ store, pool }: TypesSectionProps) {
  const index = store.typeIndex.value;
  const has = store.custom.value.types[index] !== undefined;
  return (
    <div class="grid min-h-0 grid-cols-[minmax(12rem,1fr)_minmax(0,4fr)] gap-4">
      <TypeList store={store} />
      {has ? (
        <TypeEditor key={store.ids.value[index] ?? index} store={store} index={index} pool={pool} />
      ) : (
        <EmptyState
          compact
          icon="plus"
          title={t('ammo.types.empty-title')}
          description={t('ammo.types.empty-body')}
        />
      )}
    </div>
  );
}
