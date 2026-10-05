import type { JSX } from 'preact';
import { cx } from '../cx';
import { Icon, type IconName } from './Icon';
import { Tooltip } from './Tooltip';

export type IconButtonVariant = 'ghost' | 'subtle' | 'danger';

export interface IconButtonProps extends Omit<
  JSX.HTMLAttributes<HTMLButtonElement>,
  'icon' | 'label'
> {
  /** Glyph to draw. */
  icon: IconName;
  /** Required: the accessible name and the tooltip text. */
  label: string;
  variant?: IconButtonVariant;
  /** 16 px glyph in rows, 20 px in toolbars. */
  iconSize?: 16 | 20;
  /** Toggle state; sets aria-pressed and the pressed look. */
  pressed?: boolean;
  /** Hide the tooltip when the surrounding UI already shows the label. */
  noTooltip?: boolean;
  disabled?: boolean;
}

const VARIANTS: Record<IconButtonVariant, string> = {
  ghost: 'bg-transparent text-muted hover:bg-hover hover:text-fg',
  subtle: 'bg-raised text-fg border-line-strong hover:border-accent',
  danger: 'bg-transparent text-danger hover:bg-diff-removed',
};

/** A square icon only button with a mandatory label. */
export function IconButton({
  icon,
  label,
  variant = 'ghost',
  iconSize = 16,
  pressed,
  noTooltip,
  disabled,
  class: className,
  ...rest
}: IconButtonProps) {
  const button = (
    <button
      {...rest}
      type="button"
      aria-label={label}
      aria-pressed={pressed === undefined ? undefined : pressed ? 'true' : 'false'}
      disabled={disabled}
      class={cx(
        'inline-flex size-control-sm shrink-0 items-center justify-center rounded-md border border-transparent',
        'transition-colors duration-(--rs-dur-fast) disabled:cursor-not-allowed disabled:opacity-50',
        VARIANTS[variant],
        pressed && 'bg-accent-tint text-accent border-accent',
        typeof className === 'string' && className,
      )}
    >
      <Icon name={icon} size={iconSize} />
    </button>
  );
  return noTooltip ? button : <Tooltip text={label}>{button}</Tooltip>;
}
