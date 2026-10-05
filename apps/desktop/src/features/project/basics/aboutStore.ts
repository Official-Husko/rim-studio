import { computed, signal } from '@preact/signals';
import type {
  AboutListFieldDto,
  AboutTextFieldDto,
  ApiError,
  DiagnosticDto,
  ProjectAboutDto,
  ProjectAboutPreviewDto,
  ProjectAboutUpdateDto,
} from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import { bumpProjectRevision } from '~/shared/project';
import {
  aboutGet,
  aboutPreview,
  aboutRemovePreview,
  aboutSetPreview,
  aboutUpdate,
} from '../modApi';
import { draftChanges, emptyDraft, type AboutDraft, type DraftDependency } from './aboutModel';

// State of the Basics tab. The saved model comes from the backend, the draft is what the person typed,
// and the preview is the backend's answer for the draft (the diff and the findings of the result). The
// store lives outside the tab so that switching tabs keeps unsaved edits.

/** How long typing must pause before the draft is sent for a preview. */
export const PREVIEW_DELAY_MS = 300;

export type AboutLoadState = 'idle' | 'loading' | 'ready' | 'error';

export const aboutState = signal<AboutLoadState>('idle');
export const aboutError = signal<ApiError | undefined>(undefined);
/** The file as the backend last read or wrote it. */
export const aboutModel = signal<ProjectAboutDto | undefined>(undefined);
export const aboutDraft = signal<AboutDraft>(emptyDraft());
/** The backend's answer for the current draft; undefined when there is nothing to preview. */
export const aboutPreviewResult = signal<ProjectAboutPreviewDto | undefined>(undefined);
export const previewing = signal(false);
export const previewError = signal<ApiError | undefined>(undefined);
export const aboutSaving = signal(false);
export const saveError = signal<ApiError | undefined>(undefined);
/** True when the file changed on disk after the form read it. */
export const aboutStale = signal(false);
export const lastSave = signal<ProjectAboutUpdateDto | undefined>(undefined);
export const imageBusy = signal(false);
export const imageError = signal<ApiError | undefined>(undefined);
export const imageNotice = signal<string | undefined>(undefined);

/** The changes the draft makes to the saved file. */
export const pendingChanges = computed(() => {
  const about = aboutModel.value;
  return about ? draftChanges(about, aboutDraft.value) : [];
});

/** The findings to show: the preview's when the draft changes something, else the file's. */
export const findings = computed<DiagnosticDto[]>(() => {
  const preview = aboutPreviewResult.value;
  if (preview && pendingChanges.value.length > 0) return preview.result.diagnostics;
  return aboutModel.value?.diagnostics ?? [];
});

let projectId: string | undefined;
let loadToken = 0;
let previewToken = 0;
let timer: ReturnType<typeof setTimeout> | undefined;

function clearTimer(): void {
  if (timer !== undefined) clearTimeout(timer);
  timer = undefined;
}

/** Ask the backend what the draft would do; stale answers are dropped. */
export async function runPreview(): Promise<void> {
  clearTimer();
  const about = aboutModel.peek();
  if (!about || !projectId) return;
  const changes = draftChanges(about, aboutDraft.peek());
  previewToken += 1;
  const token = previewToken;
  if (changes.length === 0) {
    aboutPreviewResult.value = undefined;
    previewError.value = undefined;
    previewing.value = false;
    return;
  }
  previewing.value = true;
  try {
    const result = await aboutPreview(projectId, changes, about.fileHash);
    if (token !== previewToken) return;
    aboutPreviewResult.value = result;
    previewError.value = undefined;
  } catch (thrown) {
    if (token !== previewToken) return;
    const error = normalizeError(thrown);
    if (error.code === 'project.file-stale') aboutStale.value = true;
    previewError.value = error;
    aboutPreviewResult.value = undefined;
  } finally {
    if (token === previewToken) previewing.value = false;
  }
}

function schedulePreview(): void {
  clearTimer();
  previewing.value = true;
  timer = setTimeout(() => void runPreview(), PREVIEW_DELAY_MS);
}

/** Read the file for a project. With `keepDraft` the edits stay (used after a stale refusal). */
export async function loadAbout(id: string, keepDraft = false): Promise<void> {
  if (projectId !== id) {
    resetAboutStore();
    projectId = id;
  }
  loadToken += 1;
  const token = loadToken;
  aboutState.value = 'loading';
  try {
    const model = await aboutGet(id, true);
    if (token !== loadToken) return;
    aboutModel.value = model;
    aboutError.value = undefined;
    aboutStale.value = false;
    saveError.value = undefined;
    aboutState.value = 'ready';
    if (!keepDraft) {
      aboutDraft.value = emptyDraft();
      aboutPreviewResult.value = undefined;
    }
    await runPreview();
  } catch (thrown) {
    if (token !== loadToken) return;
    aboutError.value = normalizeError(thrown);
    aboutState.value = 'error';
  }
}

/** Edit a text field. */
export function setText(field: AboutTextFieldDto, value: string): void {
  const draft = aboutDraft.peek();
  aboutDraft.value = { ...draft, text: { ...draft.text, [field]: value } };
  saveError.value = undefined;
  schedulePreview();
}

/** Replace a list field. */
export function setList(field: AboutListFieldDto, items: string[]): void {
  const draft = aboutDraft.peek();
  aboutDraft.value = { ...draft, lists: { ...draft.lists, [field]: items } };
  saveError.value = undefined;
  schedulePreview();
}

/** Replace the dependency rows. */
export function setDependencies(rows: DraftDependency[]): void {
  aboutDraft.value = { ...aboutDraft.peek(), deps: rows };
  saveError.value = undefined;
  schedulePreview();
}

/** Throw the edits away. */
export function discardAbout(): void {
  clearTimer();
  previewToken += 1;
  aboutDraft.value = emptyDraft();
  aboutPreviewResult.value = undefined;
  previewError.value = undefined;
  saveError.value = undefined;
  previewing.value = false;
}

/** Write the draft to About.xml. Returns true when it was written. */
export async function saveAbout(): Promise<boolean> {
  const about = aboutModel.peek();
  if (!about || !projectId) return false;
  const changes = draftChanges(about, aboutDraft.peek());
  if (changes.length === 0) return false;
  clearTimer();
  previewToken += 1;
  aboutSaving.value = true;
  saveError.value = undefined;
  try {
    const result = await aboutUpdate(projectId, changes, about.fileHash);
    aboutModel.value = result.about;
    aboutDraft.value = emptyDraft();
    aboutPreviewResult.value = undefined;
    previewError.value = undefined;
    previewing.value = false;
    aboutStale.value = false;
    lastSave.value = result;
    bumpProjectRevision();
    return true;
  } catch (thrown) {
    const error = normalizeError(thrown);
    if (error.code === 'project.file-stale') aboutStale.value = true;
    saveError.value = error;
    return false;
  } finally {
    aboutSaving.value = false;
  }
}

async function afterImageChange(id: string): Promise<void> {
  bumpProjectRevision();
  await loadAbout(id, true);
}

/** Copy a PNG to About/Preview.png; the old image is backed up by the backend. */
export async function setPreviewImage(sourcePath: string): Promise<void> {
  if (!projectId) return;
  const id = projectId;
  imageBusy.value = true;
  imageError.value = undefined;
  imageNotice.value = undefined;
  try {
    const result = await aboutSetPreview(id, sourcePath);
    imageNotice.value = result.replaced ? 'replaced' : 'added';
    await afterImageChange(id);
  } catch (thrown) {
    imageError.value = normalizeError(thrown);
  } finally {
    imageBusy.value = false;
  }
}

/** Remove About/Preview.png after a backup. */
export async function removePreviewImage(): Promise<void> {
  if (!projectId) return;
  const id = projectId;
  imageBusy.value = true;
  imageError.value = undefined;
  imageNotice.value = undefined;
  try {
    const result = await aboutRemovePreview(id);
    imageNotice.value = result.removed ? 'removed' : undefined;
    await afterImageChange(id);
  } catch (thrown) {
    imageError.value = normalizeError(thrown);
  } finally {
    imageBusy.value = false;
  }
}

/** Forget everything (tests, a different project). */
export function resetAboutStore(): void {
  clearTimer();
  loadToken += 1;
  previewToken += 1;
  projectId = undefined;
  aboutState.value = 'idle';
  aboutError.value = undefined;
  aboutModel.value = undefined;
  aboutDraft.value = emptyDraft();
  aboutPreviewResult.value = undefined;
  previewing.value = false;
  previewError.value = undefined;
  aboutSaving.value = false;
  saveError.value = undefined;
  aboutStale.value = false;
  lastSave.value = undefined;
  imageBusy.value = false;
  imageError.value = undefined;
  imageNotice.value = undefined;
}
