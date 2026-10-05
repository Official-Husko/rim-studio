import type { EstimateSummaryDto } from 'rimstudio-ipc-types';
import { KeyValueList } from 'rimstudio-ui';
import { formatNumber } from '~/shared/format';
import { t } from '~/shared/i18n';

export interface EstimateSummaryProps {
  estimate: EstimateSummaryDto;
  /** What the answers imply, by stat. */
  implied?: Record<string, string> | undefined;
}

/** The estimate of the strength: its class, the index, where it sits and the notes. */
export function EstimateSummary({ estimate, implied }: EstimateSummaryProps) {
  const items = [
    { key: t('designer.estimate.class'), value: estimate.classLabel },
    { key: t('designer.estimate.strength'), value: formatNumber(estimate.strength, 2), mono: true },
    {
      key: t('designer.estimate.percentile'),
      value: `${formatNumber(estimate.strengthPercentile * 100, 0)} %`,
      mono: true,
    },
    ...Object.entries(implied ?? {}).map(([key, value]) => ({ key, value, mono: true })),
  ];
  return (
    <div class="flex flex-col gap-2">
      <KeyValueList label={t('designer.estimate.title')} items={items} />
      {estimate.notes.map((note) => (
        <p key={note} class="text-small text-muted">
          {note}
        </p>
      ))}
    </div>
  );
}
