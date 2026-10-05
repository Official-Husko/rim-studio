import { Badge, Button, EmptyState } from 'rimstudio-ui';
import type { ProjectLayoutFixPlanDto } from 'rimstudio-ipc-types';
import { t, tn } from '~/shared/i18n';
import { FixItemRow } from './FixItemRow';
import { FixReviewList } from './FixReviewList';
import { itemTarget } from './fixLabels';

export interface FixPlanViewProps {
  plan: ProjectLayoutFixPlanDto;
  selected: readonly string[];
  rename: readonly string[];
  onToggle: (id: string, on: boolean) => void;
  onRename: (id: string, on: boolean) => void;
  onTickSafe: () => void;
  onTickNone: () => void;
}

/** The review step: counts, the applicable items with checkboxes, and the items for review only. */
export function FixPlanView({
  plan,
  selected,
  rename,
  onToggle,
  onRename,
  onTickSafe,
  onTickNone,
}: FixPlanViewProps) {
  const applicable = plan.items.filter((i) => i.applicable);
  const review = plan.items.filter((i) => !i.applicable);
  if (plan.items.length === 0) {
    return (
      <EmptyState
        compact
        icon="check"
        title={t('project.fix.empty.title')}
        description={t('project.fix.empty.hint')}
      />
    );
  }
  return (
    <div class="flex flex-col gap-4">
      <p class="m-0 text-body">{t('project.fix.intro')}</p>
      <div class="flex flex-wrap items-center gap-2">
        <Badge tone="success">{tn('project.fix.safe', plan.safe)}</Badge>
        {plan.needsReview > 0 ? (
          <Badge tone="warning">{tn('project.fix.review', plan.needsReview)}</Badge>
        ) : null}
        {plan.conflicts > 0 ? (
          <Badge tone="warning">{tn('project.fix.conflicts', plan.conflicts)}</Badge>
        ) : null}
        <span class="flex-1" />
        <Button size="sm" variant="ghost" onClick={onTickSafe}>
          {t('project.fix.tickSafe')}
        </Button>
        <Button size="sm" variant="ghost" onClick={onTickNone}>
          {t('project.fix.tickNone')}
        </Button>
      </div>
      {applicable.length > 0 ? (
        <ul
          class="m-0 flex list-none flex-col divide-y divide-line-subtle border border-line p-0"
          aria-label={t('project.fix.list')}
        >
          {applicable.map((item) => (
            <FixItemRow
              key={item.id}
              item={item}
              checked={selected.includes(item.id)}
              renamed={rename.includes(item.id)}
              requiredPaths={item.requires.map((id) => {
                const other = plan.items.find((i) => i.id === id);
                return other ? itemTarget(other) : id;
              })}
              onToggle={(on) => onToggle(item.id, on)}
              onRename={(on) => onRename(item.id, on)}
            />
          ))}
        </ul>
      ) : null}
      <FixReviewList items={review} />
    </div>
  );
}
