import type { DetectionReportDto, PathFieldDto } from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';
import { pickFolder } from '~/shared/platform';
import { overridePath } from './detectStore';
import { PathOverrideRow } from './PathOverrideRow';

/** The three overrides of detection: install, Steam root and user data folder. */
export function PathOverrides({ report }: { report: DetectionReportDto }) {
  const install = report.installs.find((i) => i.id === report.selected.install);
  const userDir = report.userDirs.find((u) => u.path === report.selected.userDir);
  const steam = report.steamRoots.find((r) => r.how === 'override') ?? report.steamRoots[0];

  const choose = async (field: PathFieldDto, start: string | undefined): Promise<void> => {
    const picked = await pickFolder(start ? { start } : {});
    if (picked) await overridePath(field, picked);
  };

  return (
    <ul class="m-0 flex list-none flex-col gap-2 p-0" aria-label={t('setup.override.title')}>
      <PathOverrideRow
        label="setup.override.install"
        value={install?.path}
        overridden={install?.how === 'override'}
        onChoose={() => void choose('game-install', install?.path)}
        onClear={() => void overridePath('game-install', undefined)}
      />
      <PathOverrideRow
        label="setup.override.steam"
        value={steam?.path}
        overridden={steam?.how === 'override'}
        onChoose={() => void choose('steam-root', steam?.path)}
        onClear={() => void overridePath('steam-root', undefined)}
      />
      <PathOverrideRow
        label="setup.override.userdir"
        value={userDir?.path}
        overridden={userDir?.kind === 'override'}
        onChoose={() => void choose('user-dir', userDir?.path)}
        onClear={() => void overridePath('user-dir', undefined)}
      />
    </ul>
  );
}
