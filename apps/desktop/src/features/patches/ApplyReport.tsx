import { Badge, Banner } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import type { ApplyResult } from './applyStore';
import { DiagnosticList } from './DiagnosticList';

export interface ApplyReportProps {
  results: readonly ApplyResult[];
}

/** What the apply did, weapon by weapon: files written, backups, the read back and the dry apply. */
export function ApplyReport({ results }: ApplyReportProps) {
  const done = results.filter((r) => r.report).length;
  const failed = results.some((r) => r.error || r.skipped);
  return (
    <div class="flex flex-col gap-3">
      <Banner tone={failed ? 'warning' : 'success'} title={tn('patches.report.title', done)}>
        {failed ? t('patches.report.partial') : t('patches.report.ok')}
      </Banner>
      {results.map((r) => (
        <section
          key={r.defName}
          aria-label={r.defName}
          class="flex flex-col gap-2 rounded-sm border border-line p-3"
        >
          <div class="flex flex-wrap items-center gap-2">
            <span class="font-mono text-mono">{r.defName}</span>
            {r.report ? (
              <Badge tone={r.report.dryApplyOk === false ? 'warning' : 'success'}>
                {r.report.dryApplyOk === undefined
                  ? t('patches.report.written')
                  : r.report.dryApplyOk
                    ? t('patches.report.dry-ok')
                    : t('patches.report.dry-failed')}
              </Badge>
            ) : (
              <Badge tone="danger">{t('patches.report.not-applied')}</Badge>
            )}
          </div>
          {r.error ? (
            <p class="m-0 text-small text-danger">
              <code class="font-mono">{r.error.code}</code> {r.error.message}
            </p>
          ) : null}
          {r.skipped && !r.error ? (
            <p class="m-0 text-small text-muted">{t('patches.report.skipped')}</p>
          ) : null}
          {r.report ? (
            <ul class="m-0 flex list-none flex-col gap-1 p-0 text-small">
              {r.report.written.map((f) => (
                <li key={f.path} class="flex flex-col gap-0.5">
                  <span class="flex flex-wrap items-center gap-2">
                    <code class="font-mono text-mono">{f.path}</code>
                    <Badge tone="neutral">{f.action}</Badge>
                    <Badge tone={f.verified ? 'success' : 'danger'}>
                      {f.verified ? t('patches.report.verified') : t('patches.report.unverified')}
                    </Badge>
                  </span>
                  {f.backupPath ? (
                    <span class="text-muted">
                      {t('patches.report.backup')}{' '}
                      <code class="font-mono text-mono-small">{f.backupPath}</code>
                    </span>
                  ) : null}
                </li>
              ))}
              {r.report.unchanged.map((p) => (
                <li key={p} class="text-muted">
                  <code class="font-mono text-mono">{p}</code> {t('patches.report.unchanged')}
                </li>
              ))}
            </ul>
          ) : null}
          {r.report && r.report.diagnostics.some((d) => d.severity !== 'info') ? (
            <DiagnosticList
              label={t('patches.report.diagnostics', { name: r.defName })}
              diagnostics={r.report.diagnostics.filter((d) => d.severity !== 'info')}
            />
          ) : null}
        </section>
      ))}
    </div>
  );
}
