import type { CeSuggestedFieldDto } from 'rimstudio-ipc-types';
import { Badge, Panel } from 'rimstudio-ui';
import { formatNumber } from '~/shared/format';
import { t } from '~/shared/i18n';
import { ratingText, ratingTone, sourceText } from './labels';

export interface CePatchNumbersProps {
  numbers: readonly CeSuggestedFieldDto[];
}

/** The numbers the patch derives although the block has no field for them: mass, range and timings. */
export function CePatchNumbers({ numbers }: CePatchNumbersProps) {
  if (numbers.length === 0) return null;
  return (
    <Panel title={t('designer.output.ce.patchNumbers')} collapsible defaultCollapsed>
      <ul class="flex flex-col gap-2">
        {numbers.map((n) => (
          <li key={n.field} class="flex flex-col gap-0.5 text-small">
            <div class="flex items-center justify-between gap-2">
              <span class="text-fg">{n.label}</span>
              <span class="font-mono text-mono text-fg">
                {n.value === undefined ? '' : formatNumber(n.value, 3)}
              </span>
            </div>
            <div class="flex flex-wrap items-center gap-2">
              {n.rating ? <Badge tone={ratingTone(n.rating)}>{ratingText(n.rating)}</Badge> : null}
              {n.source ? <span class="text-muted">{sourceText(n.source)}</span> : null}
            </div>
          </li>
        ))}
      </ul>
    </Panel>
  );
}
