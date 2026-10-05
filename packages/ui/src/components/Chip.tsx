import type { ComponentChildren } from 'preact';
import { cx } from '../cx';
import { Icon } from './Icon';

/** Source chips of a number: typed by the user, suggested by the model, an anchor item, or derived. */
export type ChipKind = 'typed' | 'suggested' | 'anchor' | 'derived' | 'neutral';

export interface ChipProps {
  kind?: ChipKind;
  /** Overrides the default text of the kind. */
  children?: ComponentChildren;
  /** Adds a remove button with this accessible name. */
  removeLabel?: string;
  onRemove?: () => void;
}

const STYLE: Record<ChipKind, { letter: string; text: string; tone: string }> = {
  typed: { letter: 'T', text: 'Typed', tone: 'text-accent border-accent' },
  suggested: { letter: 'S', text: 'Suggested', tone: 'text-info border-info' },
  anchor: { letter: 'A', text: 'Anchor', tone: 'text-rule-user border-rule-user' },
  derived: { letter: 'D', text: 'Derived', tone: 'text-muted border-line-strong' },
  neutral: { letter: '', text: '', tone: 'text-muted border-line-strong' },
};

/** A compact chip. For the four source kinds a letter keeps colour from being the only signal. */
export function Chip({ kind = 'neutral', children, removeLabel, onRemove }: ChipProps) {
  const style = STYLE[kind];
  return (
    <span
      data-kind={kind}
      class={cx(
        'inline-flex h-5 items-center gap-1 rounded-sm border bg-surface px-1.5 font-mono text-mono-small whitespace-nowrap',
        style.tone,
      )}
    >
      {style.letter ? (
        <span aria-hidden="true" class="font-semibold">
          {style.letter}
        </span>
      ) : null}
      <span>{children ?? style.text}</span>
      {onRemove ? (
        <button
          type="button"
          aria-label={removeLabel ?? 'Remove'}
          onClick={onRemove}
          class="-mr-1 inline-flex size-4 items-center justify-center rounded-sm hover:bg-hover"
        >
          <Icon name="close" />
        </button>
      ) : null}
    </span>
  );
}
