import { computed, signal } from '@preact/signals';
import type { ProjectScaffoldPreviewDto, SourceDto } from 'rimstudio-ipc-types';
import { callCommand, normalizeError } from '~/shared/ipc';
import { versionChoices } from '../basics/aboutModel';
import { listSources } from '../api';
import { scaffoldPreview } from '../modApi';
import { folderNameOf, joinPath, suggestPackageId } from '../model';
import { emptyForm, requestOf, type NewModForm } from '../scaffold';
import { createError } from '../store';

// State of the New mod window: the form, the step, and the backend's preview of what would be written.

export type CreateStep = 'identity' | 'structure' | 'review';
export const STEPS: readonly CreateStep[] = ['identity', 'structure', 'review'];

/** How long typing must pause before the backend is asked about the values. */
export const CHECK_DELAY_MS = 300;

export const createForm = signal<NewModForm>(emptyForm());
export const createParent = signal('');
export const createStep = signal<CreateStep>('identity');
export const createPreview = signal<ProjectScaffoldPreviewDto | undefined>(undefined);
export const createChecking = signal(false);
/** The installed game version as "major.minor", when detection knows it. */
export const createGameVersion = signal<string | undefined>(undefined);
/** The mod folders of Setup, offered as quick choices for the parent folder. */
export const createFolders = signal<SourceDto[]>([]);

let idTouched = false;
let folderTouched = false;
let token = 0;
let timer: ReturnType<typeof setTimeout> | undefined;

/** The full path of the folder the mod would be created in. */
export const createTarget = computed(() => {
  const parent = createParent.value.trim();
  const name = createForm.value.folderName.trim();
  return parent && name ? joinPath(parent, name) : '';
});

/** The versions the window offers: the installed one, the three before it and any already ticked. */
export const createVersionChoices = computed(() =>
  versionChoices(createGameVersion.value, createForm.value.versions),
);

async function check(): Promise<void> {
  timer = undefined;
  token += 1;
  const mine = token;
  const target = createTarget.peek();
  const form = createForm.peek();
  if (target === '') {
    createPreview.value = undefined;
    createChecking.value = false;
    return;
  }
  try {
    const answer = await scaffoldPreview(requestOf(form, target));
    if (mine !== token) return;
    createPreview.value = answer;
  } catch (thrown) {
    if (mine !== token) return;
    createPreview.value = undefined;
    createError.value = normalizeError(thrown);
  } finally {
    if (mine === token) createChecking.value = false;
  }
}

/** Ask the backend now (tests) or after the delay (typing). */
export function scheduleCheck(immediate = false): void {
  if (timer !== undefined) clearTimeout(timer);
  createChecking.value = true;
  if (immediate) void check();
  else timer = setTimeout(() => void check(), CHECK_DELAY_MS);
}

/** Change the form. The package id and the folder name follow the name and author until they are typed. */
export function changeForm(patch: Partial<NewModForm>): void {
  const next = { ...createForm.peek(), ...patch };
  if ('packageId' in patch) idTouched = true;
  if ('folderName' in patch) folderTouched = true;
  if (!idTouched && ('name' in patch || 'author' in patch)) {
    next.packageId = suggestPackageId(next.author, next.name);
  }
  if (!folderTouched && 'name' in patch) next.folderName = folderNameOf(next.name);
  createForm.value = next;
  createError.value = undefined;
  scheduleCheck();
}

/** Choose the folder that will hold the mod folder. */
export function changeParent(path: string): void {
  createParent.value = path;
  scheduleCheck();
}

/** Tick or untick a game version. */
export function toggleVersion(version: string, on: boolean): void {
  const chosen = createForm.peek().versions;
  const next = on ? [...chosen, version] : chosen.filter((v) => v !== version);
  changeForm({ versions: createVersionChoices.peek().filter((v) => next.includes(v)) });
}

/** Go to a step. */
export function goStep(step: CreateStep): void {
  createStep.value = step;
  if (step === 'review') scheduleCheck(true);
}

async function loadContext(): Promise<void> {
  try {
    const report = (await callCommand('detect_get_report', {})).report;
    const version = report?.installs.find((i) => i.version)?.version;
    if (version) {
      createGameVersion.value = `${version.major}.${version.minor}`;
      const form = createForm.peek();
      if (form.name === '' && form.versions.length === 1 && form.versions[0] === '1.6') {
        createForm.value = { ...form, versions: [`${version.major}.${version.minor}`] };
      }
    }
  } catch {
    /* the window works without it: the choices fall back to a short list */
  }
  try {
    createFolders.value = (await listSources()).filter(
      (s: SourceDto) => s.enabled && (s.kind === 'custom' || s.kind === 'game-mods'),
    );
  } catch {
    createFolders.value = [];
  }
}

/** Reset the window for a new mod. */
export function openCreate(startParent = ''): void {
  idTouched = false;
  folderTouched = false;
  token += 1;
  if (timer !== undefined) clearTimeout(timer);
  timer = undefined;
  createForm.value = emptyForm(createGameVersion.peek() ?? '1.6');
  createParent.value = startParent;
  createStep.value = 'identity';
  createPreview.value = undefined;
  createChecking.value = false;
  createError.value = undefined;
  void loadContext();
}

/** True when the identity step has what the next step needs and the backend found no error. */
export const identityReady = computed(() => {
  const form = createForm.value;
  const preview = createPreview.value;
  return (
    form.name.trim() !== '' &&
    form.packageId.trim() !== '' &&
    createTarget.value !== '' &&
    form.versions.length > 0 &&
    preview?.valid === true &&
    !createChecking.value
  );
});

/** Forget everything (tests). */
export function resetCreateStore(): void {
  idTouched = false;
  folderTouched = false;
  token += 1;
  if (timer !== undefined) clearTimeout(timer);
  timer = undefined;
  createForm.value = emptyForm();
  createParent.value = '';
  createStep.value = 'identity';
  createPreview.value = undefined;
  createChecking.value = false;
  createGameVersion.value = undefined;
  createFolders.value = [];
}
