import { h } from 'preact';
import logo from '../assets/images/logo-universal.png';
import { Button } from '../components/ui/Button';
import type { RecentProject } from '../types';

interface Props {
  recentProjects: RecentProject[];
  busy: boolean;
  onNewProject: () => void;
  onLoadProject: () => void;
  onOpenRecentProject: (path: string) => void;
}

export function StartScreen({
  recentProjects,
  busy,
  onNewProject,
  onLoadProject,
  onOpenRecentProject,
}: Props) {
  const hasRecentProjects = recentProjects.length > 0;

  return (
    <main className="start-layout compact-start-layout">
      <section className="splash-panel">
        <img className="splash-logo" src={logo} alt="RimStudio" />
        <p className="eyebrow splash-eyebrow">RimWorld Mod Toolkit</p>

        <div className="splash-actions">
          <Button disabled={busy} onClick={onNewProject} variant="primary">
            New Project
          </Button>
          <Button disabled={busy} onClick={onLoadProject} variant="secondary">
            Load Project
          </Button>
        </div>

        <div className="splash-divider" />

        <div className="splash-section">
          <div className="compact-heading">
            <p className="eyebrow">Recent Projects</p>
            <span className="small-muted">{hasRecentProjects ? 'Latest projects' : 'Nothing opened yet'}</span>
          </div>

          {hasRecentProjects ? (
            <div className="recent-list compact-recent-list">
              {recentProjects.map((project) => (
                <button
                  key={project.path}
                  className="recent-item compact-recent-item"
                  onClick={() => onOpenRecentProject(project.path)}
                  type="button"
                  disabled={busy}
                >
                  <div className="recent-item-header">
                    <strong>{project.name}</strong>
                  </div>
                  <span>{project.packageId || 'No package ID'}</span>
                  <code>{project.path}</code>
                </button>
              ))}
            </div>
          ) : (
            <div className="empty-recent-state">
              <div className="empty-recent-icon">
                <i className="fa-solid fa-folder-open" aria-hidden="true" />
              </div>
              <div>
                <strong>No recent projects</strong>
                <p>Create a new mod project or load an existing RimWorld mod folder to get started.</p>
              </div>
            </div>
          )}
        </div>
      </section>
    </main>
  );
}
