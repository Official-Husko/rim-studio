import type { DiagnosticDto, SeverityDto } from 'rimstudio-ipc-types';
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

export interface DiagnosticListProps {
  diagnostics: readonly DiagnosticDto[];
  /** Accessible name of the list. */
  label: string;
}

/** Diagnostics with their severity word, the stable code and the English text of the backend. */
export function DiagnosticList({ diagnostics, label }: DiagnosticListProps) {
  return (
    <ul aria-label={label} class="m-0 flex list-none flex-col gap-2 p-0">
      {diagnostics.map((d, index) => (
        <li
          key={`${d.code}-${index}`}
          class="flex flex-wrap items-baseline gap-x-2 gap-y-1 text-small"
        >
          <Badge tone={TONE[d.severity]}>{t(LABEL[d.severity])}</Badge>
          <code class="font-mono text-mono-small text-muted">{d.code}</code>
          <span class="min-w-0 flex-1 basis-64 text-body">{d.message}</span>
        </li>
      ))}
    </ul>
  );
}
