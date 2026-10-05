import { useMemo, useState } from 'preact/hooks';
import type { ConvertCandidateDto, ConvertStatusDto } from 'rimstudio-ipc-types';
import { Checkbox, Table, type SortDirection, type TableColumn } from 'rimstudio-ui';
import { formatNumber } from '~/shared/format';
import { t } from '~/shared/i18n';
import { familyAnswers, ownAnswers } from './answerStore';
import {
  allTags,
  filterCandidates,
  NO_FILTER,
  NUMBER_COLUMNS,
  openCount,
  sortCandidates,
  type CandidateFilter,
  type SortKey,
} from './candidateView';
import { CandidateFilters } from './CandidateFilters';
import { familyOf, isConvertible } from './model';
import { plans } from './planStore';
import { StatusChip } from './StatusChip';
import { TagChips } from './TagChips';

export interface CandidateTableProps {
  candidates: readonly ConvertCandidateDto[];
  focused: string | undefined;
  onFocus: (defName: string) => void;
  checked: ReadonlySet<string>;
  onCheck: (defName: string, on: boolean) => void;
}

const NUMBER_HEADERS = {
  damage: 'patches.table.damage',
  range: 'patches.table.range',
  cooldown: 'patches.table.cooldown',
  warmup: 'patches.table.warmup',
  mass: 'patches.table.mass',
} as const;

/** The weapons of the mod with status, tags, vanilla numbers and open questions; sortable and filterable. */
export function CandidateTable({
  candidates,
  focused,
  onFocus,
  checked,
  onCheck,
}: CandidateTableProps) {
  const [filter, setFilter] = useState<CandidateFilter>(NO_FILTER);
  const [sort, setSort] = useState<{ key: SortKey; direction: SortDirection }>({
    key: 'weapon',
    direction: 'asc',
  });
  const planned = plans.value;
  const own = ownAnswers.value;
  const family = familyAnswers.value;
  const open = (c: ConvertCandidateDto): number | undefined => {
    const familySet = familyOf(c) ? family[familyOf(c)] : undefined;
    return openCount({
      candidate: c,
      ...(planned[c.defName] ? { plan: planned[c.defName] } : {}),
      ...(own[c.defName] ? { own: own[c.defName] } : {}),
      ...(familySet ? { family: familySet } : {}),
    });
  };
  const tags = useMemo(() => allTags(candidates), [candidates]);
  const statuses = useMemo(
    () => [...new Set(candidates.map((c) => c.status))].sort() as ConvertStatusDto[],
    [candidates],
  );
  const rows = sortCandidates(filterCandidates(candidates, filter), sort.key, sort.direction, open);

  const numberColumns: TableColumn<ConvertCandidateDto>[] = NUMBER_COLUMNS.map((key) => ({
    key,
    header: t(NUMBER_HEADERS[key]),
    sortable: true,
    align: 'right',
    mono: true,
    widthClass: key === 'cooldown' || key === 'warmup' ? 'w-24' : 'w-20',
    render: (c) => {
      const value = c.vanilla?.[key];
      return value === undefined ? '-' : formatNumber(value, key === 'range' ? 0 : 1);
    },
  }));
  const columns: TableColumn<ConvertCandidateDto>[] = [
    {
      key: 'check',
      header: t('patches.table.convert'),
      widthClass: 'w-14',
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
      sortable: true,
      render: (c) => (
        <span class="flex max-w-44 min-w-0 flex-col leading-tight">
          <span class="truncate">{c.label || c.defName}</span>
          <span class="truncate font-mono text-mono-small text-muted">{c.defName}</span>
        </span>
      ),
    },
    {
      key: 'status',
      header: t('patches.table.status'),
      sortable: true,
      widthClass: 'w-32',
      render: (c) => <StatusChip status={c.status} />,
    },
    {
      key: 'open',
      header: t('patches.table.questions'),
      sortable: true,
      align: 'right',
      mono: true,
      widthClass: 'w-16',
      render: (c) => {
        const count = open(c);
        return count === undefined ? '' : String(count);
      },
    },
    {
      key: 'tags',
      header: t('patches.table.tags'),
      widthClass: 'w-32',
      render: (c) => <TagChips tags={c.tags ?? []} classes={c.weaponClasses ?? []} limit={1} />,
    },
    ...numberColumns,
  ];
  return (
    <div class="flex flex-col">
      <CandidateFilters
        filter={filter}
        onChange={setFilter}
        tags={tags}
        statuses={statuses}
        shown={rows.length}
        total={candidates.length}
      />
      <div class="overflow-x-auto">
        <div class="min-w-3xl">
          <Table
            label={t('patches.table.label')}
            columns={columns}
            rows={rows}
            getKey={(c) => c.defName}
            sort={sort}
            onSortChange={(key, direction) => setSort({ key: key as SortKey, direction })}
            selection="single"
            selectedKeys={new Set(focused ? [focused] : [])}
            onSelectionChange={(keys) => {
              const first = [...keys][0];
              if (first) onFocus(first);
            }}
            emptyText={
              candidates.length === 0 ? t('patches.table.empty') : t('patches.filter.empty')
            }
          />
        </div>
      </div>
    </div>
  );
}
