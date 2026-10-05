import type { ConvertStatusDto } from 'rimstudio-ipc-types';
import { Select } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import type { CandidateFilter } from './candidateView';
import { STATUS_LABEL } from './model';

export interface CandidateFiltersProps {
  filter: CandidateFilter;
  onChange: (filter: CandidateFilter) => void;
  /** The tags and classes that appear in the list. */
  tags: readonly string[];
  /** The statuses that appear in the list. */
  statuses: readonly ConvertStatusDto[];
  shown: number;
  total: number;
}

/** The status and tag filters above the weapon table, with how many rows pass. */
export function CandidateFilters({
  filter,
  onChange,
  tags,
  statuses,
  shown,
  total,
}: CandidateFiltersProps) {
  return (
    <div class="flex flex-wrap items-center gap-3 px-3 pb-2">
      <label class="flex items-center gap-2 text-small text-muted">
        {t('patches.filter.status')}
        <Select
          value={filter.status}
          onValueChange={(value) =>
            onChange({ ...filter, status: (value as ConvertStatusDto | '') ?? '' })
          }
          options={[
            { value: '', label: t('patches.filter.any') },
            ...statuses.map((s) => ({ value: s, label: t(STATUS_LABEL[s]) })),
          ]}
        />
      </label>
      <label class="flex items-center gap-2 text-small text-muted">
        {t('patches.filter.tag')}
        <Select
          value={filter.tag}
          onValueChange={(value) => onChange({ ...filter, tag: value })}
          options={[
            { value: '', label: t('patches.filter.any') },
            ...tags.map((tag) => ({ value: tag, label: tag })),
          ]}
        />
      </label>
      <span class="text-small text-muted" role="status">
        {t('patches.filter.shown', { shown, total })}
      </span>
    </div>
  );
}
