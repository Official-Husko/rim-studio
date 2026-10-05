import { batch, computed, signal } from '@preact/signals';
import type {
  ApiError,
  CeAmmoEntryDto,
  CeAmmoFacetDto,
  DesignerCeAmmoCatalogRequest,
  DraftDto,
} from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import { ammoCatalog, CATALOG_PAGE } from './api';

export interface CatalogDeps {
  /** The current design, so the sets the designer would suggest are marked. Optional. */
  draft?: () => DraftDto | undefined;
  /** The call that fetches one page; replaced in tests. */
  fetchPage?: typeof ammoCatalog;
}

/**
 * The catalogue of ammo sets as the browser shows it. The backend filters (words, caliber, ammo class) and
 * pages (200 at most), so the store asks for every page of a filter one after the other and keeps the
 * entries; the list that draws them is windowed. Only the "used by weapons" filter is applied here, on a
 * count the backend returned.
 */
export function createCatalogStore(deps: CatalogDeps = {}) {
  const fetchPage = deps.fetchPage ?? ammoCatalog;
  const query = signal('');
  const caliber = signal<string | undefined>(undefined);
  const ammoClass = signal<string | undefined>(undefined);
  const usedOnly = signal(false);
  const entries = signal<readonly CeAmmoEntryDto[]>([]);
  const calibers = signal<readonly CeAmmoFacetDto[]>([]);
  const classes = signal<readonly CeAmmoFacetDto[]>([]);
  const total = signal(0);
  const matching = signal(0);
  const available = signal<boolean | undefined>(undefined);
  const reason = signal<string | undefined>(undefined);
  const loading = signal(false);
  const error = signal<ApiError | undefined>(undefined);
  const selected = signal<string | undefined>(undefined);
  let seq = 0;

  const visible = computed(() =>
    usedOnly.value ? entries.value.filter((e) => e.weaponCount > 0) : entries.value,
  );
  const selectedEntry = computed(() => entries.value.find((e) => e.defName === selected.value));

  function request(page: number): DesignerCeAmmoCatalogRequest {
    const draft = deps.draft?.();
    return {
      page,
      pageSize: CATALOG_PAGE,
      ...(draft ? { draft } : {}),
      ...(query.peek().trim() !== '' ? { query: query.peek().trim() } : {}),
      ...(caliber.peek() ? { caliber: caliber.peek() as string } : {}),
      ...(ammoClass.peek() ? { class: ammoClass.peek() as string } : {}),
    };
  }

  /** Load the first page of the current filter, then the others in the background. */
  async function load(): Promise<void> {
    const mine = ++seq;
    loading.value = true;
    try {
      let page = 0;
      let kept: CeAmmoEntryDto[] = [];
      for (;;) {
        const result = await fetchPage(request(page));
        if (mine !== seq) return;
        kept = page === 0 ? [...result.entries] : [...kept, ...result.entries];
        batch(() => {
          entries.value = kept;
          total.value = result.total;
          matching.value = result.matching;
          available.value = result.available;
          reason.value = result.reason;
          error.value = undefined;
          if (page === 0) {
            calibers.value = result.calibers;
            classes.value = result.classes;
          }
          const keep = selected.peek();
          if (keep === undefined || !kept.some((e) => e.defName === keep)) {
            if (page === 0) selected.value = kept[0]?.defName;
          }
        });
        page += 1;
        if (result.entries.length === 0 || kept.length >= result.matching) break;
      }
    } catch (thrown) {
      if (mine !== seq) return;
      error.value = normalizeError(thrown);
    } finally {
      if (mine === seq) loading.value = false;
    }
  }

  /** The words of the search box; the caller debounces and calls `load`. */
  function setQuery(value: string): void {
    query.value = value;
  }

  function setCaliber(value: string | undefined): void {
    caliber.value = value === '' ? undefined : value;
  }

  function setAmmoClass(value: string | undefined): void {
    ammoClass.value = value === '' ? undefined : value;
  }

  function clearFilters(): void {
    batch(() => {
      query.value = '';
      caliber.value = undefined;
      ammoClass.value = undefined;
      usedOnly.value = false;
    });
  }

  function setUsedOnly(on: boolean): void {
    usedOnly.value = on;
  }

  function select(defName: string | undefined): void {
    selected.value = defName;
  }

  return {
    query,
    caliber,
    ammoClass,
    usedOnly,
    entries,
    visible,
    calibers,
    classes,
    total,
    matching,
    available,
    reason,
    loading,
    error,
    selected,
    selectedEntry,
    load,
    setQuery,
    setCaliber,
    setAmmoClass,
    clearFilters,
    setUsedOnly,
    select,
  };
}

export type CatalogStore = ReturnType<typeof createCatalogStore>;
