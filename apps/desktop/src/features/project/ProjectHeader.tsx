import { Badge, Banner, Button } from 'rimstudio-ui';
import { formatBytes, formatNumber } from '~/shared/format';
import { t, tn } from '~/shared/i18n';
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

/** The header of the open mod: name, package id, folder, game versions and status chips, with the page actions. */
export function ProjectHeader({
  view,
  refreshing,
  onRefresh,
  onOpen,
  onNew,
  onClose,
}: ProjectHeaderProps) {
  const { summary, tree, check } = view;
  const loadFolders = !summary.hasLoadFolders
    ? t('project.info.loadfolders.none')
    : summary.hasCeGate
      ? t('project.info.loadfolders.gated')
      : t('project.info.loadfolders.present');
  const issues = check.issues.length;
  return (
    <header class="bp-ticks flex flex-col gap-3 border border-line bg-surface p-4">
      <div class="flex flex-wrap items-start gap-3">
        <div class="min-w-0 flex-1">
          <h1 class="m-0 truncate font-display text-display font-semibold tracking-display">
            {summary.name}
          </h1>
          <p class="m-0 truncate font-mono text-mono text-muted">
            {summary.packageId ?? t('project.info.none')}
          </p>
          <p class="m-0 truncate font-mono text-mono-small text-faint" title={summary.path}>
            {summary.path}
          </p>
        </div>
        <div class="flex flex-wrap items-center gap-2">
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
      </div>
      <ul
        class="m-0 flex list-none flex-wrap items-center gap-2 p-0"
        aria-label={t('project.header.status')}
      >
        {summary.supportedVersions.length > 0 ? (
          summary.supportedVersions.map((v) => (
            <li key={v}>
              <Badge>{v}</Badge>
            </li>
          ))
        ) : (
          <li>
            <Badge tone="warning">{t('project.header.noVersions')}</Badge>
          </li>
        )}
        <li>
          <Badge tone="info">{t(PROFILE_KEYS[tree.profile])}</Badge>
        </li>
        <li>
          <Badge>{t('project.header.loadFolders', { state: loadFolders })}</Badge>
        </li>
        <li>
          {issues > 0 ? (
            <Badge tone="warning">{tn('project.header.issues', issues)}</Badge>
          ) : (
            <Badge tone="success">{t('project.header.layoutOk')}</Badge>
          )}
        </li>
        <li class="text-small text-muted">
          {t('project.info.size', {
            folders: formatNumber(tree.counts.folders, 0),
            files: formatNumber(tree.counts.files, 0),
            size: formatBytes(tree.counts.bytes),
          })}
        </li>
      </ul>
      {summary.diagnostics.map((diag) => (
        <Banner
          key={`${diag.code}:${diag.message}`}
          tone={diag.severity === 'error' ? 'error' : 'warning'}
        >
          {diag.message}
        </Banner>
      ))}
    </header>
  );
}
