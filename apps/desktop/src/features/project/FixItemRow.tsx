import { Badge, Checkbox, DiffView } from 'rimstudio-ui';
import type { LayoutFixItemDto } from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';
import { FIX_KIND_LABEL, FIX_RISK_LABEL, FIX_RISK_TONE, itemTarget } from './fixLabels';

export interface FixItemRowProps {
  item: LayoutFixItemDto;
  checked: boolean;
  renamed: boolean;
  /** The paths of the items this one is applied together with. */
  requiredPaths: string[];
  onToggle: (on: boolean) => void;
  onRename: (on: boolean) => void;
}

/** One applicable change of the plan: the checkbox, from and to, why, the risk, the conflict choice and the edit. */
export function FixItemRow({
  item,
  checked,
  renamed,
  requiredPaths,
  onToggle,
  onRename,
}: FixItemRowProps) {
  const conflict = item.conflict?.destinationExists ? item.conflict : undefined;
  const blocked = conflict !== undefined && !renamed;
  return (
    <li class="flex flex-col gap-2 px-3 py-2">
      <div class="flex flex-wrap items-center gap-2">
        <Checkbox checked={checked} disabled={blocked} onCheckedChange={onToggle}>
          <span class="font-semibold">{t(FIX_KIND_LABEL[item.kind])}</span>{' '}
          <span class="font-mono text-mono">{itemTarget(item)}</span>
        </Checkbox>
        <Badge tone={FIX_RISK_TONE[item.risk]}>{t(FIX_RISK_LABEL[item.risk])}</Badge>
      </div>
      {item.from && item.from !== item.to ? (
        <div class="flex flex-col gap-0.5 pl-6 font-mono text-mono">
          <span class="break-all text-muted">{item.from}</span>
          <span class="break-all">{`→ ${item.to}`}</span>
        </div>
      ) : null}
      <p class="m-0 pl-6 text-small">{item.why}</p>
      {requiredPaths.length > 0 ? (
        <p class="m-0 pl-6 text-small text-muted">
          {t('project.fix.requires', { paths: requiredPaths.join(', ') })}
        </p>
      ) : null}
      {conflict ? (
        <div class="flex flex-col gap-1 pl-6">
          <p class="m-0 text-small text-warning">{t('project.fix.conflict')}</p>
          {conflict.suggestedTo ? (
            <Checkbox checked={renamed} onCheckedChange={onRename}>
              <span class="font-mono text-mono">
                {t('project.fix.conflict.rename', { name: conflict.suggestedTo })}
              </span>
            </Checkbox>
          ) : null}
        </div>
      ) : null}
      {item.diff ? (
        <div class="pl-6">
          <DiffView
            class="max-h-60"
            diff={item.diff}
            label={t('project.fix.diff', { path: item.to })}
          />
        </div>
      ) : null}
    </li>
  );
}
