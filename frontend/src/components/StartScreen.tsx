import { h } from 'preact';
import logo from '../assets/images/logo-universal.png';
import type { RecentProject } from '../types';

interface Props {
  recentProjects: RecentProject[];
  placeholderProjects: RecentProject[];
  busy: boolean;
  onNewProject: () => void;
  onLoadProject: () => void;
  onOpenRecentProject: (path: string) => void;
}

export function StartScreen({
  recentProjects,
  placeholderProjects,
  busy,
  onNewProject,
  onLoadProject,
  onOpenRecentProject,
}: Props) {
  const hasRecentProjects = recentProjects.length > 0;
  const projectsToDisplay = hasRecentProjects ? recentProjects : placeholderProjects;

  return (
    <main className="start-layout compact-start-layout">
      <section className="splash-panel">
        <img className="splash-logo" src={logo} alt="RimStudio" />
        <p className="eyebrow splash-eyebrow">RimWorld Mod Toolkit</p>

        <div className="splash-actions">
          <button className="primary-button" disabled={busy} onClick={onNewProject} type="button">
            New Project
          </button>
          <button className="secondary-button" disabled={busy} onClick={onLoadProject} type="button">
            Load Project
          </button>
        </div>

        <div className="splash-divider" />

        <div className="splash-section">
          <div className="compact-heading">
            <p className="eyebrow">Recent Projects</p>
            <span className="small-muted">{hasRecentProjects ? 'Latest projects' : 'Placeholder examples'}</span>
          </div>

          <div className="recent-list compact-recent-list">
            {projectsToDisplay.map((project) => (
              <button
                key={project.path}
                className={`recent-item compact-recent-item ${!hasRecentProjects ? 'is-placeholder' : ''}`}
                onClick={() => onOpenRecentProject(project.path)}
                type="button"
                disabled={!hasRecentProjects || busy}
              >
                <div className="recent-item-header">
                  <strong>{project.name}</strong>
                  {!hasRecentProjects ? <span className="placeholder-tag">Example</span> : null}
                </div>
                <span>{project.packageId || 'No package ID'}</span>
                <code>{project.path}</code>
              </button>
            ))}
          </div>
        </div>
      </section>
    </main>
  );
}
