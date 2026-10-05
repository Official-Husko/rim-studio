import { Badge, Banner, Button, KeyValueList, Panel } from 'rimstudio-ui';
import { formatBytes, formatNumber } from '~/shared/format';
import { t } from '~/shared/i18n';
import { ProjectCounts } from './ProjectCounts';
import type { ProjectView } from './store';

const PROFILE_KEYS = {
  rimstudio: 'project.profile.rimstudio',
  'core-style': 'project.profile.core-style',
  flat: 'project.profile.flat',
} as const;

export interface ProjectHeaderProps {
  view: ProjectView;
  refreshing: boolean;
  onRefresh: () => void;
  onOpen: () => void;
  onNew: () => void;
  onClose: () => void;
}

/** The info card of the open project: identity, how it is laid out and what it holds. */
export function ProjectHeader({
  view,
  refreshing,
  onRefresh,
  onOpen,
  onNew,
  onClose,
}: ProjectHeaderProps) {
  const { summary, tree } = view;
  const loadFolders = !summary.hasLoadFolders
    ? t('project.info.loadfolders.none')
    : summary.hasCeGate
      ? t('project.info.loadfolders.gated')
      : t('project.info.loadfolders.present');
  const ce = !tree.ceFolderExists
    ? t('project.info.ce.none', { path: tree.ceFolder })
    : tree.ceFolderLegacy
      ? t('project.info.ce.legacy', { path: tree.ceFolder })
      : t('project.info.ce.standard', { path: tree.ceFolder });
  return (
    <Panel
      title={t('project.info.title')}
      framed
      collapsible
      actions={
        <div class="flex items-center gap-2">
          <Button size="sm" variant="ghost" icon="refresh" loading={refreshing} onClick={onRefresh}>
            {t('project.refresh')}
          </Button>
          <Button size="sm" variant="secondary" icon="folder" onClick={onOpen}>
            {t('project.open.action')}
          </Button>
          <Button size="sm" variant="secondary" icon="plus" onClick={onNew}>
            {t('project.new.action')}
          </Button>
          <Button size="sm" variant="ghost" icon="close" onClick={onClose}>
            {t('project.close')}
          </Button>
        </div>
      }
    >
      <div class="grid grid-cols-1 gap-x-8 gap-y-4 p-3 md:grid-cols-2 xl:grid-cols-[1fr_1fr_minmax(0,22rem)]">
        <KeyValueList
          label={t('project.info.title')}
          items={[
            { key: t('project.info.name'), value: <strong>{summary.name}</strong> },
            {
              key: t('project.info.packageId'),
              value: summary.packageId ?? t('project.info.none'),
              mono: true,
            },
            {
              key: t('project.info.versions'),
              value: (
                <span class="flex flex-wrap gap-1">
                  {summary.supportedVersions.length > 0
                    ? summary.supportedVersions.map((v) => <Badge key={v}>{v}</Badge>)
                    : t('project.info.none')}
                </span>
              ),
            },
            {
              key: t('project.info.path'),
              value: (
                <span class="block truncate" title={summary.path}>
                  {summary.path}
                </span>
              ),
              mono: true,
            },
          ]}
        />
        <KeyValueList
          label={t('project.info.layout')}
          items={[
            {
              key: t('project.info.profile'),
              value: <Badge tone="info">{t(PROFILE_KEYS[tree.profile])}</Badge>,
            },
            { key: t('project.info.weapons'), value: tree.weaponsFolder, mono: true },
            ...(tree.contentFolder
              ? [{ key: t('project.info.content'), value: tree.contentFolder, mono: true }]
              : []),
            { key: t('project.info.loadfolders'), value: loadFolders },
            { key: t('project.info.ce'), value: ce },
          ]}
        />
        <div class="flex flex-col gap-2 md:col-span-2 xl:col-span-1">
          <ProjectCounts counts={tree.counts} />
          <p class="m-0 text-small text-muted">
            {t('project.info.size', {
              folders: formatNumber(tree.counts.folders, 0),
              files: formatNumber(tree.counts.files, 0),
              size: formatBytes(tree.counts.bytes),
            })}
          </p>
        </div>
      </div>
      {summary.diagnostics.map((diag) => (
        <div key={`${diag.code}:${diag.message}`} class="px-3 pb-3">
          <Banner tone={diag.severity === 'error' ? 'error' : 'warning'}>{diag.message}</Banner>
        </div>
      ))}
    </Panel>
  );
}
