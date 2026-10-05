import { computed, signal } from '@preact/signals';
import type {
  ApiError,
  ItemKindDto,
  ReferenceItemDto,
  ReferenceListDto,
  StatPoolDto,
  TechLevelDto,
} from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import * as api from './api';

/** The reference weapons of the install: the filters, the loaded list and its pools. */
export function createReferenceStore() {
  const kind = signal<ItemKindDto>('ranged');
  const tier = signal<TechLevelDto | undefined>(undefined);
  const role = signal<string | undefined>(undefined);
  const text = signal('');
  const list = signal<ReferenceListDto | undefined>(undefined);
  const roles = signal<string[]>([]);
  const loading = signal(false);
  const error = signal<ApiError | undefined>(undefined);
  let seq = 0;

  /** The items after the text filter; the order is the backend order (by strength). */
  const items = computed<ReferenceItemDto[]>(() => {
    const needle = text.value.trim().toLowerCase();
    const all = list.value?.items ?? [];
    if (needle === '') return all;
    return all.filter(
      (i) => i.label.toLowerCase().includes(needle) || i.defName.toLowerCase().includes(needle),
    );
  });

  /** The pools by stat name, for the hints beside the fields. */
  const pools = computed<Map<string, StatPoolDto>>(
    () => new Map((list.value?.pools ?? []).map((p) => [p.stat, p])),
  );

  /** Load the list for the current filters. */
  async function load(): Promise<void> {
    const mine = ++seq;
    loading.value = true;
    try {
      const filters: { role?: string; tier?: TechLevelDto } = {};
      if (role.value !== undefined) filters.role = role.value;
      if (tier.value !== undefined) filters.tier = tier.value;
      const result = await api.referenceList(kind.value, filters);
      if (mine !== seq) return;
      list.value = result;
      error.value = undefined;
      if (role.value === undefined) {
        roles.value = [...new Set(result.items.flatMap((i) => (i.role ? [i.role] : [])))].sort();
      }
    } catch (thrown) {
      if (mine === seq) error.value = normalizeError(thrown);
    } finally {
      if (mine === seq) loading.value = false;
    }
  }

  /** Change the kind; the other filters are cleared because roles differ per kind. */
  function setKind(next: ItemKindDto): void {
    if (kind.peek() === next) return;
    kind.value = next;
    role.value = undefined;
    roles.value = [];
    void load();
  }

  function setTier(next: TechLevelDto | undefined): void {
    tier.value = next;
    void load();
  }

  function setRole(next: string | undefined): void {
    role.value = next;
    void load();
  }

  return {
    kind,
    tier,
    role,
    text,
    list,
    roles,
    items,
    pools,
    loading,
    error,
    load,
    setKind,
    setTier,
    setRole,
  };
}

export type ReferenceStore = ReturnType<typeof createReferenceStore>;
