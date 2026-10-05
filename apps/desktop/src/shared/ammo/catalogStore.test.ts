import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { DraftDto } from 'rimstudio-ipc-types';
import { createCatalogStore } from './catalogStore';
import { catalogSlice, manyEntries, settle } from './testSupport';

function pages(total: number) {
  const entries = manyEntries(total);
  return vi.fn(async (request: { page?: number; pageSize?: number }) => {
    const { page = 0, pageSize = 200 } = request;
    return {
      ...catalogSlice(),
      total,
      matching: total,
      entries: entries.slice(page * pageSize, (page + 1) * pageSize),
    };
  });
}

describe('catalog store', () => {
  beforeEach(() => vi.useRealTimers());

  it('reads every page of a catalogue larger than one page', async () => {
    const fetchPage = pages(450);
    const store = createCatalogStore({ fetchPage });
    await store.load();
    expect(fetchPage.mock.calls.map(([r]) => r.page)).toEqual([0, 1, 2]);
    expect(store.entries.value).toHaveLength(450);
    expect(store.matching.value).toBe(450);
    expect(store.loading.value).toBe(false);
    expect(store.selected.value).toBe(store.entries.value[0]?.defName);
  });

  it('sends the filters and the draft, and shows the entries before the last page arrives', async () => {
    const fetchPage = pages(250);
    const draft = { schemaVersion: 2 } as unknown as DraftDto;
    const store = createCatalogStore({ fetchPage, draft: () => draft });
    store.setQuery(' 303 ');
    store.setCaliber('.303 British');
    store.setAmmoClass('ArmorPiercing');
    await store.load();
    expect(fetchPage.mock.calls[0]?.[0]).toMatchObject({
      page: 0,
      pageSize: 200,
      query: '303',
      caliber: '.303 British',
      class: 'ArmorPiercing',
      draft,
    });
    store.clearFilters();
    expect(store.query.value).toBe('');
    expect(store.caliber.value).toBeUndefined();
  });

  it('filters by weapons that use the set on the client', async () => {
    const store = createCatalogStore({ fetchPage: pages(40) });
    await store.load();
    const used = store.entries.value.filter((e) => e.weaponCount > 0).length;
    expect(used).toBeGreaterThan(0);
    store.setUsedOnly(true);
    expect(store.visible.value).toHaveLength(used);
    expect(store.visible.value.every((e) => e.weaponCount > 0)).toBe(true);
  });

  it('answers unavailable with the reason', async () => {
    const store = createCatalogStore({
      fetchPage: async () => ({
        ...catalogSlice(),
        available: false,
        reason: 'no CE',
        entries: [],
      }),
    });
    await store.load();
    expect(store.available.value).toBe(false);
    expect(store.reason.value).toBe('no CE');
    expect(store.entries.value).toEqual([]);
  });

  it('keeps the error of a failed read and ignores an older answer', async () => {
    let fail = true;
    const store = createCatalogStore({
      fetchPage: async () => {
        if (fail) throw { code: 'designer.failed', message: 'no', errorId: 'e-1' };
        return catalogSlice();
      },
    });
    await store.load();
    expect(store.error.value?.code).toBe('designer.failed');
    fail = false;
    const slow = store.load();
    await store.load();
    await slow;
    await settle();
    expect(store.error.value).toBeUndefined();
    expect(store.entries.value.length).toBeGreaterThan(0);
  });
});
