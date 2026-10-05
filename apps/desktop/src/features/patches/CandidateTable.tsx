import type { ConvertCandidateDto } from 'rimstudio-ipc-types';
import { Checkbox, Table, type TableColumn } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { familyOf, familyParts, isConvertible } from './model';
import { StatusChip } from './StatusChip';

export interface CandidateTableProps {
  candidates: readonly ConvertCandidateDto[];
  focused: string | undefined;
  onFocus: (defName: string) => void;
  checked: ReadonlySet<string>;
  onCheck: (defName: string, on: boolean) => void;
}

/** The weapons of the mod with their status, kind, first tag and open questions. */
export function CandidateTable({
  candidates,
  focused,
  onFocus,
  checked,
  onCheck,
}: CandidateTableProps) {
  const columns: TableColumn<ConvertCandidateDto>[] = [
    {
      key: 'check',
      header: t('patches.table.convert'),
      widthClass: 'w-16',
      render: (c) =>
        isConvertible(c) ? (
          <span
            role="presentation"
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => e.stopPropagation()}
          >
            <Checkbox
              checked={checked.has(c.defName)}
              onCheckedChange={(on) => onCheck(c.defName, on)}
            >
              <span class="sr-only">
                {t('patches.table.check', { name: c.label || c.defName })}
              </span>
            </Checkbox>
          </span>
        ) : null,
    },
    {
      key: 'weapon',
      header: t('patches.table.weapon'),
      render: (c) => (
        <span class="flex min-w-0 flex-col leading-tight">
          <span class="truncate">{c.label || c.defName}</span>
          <span class="truncate font-mono text-mono-small text-muted">{c.defName}</span>
        </span>
      ),
    },
    {
      key: 'status',
      header: t('patches.table.status'),
      widthClass: 'w-36',
      render: (c) => <StatusChip status={c.status} />,
    },
    {
      key: 'kind',
      header: t('patches.table.kind'),
      widthClass: 'w-36',
      render: (c) => {
        const tag = familyParts(familyOf(c)).tag;
        return (
          <span class="truncate text-small text-muted">
            {c.kind ? t(c.kind === 'ranged' ? 'patches.kind.ranged' : 'patches.kind.melee') : '-'}
            {tag ? ` / ${tag}` : ''}
          </span>
        );
      },
    },
    {
      key: 'asks',
      header: t('patches.table.questions'),
      align: 'right',
      mono: true,
      widthClass: 'w-16',
      render: (c) => (isConvertible(c) ? String(c.asks.length) : ''),
    },
  ];
  return (
    <Table
      label={t('patches.table.label')}
      columns={columns}
      rows={[...candidates]}
      getKey={(c) => c.defName}
      selection="single"
      selectedKeys={new Set(focused ? [focused] : [])}
      onSelectionChange={(keys) => {
        const first = [...keys][0];
        if (first) onFocus(first);
      }}
      emptyText={t('patches.table.empty')}
    />
  );
}
