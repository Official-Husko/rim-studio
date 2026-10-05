import type { LintedFileDto } from 'rimstudio-ipc-types';
import { Badge } from 'rimstudio-ui';
import { formatBytes } from '~/shared/format';
import { t, tn } from '~/shared/i18n';
import { SEVERITY_ORDER } from './model';
import { LintFindingRow } from './LintFindingRow';

const STATUS_TEXT = {
  checked: 'patches.lint.status.checked',
  'parse-failed': 'patches.lint.status.parse-failed',
  doctype: 'patches.lint.status.doctype',
  'too-large': 'patches.lint.status.too-large',
  missing: 'patches.lint.status.missing',
  unreadable: 'patches.lint.status.unreadable',
  'not-a-patch': 'patches.lint.status.not-a-patch',
} as const;

export interface LintFileSectionProps {
  file: LintedFileDto;
}

/** One patch file of the project: its size, its operations and its findings. */
export function LintFileSection({ file }: LintFileSectionProps) {
  const ok = file.status === 'checked';
  // worst first; findings of the same severity stay in file order
  const findings = [...file.findings].sort(
    (a, b) => SEVERITY_ORDER.indexOf(a.severity) - SEVERITY_ORDER.indexOf(b.severity),
  );
  return (
    <section aria-label={file.path} class="flex flex-col gap-2 rounded-sm border border-line p-3">
      <div class="flex flex-wrap items-center gap-x-3 gap-y-1">
        <h2 class="m-0 font-mono text-mono font-semibold break-all">{file.path}</h2>
        <span class="text-small text-muted">
          {formatBytes(file.bytes)}
          {ok ? `, ${tn('patches.lint.operations', file.operations)}` : ''}
        </span>
        {ok ? null : <Badge tone="warning">{t(STATUS_TEXT[file.status])}</Badge>}
      </div>
      {ok && file.findings.length === 0 ? (
        <p class="m-0 text-small text-muted">{t('patches.lint.clean')}</p>
      ) : null}
      {!ok ? <p class="m-0 text-small text-muted">{t('patches.lint.skipped')}</p> : null}
      {file.findings.length > 0 ? (
        <ul
          aria-label={t('patches.lint.of', { name: file.path })}
          class="m-0 flex list-none flex-col gap-3 p-0"
        >
          {findings.map((finding, index) => (
            <LintFindingRow
              key={`${finding.code}-${finding.field ?? ''}-${index}`}
              finding={finding}
            />
          ))}
        </ul>
      ) : null}
    </section>
  );
}
