import type { LintFindingDto, SeverityDto } from 'rimstudio-ipc-types';
import { Badge, type BadgeTone } from 'rimstudio-ui';
import { t } from '~/shared/i18n';

const TONE: Record<SeverityDto, BadgeTone> = {
  error: 'danger',
  warning: 'warning',
  info: 'info',
  hint: 'neutral',
};

const LABEL = {
  error: 'patches.severity.error',
  warning: 'patches.severity.warning',
  info: 'patches.severity.info',
  hint: 'patches.severity.hint',
} as const;

export interface LintFindingRowProps {
  finding: LintFindingDto;
}

/**
 * One finding: severity word, rule id, code and the backend text, the operation it is in, and the rule
 * explanation and location one click away.
 */
export function LintFindingRow({ finding }: LintFindingRowProps) {
  const hasDetails = Boolean(finding.explanation ?? finding.field ?? finding.xpath);
  return (
    <li class="flex flex-col gap-1 text-small">
      <div class="flex flex-wrap items-baseline gap-x-2 gap-y-1">
        <Badge tone={TONE[finding.severity]}>{t(LABEL[finding.severity])}</Badge>
        {finding.ruleId ? (
          <code class="font-mono text-mono-small font-semibold">{finding.ruleId}</code>
        ) : null}
        <code class="font-mono text-mono-small text-muted">{finding.code}</code>
        {finding.operation !== undefined ? (
          <span class="text-muted">{t('patches.lint.operation', { n: finding.operation })}</span>
        ) : null}
      </div>
      <p class="m-0 text-body">{finding.message}</p>
      {hasDetails ? (
        <details class="text-muted">
          <summary class="cursor-pointer text-small">{t('patches.lint.details')}</summary>
          <div class="mt-1 flex flex-col gap-1">
            {finding.explanation ? <p class="m-0 text-body">{finding.explanation}</p> : null}
            {finding.field ? (
              <p class="m-0">
                {t('patches.lint.where')}{' '}
                <code class="font-mono text-mono-small">{finding.field}</code>
              </p>
            ) : null}
            {finding.xpath ? (
              <p class="m-0">
                {t('patches.lint.xpath')}{' '}
                <code class="font-mono text-mono-small">{finding.xpath}</code>
              </p>
            ) : null}
          </div>
        </details>
      ) : null}
    </li>
  );
}
