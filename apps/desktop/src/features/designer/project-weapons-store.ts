import { batch, signal } from '@preact/signals';
import type { ApiError, DiagnosticDto, DraftEntryDto, ProjectWeaponDto } from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import * as api from './api';
import type { DraftsStore } from './drafts-store';

export interface ProjectWeaponsDeps {
  /** The id of the current project; undefined when none is open. */
  projectId: () => string | undefined;
  /** The drafts of the session: an opened weapon is stored here and its notes are shown with it. */
  drafts: DraftsStore;
}

/**
 * The weapons the open project defines in its own files, and the way to load one of them into
 * the editor. The list is the answer of the backend; the store only keeps it current: it is read
 * again when the project or its files change, and the row of a weapon learns the id of its draft
 * the moment the weapon was opened.
 */
export function createProjectWeaponsStore(deps: ProjectWeaponsDeps) {
  const { drafts } = deps;
  const weapons = signal<ProjectWeaponDto[]>([]);
  /** Problems found while reading the project (a file that cannot be read). */
  const diagnostics = signal<DiagnosticDto[]>([]);
  const loading = signal(false);
  /** True once a list arrived for the current project; tells an empty mod from a list not read yet. */
  const loaded = signal(false);
  /** The last failure of reading the list or of opening a weapon from it. */
  const error = signal<ApiError | undefined>(undefined);
  /** The last failure of reading a weapon's file again, shown with the editing bar, not the list. */
  const reloadError = signal<ApiError | undefined>(undefined);
  /** The definition name being opened, for the busy state of its row. */
  const opening = signal<string | undefined>(undefined);
  let loadSeq = 0;
  let openSeq = 0;

  /** The stored draft of a weapon as the drafts of the session hold it; undefined when none. */
  function draftOf(weapon: ProjectWeaponDto): DraftEntryDto | undefined {
    if (weapon.draftId === undefined) return undefined;
    return drafts.entries.value.find((entry) => entry.id === weapon.draftId);
  }

  /**
   * The definition name a draft was opened from. The editor can rename the weapon after that, so
   * reading the file again must ask for the name the file holds.
   */
  function defNameOfDraft(draftId: string | undefined): string | undefined {
    if (draftId === undefined) return undefined;
    return weapons.value.find((weapon) => weapon.draftId === draftId)?.defName;
  }

  /** Read the list from the project; an older answer never replaces a newer one. */
  async function load(): Promise<void> {
    const projectId = deps.projectId();
    if (!projectId) return;
    const mine = ++loadSeq;
    loading.value = true;
    try {
      const response = await api.projectWeapons(projectId);
      if (mine !== loadSeq || deps.projectId() !== projectId) return;
      batch(() => {
        weapons.value = response.weapons;
        diagnostics.value = response.diagnostics;
        loaded.value = true;
        error.value = undefined;
      });
    } catch (thrown) {
      if (mine === loadSeq) error.value = normalizeError(thrown);
    } finally {
      if (mine === loadSeq) loading.value = false;
    }
  }

  /**
   * Store the draft that edits a weapon of the project and return its entry. A draft of the weapon
   * that the session already holds is continued as it is; `reload` reads the file again and replaces
   * the stored draft (same id). A failure is kept in `error`, or in `reloadError` for a reload.
   */
  async function open(defName: string, reload = false): Promise<DraftEntryDto | undefined> {
    const projectId = deps.projectId();
    if (!projectId || opening.peek() !== undefined) return undefined;
    if (!reload) {
      const row = weapons.peek().find((weapon) => weapon.defName === defName);
      const known = row ? draftOf(row) : undefined;
      if (known) return known;
    }
    const mine = ++openSeq;
    opening.value = defName;
    try {
      const response = await api.openWeapon(projectId, defName, reload ? true : undefined);
      if (mine !== openSeq || deps.projectId() !== projectId) return undefined;
      const entry = response.entry;
      batch(() => {
        drafts.upsert(entry);
        drafts.notes.value = response.notes ?? [];
        drafts.notesFor.value = entry.id;
        weapons.value = weapons
          .peek()
          .map((weapon) =>
            weapon.defName === defName && weapon.draftId !== entry.id
              ? { ...weapon, draftId: entry.id }
              : weapon,
          );
        if (reload) reloadError.value = undefined;
        else error.value = undefined;
      });
      // a list that was being read when the weapon was opened may not know the draft yet
      if (loading.peek()) void load();
      return entry;
    } catch (thrown) {
      if (mine === openSeq) {
        if (reload) reloadError.value = normalizeError(thrown);
        else error.value = normalizeError(thrown);
      }
      return undefined;
    } finally {
      if (mine === openSeq) opening.value = undefined;
    }
  }

  /** Forget the list and the failures (the project changed, tests). */
  function reset(): void {
    loadSeq += 1;
    openSeq += 1;
    batch(() => {
      weapons.value = [];
      diagnostics.value = [];
      loading.value = false;
      loaded.value = false;
      error.value = undefined;
      reloadError.value = undefined;
      opening.value = undefined;
    });
  }

  return {
    weapons,
    diagnostics,
    loading,
    loaded,
    error,
    reloadError,
    opening,
    draftOf,
    defNameOfDraft,
    load,
    open,
    reset,
  };
}

export type ProjectWeaponsStore = ReturnType<typeof createProjectWeaponsStore>;
