import { Badge, Banner, Button, EmptyState } from 'rimstudio-ui';
import type { ProjectLayoutCheckDto, ProjectScaffoldMissingDto } from 'rimstudio-ipc-types';
import type { ApiError } from 'rimstudio-ipc-types';
import { t, tn } from '~/shared/i18n';
import { IssueRow } from './IssueRow';

export interface LayoutPanelProps {
  check: ProjectLayoutCheckDto;
  fixing: boolean;
  fixResult: ProjectScaffoldMissingDto | undefined;
  fixError: ApiError | undefined;
  onFix: () => void;
  onShowPath: (path: string) => void;
}

/** The issues of the layout check, worst first, with the one automatic fix. */
export function LayoutPanel({
  check,
  fixing,
  fixResult,
  fixError,
  onFix,
  onShowPath,
}: LayoutPanelProps) {
  const created = fixResult && !fixResult.dryRun ? fixResult.folders : [];
  return (
    <div class="flex flex-col gap-3 p-3">
      <div class="flex flex-wrap items-center gap-2">
        {check.errors > 0 ? (
          <Badge tone="danger">{tn('project.layout.errors', check.errors)}</Badge>
        ) : null}
        {check.warnings > 0 ? (
          <Badge tone="warning">{tn('project.layout.warnings', check.warnings)}</Badge>
        ) : null}
        {check.infos > 0 ? (
          <Badge tone="info">{tn('project.layout.infos', check.infos)}</Badge>
        ) : null}
        <span class="flex-1" />
        {check.autoFixable > 0 ? (
          <Button size="sm" icon="plus" loading={fixing} onClick={onFix}>
            {tn('project.layout.fix', check.autoFixable)}
          </Button>
        ) : null}
      </div>
      {fixError ? (
        <Banner tone="error" title={t('project.layout.fix.error')}>
          {fixError.message}
        </Banner>
      ) : null}
      {created.length > 0 ? (
        <Banner tone="success" title={tn('project.layout.fixed', created.length)}>
          <span class="font-mono text-mono">{created.join(', ')}</span>
        </Banner>
      ) : null}
      {check.issues.length === 0 ? (
        <EmptyState
          compact
          icon="check"
          title={t('project.layout.clean.title')}
          description={t('project.layout.clean.hint')}
        />
      ) : (
        <ul
          class="m-0 flex list-none flex-col divide-y divide-line-subtle border border-line p-0"
          aria-label={t('project.layout.list')}
        >
          {check.issues.map((issue) => (
            <IssueRow key={`${issue.code}:${issue.path}`} issue={issue} onShowPath={onShowPath} />
          ))}
        </ul>
      )}
    </div>
  );
}
