import type { ComponentChildren } from 'preact';
import { cx } from '../cx';
import { Icon, type IconName } from './Icon';

export interface EmptyStateProps {
  title: string;
  description?: string;
  /** Primary action, usually one Button. */
  action?: ComponentChildren;
  icon?: IconName;
  /** Drops the grid canvas and padding for use inside a small region. */
  compact?: boolean;
}

/** A designed empty screen: one sentence of cause, one action. The grid sits behind it only. */
export function EmptyState({ title, description, action, icon, compact }: EmptyStateProps) {
  return (
    <div
      class={cx(
        'flex flex-col items-center justify-center gap-3 text-center',
        compact ? 'p-4' : 'bp-grid min-h-60 p-8',
      )}
    >
      {icon ? (
        <span class="text-faint">
          <Icon name={icon} size={24} />
        </span>
      ) : null}
      <h3
        class={cx(
          'font-display font-semibold tracking-display text-fg',
          compact ? 'text-title' : 'text-display',
        )}
      >
        {title}
      </h3>
      {description ? <p class="max-w-prose text-muted">{description}</p> : null}
      {action ? <div class="mt-1">{action}</div> : null}
    </div>
  );
}
