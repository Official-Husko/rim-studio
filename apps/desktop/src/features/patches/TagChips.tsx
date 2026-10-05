import { Badge } from 'rimstudio-ui';
import { t } from '~/shared/i18n';

export interface TagChipsProps {
  tags: readonly string[];
  classes: readonly string[];
  /** Show at most this many chips on one line and a count for the rest (a table cell); otherwise all wrap. */
  limit?: number;
}

/** The weapon tags (neutral) and weapon classes (info) of a definition as small labels. */
export function TagChips({ tags, classes, limit }: TagChipsProps) {
  if (tags.length === 0 && classes.length === 0) return <span class="text-muted">-</span>;
  const chips = [
    ...tags.map((name) => ({ name, tone: 'neutral' as const, title: t('patches.chips.tag') })),
    ...classes.map((name) => ({ name, tone: 'info' as const, title: t('patches.chips.class') })),
  ];
  const shown = limit === undefined ? chips : chips.slice(0, limit);
  const rest = chips.slice(shown.length);
  return (
    <ul
      aria-label={t('patches.chips.label')}
      class={`m-0 flex list-none gap-1 p-0 ${limit === undefined ? 'flex-wrap' : 'flex-nowrap'}`}
    >
      {shown.map((chip) => (
        <li key={`${chip.tone}-${chip.name}`} title={chip.title}>
          <Badge tone={chip.tone}>{chip.name}</Badge>
        </li>
      ))}
      {rest.length > 0 ? (
        <li title={rest.map((chip) => chip.name).join(', ')}>
          <Badge>{t('patches.chips.more', { n: rest.length })}</Badge>
        </li>
      ) : null}
    </ul>
  );
}
