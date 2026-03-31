import type {
  AppBootstrap,
  ProjectSettings,
  RecentProject,
  WorkspaceTabDefinition,
} from './types';

export const supportedVersions = ['1.1', '1.2', '1.3', '1.4', '1.5', '1.6'] as const;

export const workspaceTabs: WorkspaceTabDefinition[] = [
  { key: 'basics', label: 'Basics', icon: 'Ba' },
  { key: 'weapons', label: 'Weapons', icon: 'We' },
  { key: 'race', label: 'Race', icon: 'Ra' },
  { key: 'clothing', label: 'Clothing', icon: 'Cl' },
  { key: 'settings', label: 'Settings', icon: 'Se' },
];

export const placeholderRecentProjects: RecentProject[] = [
  {
    name: 'Frontier Firearms',
    packageId: 'example.frontierfirearms',
    path: '/mods/FrontierFirearms',
    lastOpened: '',
  },
  {
    name: 'Dustborn Apparel',
    packageId: 'example.dustbornapparel',
    path: '/mods/DustbornApparel',
    lastOpened: '',
  },
  {
    name: 'Geneforge Colonists',
    packageId: 'example.geneforgecolonists',
    path: '/mods/GeneforgeColonists',
    lastOpened: '',
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
  settings: {
    gamePath: '',
    scanModsEnabled: false,
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
  },
  availableMods: [],
};
