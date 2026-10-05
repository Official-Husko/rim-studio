import type { DiagnosticDto } from 'rimstudio-ipc-types';
import { Badge, Button, Panel } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import { severityText } from './DiagnosticNotes';

export interface DiagnosticListProps {
  diagnostics: readonly DiagnosticDto[];
  /** Move the focus to the field a pointer names. */
  onFocusField: (pointer: string) => void;
}

const ORDER: Record<DiagnosticDto['severity'], number> = { error: 0, warning: 1, info: 2, hint: 3 };
const TONE = { error: 'danger', warning: 'warning', info: 'info', hint: 'neutral' } as const;

/** Every diagnostic of the draft with its code and a jump to the field. Errors come first. */
export function DiagnosticList({ diagnostics, onFocusField }: DiagnosticListProps) {
  const sorted = [...diagnostics].sort((a, b) => ORDER[a.severity] - ORDER[b.severity]);
  const errors = diagnostics.filter((d) => d.severity === 'error').length;
  return (
    <Panel
      title={t('designer.panel.diagnostics')}
      collapsible
      actions={
        <span class="font-mono text-mono-small text-muted">
          {errors > 0
            ? tn('designer.diagnostics.errors', errors)
            : t('designer.diagnostics.noErrors')}
        </span>
      }
    >
      {sorted.length === 0 ? (
        <p class="text-small text-muted">{t('designer.diagnostics.none')}</p>
      ) : (
        <ul class="flex flex-col gap-2">
          {sorted.map((d, i) => (
            <li key={`${d.code}-${d.field ?? ''}-${i}`} class="flex items-start gap-2 text-body">
              <Badge tone={TONE[d.severity]}>{severityText(d.severity)}</Badge>
              <div class="min-w-0 flex-1">
                <span>{d.message}</span>{' '}
                <span class="font-mono text-mono-small text-faint">{d.code}</span>
              </div>
              {d.field ? (
                <Button size="sm" variant="ghost" onClick={() => onFocusField(d.field as string)}>
                  {t('designer.diagnostics.goTo')}
                </Button>
              ) : null}
            </li>
          ))}
        </ul>
      )}
    </Panel>
  );
}
