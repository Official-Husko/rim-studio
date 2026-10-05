import { Badge, type BadgeTone } from 'rimstudio-ui';
import type { DiagnosticDto, SeverityDto } from 'rimstudio-ipc-types';
import { t, type MessageKey } from '~/shared/i18n';

const TONE: Record<SeverityDto, BadgeTone> = {
  error: 'danger',
  warning: 'warning',
  info: 'info',
  hint: 'neutral',
};

const LABEL: Record<SeverityDto, MessageKey> = {
  error: 'project.basics.severity.error',
  warning: 'project.basics.severity.warning',
  info: 'project.basics.severity.info',
  hint: 'project.basics.severity.hint',
};

export interface FieldFindingsProps {
  items: readonly DiagnosticDto[];
}

/** The backend's findings about one field, each with its severity in words. Nothing when there are none. */
export function FieldFindings({ items }: FieldFindingsProps) {
  if (items.length === 0) return null;
  return (
    <ul class="m-0 flex list-none flex-col gap-1 p-0">
      {items.map((item) => (
        <li
          key={`${item.code}:${item.field ?? ''}:${item.message}`}
          class="flex items-start gap-2 text-small"
        >
          <span class="shrink-0">
            <Badge tone={TONE[item.severity]}>{t(LABEL[item.severity])}</Badge>
          </span>
          <span class="min-w-0 text-muted">{item.message}</span>
        </li>
      ))}
    </ul>
  );
}
