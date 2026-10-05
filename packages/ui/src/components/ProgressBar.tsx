import { cx } from '../cx';

export interface ProgressBarProps {
  /** Fraction from 0 to 1. Leave undefined for an indeterminate bar. */
  value?: number;
  /** Accessible name, for example "Library scan". */
  label: string;
  /** Visible caption, such as "142 of 600". */
  caption?: string;
  tone?: 'accent' | 'warning' | 'danger';
}

const FILL = { accent: 'bg-accent', warning: 'bg-warning', danger: 'bg-danger' } as const;

/** A thin determinate or indeterminate progress line. */
export function ProgressBar({ value, label, caption, tone = 'accent' }: ProgressBarProps) {
  const determinate = typeof value === 'number' && Number.isFinite(value);
  const pct = determinate ? Math.round(Math.min(1, Math.max(0, value)) * 100) : undefined;
  return (
    <div class="flex flex-col gap-1">
      <div
        role="progressbar"
        aria-label={label}
        aria-valuemin={determinate ? 0 : undefined}
        aria-valuemax={determinate ? 100 : undefined}
        aria-valuenow={pct}
        class="relative h-1 w-full overflow-hidden rounded-sm bg-raised"
      >
        {determinate ? (
          <div class={cx('h-full', FILL[tone])} style={{ width: `${pct}%` }} />
        ) : (
          <div class={cx('rs-indeterminate absolute inset-y-0 left-0 w-2/5', FILL[tone])} />
        )}
      </div>
      {caption ? <span class="font-mono text-mono-small text-faint">{caption}</span> : null}
    </div>
  );
}
