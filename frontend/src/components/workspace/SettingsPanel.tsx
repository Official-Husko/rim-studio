import { h } from 'preact';
import { Button } from '../ui/Button';
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
import { ThemeCarousel } from './ThemeCarousel';

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

        <ThemeCarousel
          activeThemeID={globalDraft.themeId}
          themes={builtInThemes}
          onSelectTheme={onGlobalThemeIDChange}
        />

        <div className="theme-folder-heading">
          <div>
            <p className="eyebrow">External Themes</p>
            <h4>Loaded From `data/themes`</h4>
          </div>
          <Button disabled={!globalDraft.customCssPath} onClick={onClearCustomCSSPath} variant="ghost">
            Clear Override
          </Button>
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
          <Button disabled={busy} onClick={onTriggerRescan} variant="secondary">
            Rescan Data
          </Button>
        </div>

        <label>
          <span>RimWorld Install Path</span>
          <div className="inline-field">
            <input
              value={globalDraft.gamePath}
              onInput={(event) => onGlobalGamePathChange(getInputValue(event))}
              placeholder="/path/to/RimWorld"
            />
            <Button onClick={onBrowseGamePath} variant="secondary">
              Browse
            </Button>
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

        <Button disabled={busy} onClick={onSaveGlobalSettings} variant="primary">
          Save Global Settings
        </Button>
      </section>

      <section className="panel-card">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Compatibility Targets</p>
            <h3>Project Mod Selection Scaffold</h3>
          </div>
          <Button disabled={busy} onClick={onSaveProjectSettings} variant="primary">
            Save Project Settings
          </Button>
        </div>

        <div className="selection-mode">
          <Button
            active={projectDraft.compatibility.mode === 'all'}
            onClick={() =>
              onProjectSettingsChange({
                ...projectDraft,
                compatibility: {
                  ...projectDraft.compatibility,
                  mode: 'all',
                },
              })
            }
            variant="mode"
          >
            All Mods
          </Button>
          <Button
            active={projectDraft.compatibility.mode === 'selected'}
            onClick={() =>
              onProjectSettingsChange({
                ...projectDraft,
                compatibility: {
                  ...projectDraft.compatibility,
                  mode: 'selected',
                },
              })
            }
            variant="mode"
          >
            Selected Mods Only
          </Button>
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
