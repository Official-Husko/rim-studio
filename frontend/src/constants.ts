import type {
  AppBootstrap,
  ProjectSettings,
  ThemeDefinition,
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

export const builtInThemes: ThemeDefinition[] = [
  {
    id: 'rim-neutral',
    name: 'Rim Neutral',
    description: 'Charcoal, gunmetal, silver, and off-white with a muted Rim-style blue accent.',
    previewClassName: 'theme-preview-rim-neutral',
  },
  {
    id: 'workshop',
    name: 'Workshop',
    description: 'Dark graphite and steel with amber and pale cyan for warnings, builds, and diagnostics.',
    previewClassName: 'theme-preview-workshop',
  },
  {
    id: 'blueprint',
    name: 'Blueprint',
    description: 'Deep navy, slate grey, white linework, and electric cyan for technical editing.',
    previewClassName: 'theme-preview-blueprint',
  },
  {
    id: 'foundry',
    name: 'Foundry',
    description: 'Burnt metal and brass for a hotter toolbench-style workspace.',
    previewClassName: 'theme-preview-foundry',
  },
  {
    id: 'archive',
    name: 'Archive',
    description: 'Dusty parchment, iron ink, and subdued brass for XML-heavy reference work.',
    previewClassName: 'theme-preview-archive',
  },
  {
    id: 'relay',
    name: 'Relay',
    description: 'Cold relay greens and signal blues for a cleaner terminal-inspired cockpit.',
    previewClassName: 'theme-preview-relay',
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
  },
  availableMods: [],
  availableCustomThemes: [],
};
