import type { ComponentChildren } from 'preact';
import { cx } from '../cx';

export type BadgeTone = 'neutral' | 'info' | 'warning' | 'danger' | 'success';

export interface BadgeProps {
  tone?: BadgeTone;
  children: ComponentChildren;
}

const TONES: Record<BadgeTone, string> = {
  neutral: 'text-muted border-line-strong',
  info: 'text-info border-info',
  warning: 'text-warning border-warning',
  danger: 'text-danger border-danger',
  success: 'text-success border-success',
};

/** A small label with a tone. The tone never carries meaning alone: the text always says it. */
export function Badge({ tone = 'neutral', children }: BadgeProps) {
  return (
    <span
      class={cx(
        'inline-flex h-5 items-center rounded-sm border bg-surface px-1.5 font-mono text-mono-small whitespace-nowrap',
        TONES[tone],
      )}
    >
      {children}
    </span>
  );
}
