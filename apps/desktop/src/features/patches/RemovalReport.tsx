import type { ApplyReportDto, WritePlanDto } from 'rimstudio-ipc-types';
import { Badge, Banner } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import { DiagnosticList } from './DiagnosticList';
import { removalActionText } from './RemovalReview';
import type { RemovalTarget } from './removeStore';

export interface RemovalReportProps {
  target: RemovalTarget;
  /** The plan that was applied: a report with fewer files than the plan did not finish. */
  plan: WritePlanDto;
  report: ApplyReportDto;
}

/** What the removal did: the files changed, where each backup is, and anything that went wrong. */
export function RemovalReport({ target, plan, report }: RemovalReportProps) {
  // the backend stops at the first file it cannot write and reports what it did write
  const partial =
    report.written.length < plan.files.length ||
    report.diagnostics.some((d) => d.severity === 'error');
  const shown = report.diagnostics.filter((d) => d.severity !== 'info');
  return (
    <div class="flex flex-col gap-3">
      <Banner
        tone={partial ? 'warning' : 'success'}
        title={partial ? t('patches.remove.partial.title') : t('patches.remove.done.title')}
      >
        {partial
          ? t('patches.remove.partial.body')
          : tn('patches.remove.done.body', report.written.length)}
      </Banner>
      <ul aria-label={t('patches.remove.written')} class="m-0 flex list-none flex-col gap-2 p-0">
        {report.written.map((file) => (
          <li
            key={file.path}
            class="flex flex-col gap-1 rounded-sm border border-line p-3 text-small"
          >
            <span class="flex flex-wrap items-center gap-x-3 gap-y-1">
              <code class="min-w-0 font-mono text-mono break-all">{file.path}</code>
              <Badge tone={file.action === 'delete' ? 'danger' : 'info'}>
                {removalActionText(file.action)}
              </Badge>
              <Badge tone={file.verified ? 'success' : 'danger'}>
                {file.verified ? t('patches.remove.verified') : t('patches.report.unverified')}
              </Badge>
            </span>
            <span class="text-muted">
              {file.backupPath ? (
                <>
                  {t('patches.report.backup')}{' '}
                  <code class="font-mono text-mono-small">{file.backupPath}</code>
                </>
              ) : (
                t('patches.remove.backup-same')
              )}
            </span>
          </li>
        ))}
      </ul>
      {shown.length > 0 ? (
        <DiagnosticList
          diagnostics={shown}
          label={t('patches.report.diagnostics', { name: target.defName })}
        />
      ) : null}
      {!partial && target.mode === 'replace' ? (
        <p class="m-0 text-body">{t('patches.remove.replace-done', { name: target.name })}</p>
      ) : null}
    </div>
  );
}
