import type {
  AppBootstrap,
  ProjectSettings,
  ThemeDefinition,
  WorkspaceTabDefinition,
} from './types';

export const supportedVersions = ['1.1', '1.2', '1.3', '1.4', '1.5', '1.6'] as const;

export const workspaceTabs: WorkspaceTabDefinition[] = [
  { key: 'basics', label: 'Basics', icon: 'fa-cubes-stacked' },
  { key: 'weapons', label: 'Weapons', icon: 'fa-gun' },
  { key: 'race', label: 'Race', icon: 'fa-dna' },
  { key: 'clothing', label: 'Clothing', icon: 'fa-shirt' },
  { key: 'settings', label: 'Settings', icon: 'fa-gear' },
  { key: 'tests', label: 'Tests', icon: 'fa-flask' },
];

export const builtInThemes: ThemeDefinition[] = [
  {
    id: 'rim-neutral',
    name: 'Rim Neutral',
    previewClassName: 'theme-preview-rim-neutral',
  },
  {
    id: 'workshop',
    name: 'Workshop',
    previewClassName: 'theme-preview-workshop',
  },
  {
    id: 'blueprint',
    name: 'Blueprint',
    previewClassName: 'theme-preview-blueprint',
  },
  {
    id: 'foundry',
    name: 'Foundry',
    previewClassName: 'theme-preview-foundry',
  },
  {
    id: 'archive',
    name: 'Archive',
    previewClassName: 'theme-preview-archive',
  },
  {
    id: 'relay',
    name: 'Relay',
    previewClassName: 'theme-preview-relay',
  },
  {
    id: 'daylight',
    name: 'Daylight',
    previewClassName: 'theme-preview-daylight',
  },
  {
    id: 'halloween',
    name: 'Halloween',
    previewClassName: 'theme-preview-halloween',
  },
];

export const emptyProjectSettings = (): ProjectSettings => ({
  targetVersion: '1.6',
  notes: '',
  compatibility: {
    mode: 'all',
    selectedModIds: [],
    patchEntries: {},
  },
});

export const emptyBootstrap: AppBootstrap = {
  appInfo: {
    version: 'v0.1.0',
    commitShort: 'dev000',
  },
  settings: {
    gamePath: '',
    scanModsEnabled: false,
    themeId: 'rim-neutral',
    customCssPath: '',
    cachedModIndex: [],
    recentProjects: [],
  },
  recentProjects: [],
  currentProject: null,
  scanStatus: {
    state: 'idle',
    message: 'Set a RimWorld install path in Settings to index game data.',
    scannedSources: 0,
    availableModCount: 0,
    dlcLoadedCount: 0,
  },
  availableMods: [],
  availableCustomThemes: [],
};
