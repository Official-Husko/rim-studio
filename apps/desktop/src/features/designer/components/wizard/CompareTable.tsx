import type { FitReportDto, StatFitDto } from 'rimstudio-ipc-types';
import { Panel, Table, type TableColumn } from 'rimstudio-ui';
import { formatNumber } from '~/shared/format';
import { t, type MessageKey } from '~/shared/i18n';
import { statLabel } from '../../model/labels';

const LEVEL: Record<StatFitDto['level'], MessageKey> = {
  typical: 'designer.fit.typical',
  plausible: 'designer.fit.plausible',
  unusual: 'designer.fit.unusual',
};

export interface CompareTableProps {
  report: FitReportDto;
}

/** The numbers next to what the install's weapons of the class have: the typical value and range. */
export function CompareTable({ report }: CompareTableProps) {
  const columns: Array<TableColumn<StatFitDto>> = [
    { key: 'stat', header: t('designer.wizard.compare.stat'), render: (r) => statLabel(r.stat) },
    {
      key: 'value',
      header: t('designer.wizard.compare.yours'),
      align: 'right',
      mono: true,
      render: (r) => formatNumber(r.value, 3),
    },
    {
      key: 'typical',
      header: t('designer.wizard.compare.typical'),
      align: 'right',
      mono: true,
      render: (r) => formatNumber(r.predicted, 3),
    },
    {
      key: 'range',
      header: t('designer.wizard.compare.range'),
      align: 'right',
      mono: true,
      render: (r) => `${formatNumber(r.p50.low, 3)} to ${formatNumber(r.p50.high, 3)}`,
    },
    { key: 'fit', header: t('designer.wizard.compare.fit'), render: (r) => t(LEVEL[r.level]) },
  ];
  return (
    <Panel title={t('designer.wizard.compare.title')} collapsible>
      <p class="pb-2 text-small text-muted">{t('designer.wizard.compare.note')}</p>
      <Table
        label={t('designer.wizard.compare.title')}
        columns={columns}
        rows={report.perStat}
        getKey={(r) => r.stat}
        dense
      />
    </Panel>
  );
}
