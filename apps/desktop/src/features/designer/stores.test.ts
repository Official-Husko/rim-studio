import { describe, expect, it } from 'vitest';
import { createDesigner } from './stores';
import {
  cloneEntry,
  installTransport,
  manualScheduler,
  openWeaponAnswer,
  settle,
} from './testSupport';

const DAMAGE = '/ranged/damage';

function setup() {
  const transport = installTransport();
  const clock = manualScheduler();
  const stores = createDesigner(() => 'p-1', clock.scheduler);
  const saves = () => transport.calls.filter((c) => c.name === 'designer_draft_save');
  return { transport, clock, stores, saves };
}

describe('createDesigner: weapons of the project', () => {
  it('replaces the open draft by the weapon read again and drops its unsaved edits', async () => {
    const { clock, stores, saves } = setup();
    const entry = openWeaponAnswer().entry;
    stores.editor.open(entry);
    stores.editor.setField(DAMAGE, { value: 30, source: 'typed' });
    expect(stores.editor.saveState.value).toBe('dirty');

    const reread = { ...entry, updatedAtMs: entry.updatedAtMs + 5 };
    await stores.reopen(reread);

    expect(stores.editor.draft.value).toBe(reread.draft);
    expect(stores.editor.entryId.value).toBe(entry.id);
    expect(stores.editor.saveState.value).toBe('saved');
    expect(stores.drafts.selectedId.value).toBe(entry.id);
    // neither the explicit save nor the one the idle timer would have made writes the old edits
    clock.runTimers();
    await settle();
    expect(saves()).toHaveLength(0);
  });

  it('selects another draft the usual way, which saves the open one first', async () => {
    const { stores, saves } = setup();
    stores.editor.open(openWeaponAnswer().entry);
    stores.editor.setField(DAMAGE, { value: 30, source: 'typed' });

    await stores.reopen(cloneEntry());

    expect(saves()).toHaveLength(1);
    expect(stores.editor.entryId.value).toBe('d-clone');
  });

  it('forgets the failure of reading a file again when another draft is selected', async () => {
    const { stores } = setup();
    stores.weapons.reloadError.value = { code: 'x.y', message: 'old', errorId: 'e-1' };
    await stores.select(cloneEntry());
    expect(stores.weapons.reloadError.value).toBeUndefined();
  });

  it('forgets the weapons of the project with the rest on reset', async () => {
    const { stores } = setup();
    await stores.weapons.load();
    expect(stores.weapons.weapons.value).toHaveLength(3);
    stores.editor.open(openWeaponAnswer().entry);

    stores.reset();

    expect(stores.weapons.weapons.value).toEqual([]);
    expect(stores.weapons.loaded.value).toBe(false);
    expect(stores.editor.draft.value).toBeUndefined();
  });
});
