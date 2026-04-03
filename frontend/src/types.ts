export type WorkspaceTab = 'basics' | 'weapons' | 'race' | 'clothing' | 'settings' | 'tests';
export type NotificationType = 'info' | 'warning' | 'error' | 'tip';
export type ThemeID =
  | 'rim-neutral'
  | 'workshop'
  | 'blueprint'
  | 'foundry'
  | 'archive'
  | 'relay'
  | 'daylight'
  | 'halloween';

export interface NotificationAction {
  label: string;
  onClick: () => void;
}

export interface NotificationItem {
  id: string;
  key?: string;
  type: NotificationType;
  title: string;
  message: string;
  icon: string;
  persistent?: boolean;
  durationMs?: number;
  actions?: NotificationAction[];
}

export interface WorkspaceTabDefinition {
  key: WorkspaceTab;
  label: string;
  icon: string;
}

export interface ThemeDefinition {
  id: ThemeID;
  name: string;
  previewClassName: string;
}

export interface CustomThemeSummary {
  id: string;
  name: string;
  path: string;
}

export interface AppInfo {
  version: string;
  commitShort: string;
}

export interface BackgroundTask {
  id: string;
  label: string;
  progress?: number;
  details?: string;
  startedAt: number;
}

export interface ProjectContentCounts {
  defs: number;
  textures: number;
  patches: number;
}

export interface RecentProject {
  name: string;
  path: string;
  packageId: string;
  lastOpened: string;
}

export interface ProjectSummary {
  name: string;
  path: string;
  packageId: string;
  author: string;
  targetVersion: string;
  hasRimStudioConfig: boolean;
}

export interface CompatibilityPatchEntry {
  modId: string;
  displayName: string;
  notes: string;
  generated: boolean;
  userEdited: boolean;
  lastModified?: string;
}

export interface CompatibilitySettings {
  mode: 'all' | 'selected';
  selectedModIds: string[];
  patchEntries: Record<string, CompatibilityPatchEntry>;
  lastGeneratedAt?: string;
}

export interface ProjectSettings {
  targetVersion: string;
  notes: string;
  compatibility: CompatibilitySettings;
}

export interface ProjectState {
  summary: ProjectSummary;
  settings: ProjectSettings;
  contentCounts: ProjectContentCounts;
}

export interface ScannedModSummary {
  id: string;
  name: string;
  packageId: string;
  path: string;
  source: string;
}

export interface GlobalSettings {
  gamePath: string;
  scanModsEnabled: boolean;
  themeId: ThemeID;
  customCssPath: string;
  cachedModIndex: ScannedModSummary[];
  recentProjects: RecentProject[];
}

export interface GameScanStatus {
  state: 'idle' | 'scanning' | 'ready' | 'error';
  message: string;
  lastUpdated?: string;
  scannedSources: number;
  availableModCount: number;
  dlcLoadedCount: number;
}

export interface AppBootstrap {
  appInfo: AppInfo;
  settings: GlobalSettings;
  recentProjects: RecentProject[];
  currentProject?: ProjectSummary | null;
  scanStatus: GameScanStatus;
  availableMods: ScannedModSummary[];
  availableCustomThemes: CustomThemeSummary[];
}

export interface GameScanSnapshot {
  scanStatus: GameScanStatus;
  availableMods: ScannedModSummary[];
}

export interface CreateProjectInput {
  name: string;
  packageId: string;
  author: string;
  location: string;
  targetVersion: string;
}

export interface UpdateProjectSettingsInput {
  projectPath: string;
  settings: ProjectSettings;
}

export interface UpdateGlobalSettingsInput {
  gamePath: string;
  scanModsEnabled: boolean;
  themeId: ThemeID;
  customCssPath: string;
}
