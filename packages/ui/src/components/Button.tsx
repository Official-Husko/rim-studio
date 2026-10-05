import type { ComponentChildren, JSX } from 'preact';
import { cx } from '../cx';
import { Icon, type IconName } from './Icon';
import { Spinner } from './Spinner';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';
export type ButtonSize = 'sm' | 'md';

export interface ButtonProps extends Omit<JSX.HTMLAttributes<HTMLButtonElement>, 'size' | 'icon'> {
  /** Visual weight. One primary button per region. */
  variant?: ButtonVariant;
  /** sm is 28 px high, md follows the density control height. */
  size?: ButtonSize;
  /** Shows a spinner, disables the button and sets aria-busy. */
  loading?: boolean;
  /** Leading icon from the icon set. */
  icon?: IconName;
  /** Trailing icon from the icon set. */
  iconEnd?: IconName;
  disabled?: boolean;
  type?: 'button' | 'submit' | 'reset';
  children?: ComponentChildren;
}

const VARIANTS: Record<ButtonVariant, string> = {
  primary:
    'bg-accent text-on-accent border-transparent hover:bg-accent-hover active:bg-accent-press',
  secondary: 'bg-raised text-fg border-line-strong hover:border-accent active:bg-hover',
  ghost: 'bg-transparent text-fg border-transparent hover:bg-hover active:bg-hover',
  danger: 'bg-transparent text-danger border-danger hover:bg-diff-removed active:bg-diff-removed',
};

const SIZES: Record<ButtonSize, string> = {
  sm: 'h-control-sm px-2 text-small gap-1',
  md: 'h-control px-3 text-body gap-2',
};

/** The standard button. Names come from children; icon only buttons use IconButton. */
export function Button({
  variant = 'secondary',
  size = 'md',
  loading = false,
  icon,
  iconEnd,
  disabled,
  type = 'button',
  class: className,
  children,
  ...rest
}: ButtonProps) {
  return (
    <button
      {...rest}
      type={type}
      disabled={disabled || loading}
      aria-busy={loading ? 'true' : undefined}
      class={cx(
        'inline-flex shrink-0 items-center justify-center rounded-md border font-sans font-semibold whitespace-nowrap',
        'transition-colors duration-(--rs-dur-fast) disabled:cursor-not-allowed disabled:opacity-50',
        VARIANTS[variant],
        SIZES[size],
        typeof className === 'string' && className,
      )}
    >
      {loading ? <Spinner size="sm" label="Working" /> : icon ? <Icon name={icon} /> : null}
      {children}
      {iconEnd && !loading ? <Icon name={iconEnd} /> : null}
    </button>
  );
}
