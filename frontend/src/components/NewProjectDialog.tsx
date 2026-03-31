import { h } from 'preact';
import type { CreateProjectInput } from '../types';
import { getInputValue } from '../utils/ui';

interface Props {
  open: boolean;
  busy: boolean;
  project: CreateProjectInput;
  onClose: () => void;
  onBrowseLocation: () => void;
  onSubmit: (event: Event) => void;
  onChange: (field: keyof CreateProjectInput, value: string) => void;
}

export function NewProjectDialog({
  open,
  busy,
  project,
  onClose,
  onBrowseLocation,
  onSubmit,
  onChange,
}: Props) {
  if (!open) {
    return null;
  }

  return (
    <section className="dialog-backdrop">
      <form className="dialog-card" onSubmit={onSubmit}>
        <div className="dialog-header">
          <div>
            <p className="eyebrow">New Project</p>
            <h2>Create A RimWorld Mod Skeleton</h2>
          </div>
          <button className="ghost-button" onClick={onClose} type="button">
            Close
          </button>
        </div>

        <label>
          <span>Mod Name</span>
          <input
            value={project.name}
            onInput={(event) => onChange('name', getInputValue(event))}
            placeholder="Example: Steel Frontier"
          />
        </label>

        <label>
          <span>Package ID</span>
          <input
            value={project.packageId}
            onInput={(event) => onChange('packageId', getInputValue(event))}
            placeholder="author.modname"
          />
        </label>

        <label>
          <span>Author</span>
          <input
            value={project.author}
            onInput={(event) => onChange('author', getInputValue(event))}
            placeholder="Author name"
          />
        </label>

        <label>
          <span>Parent Directory</span>
          <div className="inline-field">
            <input
              value={project.location}
              onInput={(event) => onChange('location', getInputValue(event))}
              placeholder="/path/to/mods/folder"
            />
            <button className="secondary-button" onClick={onBrowseLocation} type="button">
              Browse
            </button>
          </div>
        </label>

        <label>
          <span>Target Version</span>
          <select value={project.targetVersion} onInput={(event) => onChange('targetVersion', getInputValue(event))}>
            <option value="1.6">1.6</option>
          </select>
        </label>

        <div className="dialog-actions">
          <button className="primary-button" disabled={busy} type="submit">
            Create Project
          </button>
        </div>
      </form>
    </section>
  );
}
