import type { DiagnosticDto } from 'rimstudio-ipc-types';
import { Badge, Table, type TableColumn } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { parseDerived, type DerivedValue } from './model';

export interface DerivedTableProps {
  diagnostics: readonly DiagnosticDto[];
}

/** The numbers the conversion filled in, each with the predictor and the rating the backend gave it. */
export function DerivedTable({ diagnostics }: DerivedTableProps) {
  const rows = diagnostics.map(parseDerived);
  const columns: TableColumn<DerivedValue>[] = [
    {
      key: 'field',
      header: t('patches.derived.field'),
      widthClass: 'w-40',
      mono: true,
      render: (r) => r.field,
    },
    {
      key: 'value',
      header: t('patches.derived.value'),
      widthClass: 'w-24',
      mono: true,
      align: 'right',
      render: (r) => <span title={r.how}>{r.value}</span>,
    },
    {
      key: 'predictor',
      header: t('patches.derived.predictor'),
      widthClass: 'w-32',
      render: (r) => r.predictor ?? t('patches.derived.vanilla'),
    },
    {
      key: 'rating',
      header: t('patches.derived.rating'),
      widthClass: 'w-28',
      render: (r) =>
        r.rating ? (
          <Badge tone={r.rating === 'reliable' ? 'success' : 'warning'}>{r.rating}</Badge>
        ) : null,
    },
  ];
  return (
    <Table
      dense
      label={t('patches.derived.label')}
      columns={columns}
      rows={rows}
      getKey={(r) => r.field}
      emptyText={t('patches.derived.empty')}
    />
  );
}
