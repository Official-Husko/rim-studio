import { h } from 'preact';
import { workspaceTabs } from '../../constants';
import { TestLabPanel } from '../../features/tests/TestLabPanel';
import type { SimulatedTaskPlan } from '../../features/tests/taskSimulator';
import type {
  AppInfo,
  AppBootstrap,
  BackgroundTask,
  CustomThemeSummary,
  GlobalSettings,
  ProjectSettings,
  ProjectState,
  ScannedModSummary,
  ThemeID,
  WorkspaceTab,
} from '../../types';
import { BasicsPanel } from './BasicsPanel';
import { PlaceholderPanel } from './PlaceholderPanel';
import { WorkspaceBottomBar } from './WorkspaceBottomBar';
import { SettingsPanel } from './SettingsPanel';
import { WorkspaceSidebar } from './WorkspaceSidebar';

interface Props {
  activeTab: WorkspaceTab;
  appInfo: AppInfo;
  projectState: ProjectState;
  projectDraft: ProjectSettings;
  selectedMods: ScannedModSummary[];
  availableMods: ScannedModSummary[];
  availableCustomThemes: CustomThemeSummary[];
  globalDraft: GlobalSettings;
  busy: boolean;
  scanStatus: AppBootstrap['scanStatus'];
  backgroundTasks: BackgroundTask[];
  activeDemoTaskCount: number;
  onTabChange: (tab: WorkspaceTab) => void;
  onProjectSettingsChange: (settings: ProjectSettings) => void;
  onToggleSelectedMod: (mod: ScannedModSummary) => void;
  onSaveProjectSettings: () => void;
  onGlobalGamePathChange: (value: string) => void;
  onGlobalScanModsEnabledChange: (enabled: boolean) => void;
  onGlobalThemeIDChange: (themeId: ThemeID) => void;
  onGlobalCustomCSSPathChange: (value: string) => void;
  onBrowseGamePath: () => void;
  onClearCustomCSSPath: () => void;
  onSaveGlobalSettings: () => void;
  onTriggerRescan: () => void;
  onRunSimulatedTasks: (plans: SimulatedTaskPlan[]) => void;
  onClearSimulatedTasks: () => void;
}

export function WorkspaceShell({
  activeTab,
  appInfo,
  projectState,
  projectDraft,
  selectedMods,
  availableMods,
  availableCustomThemes,
  globalDraft,
  busy,
  scanStatus,
  backgroundTasks,
  activeDemoTaskCount,
  onTabChange,
  onProjectSettingsChange,
  onToggleSelectedMod,
  onSaveProjectSettings,
  onGlobalGamePathChange,
  onGlobalScanModsEnabledChange,
  onGlobalThemeIDChange,
  onGlobalCustomCSSPathChange,
  onBrowseGamePath,
  onClearCustomCSSPath,
  onSaveGlobalSettings,
  onTriggerRescan,
  onRunSimulatedTasks,
  onClearSimulatedTasks,
}: Props) {
  return (
    <main className="workspace-shell">
      <header className="workspace-topbar">
        <div>
          <p className="eyebrow">Current Project</p>
          <div className="workspace-title-row">
            <h2>{projectState.summary.name}</h2>
            <span className="workspace-package-id">{projectState.summary.packageId || 'No package ID'}</span>
          </div>
          <p className="muted-path">{projectState.summary.path}</p>
        </div>

        <div className="workspace-topbar-status">
          <span className={`status-pill status-${scanStatus.state}`}>{scanStatus.state}</span>
          <p>{scanStatus.message}</p>
        </div>
      </header>

      <div className="workspace-layout">
        <WorkspaceSidebar activeTab={activeTab} onTabChange={onTabChange} />

        <section className="workspace-main">
          <div className="workspace-content">
            {activeTab === 'basics' ? (
              <BasicsPanel
                busy={busy}
                projectDraft={projectDraft}
                projectState={projectState}
                selectedMods={selectedMods}
                onSaveProjectSettings={onSaveProjectSettings}
              />
            ) : null}

            {activeTab === 'settings' ? (
              <SettingsPanel
                availableMods={availableMods}
                availableCustomThemes={availableCustomThemes}
                busy={busy}
                globalDraft={globalDraft}
                projectDraft={projectDraft}
                scanStatus={scanStatus}
                onBrowseGamePath={onBrowseGamePath}
                onClearCustomCSSPath={onClearCustomCSSPath}
                onGlobalCustomCSSPathChange={onGlobalCustomCSSPathChange}
                onGlobalGamePathChange={onGlobalGamePathChange}
                onGlobalScanModsEnabledChange={onGlobalScanModsEnabledChange}
                onGlobalThemeIDChange={onGlobalThemeIDChange}
                onProjectSettingsChange={onProjectSettingsChange}
                onSaveGlobalSettings={onSaveGlobalSettings}
                onSaveProjectSettings={onSaveProjectSettings}
                onToggleSelectedMod={onToggleSelectedMod}
                onTriggerRescan={onTriggerRescan}
              />
            ) : null}

            {activeTab === 'tests' ? (
              <TestLabPanel
                activeDemoTaskCount={activeDemoTaskCount}
                totalActiveTaskCount={backgroundTasks.length}
                onClearSimulatedTasks={onClearSimulatedTasks}
                onRunSimulatedTasks={onRunSimulatedTasks}
              />
            ) : null}

            {activeTab !== 'basics' && activeTab !== 'settings' && activeTab !== 'tests' ? (
              <PlaceholderPanel />
            ) : null}
          </div>
        </section>
      </div>

      <WorkspaceBottomBar
        appInfo={appInfo}
        backgroundTasks={backgroundTasks}
        contentCounts={projectState.contentCounts}
        scanStatus={scanStatus}
      />
    </main>
  );
}
