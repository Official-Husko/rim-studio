import type { ComponentChildren } from 'preact';
import { cx } from '../cx';

export interface CardProps {
  tone?: 'flat' | 'raised';
  /** When set the card is a toggle button with this selected state. */
  selected?: boolean;
  /** Makes the card selectable; called on click, Enter and Space. */
  onSelect?: () => void;
  /** Accessible name for a selectable card, when the content is not enough. */
  label?: string;
  children: ComponentChildren;
}

const BASE = 'block w-full rounded-md border p-3 text-left';

/** A bordered block. Selectable cards are real buttons, so keyboard and screen readers work. */
export function Card({ tone = 'flat', selected, onSelect, label, children }: CardProps) {
  const surface = tone === 'raised' ? 'bg-raised border-line-strong' : 'bg-surface border-line';
  if (onSelect) {
    return (
      <button
        type="button"
        aria-pressed={selected ? 'true' : 'false'}
        aria-label={label}
        onClick={onSelect}
        class={cx(
          BASE,
          surface,
          'transition-colors duration-(--rs-dur-fast) hover:border-accent',
          selected && 'border-accent bg-accent-tint',
        )}
      >
        {children}
      </button>
    );
  }
  return <div class={cx(BASE, surface)}>{children}</div>;
}
