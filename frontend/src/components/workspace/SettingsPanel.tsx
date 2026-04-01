import { h } from 'preact';
import { builtInThemes } from '../../constants';
import type {
  CustomThemeSummary,
  GameScanStatus,
  GlobalSettings,
  ProjectSettings,
  ScannedModSummary,
  ThemeID,
} from '../../types';
import { getInputValue } from '../../utils/ui';

const themeScrollerThemes = [...builtInThemes, ...builtInThemes];

interface Props {
  projectDraft: ProjectSettings;
  globalDraft: GlobalSettings;
  availableMods: ScannedModSummary[];
  availableCustomThemes: CustomThemeSummary[];
  busy: boolean;
  scanStatus: GameScanStatus;
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

export function SettingsPanel({
  projectDraft,
  globalDraft,
  availableMods,
  availableCustomThemes,
  busy,
  scanStatus,
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
    <div className="content-stack">
      <section className="panel-card">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Appearance</p>
            <h3>Theme Library</h3>
          </div>
        </div>

        <div className="theme-scroller">
          <div className="theme-track">
            {themeScrollerThemes.map((theme, index) => (
              <button
                key={`${theme.id}-${index}`}
                className={`theme-option ${globalDraft.themeId === theme.id ? 'is-active' : ''}`}
                onClick={() => onGlobalThemeIDChange(theme.id)}
                type="button"
              >
                <div className={`theme-preview ${theme.previewClassName}`} aria-hidden="true" />
                <div className="theme-option-copy">
                  <strong>{theme.name}</strong>
                  <p>{theme.description}</p>
                </div>
              </button>
            ))}
          </div>
        </div>

        <div className="empty-card">
          <h4>Six built-in palettes are available now</h4>
          <p>
            The first three follow the Rim-like neutral, workshop, and blueprint directions directly. The rest give you
            warmer, archivist, and relay-style variants without changing the theme system again later.
          </p>
        </div>

        <div className="theme-folder-heading">
          <div>
            <p className="eyebrow">External Themes</p>
            <h4>Loaded From `data/themes`</h4>
          </div>
          <button
            className="ghost-button"
            disabled={!globalDraft.customCssPath}
            onClick={onClearCustomCSSPath}
            type="button"
          >
            Clear Override
          </button>
        </div>

        {availableCustomThemes.length === 0 ? (
          <div className="empty-card">
            <h4>No external theme files found</h4>
            <p>
              Place `.css` files in the app&apos;s `data/themes` folder next to the executable. Rim-Studio will discover
              them automatically and load them as optional overrides on top of the selected built-in palette.
            </p>
          </div>
        ) : (
          <div className="custom-theme-list">
            {availableCustomThemes.map((theme) => {
              const isActive = globalDraft.customCssPath === theme.path;
              return (
                <button
                  key={theme.path}
                  className={`custom-theme-item ${isActive ? 'is-active' : ''}`}
                  onClick={() => onGlobalCustomCSSPathChange(isActive ? '' : theme.path)}
                  type="button"
                >
                  <div>
                    <strong>{theme.name}</strong>
                    <code>{theme.path}</code>
                  </div>
                  <span>{isActive ? 'Active' : 'Load'}</span>
                </button>
              );
            })}
          </div>
        )}
      </section>

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
