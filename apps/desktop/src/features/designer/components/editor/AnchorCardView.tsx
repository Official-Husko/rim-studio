import type { AnchorCardDto } from 'rimstudio-ipc-types';
import { KeyValueList } from 'rimstudio-ui';
import { formatNumber } from '~/shared/format';
import { t } from '~/shared/i18n';
import { statLabel } from '../../model/labels';

export interface AnchorCardViewProps {
  card: AnchorCardDto;
  /** Which end of a pair this card is, shown as a caption. */
  caption?: string;
}

/** A reference weapon shown in the quiz: its name, role and real numbers. */
export function AnchorCardView({ card, caption }: AnchorCardViewProps) {
  const stats = Object.entries(card.stats).sort(([a], [b]) => a.localeCompare(b));
  return (
    <section
      class="flex min-w-0 flex-col gap-2 border border-line bg-bg p-3"
      aria-label={card.label}
    >
      {caption ? <span class="text-small text-muted">{caption}</span> : null}
      <h3 class="text-title font-semibold text-fg">{card.label}</h3>
      <p class="font-mono text-mono-small text-faint">
        {card.role} / {t('designer.quiz.strength', { value: formatNumber(card.strength, 2) })}
      </p>
      <KeyValueList
        label={card.label}
        items={stats.map(([key, value]) => ({
          key: statLabel(key),
          value: formatNumber(value, 3),
          mono: true,
        }))}
      />
    </section>
  );
}
