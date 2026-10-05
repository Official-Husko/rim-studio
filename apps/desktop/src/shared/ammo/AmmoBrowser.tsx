import { useEffect } from 'preact/hooks';
import { Banner, Button, Dialog, EmptyState, Spinner } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import { AmmoDetail } from './AmmoDetail';
import { AmmoFilters } from './AmmoFilters';
import { AmmoRow, AMMO_ROW_HEIGHT } from './AmmoRow';
import type { CatalogStore } from './catalogStore';
import { VirtualList } from './VirtualList';

export interface AmmoBrowserProps {
  open: boolean;
  store: CatalogStore;
  /** The def name of the ammo set the weapon uses now. */
  current?: string | undefined;
  onSelect: (defName: string) => void;
  onClose: () => void;
  /** Open the custom ammo window. Absent when it is not offered. */
  onCreateCustom?: (() => void) | undefined;
  createHint?: string | undefined;
  /** Milliseconds the search waits after the last key before it asks again. */
  searchDelayMs?: number;
}

/**
 * Every ammo set of the user's Combat Extended: searchable, filterable, with the numbers of each ammo type
 * and the weapons that use it. A set is chosen with Select; a new caliber is made with Create custom ammo.
 */
export function AmmoBrowser({
  open,
  store,
  current,
  onSelect,
  onClose,
  onCreateCustom,
  createHint,
  searchDelayMs = 250,
}: AmmoBrowserProps) {
  const query = store.query.value;
  const caliber = store.caliber.value;
  const ammoClass = store.ammoClass.value;

  // the first read (when the window opens) is immediate; a changed filter waits for a pause
  useEffect(() => {
    if (!open) return undefined;
    const first = store.available.value === undefined && !store.loading.peek();
    const timer = setTimeout(() => void store.load(), first ? 0 : searchDelayMs);
    return () => clearTimeout(timer);
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [open, query, caliber, ammoClass]);

  const entries = store.visible.value;
  const selected = store.selectedEntry.value;
  const loading = store.loading.value;
  const error = store.error.value;
  const unavailable = store.available.value === false;
  const reading = loading || (store.available.value === undefined && !error);
  const filtered =
    query.trim() !== '' || caliber !== undefined || ammoClass !== undefined || store.usedOnly.value;
  const count =
    store.matching.value === store.total.value && entries.length === store.matching.value
      ? tn('ammo.count.sets', store.total.value)
      : t('ammo.count.of', { shown: entries.length, total: store.total.value });

  return (
    <Dialog
      open={open}
      size="full"
      title={t('ammo.browser.title')}
      onClose={onClose}
      footer={<Button onClick={onClose}>{t('ammo.close')}</Button>}
    >
      <div class="flex min-h-0 flex-1 flex-col gap-3">
        {unavailable ? (
          <EmptyState
            compact
            icon="info"
            title={t('ammo.unavailable.title')}
            description={store.reason.value ?? t('ammo.unavailable.reason')}
          />
        ) : (
          <>
            <AmmoFilters store={store} onCreateCustom={onCreateCustom} createHint={createHint} />
            {error ? (
              <Banner
                tone="error"
                title={error.code}
                action={
                  <Button size="sm" onClick={() => void store.load()}>
                    {t('ammo.retry')}
                  </Button>
                }
              >
                {error.message}
              </Banner>
            ) : null}
            <p class="m-0 flex items-center gap-2 text-small text-muted" aria-live="polite">
              {reading ? <Spinner size="sm" label={t('ammo.loading')} /> : null}
              {reading && entries.length === 0 ? t('ammo.loading-first') : count}
              {loading && entries.length > 0 ? t('ammo.loading-more') : null}
            </p>
            {entries.length === 0 && !reading && !error ? (
              <EmptyState
                compact
                icon="search"
                title={t('ammo.empty.title')}
                description={filtered ? t('ammo.empty.filtered') : t('ammo.empty.none')}
                action={
                  filtered ? (
                    <Button onClick={() => store.clearFilters()}>{t('ammo.empty.clear')}</Button>
                  ) : null
                }
              />
            ) : (
              <div class="grid min-h-0 flex-1 grid-cols-[minmax(0,2fr)_minmax(0,3fr)] gap-3 max-md:grid-cols-1">
                <div class="flex min-h-0 flex-col">
                  <VirtualList
                    items={entries}
                    rowHeight={AMMO_ROW_HEIGHT}
                    label={t('ammo.list.label')}
                    getKey={(entry) => entry.defName}
                    selectedKey={store.selected.value}
                    onSelect={store.select}
                    renderRow={(entry) => <AmmoRow entry={entry} />}
                  />
                </div>
                <div class="flex min-h-0 flex-col border border-line bg-surface">
                  {selected ? (
                    <AmmoDetail
                      entry={selected}
                      chosen={selected.defName === current}
                      onSelect={onSelect}
                    />
                  ) : (
                    <EmptyState compact title={t('ammo.detail.none')} />
                  )}
                </div>
              </div>
            )}
          </>
        )}
      </div>
    </Dialog>
  );
}
