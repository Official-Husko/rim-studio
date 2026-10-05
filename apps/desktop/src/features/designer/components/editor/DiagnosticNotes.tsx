import type { DiagnosticDto } from 'rimstudio-ipc-types';
import { Badge, type BadgeTone } from 'rimstudio-ui';
import { t } from '~/shared/i18n';

const TONE: Record<DiagnosticDto['severity'], BadgeTone> = {
  error: 'danger',
  warning: 'warning',
  info: 'info',
  hint: 'neutral',
};

export interface DiagnosticNotesProps {
  diagnostics: readonly DiagnosticDto[];
}

/** Severity word for a diagnostic, so colour is never the only signal. */
export function severityText(severity: DiagnosticDto['severity']): string {
  switch (severity) {
    case 'error':
      return t('designer.severity.error');
    case 'warning':
      return t('designer.severity.warning');
    case 'info':
      return t('designer.severity.info');
    default:
      return t('designer.severity.hint');
  }
}

/** The warnings and notes of a field, one line each. Errors are shown by the field itself. */
export function DiagnosticNotes({ diagnostics }: DiagnosticNotesProps) {
  const notes = diagnostics.filter((d) => d.severity !== 'error');
  if (notes.length === 0) return null;
  return (
    <ul class="flex flex-col gap-1">
      {notes.map((d, i) => (
        <li key={`${d.code}-${i}`} class="flex items-start gap-2 text-small text-muted">
          <Badge tone={TONE[d.severity]}>{severityText(d.severity)}</Badge>
          <span class="min-w-0">
            {d.message} <span class="font-mono text-mono-small text-faint">{d.code}</span>
          </span>
        </li>
      ))}
    </ul>
  );
}
