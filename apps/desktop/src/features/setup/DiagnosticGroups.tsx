import { Badge, type BadgeProps } from 'rimstudio-ui';
import type { DiagnosticSummaryDto } from 'rimstudio-ipc-types';
import { t, tn } from '~/shared/i18n';
import { DIAGNOSTIC_KEYS } from './codes';
import { groupDiagnostics } from './model';

const TONES: Record<string, BadgeProps['tone']> = {
  error: 'danger',
  warning: 'warning',
  info: 'info',
  hint: 'neutral',
};

/** Scan diagnostics grouped by code with the count and the samples the backend kept. */
export function DiagnosticGroups({ summary }: { summary: DiagnosticSummaryDto }) {
  const groups = groupDiagnostics(summary);
  if (groups.length === 0) {
    return <p class="m-0 text-muted">{t('setup.scan.diag.none')}</p>;
  }
  return (
    <ul class="m-0 flex list-none flex-col gap-3 p-0" aria-label={t('setup.scan.diag.title')}>
      {groups.map((group) => {
        const key = DIAGNOSTIC_KEYS[group.code];
        return (
          <li key={group.code} class="flex flex-col gap-1">
            <div class="flex flex-wrap items-center gap-2">
              <Badge tone={TONES[group.severity]}>{group.severity}</Badge>
              <span class="font-mono text-mono">{group.code}</span>
              <span class="text-muted">{tn('setup.scan.diag.count', group.count)}</span>
            </div>
            {key ? <p class="m-0 text-small text-muted">{t(key)}</p> : null}
            {group.samples.length > 0 ? (
              <details>
                <summary class="cursor-pointer text-small text-muted">
                  {tn('setup.scan.diag.samples', group.samples.length)}
                </summary>
                <ul class="m-0 mt-1 list-disc pl-5 text-small">
                  {group.samples.map((sample, i) => (
                    <li key={`${sample.modIdx ?? 'x'}-${i}`} class="break-words">
                      {sample.message}
                    </li>
                  ))}
                </ul>
              </details>
            ) : null}
          </li>
        );
      })}
      {summary.truncated ? (
        <li class="text-small text-muted">{t('setup.scan.diag.truncated')}</li>
      ) : null}
    </ul>
  );
}
