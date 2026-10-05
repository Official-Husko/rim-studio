import type { ConvertStatusDto } from 'rimstudio-ipc-types';
import { Badge } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { STATUS_LABEL, STATUS_TONE } from './model';

export interface StatusChipProps {
  status: ConvertStatusDto;
}

/** The status of a weapon as a badge: the word is the signal, the tone only supports it. */
export function StatusChip({ status }: StatusChipProps) {
  return <Badge tone={STATUS_TONE[status]}>{t(STATUS_LABEL[status])}</Badge>;
}
