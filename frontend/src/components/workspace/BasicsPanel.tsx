import { h } from 'preact';
import { Button } from '../ui/Button';
import { supportedVersions } from '../../constants';
import type { ProjectSettings, ProjectState, ScannedModSummary } from '../../types';

interface Props {
  projectState: ProjectState;
  projectDraft: ProjectSettings;
  selectedMods: ScannedModSummary[];
  busy: boolean;
  onSaveProjectSettings: () => void;
}

export function BasicsPanel({ projectState, projectDraft, selectedMods, busy, onSaveProjectSettings }: Props) {
  return (
    <div className="content-stack">
      <section className="panel-card split-card">
        <div>
          <p className="eyebrow">Overview</p>
          <h3>Project Summary</h3>
          <dl className="summary-grid">
            <div>
              <dt>Name</dt>
              <dd>{projectState.summary.name}</dd>
            </div>
            <div>
              <dt>Package ID</dt>
              <dd>{projectState.summary.packageId || 'Not set'}</dd>
            </div>
            <div>
              <dt>Author</dt>
              <dd>{projectState.summary.author || 'Not set'}</dd>
            </div>
            <div>
              <dt>Config State</dt>
              <dd>{projectState.summary.hasRimStudioConfig ? 'Managed by RimStudio' : 'Imported mod'}</dd>
            </div>
          </dl>
        </div>

        <div>
          <p className="eyebrow">Target Version</p>
          <h3>Version Readiness</h3>
          <div className="version-grid">
            {supportedVersions.map((version) => (
              <button
                key={version}
                className={`version-pill ${version === '1.6' ? 'is-current' : 'is-disabled'}`}
                disabled={version !== '1.6'}
                type="button"
              >
                {version}
              </button>
            ))}
          </div>
          <p className="small-muted">Only 1.6 is active now. Older versions stay visible as disabled future hooks.</p>
        </div>
      </section>

      <section className="panel-card">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Compatibility Scaffold</p>
            <h3>Patch Workflow Placeholder</h3>
          </div>
          <Button disabled={busy} onClick={onSaveProjectSettings} variant="primary">
            Save Project Settings
          </Button>
        </div>

        <div className="compat-overview">
          <div className="stat-card">
            <strong>{projectDraft.compatibility.mode === 'selected' ? 'Selected Mods Only' : 'All Mods'}</strong>
            <span>Future data-loading mode</span>
          </div>
          <div className="stat-card">
            <strong>{selectedMods.length}</strong>
            <span>Compatibility targets queued</span>
          </div>
          <div className="stat-card">
            <strong>{Object.keys(projectDraft.compatibility.patchEntries).length}</strong>
            <span>Placeholder patch entries</span>
          </div>
        </div>

        <div className="empty-card">
          <h4>Auto-generation and editing are reserved for the next compatibility pass</h4>
          <p>
            RimStudio already stores selected external mods and placeholder patch entries in project metadata so the
            generator and editor can be added later without reshaping your project format.
          </p>
          <div className="placeholder-actions">
            <Button disabled variant="secondary">
              Generate Patch
            </Button>
            <Button disabled variant="secondary">
              Edit Compatibility
            </Button>
          </div>
        </div>

        <div className="selected-mods">
          {selectedMods.length === 0 ? (
            <p className="small-muted">No external mods selected yet. Use Settings to configure compatibility targets.</p>
          ) : (
            selectedMods.map((mod) => (
              <div className="selected-mod-chip" key={mod.id}>
                <strong>{mod.name}</strong>
                <span>{mod.packageId || mod.id}</span>
              </div>
            ))
          )}
        </div>
      </section>
    </div>
  );
}
