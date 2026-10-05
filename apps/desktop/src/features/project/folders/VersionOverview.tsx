import { Badge, Button, Panel } from 'rimstudio-ui';
import type { ProjectLoadFoldersDto } from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';
import { applyFoldersOp } from './folderStore';

export interface VersionOverviewProps {
  data: ProjectLoadFoldersDto;
  disabled: boolean;
  onAddFolder: () => void;
}

/** For each supported game version: does its folder exist, and does LoadFolders.xml have a block for it. */
export function VersionOverview({ data, disabled, onAddFolder }: VersionOverviewProps) {
  const blocked = new Set(data.blocks.map((b) => b.key));
  const rows = data.supportedVersions;
  return (
    <Panel
      title={t('project.folders.overview')}
      framed
      actions={
        <Button size="sm" icon="plus" onClick={onAddFolder}>
          {t('project.folders.addVersion')}
        </Button>
      }
    >
      <div class="flex flex-col gap-2 p-3">
        <p class="m-0 text-small text-muted">{t('project.folders.overview.help')}</p>
        {rows.length === 0 ? (
          <p class="m-0 text-small text-faint">{t('project.folders.overview.none')}</p>
        ) : (
          <table class="w-full border-collapse text-body">
            <caption class="sr-only">{t('project.folders.overview')}</caption>
            <thead>
              <tr class="text-left text-small text-muted">
                <th scope="col" class="py-1 pr-4 font-semibold">
                  {t('project.folders.col.version')}
                </th>
                <th scope="col" class="py-1 pr-4 font-semibold">
                  {t('project.folders.col.folder')}
                </th>
                <th scope="col" class="py-1 font-semibold">
                  {t('project.folders.col.block')}
                </th>
              </tr>
            </thead>
            <tbody>
              {rows.map((version) => (
                <tr key={version} class="border-t border-line-subtle">
                  <th scope="row" class="py-1 pr-4 text-left font-mono font-normal">
                    {version}
                  </th>
                  <td class="py-1 pr-4">
                    {data.versionFolders.includes(version) ? (
                      <Badge tone="success">{t('project.folders.exists')}</Badge>
                    ) : (
                      <Badge>{t('project.folders.none')}</Badge>
                    )}
                  </td>
                  <td class="py-1">
                    {blocked.has(version) ? (
                      <Badge tone="success">{t('project.folders.hasBlock')}</Badge>
                    ) : data.exists ? (
                      <Button
                        size="sm"
                        variant="secondary"
                        disabled={disabled}
                        aria-label={t('project.folders.addBlock.for', { version })}
                        onClick={() =>
                          void applyFoldersOp({
                            op: 'add-block',
                            version,
                            entries: [{ path: '/' }],
                          })
                        }
                      >
                        {t('project.folders.addBlock')}
                      </Button>
                    ) : (
                      <Badge>{t('project.folders.none')}</Badge>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </Panel>
  );
}
