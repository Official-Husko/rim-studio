import { computed, signal } from '@preact/signals';
import type {
  ApiError,
  LoadFoldersChangeDto,
  ProjectLoadFoldersDto,
  ProjectLoadFoldersUpdateDto,
} from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import { bumpProjectRevision } from '~/shared/project';
import { loadFoldersGet, loadFoldersUpdate } from '../modApi';

// State of the Versions and folders tab. The file is read by position, so the edits are kept as the
// ordered list of operations the backend takes, and every operation is checked with a dry run: the tab
// shows the dry run's answer, which is the file as it will be after the save.

export type FoldersLoadState = 'idle' | 'loading' | 'ready' | 'error';

export const foldersState = signal<FoldersLoadState>('idle');
export const foldersError = signal<ApiError | undefined>(undefined);
/** The file as the backend last read or wrote it. */
export const foldersModel = signal<ProjectLoadFoldersDto | undefined>(undefined);
export const foldersOps = signal<LoadFoldersChangeDto[]>([]);
/** True when the person asked to create the file (it does not exist yet). */
export const foldersCreate = signal(false);
/** The dry run of the current edits. */
export const foldersPreview = signal<ProjectLoadFoldersUpdateDto | undefined>(undefined);
export const foldersBusy = signal(false);
export const foldersEditError = signal<ApiError | undefined>(undefined);
export const foldersSaving = signal(false);
export const foldersStale = signal(false);
export const foldersLastSave = signal<ProjectLoadFoldersUpdateDto | undefined>(undefined);

/** What the tab shows: the file after the edits when there are any, else the file. */
export const foldersShown = computed<ProjectLoadFoldersDto | undefined>(
  () => foldersPreview.value?.loadFolders ?? foldersModel.value,
);

/** How many edits are unsaved; creating the file counts as one. */
export const foldersPending = computed(
  () => foldersOps.value.length + (foldersCreate.value && foldersOps.value.length === 0 ? 1 : 0),
);

let projectId: string | undefined;
let loadToken = 0;

/** Read LoadFolders.xml for a project. With `keepEdits` the edits are checked again against the new file. */
export async function loadFolders(id: string, keepEdits = false): Promise<void> {
  if (projectId !== id) {
    resetFoldersStore();
    projectId = id;
  }
  loadToken += 1;
  const token = loadToken;
  foldersState.value = 'loading';
  try {
    const model = await loadFoldersGet(id);
    if (token !== loadToken) return;
    foldersModel.value = model;
    foldersError.value = undefined;
    foldersStale.value = false;
    foldersEditError.value = undefined;
    foldersState.value = 'ready';
    if (!keepEdits) clearEdits();
    else await dryRun();
  } catch (thrown) {
    if (token !== loadToken) return;
    foldersError.value = normalizeError(thrown);
    foldersState.value = 'error';
  }
}

function clearEdits(): void {
  foldersOps.value = [];
  foldersCreate.value = false;
  foldersPreview.value = undefined;
  foldersEditError.value = undefined;
}

async function dryRun(): Promise<boolean> {
  const model = foldersModel.peek();
  if (!model || !projectId) return false;
  const ops = foldersOps.peek();
  const create = foldersCreate.peek();
  if (ops.length === 0 && !create) {
    foldersPreview.value = undefined;
    return true;
  }
  foldersBusy.value = true;
  try {
    foldersPreview.value = await loadFoldersUpdate(projectId, ops, {
      create: create && !model.exists,
      dryRun: true,
      ...(model.fileHash ? { expectedHash: model.fileHash } : {}),
    });
    foldersEditError.value = undefined;
    return true;
  } catch (thrown) {
    const error = normalizeError(thrown);
    if (error.code === 'project.file-stale') foldersStale.value = true;
    foldersEditError.value = error;
    return false;
  } finally {
    foldersBusy.value = false;
  }
}

/** Add one edit and check it. An edit the backend refuses is dropped and its reason shown. */
export async function applyFoldersOp(op: LoadFoldersChangeDto): Promise<boolean> {
  if (foldersBusy.peek()) return false;
  const before = foldersOps.peek();
  foldersOps.value = [...before, op];
  const ok = await dryRun();
  if (!ok) foldersOps.value = before;
  return ok;
}

/** Ask to create the file; the dry run shows the empty file the save would write. */
export async function startFoldersCreate(): Promise<void> {
  foldersCreate.value = true;
  await dryRun();
}

/** Throw the edits away. */
export function discardFolders(): void {
  clearEdits();
}

/** Write the edits. Returns true when the file was written. */
export async function saveFolders(): Promise<boolean> {
  const model = foldersModel.peek();
  if (!model || !projectId) return false;
  foldersSaving.value = true;
  foldersEditError.value = undefined;
  try {
    const result = await loadFoldersUpdate(projectId, foldersOps.peek(), {
      create: foldersCreate.peek() && !model.exists,
      dryRun: false,
      ...(model.fileHash ? { expectedHash: model.fileHash } : {}),
    });
    foldersModel.value = result.loadFolders;
    clearEdits();
    foldersStale.value = false;
    foldersLastSave.value = result;
    bumpProjectRevision();
    return true;
  } catch (thrown) {
    const error = normalizeError(thrown);
    if (error.code === 'project.file-stale') foldersStale.value = true;
    foldersEditError.value = error;
    return false;
  } finally {
    foldersSaving.value = false;
  }
}

/** Forget everything (tests, a different project). */
export function resetFoldersStore(): void {
  loadToken += 1;
  projectId = undefined;
  foldersState.value = 'idle';
  foldersError.value = undefined;
  foldersModel.value = undefined;
  foldersBusy.value = false;
  foldersSaving.value = false;
  foldersStale.value = false;
  foldersLastSave.value = undefined;
  clearEdits();
}
