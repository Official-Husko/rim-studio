import {
  ChooseDirectory,
  CloseProject,
  CreateProject,
  GetAppBootstrap,
  GetAvailableMods,
  GetProjectState,
  OpenProject,
  RescanGameData,
  UpdateGlobalSettings,
  UpdateProjectSettings,
} from '../wailsjs/go/main/App';
import { main } from '../wailsjs/go/models';

import type {
  AppBootstrap,
  CompatibilityPatchEntry,
  CompatibilitySettings,
  CreateProjectInput,
  GameScanSnapshot,
  GameScanStatus,
  ProjectSettings,
  ProjectState,
  ScannedModSummary,
  UpdateGlobalSettingsInput,
  UpdateProjectSettingsInput,
} from './types';

export const api = {
  chooseDirectory(title: string, defaultPath = ''): Promise<string> {
    return ChooseDirectory(title, defaultPath);
  },
  async closeProject(): Promise<AppBootstrap> {
    return normalizeAppBootstrap(await CloseProject());
  },
  async createProject(input: CreateProjectInput): Promise<ProjectState> {
    return normalizeProjectState(await CreateProject(new main.CreateProjectInput(input)));
  },
  async getAppBootstrap(): Promise<AppBootstrap> {
    return normalizeAppBootstrap(await GetAppBootstrap());
  },
  async getAvailableMods(): Promise<ScannedModSummary[]> {
    return normalizeScannedMods(await GetAvailableMods());
  },
  async getProjectState(projectPath: string): Promise<ProjectState> {
    return normalizeProjectState(await GetProjectState(projectPath));
  },
  async openProject(projectPath: string): Promise<ProjectState> {
    return normalizeProjectState(await OpenProject(projectPath));
  },
  async rescanGameData(): Promise<GameScanSnapshot> {
    return normalizeGameScanSnapshot(await RescanGameData());
  },
  async updateGlobalSettings(input: UpdateGlobalSettingsInput): Promise<AppBootstrap> {
    return normalizeAppBootstrap(await UpdateGlobalSettings(new main.UpdateGlobalSettingsInput(input)));
  },
  async updateProjectSettings(input: UpdateProjectSettingsInput): Promise<ProjectState> {
    return normalizeProjectState(await UpdateProjectSettings(new main.UpdateProjectSettingsInput({
      projectPath: input.projectPath,
      settings: new main.ProjectSettings({
        targetVersion: input.settings.targetVersion,
        notes: input.settings.notes,
        compatibility: new main.CompatibilitySettings({
          mode: input.settings.compatibility.mode,
          selectedModIds: input.settings.compatibility.selectedModIds,
          patchEntries: input.settings.compatibility.patchEntries,
          lastGeneratedAt: input.settings.compatibility.lastGeneratedAt,
        }),
      }),
    })));
  },
};

function normalizeAppBootstrap(source: main.AppBootstrap): AppBootstrap {
  return {
    settings: {
      gamePath: source.settings?.gamePath ?? '',
      scanModsEnabled: Boolean(source.settings?.scanModsEnabled),
      cachedModIndex: normalizeScannedMods(source.settings?.cachedModIndex ?? []),
      recentProjects: normalizeRecentProjects(source.settings?.recentProjects ?? []),
    },
    recentProjects: normalizeRecentProjects(source.recentProjects ?? []),
    currentProject: source.currentProject ? normalizeProjectSummary(source.currentProject) : null,
    scanStatus: normalizeGameScanStatus(source.scanStatus),
    availableMods: normalizeScannedMods(source.availableMods ?? []),
  };
}

function normalizeGameScanSnapshot(source: main.GameScanSnapshot): GameScanSnapshot {
  return {
    scanStatus: normalizeGameScanStatus(source.scanStatus),
    availableMods: normalizeScannedMods(source.availableMods ?? []),
  };
}

function normalizeProjectState(source: main.ProjectState): ProjectState {
  return {
    summary: normalizeProjectSummary(source.summary),
    settings: normalizeProjectSettings(source.settings),
  };
}

function normalizeProjectSummary(source: main.ProjectSummary) {
  return {
    name: source.name ?? '',
    path: source.path ?? '',
    packageId: source.packageId ?? '',
    author: source.author ?? '',
    targetVersion: source.targetVersion ?? '1.6',
    hasRimStudioConfig: Boolean(source.hasRimStudioConfig),
  };
}

function normalizeProjectSettings(source: main.ProjectSettings | undefined): ProjectSettings {
  return {
    targetVersion: source?.targetVersion ?? '1.6',
    notes: source?.notes ?? '',
    compatibility: normalizeCompatibilitySettings(source?.compatibility),
  };
}

function normalizeCompatibilitySettings(source: main.CompatibilitySettings | undefined): CompatibilitySettings {
  const patchEntries = source?.patchEntries ?? {};
  const selectedModIds = source?.selectedModIds ?? [];
  const normalizedPatchEntries: Record<string, CompatibilityPatchEntry> = {};

  for (const [key, value] of Object.entries(patchEntries)) {
    normalizedPatchEntries[key] = {
      modId: value?.modId ?? key,
      displayName: value?.displayName ?? key,
      notes: value?.notes ?? '',
      generated: Boolean(value?.generated),
      userEdited: Boolean(value?.userEdited),
      lastModified: value?.lastModified,
    };
  }

  return {
    mode: source?.mode === 'selected' ? 'selected' : 'all',
    selectedModIds: Array.isArray(selectedModIds) ? selectedModIds.filter(Boolean) : [],
    patchEntries: normalizedPatchEntries,
    lastGeneratedAt: source?.lastGeneratedAt,
  };
}

function normalizeGameScanStatus(source: main.GameScanStatus | undefined): GameScanStatus {
  const state = source?.state;

  return {
    state: state === 'scanning' || state === 'ready' || state === 'error' ? state : 'idle',
    message: source?.message ?? '',
    lastUpdated: source?.lastUpdated,
    scannedSources: source?.scannedSources ?? 0,
    availableModCount: source?.availableModCount ?? 0,
  };
}

function normalizeScannedMods(source: main.ScannedModSummary[]): ScannedModSummary[] {
  return source.map((mod) => ({
    id: mod.id ?? '',
    name: mod.name ?? '',
    packageId: mod.packageId ?? '',
    path: mod.path ?? '',
    source: mod.source ?? '',
  }));
}

function normalizeRecentProjects(source: main.RecentProject[]) {
  return source.map((project) => ({
    name: project.name ?? '',
    path: project.path ?? '',
    packageId: project.packageId ?? '',
    lastOpened: project.lastOpened ?? '',
  }));
}
