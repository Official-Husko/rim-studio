import type { DiagnosticDto } from 'rimstudio-ipc-types';
import { Badge, Panel } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import { severityText } from '../editor/DiagnosticNotes';
import { severityTone } from './labels';

export interface CeLintProps {
  /** The CEP findings and the rules that were not checked. */
  lint: readonly DiagnosticDto[];
}

/** The lint of the generated patch: each finding with its rule code and the reason in plain words. */
export function CeLint({ lint }: CeLintProps) {
  const findings = lint.filter((d) => d.code !== 'ce.not-checked');
  const unchecked = lint.filter((d) => d.code === 'ce.not-checked');
  return (
    <Panel
      title={t('designer.output.lint')}
      collapsible
      actions={
        <span class="font-mono text-mono-small text-muted">
          {findings.length === 0
            ? t('designer.output.lintClean')
            : tn('designer.output.lintCount', findings.length)}
        </span>
      }
    >
      <div class="flex flex-col gap-3">
        {findings.length === 0 ? (
          <p class="text-small text-muted">{t('designer.output.lintNone')}</p>
        ) : (
          <ul class="flex flex-col gap-2">
            {findings.map((d, i) => (
              <li key={`${d.code}-${i}`} class="flex flex-col gap-1 text-body">
                <div class="flex items-start gap-2">
                  <Badge tone={severityTone(d.severity)}>{severityText(d.severity)}</Badge>
                  <span class="min-w-0 flex-1 break-words">{d.message}</span>
                </div>
                <span class="break-all pl-1 font-mono text-mono-small text-faint">{d.code}</span>
              </li>
            ))}
          </ul>
        )}
        {unchecked.length > 0 ? (
          <div class="flex flex-col gap-1">
            <h3 class="font-display text-label font-semibold tracking-label text-muted uppercase">
              {t('designer.output.lintUnchecked')}
            </h3>
            <ul class="flex flex-col gap-1">
              {unchecked.map((d, i) => (
                <li key={i} class="text-small text-muted">
                  {d.message}
                </li>
              ))}
            </ul>
          </div>
        ) : null}
      </div>
    </Panel>
  );
}
