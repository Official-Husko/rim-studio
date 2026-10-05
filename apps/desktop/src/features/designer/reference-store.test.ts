import { describe, expect, it } from 'vitest';
import { createReferenceStore } from './reference-store';
import { installTransport, settle } from './testSupport';

describe('reference store', () => {
  it('loads the real ranged weapons sorted by strength', async () => {
    installTransport();
    const store = createReferenceStore();
    await store.load();
    expect(store.list.value?.total).toBe(20);
    const strengths = store.items.value.map((i) => i.strength ?? 0);
    expect(strengths).toEqual([...strengths].sort((a, b) => a - b));
    expect(store.roles.value).toContain('sniper');
    expect(store.pools.value.get('damage')?.max).toBe(25);
  });

  it('filters by text on the loaded list', async () => {
    installTransport();
    const store = createReferenceStore();
    await store.load();
    store.text.value = 'bolt';
    expect(store.items.value.map((i) => i.defName)).toEqual(['Gun_BoltActionRifle']);
  });

  it('sends the tier and role filters to the backend and clears them for another kind', async () => {
    const transport = installTransport();
    const store = createReferenceStore();
    store.setTier('industrial');
    store.setRole('rifle');
    await settle();
    const last = transport.calls.at(-1)?.request as { tier?: string; role?: string };
    expect(last).toMatchObject({ tier: 'industrial', role: 'rifle' });
    store.setKind('melee');
    await settle();
    const melee = transport.calls.at(-1)?.request as { kind: string; role?: string };
    expect(melee.kind).toBe('melee');
    expect(melee.role).toBeUndefined();
  });

  it('keeps the error of a failed load', async () => {
    installTransport({
      designer_reference_list: () => {
        throw { code: 'designer.reference-unavailable', message: 'no install', errorId: 'e-4' };
      },
    });
    const store = createReferenceStore();
    await store.load();
    expect(store.error.value?.code).toBe('designer.reference-unavailable');
    expect(store.loading.value).toBe(false);
  });
});
