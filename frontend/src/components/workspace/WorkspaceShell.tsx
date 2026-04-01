import { h } from 'preact';
import logo from '../../assets/images/logo-universal.png';
import { workspaceTabs } from '../../constants';
import type {
  AppBootstrap,
  GlobalSettings,
  ProjectSettings,
  ProjectState,
  ScannedModSummary,
  WorkspaceTab,
} from '../../types';
import { BasicsPanel } from './BasicsPanel';
import { PlaceholderPanel } from './PlaceholderPanel';
import { SettingsPanel } from './SettingsPanel';

const placeholderCopy: Record<'weapons' | 'race' | 'clothing', string> = {
  weapons: 'Weapon editors will live here once the workplace shell is stable.',
  race: 'Race authoring and inherited defs will slot into this workspace panel next.',
  clothing: 'Clothing item editors and textile helpers are reserved for this panel.',
};

interface Props {
  activeTab: WorkspaceTab;
  projectState: ProjectState;
  projectDraft: ProjectSettings;
  selectedMods: ScannedModSummary[];
  availableMods: ScannedModSummary[];
  globalDraft: GlobalSettings;
  busy: boolean;
  scanStatus: AppBootstrap['scanStatus'];
  onTabChange: (tab: WorkspaceTab) => void;
  onCloseProject: () => void;
  onProjectSettingsChange: (settings: ProjectSettings) => void;
  onToggleSelectedMod: (mod: ScannedModSummary) => void;
  onSaveProjectSettings: () => void;
  onGlobalGamePathChange: (value: string) => void;
  onGlobalScanModsEnabledChange: (enabled: boolean) => void;
  onBrowseGamePath: () => void;
  onSaveGlobalSettings: () => void;
  onTriggerRescan: () => void;
}

export function WorkspaceShell({
  activeTab,
  projectState,
  projectDraft,
  selectedMods,
  availableMods,
  globalDraft,
  busy,
  scanStatus,
  onTabChange,
  onCloseProject,
  onProjectSettingsChange,
  onToggleSelectedMod,
  onSaveProjectSettings,
  onGlobalGamePathChange,
  onGlobalScanModsEnabledChange,
  onBrowseGamePath,
  onSaveGlobalSettings,
  onTriggerRescan,
}: Props) {
  return (
    <main className="workspace-layout">
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
        <header className="workspace-header">
          <div>
            <p className="eyebrow">Current Project</p>
            <h2>{projectState.summary.name}</h2>
            <p className="muted-path">{projectState.summary.path}</p>
          </div>

          <div className="status-panel">
            <span className={`status-pill status-${scanStatus.state}`}>{scanStatus.state}</span>
            <p>{scanStatus.message}</p>
          </div>
        </header>

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
            busy={busy}
            globalDraft={globalDraft}
            projectDraft={projectDraft}
            scanStatus={scanStatus}
            onBrowseGamePath={onBrowseGamePath}
            onGlobalGamePathChange={onGlobalGamePathChange}
            onGlobalScanModsEnabledChange={onGlobalScanModsEnabledChange}
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
      </section>
    </main>
  );
}
