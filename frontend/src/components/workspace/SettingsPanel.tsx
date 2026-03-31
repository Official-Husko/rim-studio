import { h } from 'preact';
import type { GameScanStatus, GlobalSettings, ProjectSettings, ScannedModSummary } from '../../types';
import { getInputValue } from '../../utils/ui';

interface Props {
  projectDraft: ProjectSettings;
  globalDraft: GlobalSettings;
  availableMods: ScannedModSummary[];
  busy: boolean;
  scanStatus: GameScanStatus;
  onProjectSettingsChange: (settings: ProjectSettings) => void;
  onToggleSelectedMod: (mod: ScannedModSummary) => void;
  onSaveProjectSettings: () => void;
  onGlobalGamePathChange: (value: string) => void;
  onGlobalScanModsEnabledChange: (enabled: boolean) => void;
  onBrowseGamePath: () => void;
  onSaveGlobalSettings: () => void;
  onTriggerRescan: () => void;
}

export function SettingsPanel({
  projectDraft,
  globalDraft,
  availableMods,
  busy,
  scanStatus,
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
    <div className="content-stack">
      <section className="panel-card">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Game Data</p>
            <h3>RimWorld Install And Scan Controls</h3>
          </div>
          <button className="secondary-button" disabled={busy} onClick={onTriggerRescan} type="button">
            Rescan Data
          </button>
        </div>

        <label>
          <span>RimWorld Install Path</span>
          <div className="inline-field">
            <input
              value={globalDraft.gamePath}
              onInput={(event) => onGlobalGamePathChange(getInputValue(event))}
              placeholder="/path/to/RimWorld"
            />
            <button className="secondary-button" onClick={onBrowseGamePath} type="button">
              Browse
            </button>
          </div>
        </label>

        <label className="toggle-card">
          <input
            checked={globalDraft.scanModsEnabled}
            onChange={(event) => onGlobalScanModsEnabledChange((event.currentTarget as HTMLInputElement).checked)}
            type="checkbox"
          />
          <div>
            <strong>Scan installed mods metadata</strong>
            <p>
              Off by default. Enabling this indexes installed external mods for selection and future compatibility work,
              but may increase scan time and surface low-quality data.
            </p>
          </div>
        </label>

        <div className="scan-strip">
          <span className={`status-pill status-${scanStatus.state}`}>{scanStatus.state}</span>
          <p>{scanStatus.message}</p>
          <p className="small-muted">
            Sources indexed: {scanStatus.scannedSources} · Available mods: {scanStatus.availableModCount}
          </p>
        </div>

        <button className="primary-button" disabled={busy} onClick={onSaveGlobalSettings} type="button">
          Save Global Settings
        </button>
      </section>

      <section className="panel-card">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Compatibility Targets</p>
            <h3>Project Mod Selection Scaffold</h3>
          </div>
          <button className="primary-button" disabled={busy} onClick={onSaveProjectSettings} type="button">
            Save Project Settings
          </button>
        </div>

        <div className="selection-mode">
          <button
            className={`mode-button ${projectDraft.compatibility.mode === 'all' ? 'is-active' : ''}`}
            onClick={() =>
              onProjectSettingsChange({
                ...projectDraft,
                compatibility: {
                  ...projectDraft.compatibility,
                  mode: 'all',
                },
              })
            }
            type="button"
          >
            All Mods
          </button>
          <button
            className={`mode-button ${projectDraft.compatibility.mode === 'selected' ? 'is-active' : ''}`}
            onClick={() =>
              onProjectSettingsChange({
                ...projectDraft,
                compatibility: {
                  ...projectDraft.compatibility,
                  mode: 'selected',
                },
              })
            }
            type="button"
          >
            Selected Mods Only
          </button>
        </div>

        <p className="small-muted">
          The storage model is ready now. Targeted loading is represented by project metadata and UI state so the next
          pass can reduce loaded data without changing file format.
        </p>

        {availableMods.length === 0 ? (
          <div className="empty-card">
            <h4>No scanned external mods available</h4>
            <p>
              Set the game path and enable mod scanning to populate compatibility targets. Base game and DLC indexing can
              still run without external mod metadata.
            </p>
          </div>
        ) : (
          <div className="mod-list">
            {availableMods.map((mod) => {
              const checked = projectDraft.compatibility.selectedModIds.includes(mod.id);
              return (
                <label className={`mod-item ${checked ? 'is-checked' : ''}`} key={mod.id}>
                  <input checked={checked} onChange={() => onToggleSelectedMod(mod)} type="checkbox" />
                  <div>
                    <strong>{mod.name}</strong>
                    <span>{mod.packageId || mod.id}</span>
                  </div>
                </label>
              );
            })}
          </div>
        )}
      </section>
    </div>
  );
}
