import { computed, signal } from '@preact/signals';
import type { ProjectSummaryDto } from 'rimstudio-ipc-types';
import { callCommand } from '~/shared/ipc';

// The project the user works in. The Project, Weapons and Patches pages read it; the Project page
// and the top bar selector write it. Only the path and a short list of recent projects are kept in
// the browser (a convenience, never the truth): the backend owns the project itself.

/** The current project: what other pages need to call project and designer commands. */
export interface ProjectRef {
  projectId: string;
  path: string;
  name: string;
  packageId?: string;
}

/** One entry of the recent list. */
export interface RecentProject {
  path: string;
  name: string;
  packageId?: string;
  /** Milliseconds since the epoch. */
  openedAt: number;
}

const CURRENT_KEY = 'rimstudio.project.current';
const RECENT_KEY = 'rimstudio.project.recent';
/** How many recent projects are remembered. */
export const RECENT_LIMIT = 8;

function readStorage(key: string): string | null {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

function writeStorage(key: string, value: string | null): void {
  try {
    if (value === null) window.localStorage.removeItem(key);
    else window.localStorage.setItem(key, value);
  } catch {
    /* storage is a convenience; the page works without it */
  }
}

function parseRecent(raw: string | null): RecentProject[] {
  if (!raw) return [];
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    const out: RecentProject[] = [];
    for (const item of parsed) {
      if (typeof item !== 'object' || item === null) continue;
      const v = item as Record<string, unknown>;
      if (typeof v.path !== 'string' || typeof v.name !== 'string') continue;
      out.push({
        path: v.path,
        name: v.name,
        ...(typeof v.packageId === 'string' ? { packageId: v.packageId } : {}),
        openedAt: typeof v.openedAt === 'number' ? v.openedAt : 0,
      });
    }
    return out.slice(0, RECENT_LIMIT);
  } catch {
    return [];
  }
}

/** The current project, or undefined when none is open. */
export const currentProject = signal<ProjectRef | undefined>(undefined);

/** The path of the current project. */
export const currentProjectPath = computed(() => currentProject.value?.path);

/** The recent projects, newest first. */
export const recentProjects = signal<readonly RecentProject[]>(
  parseRecent(readStorage(RECENT_KEY)),
);

/** Counts up when a page changed files of the project, so views of the tree know to reload. */
export const projectRevision = signal(0);

/** The current project without subscribing; for event handlers and stores. */
export function getCurrentProject(): ProjectRef | undefined {
  return currentProject.peek();
}

/** The current project path without subscribing. */
export function getCurrentProjectPath(): string | undefined {
  return currentProject.peek()?.path;
}

/** The current project; reading it inside a component makes the component follow it. */
export function useCurrentProject(): ProjectRef | undefined {
  return currentProject.value;
}

/** Tell the views of the project that files changed on disk. */
export function bumpProjectRevision(): void {
  projectRevision.value += 1;
}

function remember(ref: ProjectRef): void {
  const entry: RecentProject = {
    path: ref.path,
    name: ref.name,
    ...(ref.packageId ? { packageId: ref.packageId } : {}),
    openedAt: Date.now(),
  };
  const rest = recentProjects.peek().filter((r) => r.path !== ref.path);
  recentProjects.value = [entry, ...rest].slice(0, RECENT_LIMIT);
  writeStorage(RECENT_KEY, JSON.stringify(recentProjects.value));
}

/** Make a project current (or none with undefined). A project is added to the recent list. */
export function setCurrentProject(ref: ProjectRef | undefined): void {
  currentProject.value = ref;
  writeStorage(CURRENT_KEY, ref ? ref.path : null);
  if (ref) remember(ref);
}

/** Take one path out of the recent list. The folder is never touched. */
export function forgetRecent(path: string): void {
  recentProjects.value = recentProjects.peek().filter((r) => r.path !== path);
  writeStorage(RECENT_KEY, JSON.stringify(recentProjects.value));
}

/** Empty the recent list. */
export function clearRecent(): void {
  recentProjects.value = [];
  writeStorage(RECENT_KEY, null);
}

/** The reference of an opened project. */
export function refOf(summary: ProjectSummaryDto): ProjectRef {
  return {
    projectId: summary.projectId,
    path: summary.path,
    name: summary.name,
    ...(summary.packageId ? { packageId: summary.packageId } : {}),
  };
}

/** Open a mod folder through the backend and make it current. Throws the ApiError on failure. */
export async function openProjectAt(path: string): Promise<ProjectSummaryDto> {
  const summary = await callCommand('project_open', { path });
  setCurrentProject(refOf(summary));
  return summary;
}

/** Open the project of the last session, once. A folder that is gone is dropped silently. */
export async function restoreCurrentProject(): Promise<void> {
  if (currentProject.peek()) return;
  const path = readStorage(CURRENT_KEY);
  if (!path) return;
  try {
    await openProjectAt(path);
  } catch {
    writeStorage(CURRENT_KEY, null);
  }
}

/** Reset the state and the stored values (tests). */
export function resetProjectStore(): void {
  currentProject.value = undefined;
  recentProjects.value = [];
  projectRevision.value = 0;
  writeStorage(CURRENT_KEY, null);
  writeStorage(RECENT_KEY, null);
}
