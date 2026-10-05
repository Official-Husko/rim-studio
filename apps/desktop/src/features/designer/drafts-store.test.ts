import { describe, expect, it } from 'vitest';
import type { DraftEntryDto } from 'rimstudio-ipc-types';
import { createDraftsStore } from './drafts-store';
import { cloneEntry, fixture, installTransport } from './testSupport';

describe('drafts store', () => {
  it('loads the list and orders new saves first', async () => {
    const older: DraftEntryDto = { ...cloneEntry(), id: 'd-old', updatedAtMs: 1 };
    installTransport({ designer_draft_list: () => ({ drafts: [older] }) });
    const store = createDraftsStore({ projectId: () => 'p-1' });
    await store.load();
    expect(store.entries.value.map((e) => e.id)).toEqual(['d-old']);
    store.upsert({ ...older, id: 'd-new', updatedAtMs: 5 });
    expect(store.entries.value.map((e) => e.id)).toEqual(['d-new', 'd-old']);
  });

  it('creates an empty draft of a kind', async () => {
    installTransport();
    const store = createDraftsStore({ projectId: () => 'p-1' });
    const entry = await store.create('melee', 'TM_Axe', 'axe');
    expect(entry?.draft.kind).toBe('melee');
    expect(store.entries.value[0]?.defName).toBe('TM_Axe');
  });

  it('clones a reference weapon and keeps the notes', async () => {
    installTransport();
    const store = createDraftsStore({ projectId: () => 'p-1' });
    const entry = await store.clone('Gun_BoltActionRifle', 'TM_Fixture', 'fixture rifle');
    expect(entry?.draft.clonedFrom).toBe('Gun_BoltActionRifle');
    expect(store.notes.value.length).toBeGreaterThan(0);
  });

  it('clones with the own projectile choice and ties the notes to the new draft', async () => {
    const transport = installTransport();
    const store = createDraftsStore({ projectId: () => 'p-1' });
    const entry = await store.clone('Gun_BoltActionRifle', 'TM_Fixture', 'fixture rifle', false);
    const sent = transport.calls.find((c) => c.name === 'designer_clone')?.request as {
      ownProjectile?: boolean;
    };
    expect(sent.ownProjectile).toBe(false);
    expect(store.notesFor.value).toBe(entry?.id);
  });

  it('leaves the choice out of the request when it is not given', async () => {
    const transport = installTransport();
    const store = createDraftsStore({ projectId: () => 'p-1' });
    await store.clone('Gun_BoltActionRifle', 'TM_Fixture');
    const sent = transport.calls.find((c) => c.name === 'designer_clone')?.request as object;
    expect('ownProjectile' in sent).toBe(false);
  });

  it('shows the backend error of a refused name', async () => {
    installTransport({
      designer_clone: () => {
        throw { code: 'designer.invalid-draft', message: 'name in use', errorId: 'e-3' };
      },
    });
    const store = createDraftsStore({ projectId: () => 'p-1' });
    expect(await store.clone('Gun_BoltActionRifle', 'Gun_Revolver')).toBeUndefined();
    expect(store.error.value?.code).toBe('designer.invalid-draft');
  });

  it('deletes a draft and clears the selection', async () => {
    installTransport({ designer_draft_delete: () => ({ deleted: true }) });
    const store = createDraftsStore({ projectId: () => 'p-1' });
    store.upsert(fixture('designer_clone') as never as DraftEntryDto);
    store.upsert(cloneEntry());
    store.selectedId.value = 'd-clone';
    expect(await store.remove('d-clone')).toBe(true);
    expect(store.entries.value.some((e) => e.id === 'd-clone')).toBe(false);
    expect(store.selectedId.value).toBeUndefined();
  });

  it('does nothing without a project', async () => {
    const transport = installTransport();
    const store = createDraftsStore({ projectId: () => undefined });
    await store.load();
    expect(transport.calls).toEqual([]);
  });
});
