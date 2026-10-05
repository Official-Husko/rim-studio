import { useEffect, useMemo, useRef } from 'preact/hooks';
import type { CustomAmmoDto, WritePlanDto } from 'rimstudio-ipc-types';
import { Banner, Button, Dialog, Spinner, Tabs } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import type { ammoSuggest } from '../api';
import { createCatalogStore, type CatalogStore } from '../catalogStore';
import { ArtSection } from './ArtSection';
import { IdentitySection } from './IdentitySection';
import { type Section } from './model';
import { ReviewSection } from './ReviewSection';
import { SetSection } from './SetSection';
import { createCustomAmmoStore, type CustomAmmoStore } from './store';
import { TypesSection } from './TypesSection';

export interface CustomAmmoDialogProps {
  open: boolean;
  /** The custom ammo to edit; a new caliber when absent. */
  initial?: CustomAmmoDto | undefined;
  /** Plans the weapon with the custom ammo so the backend can check it. Absent without an open project. */
  plan?: ((custom: CustomAmmoDto) => Promise<WritePlanDto>) | undefined;
  onSave: (custom: CustomAmmoDto) => void;
  /** Take the custom ammo out of the weapon. Offered only when there is something to remove. */
  onRemove?: (() => void) | undefined;
  onClose: () => void;
  /** Replaced in tests. */
  suggest?: typeof ammoSuggest;
  debounceMs?: number;
  /** A catalogue the window reads the ammo classes and the ammunition to copy from; a new one when absent. */
  pool?: CatalogStore;
}

/** Moves the keyboard focus to the field a pointer names, trying shorter pointers until an element is found. */
function focusPointer(root: HTMLElement | null, pointer: string): boolean {
  if (!root) return false;
  let at = pointer;
  while (at.length > 1) {
    const holder = root.querySelector<HTMLElement>(`[data-ammo-field="${at}"]`);
    const target = holder?.querySelector<HTMLElement>('input, select, textarea, button');
    if (target) {
      target.focus();
      target.scrollIntoView?.({ block: 'center' });
      return true;
    }
    at = at.slice(0, at.lastIndexOf('/'));
  }
  return false;
}

function Window(props: CustomAmmoDialogProps) {
  const { initial, onSave, onRemove, onClose } = props;
  const store: CustomAmmoStore = useMemo(
    () =>
      createCustomAmmoStore({
        initial,
        plan: props.plan,
        ...(props.suggest ? { suggest: props.suggest } : {}),
        ...(props.debounceMs !== undefined ? { debounceMs: props.debounceMs } : {}),
      }),
    // the window edits one working copy for as long as it is open
    // oxlint-disable-next-line react-hooks/exhaustive-deps
    [],
  );
  const pool = useMemo(() => props.pool ?? createCatalogStore(), [props.pool]);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    store.check();
    if (pool.available.peek() === undefined && !pool.loading.peek()) void pool.load();
    return () => store.dispose();
  }, [store, pool]);

  const focus = store.focusField.value;
  useEffect(() => {
    if (!focus) return undefined;
    const timer = setTimeout(() => {
      focusPointer(root.current, focus);
      store.focusField.value = undefined;
    }, 0);
    return () => clearTimeout(timer);
  }, [focus, store]);

  const custom = store.custom.value;
  const errors = store.errors.value.length;
  const section = store.section.value;
  const typeCount = custom.types.length;
  const reason =
    typeCount === 0
      ? t('ammo.save.no-types')
      : errors > 0
        ? tn('ammo.save.errors', errors)
        : store.planning.value || store.stale.value
          ? t('ammo.save.checking')
          : undefined;
  const tabs = [
    { id: 'identity', label: t('ammo.section.identity') },
    { id: 'types', label: t('ammo.section.types'), badge: String(typeCount) },
    { id: 'set', label: t('ammo.section.set') },
    { id: 'art', label: t('ammo.section.art') },
    {
      id: 'review',
      label: t('ammo.section.review'),
      ...(errors > 0 ? { badge: String(errors) } : {}),
    },
  ];
  return (
    <Dialog
      open
      size="full"
      title={initial ? t('ammo.custom.title-edit') : t('ammo.custom.title-new')}
      onClose={onClose}
      footer={
        <div class="flex w-full flex-wrap items-center gap-2">
          {onRemove ? (
            <Button variant="danger" icon="trash" onClick={onRemove}>
              {t('ammo.custom.remove')}
            </Button>
          ) : null}
          <span class="ml-auto flex items-center gap-2 text-small text-muted" aria-live="polite">
            {store.planning.value ? <Spinner size="sm" label={t('ammo.save.checking')} /> : null}
            {reason}
          </span>
          <Button onClick={onClose}>{t('ammo.cancel')}</Button>
          <Button
            variant="primary"
            icon="check"
            disabled={!store.canSave.value && store.hasPlan}
            onClick={() => onSave(store.custom.peek())}
          >
            {t('ammo.save')}
          </Button>
        </div>
      }
    >
      <div ref={root} class="flex min-h-0 flex-1 flex-col gap-3">
        {!store.hasPlan ? <Banner tone="info">{t('ammo.custom.unchecked')}</Banner> : null}
        {store.planError.value && section !== 'review' ? (
          <Banner tone="warning" title={store.planError.value.code}>
            {store.planError.value.message}
          </Banner>
        ) : null}
        <Tabs
          tabs={tabs}
          value={section}
          onValueChange={(id) => {
            store.section.value = id as Section;
          }}
          label={t('ammo.section.label')}
        >
          {(id) => (
            <div class="pt-4">
              {id === 'identity' ? <IdentitySection store={store} /> : null}
              {id === 'types' ? <TypesSection store={store} pool={pool} /> : null}
              {id === 'set' ? <SetSection store={store} pool={pool} /> : null}
              {id === 'art' ? <ArtSection store={store} /> : null}
              {id === 'review' ? <ReviewSection store={store} /> : null}
            </div>
          )}
        </Tabs>
      </div>
    </Dialog>
  );
}

/**
 * The window for a custom Combat Extended caliber: its name and caliber, every ammo type with the data of
 * the projectile, the ammo item and the recipe, how the set is grouped, the art and the review of the files.
 * The backend checks the spec as it is typed; Save puts it into the Combat Extended block of the host.
 */
export function CustomAmmoDialog(props: CustomAmmoDialogProps) {
  return props.open ? <Window {...props} /> : null;
}
