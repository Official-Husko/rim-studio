import type { ApplyReportDto } from 'rimstudio-ipc-types';
import { Badge, Banner } from 'rimstudio-ui';
import { formatBytes } from '~/shared/format';
import { t, tn } from '~/shared/i18n';
import { folderOf } from '../../output-model';
import { severityText } from '../editor/DiagnosticNotes';
import { shortHash } from '../../model/assets';
import { actionText, actionTone, kindText, severityTone } from './labels';

export interface ApplyResultProps {
  report: ApplyReportDto;
}

/** What a finished apply did: the files written, the backups, the dry run and what to look at. */
export function ApplyResult({ report }: ApplyResultProps) {
  const backups = report.written.filter((f) => f.backupPath !== undefined);
  const backupFolder = backups[0]?.backupPath ? folderOf(folderOf(backups[0].backupPath)) : '';
  const notable = report.diagnostics.filter(
    (d) => d.severity === 'error' || d.severity === 'warning',
  );
  return (
    <div class="flex flex-col gap-3">
      <Banner tone="success" title={tn('designer.output.result.written', report.written.length)}>
        {report.unchanged.length > 0
          ? tn('designer.output.result.unchanged', report.unchanged.length)
          : t('designer.output.result.allWritten')}
      </Banner>
      <ul aria-label={t('designer.output.result.filesLabel')} class="flex flex-col gap-2">
        {report.written.map((file) => (
          <li key={file.path} class="flex flex-col gap-1 border border-line p-2">
            <span class="break-all font-mono text-mono text-fg">{file.path}</span>
            <span class="flex flex-wrap items-center gap-2">
              <Badge tone={actionTone(file.action)}>{actionText(file.action)}</Badge>
              {file.kind ? <Badge>{kindText(file.kind, file.path)}</Badge> : null}
              <span class="font-mono text-mono-small text-muted">{formatBytes(file.bytes)}</span>
              {file.sha256 ? (
                <span class="font-mono text-mono-small text-faint">{shortHash(file.sha256)}</span>
              ) : null}
              <Badge tone={file.verified ? 'success' : 'danger'}>
                {file.verified
                  ? t('designer.output.result.verified')
                  : t('designer.output.result.notVerified')}
              </Badge>
            </span>
            {file.backupPath ? (
              <span class="break-all font-mono text-mono-small text-faint">
                {t('designer.output.result.backupOf', { path: file.backupPath })}
              </span>
            ) : null}
          </li>
        ))}
      </ul>
      <p class="text-small text-muted">
        {backups.length > 0
          ? t('designer.output.result.backupFolder', { folder: backupFolder })
          : t('designer.output.result.noBackup')}
      </p>
      {report.dryApplyOk === true ? (
        <Banner tone="success">{t('designer.output.result.dryOk')}</Banner>
      ) : null}
      {report.dryApplyOk === false ? (
        <Banner tone="error" title={t('designer.output.result.dryFailed')}>
          {t('designer.output.result.dryFailedBody')}
        </Banner>
      ) : null}
      {notable.length > 0 ? (
        <ul aria-label={t('designer.output.result.notesLabel')} class="flex flex-col gap-2">
          {notable.map((d, i) => (
            <li key={`${d.code}-${i}`} class="flex items-start gap-2 text-body">
              <Badge tone={severityTone(d.severity)}>{severityText(d.severity)}</Badge>
              <span class="min-w-0 flex-1 break-words">
                {d.message} <span class="font-mono text-mono-small text-faint">{d.code}</span>
              </span>
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}
