import { Tabs } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import type { CatalogStore } from '../catalogStore';
import { ItemFields } from './ItemFields';
import { diagnosticsUnder, typePointer, worst, type TypeTab } from './model';
import { ProjectileFields } from './ProjectileFields';
import { RecipeFields } from './RecipeFields';
import type { CustomAmmoStore } from './store';
import { TypeHeader } from './TypeHeader';

export interface TypeEditorProps {
  store: CustomAmmoStore;
  index: number;
  pool: CatalogStore;
}

/** One ammo type: the header, then the projectile, the ammo item and the crafting recipe as tabs. */
export function TypeEditor({ store, index, pool }: TypeEditorProps) {
  const mark = (sub: string): string | undefined => {
    const level = worst(diagnosticsUnder(store.diagnostics.value, typePointer(index, sub)));
    return level === 'error'
      ? t('ammo.badge.error')
      : level === 'warning'
        ? t('ammo.badge.warning')
        : undefined;
  };
  const tabs = [
    { id: 'projectile', label: t('ammo.tab.projectile'), badge: mark('/projectile') },
    { id: 'item', label: t('ammo.tab.item'), badge: mark('/item') },
    { id: 'recipe', label: t('ammo.tab.recipe'), badge: mark('/recipe') },
  ].map(({ badge, ...rest }) => (badge ? { ...rest, badge } : rest));
  return (
    <div class="flex min-w-0 flex-col gap-4">
      <TypeHeader store={store} index={index} pool={pool} />
      <Tabs
        tabs={tabs}
        value={store.tab.value}
        onValueChange={(id) => {
          store.tab.value = id as TypeTab;
        }}
        label={t('ammo.tab.label')}
      >
        {(id) => (
          <div class="pt-3">
            {id === 'projectile' ? <ProjectileFields store={store} index={index} /> : null}
            {id === 'item' ? <ItemFields store={store} index={index} /> : null}
            {id === 'recipe' ? <RecipeFields store={store} index={index} /> : null}
          </div>
        )}
      </Tabs>
    </div>
  );
}
