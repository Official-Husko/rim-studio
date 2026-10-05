import { loadFixture } from 'rimstudio-testkit';
import type { SourcesProbeFolderResponse } from 'rimstudio-ipc-types';
import { describe, expect, it } from 'vitest';
import {
  adding,
  cancelAdd,
  confirmAdd,
  forgetSource,
  loadSources,
  moveSource,
  pending,
  renameSource,
  setSourceEnabled,
  sources,
  startAdd,
} from './sourcesStore';
import { installTransport } from './testSupport';

describe('sourcesStore', () => {
  it('lists the three built in sources of a real install', async () => {
    installTransport();
    await loadSources();
    expect(sources.value.map((s) => s.kind)).toEqual(['game-data', 'game-mods', 'workshop']);
  });

  it('probes before adding and adds nothing until confirmed', async () => {
    const transport = installTransport();
    await startAdd('/mods');
    expect(pending.value?.probe?.modCount).toBe(19);
    expect(transport.calls.map((c) => c.name)).toEqual(['sources_probe_folder']);
    cancelAdd();
    expect(pending.value).toBeUndefined();
    expect(transport.calls.some((c) => c.name === 'sources_add_folder')).toBe(false);
  });

  it('adds with the probe suggestions and the label, then reloads', async () => {
    const transport = installTransport();
    await startAdd('/mods');
    expect(await confirmAdd('  My mods ')).toBe(true);
    const add = transport.calls.find((c) => c.name === 'sources_add_folder');
    expect(add?.request).toEqual({
      path: '/mods',
      label: 'My mods',
      layout: 'mods-root',
      scanDepth: 1,
    });
    expect(pending.value).toBeUndefined();
    expect(adding.value).toBe(false);
    expect(transport.calls.at(-2)?.name).toBe('sources_list');
  });

  it('refuses to add a folder the probe blocked', async () => {
    const blocked = { ...loadFixture<SourcesProbeFolderResponse>('probe-overlap') };
    const transport = installTransport({ sources_probe_folder: () => blocked });
    await startAdd('/workshop');
    expect(pending.value?.probe?.canSave).toBe(false);
    expect(await confirmAdd('')).toBe(false);
    expect(transport.calls.some((c) => c.name === 'sources_add_folder')).toBe(false);
  });

  it('keeps the dialog open with the error when the add fails', async () => {
    installTransport({
      sources_add_folder: () => {
        throw { code: 'deploy.source-overlap', message: 'overlap', errorId: 'e2' };
      },
    });
    await startAdd('/mods');
    expect(await confirmAdd('')).toBe(false);
    expect(pending.value?.error?.code).toBe('deploy.source-overlap');
  });

  it('sends label, enabled, order and remove requests', async () => {
    const transport = installTransport();
    await renameSource('cf_1', 'New');
    await setSourceEnabled('cf_1', false);
    await moveSource('cf_1', 0);
    await forgetSource('cf_1');
    const sent = transport.calls.filter(
      (c) => c.name !== 'sources_list' && c.name !== 'settings_get',
    );
    expect(sent.map((c) => c.request)).toEqual([
      { id: 'cf_1', label: 'New' },
      { id: 'cf_1', enabled: false },
      { id: 'cf_1', order: 0 },
      { id: 'cf_1' },
    ]);
  });
});
