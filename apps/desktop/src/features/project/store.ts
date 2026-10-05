import { signal } from '@preact/signals';
import type {
  ApiError,
  ProjectCreateRequest,
  ProjectFileDto,
  ProjectLayoutCheckDto,
  ProjectScaffoldMissingDto,
  ProjectSummaryDto,
  ProjectTreeDto,
} from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import { openProjectAt, refOf, setCurrentProject, type ProjectRef } from '~/shared/project';
import {
  checkLayout,
  createProject,
  openProject,
  readProjectFile,
  readTree,
  scaffoldMissing,
} from './api';

/** Everything the page shows about the current project. */
export interface ProjectView {
  summary: ProjectSummaryDto;
  tree: ProjectTreeDto;
  check: ProjectLayoutCheckDto;
}

export type LoadState = 'idle' | 'loading' | 'ready' | 'error';

export const view = signal<ProjectView | undefined>(undefined);
export const loadState = signal<LoadState>('idle');
export const loadError = signal<ApiError | undefined>(undefined);

/** The file shown in the viewer. */
export interface FileView {
  path: string;
  loading: boolean;
  file?: ProjectFileDto;
  error?: ApiError;
}
export const selectedPath = signal<string | undefined>(undefined);
export const fileView = signal<FileView | undefined>(undefined);

/** State of the open and create flows. */
export const opening = signal(false);
export const openError = signal<ApiError | undefined>(undefined);
export const creating = signal(false);
export const createError = signal<ApiError | undefined>(undefined);

/** State of the Fix button of the layout panel. */
export const fixing = signal(false);
export const fixResult = signal<ProjectScaffoldMissingDto | undefined>(undefined);
export const fixError = signal<ApiError | undefined>(undefined);

let loadToken = 0;
let fileToken = 0;

/** Read the summary, the tree and the layout check of a project. Stale answers are dropped. */
export async function loadProject(ref: ProjectRef): Promise<void> {
  loadToken += 1;
  const token = loadToken;
  const sameProject = view.peek()?.summary.projectId === ref.projectId;
  if (!sameProject) {
    view.value = undefined;
    selectedPath.value = undefined;
    fileView.value = undefined;
    fixResult.value = undefined;
    fixError.value = undefined;
  }
  loadState.value = 'loading';
  try {
    const summary = await openProject(ref.path);
    const [tree, check] = await Promise.all([
      readTree(summary.projectId),
      checkLayout(summary.projectId),
    ]);
    if (token !== loadToken) return;
    view.value = { summary, tree, check };
    loadError.value = undefined;
    loadState.value = 'ready';
    const path = selectedPath.peek();
    if (path && sameProject) void showFile(path);
  } catch (thrown) {
    if (token !== loadToken) return;
    loadError.value = normalizeError(thrown);
    loadState.value = 'error';
  }
}

/** Show a file of the project in the viewer. */
export async function showFile(path: string): Promise<void> {
  const current = view.peek();
  if (!current) return;
  selectedPath.value = path;
  fileToken += 1;
  const token = fileToken;
  fileView.value = { path, loading: true };
  try {
    const file = await readProjectFile(current.summary.projectId, path);
    if (token === fileToken) fileView.value = { path, loading: false, file };
  } catch (thrown) {
    if (token === fileToken) {
      fileView.value = { path, loading: false, error: normalizeError(thrown) };
    }
  }
}

/** Leave the viewer without a file. */
export function clearFile(): void {
  fileToken += 1;
  selectedPath.value = undefined;
  fileView.value = undefined;
}

/** Open a mod folder and make it the current project. Returns true on success. */
export async function openFolder(path: string): Promise<boolean> {
  opening.value = true;
  openError.value = undefined;
  try {
    await openProjectAt(path);
    return true;
  } catch (thrown) {
    openError.value = normalizeError(thrown);
    return false;
  } finally {
    opening.value = false;
  }
}

/** Create a new mod and make it the current project. Returns true on success. */
export async function createMod(request: ProjectCreateRequest): Promise<boolean> {
  creating.value = true;
  createError.value = undefined;
  try {
    const summary = await createProject(request);
    setCurrentProject(refOf(summary));
    return true;
  } catch (thrown) {
    createError.value = normalizeError(thrown);
    return false;
  } finally {
    creating.value = false;
  }
}

/** Create the missing standard folders, then read the project again. */
export async function fixMissingFolders(): Promise<void> {
  const current = view.peek();
  if (!current) return;
  fixing.value = true;
  fixError.value = undefined;
  try {
    fixResult.value = await scaffoldMissing(current.summary.projectId, false);
    await loadProject({
      projectId: current.summary.projectId,
      path: current.summary.path,
      name: current.summary.name,
    });
  } catch (thrown) {
    fixError.value = normalizeError(thrown);
  } finally {
    fixing.value = false;
  }
}

/** Forget everything (tests, project change). */
export function resetProjectView(): void {
  loadToken += 1;
  fileToken += 1;
  view.value = undefined;
  loadState.value = 'idle';
  loadError.value = undefined;
  selectedPath.value = undefined;
  fileView.value = undefined;
  opening.value = false;
  openError.value = undefined;
  creating.value = false;
  createError.value = undefined;
  fixing.value = false;
  fixResult.value = undefined;
  fixError.value = undefined;
}
