import { h } from 'preact';
import type { AppInfo, GameScanStatus, ProjectContentCounts } from '../../types';

interface Props {
  appInfo: AppInfo;
  contentCounts: ProjectContentCounts;
  scanStatus: GameScanStatus;
  activeTaskLabel: string;
}

export function WorkspaceBottomBar({ appInfo, contentCounts, scanStatus, activeTaskLabel }: Props) {
  return (
    <footer className="workspace-bottom-bar">
      <div className="bottom-bar-group bottom-bar-group-left">
        <span className="bottom-bar-text">
          defs <strong>{contentCounts.defs}</strong>
        </span>
        <span className="bottom-bar-text">
          textures <strong>{contentCounts.textures}</strong>
        </span>
        <span className="bottom-bar-text">
          patches <strong>{contentCounts.patches}</strong>
        </span>
      </div>

      <div className="bottom-bar-task">
        <i className={`fa-solid ${activeTaskLabel ? 'fa-loader fa-spin' : 'fa-circle-dot'}`} aria-hidden="true" />
        <span>{activeTaskLabel || 'Idle'}</span>
      </div>

      <div className="bottom-bar-group bottom-bar-group-right">
        <span className="bottom-bar-text">
          status <strong className={`status-${scanStatus.state}`}>{scanStatus.state}</strong>
        </span>
        <span className="bottom-bar-text">
          mods <strong className="status-ready">{scanStatus.availableModCount}</strong>
        </span>
        <span className="bottom-bar-text">
          dlcs <strong className="status-scanning">{scanStatus.dlcLoadedCount}</strong>
        </span>
        <span className="bottom-version">
          {appInfo.version} (h{appInfo.commitShort})
        </span>
      </div>
    </footer>
  );
}
