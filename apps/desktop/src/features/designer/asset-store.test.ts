import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createAssetStore } from './asset-store';
import { installAssetTransport, installTransport, settle } from './testSupport';

const pickFile = vi.hoisted(() => vi.fn());
vi.mock('~/shared/platform', () => ({ pickFile }));

const PNG = '/home/user/Art/TLWWP_Eagle_Carbine.png';

describe('asset store', () => {
  beforeEach(() => pickFile.mockReset());

  it('reads the facts of a file once and keeps them', async () => {
    const transport = installAssetTransport();
    const store = createAssetStore({ projectId: () => 'p-1' });
    await store.load(PNG);
    await store.load(PNG);
    const state = store.facts.value.get(PNG);
    expect(state?.status === 'ready' && state.info.width).toBe(512);
    expect(transport.calls.filter((c) => c.name === 'designer_asset_info')).toHaveLength(1);
    expect(transport.calls[0]?.request).toEqual({ path: PNG, projectId: 'p-1' });
    await store.load(PNG, true);
    expect(transport.calls.filter((c) => c.name === 'designer_asset_info')).toHaveLength(2);
  });

  it('keeps the error of a failed read and tries again on the next load', async () => {
    let fail = true;
    installAssetTransport({
      designer_asset_info: () => {
        if (fail) throw { code: 'designer.asset-failed', message: 'no', errorId: 'e-1' };
        return { path: PNG, status: 'missing' };
      },
    });
    const store = createAssetStore({ projectId: () => undefined });
    await store.load(PNG);
    expect(store.facts.value.get(PNG)).toMatchObject({ status: 'failed' });
    fail = false;
    await store.load(PNG);
    expect(store.facts.value.get(PNG)).toMatchObject({ status: 'ready' });
  });

  it('reads the facts of a chosen file and returns its path; a cancel changes nothing', async () => {
    installAssetTransport();
    const store = createAssetStore({ projectId: () => 'p-1' });
    pickFile.mockResolvedValueOnce(PNG);
    expect(await store.choose([{ name: 'PNG', extensions: ['png'] }])).toBe(PNG);
    expect(store.facts.value.has(PNG)).toBe(true);
    pickFile.mockResolvedValueOnce(null);
    expect(await store.choose([])).toBeNull();
    expect(store.facts.value.size).toBe(1);
  });

  it('keeps the error of a failed dialog', async () => {
    installAssetTransport();
    const store = createAssetStore({ projectId: () => undefined });
    pickFile.mockRejectedValueOnce({ code: 'platform.native-failed', message: 'x', errorId: 'e' });
    expect(await store.choose([])).toBeNull();
    expect(store.pickError.value?.code).toBe('platform.native-failed');
  });

  it('searches sounds and drops a late answer of an older search', async () => {
    let release: (() => void) | undefined;
    installAssetTransport({
      defs_search: async (request) => {
        const query = (request as { query: string }).query;
        if (query === 'slow') await new Promise<void>((r) => (release = r));
        return {
          queryId: 'x',
          total: 1,
          offset: 0,
          items: [
            {
              defType: 'SoundDef',
              defName: `Found_${query}`,
              isAbstract: false,
              modId: 'm',
              file: 'f',
            },
          ],
        };
      },
    });
    const store = createAssetStore({ projectId: () => undefined });
    const first = store.searchSounds('slow');
    await settle();
    await store.searchSounds('fast');
    release?.();
    await first;
    expect(store.sounds.value.rows[0]?.defName).toBe('Found_fast');
    expect(store.sounds.value.status).toBe('ready');
  });

  it('keeps the error of a failed search', async () => {
    installTransport({
      defs_search: () => {
        throw { code: 'defs.no-session', message: 'no session', errorId: 'e-2' };
      },
    });
    const store = createAssetStore({ projectId: () => undefined });
    await store.searchSounds('x');
    expect(store.sounds.value.status).toBe('failed');
    expect(store.sounds.value.error?.code).toBe('defs.no-session');
  });
});
