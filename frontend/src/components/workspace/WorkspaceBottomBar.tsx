import { h } from 'preact';
import type { AppInfo, BackgroundTask, GameScanStatus, ProjectContentCounts } from '../../types';
import { BackgroundTaskStatus } from '../background-tasks/BackgroundTaskStatus';

interface Props {
  appInfo: AppInfo;
  contentCounts: ProjectContentCounts;
  scanStatus: GameScanStatus;
  backgroundTasks: BackgroundTask[];
}

export function WorkspaceBottomBar({ appInfo, contentCounts, scanStatus, backgroundTasks }: Props) {
  const scanStatusLabel = scanStatus.state === 'scanning'
    ? 'processing'
    : scanStatus.state === 'ready'
      ? 'indexed'
      : 'unindexed';

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

      <BackgroundTaskStatus tasks={backgroundTasks} />

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
