import { h, type ComponentChildren, type JSX } from 'preact';

type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'mode';

interface Props extends Omit<JSX.ButtonHTMLAttributes<HTMLButtonElement>, 'class' | 'className'> {
  active?: boolean;
  children: ComponentChildren;
  className?: string;
  variant?: ButtonVariant;
}

export function Button({
  active = false,
  children,
  className,
  type = 'button',
  variant = 'secondary',
  ...props
}: Props) {
  const variantClass =
    variant === 'primary'
      ? 'primary-button'
      : variant === 'ghost'
        ? 'ghost-button'
        : variant === 'mode'
          ? 'mode-button'
          : 'secondary-button';

  const classes = [variantClass, active ? 'is-active' : '', className ?? ''].filter(Boolean).join(' ');

  return (
    <button {...props} className={classes} type={type}>
      {children}
    </button>
  );
}
