import { signal } from '@preact/signals';
import type { ApiError, SourceDto, SourcesProbeFolderResponse } from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import { addFolder, listSources, probeFolder, removeSource, updateSource } from './api';
import { loadSettings } from './settingsStore';

export const sources = signal<readonly SourceDto[]>([]);
export const sourcesLoaded = signal(false);
export const sourcesError = signal<ApiError | undefined>(undefined);

/** The folder being added: what the user picked and what the probe said about it. */
export interface PendingFolder {
  path: string;
  probe?: SourcesProbeFolderResponse;
  error?: ApiError;
  probing: boolean;
}
export const pending = signal<PendingFolder | undefined>(undefined);
export const adding = signal(false);

/** Read the list from the backend. */
export async function loadSources(): Promise<void> {
  try {
    sources.value = await listSources();
    sourcesError.value = undefined;
    // custom folders carry their drive hint in the settings
    await loadSettings();
  } catch (thrown) {
    sourcesError.value = normalizeError(thrown);
  } finally {
    sourcesLoaded.value = true;
  }
}

/** Start the add flow: probe the picked folder; nothing is added yet. */
export async function startAdd(path: string): Promise<void> {
  pending.value = { path, probing: true };
  try {
    const probe = await probeFolder(path);
    if (pending.value?.path === path) pending.value = { path, probe, probing: false };
  } catch (thrown) {
    if (pending.value?.path === path) {
      pending.value = { path, error: normalizeError(thrown), probing: false };
    }
  }
}

/** Close the add dialog without adding. */
export function cancelAdd(): void {
  pending.value = undefined;
}

/** Add the pending folder with the probe suggestions. Returns true when it was added. */
export async function confirmAdd(label: string): Promise<boolean> {
  const current = pending.value;
  if (!current?.probe?.canSave) return false;
  adding.value = true;
  try {
    const trimmed = label.trim();
    await addFolder({
      path: current.path,
      ...(trimmed ? { label: trimmed } : {}),
      layout: current.probe.suggestedLayout,
      scanDepth: current.probe.suggestedDepth,
    });
    pending.value = undefined;
    await loadSources();
    return true;
  } catch (thrown) {
    pending.value = { ...current, error: normalizeError(thrown) };
    return false;
  } finally {
    adding.value = false;
  }
}

async function change(run: () => Promise<unknown>): Promise<void> {
  try {
    await run();
    sourcesError.value = undefined;
  } catch (thrown) {
    sourcesError.value = normalizeError(thrown);
  }
  await loadSources();
}

/** Rename a source. */
export const renameSource = (id: string, label: string): Promise<void> =>
  change(() => updateSource({ id, label }));

/** Take a source in or out of scans. */
export const setSourceEnabled = (id: string, enabled: boolean): Promise<void> =>
  change(() => updateSource({ id, enabled }));

/** Move a source to a zero based position of the scan order. */
export const moveSource = (id: string, order: number): Promise<void> =>
  change(() => updateSource({ id, order }));

/** Forget a custom folder. The backend never touches its files. */
export const forgetSource = (id: string): Promise<void> => change(() => removeSource(id));

/** Forget everything (tests, session change). */
export function resetSources(): void {
  sources.value = [];
  sourcesLoaded.value = false;
  sourcesError.value = undefined;
  pending.value = undefined;
  adding.value = false;
}
