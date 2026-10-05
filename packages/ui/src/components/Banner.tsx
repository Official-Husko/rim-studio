import type { ComponentChildren } from 'preact';
import { cx } from '../cx';
import { Icon, type IconName } from './Icon';
import { IconButton } from './IconButton';

export type BannerTone = 'info' | 'warning' | 'error' | 'success';

export interface BannerProps {
  tone?: BannerTone;
  /** Short bold lead in. */
  title?: string;
  children?: ComponentChildren;
  /** One action, usually a Button. */
  action?: ComponentChildren;
  /** When set, a close button is shown with this label. */
  dismissLabel?: string;
  onDismiss?: () => void;
}

const TONES: Record<BannerTone, { icon: IconName; tone: string; role: 'status' | 'alert' }> = {
  info: { icon: 'info', tone: 'text-info border-info', role: 'status' },
  success: { icon: 'check', tone: 'text-success border-success', role: 'status' },
  warning: { icon: 'warning', tone: 'text-warning border-warning', role: 'alert' },
  error: { icon: 'error', tone: 'text-danger border-danger', role: 'alert' },
};

/** An inline message. Shape of the icon and the words carry the severity, not colour alone. */
export function Banner({
  tone = 'info',
  title,
  children,
  action,
  dismissLabel,
  onDismiss,
}: BannerProps) {
  const style = TONES[tone];
  return (
    <div
      role={style.role}
      data-tone={tone}
      class={cx('flex items-start gap-3 border bg-surface px-3 py-2 text-body', style.tone)}
    >
      <span class="mt-0.5">
        <Icon name={style.icon} />
      </span>
      <div class="min-w-0 flex-1 text-fg">
        {title ? <strong class="mr-2 font-semibold">{title}</strong> : null}
        {children}
      </div>
      {action ? <div class="shrink-0">{action}</div> : null}
      {onDismiss ? (
        <IconButton icon="close" label={dismissLabel ?? 'Dismiss'} onClick={onDismiss} noTooltip />
      ) : null}
    </div>
  );
}
