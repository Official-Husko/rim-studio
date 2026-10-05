import type { ComponentChildren } from 'preact';
import { cx } from '../cx';

export interface SwitchProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  /** Visible label text. */
  children: ComponentChildren;
  disabled?: boolean;
}

/** An on or off switch. Space and Enter toggle it; the state is also stated to assistive tech. */
export function Switch({ checked, onCheckedChange, children, disabled }: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked ? 'true' : 'false'}
      disabled={disabled}
      onClick={() => onCheckedChange(!checked)}
      class="inline-flex min-h-control-sm items-center gap-2 text-body text-fg disabled:cursor-not-allowed disabled:opacity-50"
    >
      <span
        aria-hidden="true"
        class={cx(
          'inline-flex h-5 w-9 shrink-0 items-center rounded-full border px-0.5 transition-colors duration-(--rs-dur-fast)',
          checked ? 'border-accent bg-accent' : 'border-line-strong bg-raised',
        )}
      >
        <span
          class={cx(
            'size-3.5 rounded-full transition-transform duration-(--rs-dur-fast)',
            checked ? 'translate-x-4 bg-on-accent' : 'translate-x-0 bg-muted',
          )}
        />
      </span>
      <span>{children}</span>
    </button>
  );
}
