import { h } from 'preact';
import logo from '../../assets/images/logo-universal.png';
import { workspaceTabs } from '../../constants';
import type {
  AppInfo,
  AppBootstrap,
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

const placeholderCopy: Record<'weapons' | 'race' | 'clothing', string> = {
  weapons: 'Weapon editors will live here once the workplace shell is stable.',
  race: 'Race authoring and inherited defs will slot into this workspace panel next.',
  clothing: 'Clothing item editors and textile helpers are reserved for this panel.',
};

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
  activeTaskLabel: string;
  onTabChange: (tab: WorkspaceTab) => void;
  onCloseProject: () => void;
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
  activeTaskLabel,
  onTabChange,
  onCloseProject,
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
        <aside className="workspace-sidebar">
          <div className="sidebar-brand">
            <img className="sidebar-logo" src={logo} alt="RimStudio" />
            <div>
              <p className="eyebrow">RimStudio</p>
              <h1>{projectState.summary.name}</h1>
            </div>
          </div>

          <nav className="tab-list" aria-label="Workspace Sections">
            {workspaceTabs.map((tab) => (
              <button
                key={tab.key}
                className={`tab-button ${activeTab === tab.key ? 'is-active' : ''}`}
                onClick={() => onTabChange(tab.key)}
                type="button"
              >
                <span className="tab-icon">{tab.icon}</span>
                <span>{tab.label}</span>
              </button>
            ))}
          </nav>

          <div className="sidebar-foot">
            <button className="ghost-button" onClick={onCloseProject} type="button">
              Return To Start
            </button>
          </div>
        </aside>

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

            {activeTab !== 'basics' && activeTab !== 'settings' ? (
              <PlaceholderPanel
                label={workspaceTabs.find((tab) => tab.key === activeTab)?.label ?? activeTab}
                copy={placeholderCopy[activeTab as 'weapons' | 'race' | 'clothing']}
              />
            ) : null}
          </div>
        </section>
      </div>

      <WorkspaceBottomBar
        activeTaskLabel={activeTaskLabel}
        appInfo={appInfo}
        contentCounts={projectState.contentCounts}
        scanStatus={scanStatus}
      />
    </main>
  );
}
