import type { ValueSourceDto } from 'rimstudio-ipc-types';
import { Chip } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { chipKind } from '../../model/draft';

export interface SourceChipProps {
  source: ValueSourceDto;
}

/** The chip that says where a number came from: typed, suggested, anchor or an answer. */
export function SourceChip({ source }: SourceChipProps) {
  const kind = chipKind(source);
  if (kind === 'neutral') return <Chip kind="neutral">{t('designer.source.answered')}</Chip>;
  const text =
    kind === 'typed'
      ? t('designer.source.typed')
      : kind === 'suggested'
        ? t('designer.source.suggested')
        : t('designer.source.anchor');
  return <Chip kind={kind}>{text}</Chip>;
}
