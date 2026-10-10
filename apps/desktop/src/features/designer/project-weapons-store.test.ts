import { describe, expect, it } from 'vitest';
import type {
  DesignerOpenWeaponResponse,
  DesignerProjectWeaponsResponse,
} from 'rimstudio-ipc-types';
import { createDraftsStore } from './drafts-store';
import { createProjectWeaponsStore } from './project-weapons-store';
import { installTransport, openWeaponAnswer, projectWeaponsAnswer, settle } from './testSupport';

const RANGER = 'RS_Gun_Ranger';
const RANGER_DRAFT = 'd-rs-ranger';

function setup(projectId: () => string | undefined = () => 'p-1') {
  const drafts = createDraftsStore({ projectId });
  const store = createProjectWeaponsStore({ projectId, drafts });
  return { drafts, store };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

/** The list of the fixture where the first weapon already has a stored draft. */
function listWithDraft(): DesignerProjectWeaponsResponse {
  const answer = projectWeaponsAnswer();
  const first = answer.weapons[0];
  if (first) first.draftId = RANGER_DRAFT;
  return answer;
}

describe('project weapons store: the list', () => {
  it('loads the weapons and the problems of the project', async () => {
    const transport = installTransport({
      designer_project_weapons: () => ({
        ...projectWeaponsAnswer(),
        diagnostics: [
          { code: 'designer.def-file-unreadable', severity: 'warning', message: 'Broken.xml' },
        ],
      }),
    });
    const { store } = setup();
    expect(store.loaded.value).toBe(false);
    const pending = store.load();
    expect(store.loading.value).toBe(true);
    await pending;
    expect(store.loading.value).toBe(false);
    expect(store.loaded.value).toBe(true);
    expect(store.weapons.value.map((w) => w.defName)).toEqual([
      RANGER,
      'RS_Gun_Scatter',
      'RS_Melee_Cleaver',
    ]);
    expect(store.diagnostics.value.map((d) => d.code)).toEqual(['designer.def-file-unreadable']);
    expect(transport.calls[0]).toEqual({
      name: 'designer_project_weapons',
      request: { projectId: 'p-1' },
    });
  });

  it('asks for nothing without a project', async () => {
    const transport = installTransport();
    const { store } = setup(() => undefined);
    await store.load();
    expect(await store.open(RANGER)).toBeUndefined();
    expect(transport.calls).toEqual([]);
  });

  it('keeps the failure of a load, and the list it had, until a load works', async () => {
    let fail = false;
    installTransport({
      designer_project_weapons: () => {
        if (fail) {
          throw { code: 'designer.reference-unavailable', message: 'no install', errorId: 'e-1' };
        }
        return projectWeaponsAnswer();
      },
    });
    const { store } = setup();
    await store.load();
    fail = true;
    await store.load();
    expect(store.error.value?.code).toBe('designer.reference-unavailable');
    expect(store.loading.value).toBe(false);
    expect(store.weapons.value).toHaveLength(3);
    fail = false;
    await store.load();
    expect(store.error.value).toBeUndefined();
  });

  it('lets the newest answer win even when an older one arrives last', async () => {
    const older = deferred<DesignerProjectWeaponsResponse>();
    const answers = [older.promise, Promise.resolve(projectWeaponsAnswer())];
    installTransport({ designer_project_weapons: () => answers.shift() });
    const { store } = setup();
    const first = store.load();
    await store.load();
    older.resolve({ weapons: [], diagnostics: [] });
    await first;
    expect(store.weapons.value).toHaveLength(3);
    expect(store.loading.value).toBe(false);
  });

  it('drops an answer that arrives after the project changed', async () => {
    const answer = deferred<DesignerProjectWeaponsResponse>();
    installTransport({ designer_project_weapons: () => answer.promise });
    let project = 'p-1';
    const { store } = setup(() => project);
    const pending = store.load();
    project = 'p-2';
    answer.resolve(projectWeaponsAnswer());
    await pending;
    expect(store.weapons.value).toEqual([]);
    expect(store.loaded.value).toBe(false);
  });

  it('forgets the list and the failures on reset, and an answer still on its way', async () => {
    const answer = deferred<DesignerProjectWeaponsResponse>();
    let wait = false;
    installTransport({
      designer_project_weapons: () => (wait ? answer.promise : projectWeaponsAnswer()),
    });
    const { store } = setup();
    await store.load();
    store.error.value = { code: 'x.y', message: 'no', errorId: 'e-3' };
    wait = true;
    const pending = store.load();
    store.reset();
    answer.resolve(projectWeaponsAnswer());
    await pending;
    expect(store.weapons.value).toEqual([]);
    expect(store.loaded.value).toBe(false);
    expect(store.loading.value).toBe(false);
    expect(store.error.value).toBeUndefined();
  });
});

describe('project weapons store: opening a weapon', () => {
  it('stores the draft, keeps the notes with it and gives the row its draft id', async () => {
    const transport = installTransport();
    const { drafts, store } = setup();
    await store.load();
    const entry = await store.open(RANGER);
    expect(transport.calls.at(-1)).toEqual({
      name: 'designer_open_weapon',
      request: { projectId: 'p-1', defName: RANGER },
    });
    expect(entry?.draft.editsFile).toBe('Defs/ThingDefs_Weapons/RS_Weapons.xml');
    expect(drafts.entries.value.map((e) => e.id)).toEqual([RANGER_DRAFT]);
    expect(drafts.notesFor.value).toBe(RANGER_DRAFT);
    expect(drafts.notes.value).toEqual(openWeaponAnswer().notes);
    const row = store.weapons.value.find((w) => w.defName === RANGER);
    expect(row?.draftId).toBe(RANGER_DRAFT);
    expect(row && store.draftOf(row)?.id).toBe(RANGER_DRAFT);
    expect(store.opening.value).toBeUndefined();
    expect(store.error.value).toBeUndefined();
  });

  it('marks the weapon as busy and makes the others wait', async () => {
    const answer = deferred<DesignerOpenWeaponResponse>();
    const transport = installTransport({ designer_open_weapon: () => answer.promise });
    const { store } = setup();
    await store.load();
    const pending = store.open(RANGER);
    expect(store.opening.value).toBe(RANGER);
    expect(await store.open('RS_Melee_Cleaver')).toBeUndefined();
    expect(transport.calls.filter((c) => c.name === 'designer_open_weapon')).toHaveLength(1);
    answer.resolve(openWeaponAnswer());
    await pending;
    expect(store.opening.value).toBeUndefined();
  });

  it('continues a draft the session holds as it is, without asking the backend', async () => {
    const stored = openWeaponAnswer().entry;
    const transport = installTransport({ designer_project_weapons: listWithDraft });
    const { drafts, store } = setup();
    drafts.upsert(stored);
    await store.load();
    const entry = await store.open(RANGER);
    expect(entry).toBe(stored);
    expect(transport.calls.some((c) => c.name === 'designer_open_weapon')).toBe(false);
  });

  it('shows no draft for a row whose draft the session no longer holds', async () => {
    installTransport({ designer_project_weapons: listWithDraft });
    const { drafts, store } = setup();
    drafts.upsert(openWeaponAnswer().entry);
    await store.load();
    const row = store.weapons.value[0];
    expect(row?.draftId).toBe(RANGER_DRAFT);
    expect(row && store.draftOf(row)).toBeDefined();
    // the draft was deleted from the list of drafts: the row follows without another read
    drafts.entries.value = [];
    expect(row && store.draftOf(row)).toBeUndefined();
  });

  it('takes the draft the backend continues when the session does not know it', async () => {
    const continued: DesignerOpenWeaponResponse = { ...openWeaponAnswer(), reused: true };
    delete continued.notes;
    installTransport({ designer_open_weapon: () => continued });
    const { drafts, store } = setup();
    drafts.notes.value = ['an older note'];
    await store.load();
    const entry = await store.open(RANGER);
    expect(entry?.id).toBe(RANGER_DRAFT);
    expect(drafts.entries.value).toHaveLength(1);
    expect(drafts.notes.value).toEqual([]);
  });

  it('reads the file again when asked and replaces the stored draft of the same id', async () => {
    const reread = openWeaponAnswer();
    reread.entry = { ...reread.entry, updatedAtMs: 9_999_999_999_999 };
    reread.notes = ['read again'];
    const transport = installTransport({
      designer_project_weapons: listWithDraft,
      designer_open_weapon: () => reread,
    });
    const { drafts, store } = setup();
    drafts.upsert(openWeaponAnswer().entry);
    await store.load();
    const entry = await store.open(RANGER, true);
    expect(transport.calls.at(-1)?.request).toEqual({
      projectId: 'p-1',
      defName: RANGER,
      reload: true,
    });
    expect(entry?.updatedAtMs).toBe(9_999_999_999_999);
    expect(drafts.entries.value).toHaveLength(1);
    expect(drafts.entries.value[0]?.updatedAtMs).toBe(9_999_999_999_999);
    expect(drafts.notes.value).toEqual(['read again']);
    expect(drafts.notesFor.value).toBe(RANGER_DRAFT);
  });

  it('keeps the failure of opening apart from the failure of reading the file again', async () => {
    installTransport({
      designer_open_weapon: () => {
        throw { code: 'designer.invalid-draft', message: 'not a weapon', errorId: 'e-2' };
      },
    });
    const { store } = setup();
    await store.load();
    expect(await store.open(RANGER)).toBeUndefined();
    expect(store.error.value?.code).toBe('designer.invalid-draft');
    expect(store.reloadError.value).toBeUndefined();
    expect(store.opening.value).toBeUndefined();
    expect(await store.open(RANGER, true)).toBeUndefined();
    expect(store.reloadError.value?.message).toBe('not a weapon');
  });

  it('ignores an opened weapon when the project changed meanwhile', async () => {
    const answer = deferred<DesignerOpenWeaponResponse>();
    installTransport({ designer_open_weapon: () => answer.promise });
    let project = 'p-1';
    const { drafts, store } = setup(() => project);
    await store.load();
    const pending = store.open(RANGER);
    project = 'p-2';
    answer.resolve(openWeaponAnswer());
    expect(await pending).toBeUndefined();
    expect(drafts.entries.value).toEqual([]);
  });

  it('reads the list again when one was being read while the weapon was opened', async () => {
    const stale = deferred<DesignerProjectWeaponsResponse>();
    const answers = [Promise.resolve(projectWeaponsAnswer()), stale.promise, listWithDraft()];
    const transport = installTransport({ designer_project_weapons: () => answers.shift() });
    const { store } = setup();
    await store.load();
    const inFlight = store.load();
    await store.open(RANGER);
    await settle();
    stale.resolve(projectWeaponsAnswer());
    await inFlight;
    await settle();
    expect(transport.calls.filter((c) => c.name === 'designer_project_weapons')).toHaveLength(3);
    expect(store.weapons.value[0]?.draftId).toBe(RANGER_DRAFT);
    expect(store.loading.value).toBe(false);
  });

  it('knows which weapon a draft was opened from', async () => {
    installTransport();
    const { store } = setup();
    await store.load();
    await store.open(RANGER);
    expect(store.defNameOfDraft(RANGER_DRAFT)).toBe(RANGER);
    expect(store.defNameOfDraft('d-other')).toBeUndefined();
    expect(store.defNameOfDraft(undefined)).toBeUndefined();
  });
});
