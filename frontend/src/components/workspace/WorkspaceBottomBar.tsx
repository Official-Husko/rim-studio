import { h } from 'preact';
import type { AppInfo, GameScanStatus, ProjectContentCounts } from '../../types';

interface Props {
  appInfo: AppInfo;
  contentCounts: ProjectContentCounts;
  scanStatus: GameScanStatus;
  activeTaskLabel: string;
}

export function WorkspaceBottomBar({ appInfo, contentCounts, scanStatus, activeTaskLabel }: Props) {
  const scanStatusLabel = scanStatus.state === 'scanning'
    ? 'processing'
    : scanStatus.state === 'ready'
      ? 'indexed'
      : 'unindexed';
  const hasActiveTask = Boolean(activeTaskLabel);

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

      <div className={`bottom-bar-task ${hasActiveTask ? 'is-active' : ''}`}>
        <span className={`bottom-task-indicator ${hasActiveTask ? 'is-active' : ''}`} aria-hidden="true" />
        <span className={`bottom-task-text ${hasActiveTask ? 'is-active' : ''}`}>
          {activeTaskLabel || 'No background task'}
        </span>
      </div>

      <div className="bottom-bar-group bottom-bar-group-right">
        <span className="bottom-bar-text">
          status <strong className={`bottom-status-value status-${scanStatus.state}`}>{scanStatusLabel}</strong>
        </span>
        <span className="bottom-bar-text">
          mods <strong className="bottom-status-value status-ready">{scanStatus.availableModCount}</strong>
        </span>
        <span className="bottom-bar-text">
          dlcs <strong className="bottom-status-value status-scanning">{scanStatus.dlcLoadedCount}</strong>
        </span>
        <span className="bottom-version">
          {appInfo.version} (h{appInfo.commitShort})
        </span>
      </div>
    </footer>
  );
}
