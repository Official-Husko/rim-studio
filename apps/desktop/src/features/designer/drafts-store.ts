import { signal } from '@preact/signals';
import type { ApiError, DraftEntryDto, ItemKindDto } from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import * as api from './api';
import { newDraft } from './model/draft';

export interface DraftsDeps {
  projectId: () => string | undefined;
}

/** The drafts of the current project: the list, the selection and the commands that change it. */
export function createDraftsStore(deps: DraftsDeps) {
  const entries = signal<DraftEntryDto[]>([]);
  const selectedId = signal<string | undefined>(undefined);
  const loading = signal(false);
  const error = signal<ApiError | undefined>(undefined);
  const notes = signal<string[]>([]);
  /** The draft the notes belong to: the notes of a clone are shown with that draft only. */
  const notesFor = signal<string | undefined>(undefined);

  const guard = async <T>(work: (projectId: string) => Promise<T>): Promise<T | undefined> => {
    const projectId = deps.projectId();
    if (!projectId) return undefined;
    try {
      const result = await work(projectId);
      error.value = undefined;
      return result;
    } catch (thrown) {
      error.value = normalizeError(thrown);
      return undefined;
    }
  };

  /** Insert or replace an entry (after a save), keeping the newest first. */
  function upsert(entry: DraftEntryDto): void {
    const rest = entries.peek().filter((e) => e.id !== entry.id);
    entries.value = [entry, ...rest].sort((a, b) => b.updatedAtMs - a.updatedAtMs);
  }

  /** Load the list from the backend. */
  async function load(): Promise<void> {
    loading.value = true;
    const list = await guard((projectId) => api.draftList(projectId));
    if (list) entries.value = list.drafts;
    loading.value = false;
  }

  /** Store a new empty draft of a kind and return its entry. */
  async function create(
    kind: ItemKindDto,
    defName: string,
    label: string,
  ): Promise<DraftEntryDto | undefined> {
    return guard(async (projectId) => {
      const draft = newDraft(kind, defName, label);
      const saved = await api.draftSave(projectId, draft);
      const entry: DraftEntryDto = {
        id: saved.id,
        defName,
        label,
        kind,
        updatedAtMs: saved.savedAtMs,
        draft,
      };
      upsert(entry);
      return entry;
    });
  }

  /** Clone a reference weapon into a new draft and return its entry. */
  async function clone(
    source: string,
    defName: string,
    label?: string,
    ownProjectile?: boolean,
  ): Promise<DraftEntryDto | undefined> {
    return guard(async (projectId) => {
      const response = await api.cloneWeapon(projectId, source, defName, label, ownProjectile);
      upsert(response.entry);
      notes.value = response.notes ?? [];
      notesFor.value = response.entry.id;
      return response.entry;
    });
  }

  /** Delete a stored draft. */
  async function remove(id: string): Promise<boolean> {
    const deleted = await guard((projectId) => api.draftDelete(projectId, id));
    if (deleted) {
      entries.value = entries.peek().filter((e) => e.id !== id);
      if (selectedId.peek() === id) selectedId.value = undefined;
    }
    return deleted === true;
  }

  /** Forget everything (project change, tests). */
  function reset(): void {
    entries.value = [];
    selectedId.value = undefined;
    error.value = undefined;
    notes.value = [];
    notesFor.value = undefined;
  }

  return {
    entries,
    selectedId,
    loading,
    error,
    notes,
    notesFor,
    upsert,
    load,
    create,
    clone,
    remove,
    reset,
  };
}

export type DraftsStore = ReturnType<typeof createDraftsStore>;
