import { Banner } from 'rimstudio-ui';
import type { LayoutFixItemDto } from 'rimstudio-ipc-types';
import { t, tn } from '~/shared/i18n';
import { confirmKey } from './fixLabels';

export interface FixConfirmProps {
  items: LayoutFixItemDto[];
  rename: readonly string[];
  folder: string;
}

/** The confirmation: what moves where, and where the undo journal and the backups go. */
export function FixConfirm({ items, rename, folder }: FixConfirmProps) {
  return (
    <div class="flex flex-col gap-3">
      <p class="m-0 text-body">{tn('project.fix.confirm.intro', items.length, { folder })}</p>
      <ul
        class="m-0 flex list-none flex-col divide-y divide-line-subtle border border-line p-0"
        aria-label={t('project.fix.confirm.list')}
      >
        {items.map((item) => {
          const to =
            rename.includes(item.id) && item.conflict?.suggestedTo
              ? item.conflict.suggestedTo
              : item.to;
          return (
            <li key={item.id} class="px-3 py-2 font-mono text-mono break-all">
              {t(confirmKey(item.kind), { from: item.from, to })}
              {rename.includes(item.id) ? (
                <span class="font-sans text-small text-muted">
                  {' '}
                  ({t('project.fix.confirm.numbered')})
                </span>
              ) : null}
            </li>
          );
        })}
      </ul>
      <Banner tone="info" title={t('project.fix.confirm.undoTitle')}>
        {t('project.fix.confirm.undo')}
      </Banner>
    </div>
  );
}
