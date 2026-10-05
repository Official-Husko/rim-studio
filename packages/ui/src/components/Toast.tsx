import type { ComponentChildren } from 'preact';
import { cx } from '../cx';
import { Icon, type IconName } from './Icon';
import { IconButton } from './IconButton';

export type ToastTone = 'info' | 'success' | 'warning' | 'error';

export interface ToastProps {
  tone?: ToastTone;
  title?: string;
  message: string;
  /** One action, for example Undo or Show. */
  actionLabel?: string;
  onAction?: () => void;
  onDismiss?: () => void;
  dismissLabel?: string;
}

const TONES: Record<ToastTone, { icon: IconName; tone: string; role: 'status' | 'alert' }> = {
  info: { icon: 'info', tone: 'border-info text-info', role: 'status' },
  success: { icon: 'check', tone: 'border-success text-success', role: 'status' },
  warning: { icon: 'warning', tone: 'border-warning text-warning', role: 'alert' },
  error: { icon: 'error', tone: 'border-danger text-danger', role: 'alert' },
};

/** One notification. Errors and warnings use the alert role so they are announced at once. */
export function Toast({
  tone = 'info',
  title,
  message,
  actionLabel,
  onAction,
  onDismiss,
  dismissLabel = 'Dismiss',
}: ToastProps) {
  const style = TONES[tone];
  return (
    <div
      role={style.role}
      data-tone={tone}
      class={cx(
        'flex w-drawer items-start gap-3 border bg-raised px-3 py-2 text-body shadow-raised',
        style.tone,
      )}
    >
      <span class="mt-0.5">
        <Icon name={style.icon} />
      </span>
      <div class="min-w-0 flex-1 text-fg">
        {title ? <p class="font-semibold">{title}</p> : null}
        <p class="break-words">{message}</p>
        {actionLabel && onAction ? (
          <button
            type="button"
            onClick={onAction}
            class="mt-1 text-accent underline underline-offset-2"
          >
            {actionLabel}
          </button>
        ) : null}
      </div>
      {onDismiss ? (
        <IconButton icon="close" label={dismissLabel} onClick={onDismiss} noTooltip />
      ) : null}
    </div>
  );
}

export interface ToastStackProps {
  /** Accessible name of the region. */
  label?: string;
  children?: ComponentChildren;
}

/** Fixed bottom right stack that holds the toasts. */
export function ToastStack({ label = 'Notifications', children }: ToastStackProps) {
  return (
    <div
      role="region"
      aria-label={label}
      class="pointer-events-none fixed right-4 bottom-4 z-(--rs-z-toast) flex flex-col gap-2 *:pointer-events-auto"
    >
      {children}
    </div>
  );
}
