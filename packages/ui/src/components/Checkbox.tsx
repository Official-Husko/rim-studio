import type { ComponentChildren } from 'preact';
import { useEffect, useRef } from 'preact/hooks';
import { cx } from '../cx';

export interface CheckboxProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  /** Visible label text. */
  children: ComponentChildren;
  /** Shows the mixed state; the click still resolves to checked or unchecked. */
  indeterminate?: boolean;
  invalid?: boolean;
  disabled?: boolean;
}

/** A native checkbox with a label. */
export function Checkbox({
  checked,
  onCheckedChange,
  children,
  indeterminate,
  invalid,
  disabled,
}: CheckboxProps) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (ref.current) ref.current.indeterminate = Boolean(indeterminate);
  }, [indeterminate, checked]);
  return (
    <label
      class={cx(
        'inline-flex min-h-control-sm items-center gap-2 text-body',
        disabled && 'cursor-not-allowed opacity-50',
      )}
    >
      <input
        ref={ref}
        type="checkbox"
        checked={checked}
        disabled={disabled}
        aria-invalid={invalid ? 'true' : undefined}
        onChange={(e) => onCheckedChange(e.currentTarget.checked)}
        class={cx('size-4 accent-accent', invalid && 'outline outline-danger')}
      />
      <span>{children}</span>
    </label>
  );
}
