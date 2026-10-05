import type { ComponentChildren } from 'preact';
import { useId } from 'preact/hooks';
import { cx } from '../cx';

export interface TooltipProps {
  /** The tooltip text. Keep it short; it is also read as a description. */
  text: string;
  /** Where the bubble sits relative to the trigger. */
  side?: 'top' | 'bottom';
  /** The trigger; it should be focusable so keyboard users see the tooltip. */
  children: ComponentChildren;
}

/** A CSS only tooltip: shown on hover and focus within, no scripting, no portal. */
export function Tooltip({ text, side = 'top', children }: TooltipProps) {
  const id = useId();
  return (
    <span class="rs-tip" aria-describedby={id}>
      {children}
      <span
        role="tooltip"
        id={id}
        class={cx(
          'pointer-events-none absolute left-1/2 z-(--rs-z-menu) -translate-x-1/2 rounded-sm border border-line-strong bg-raised px-2 py-1 text-small whitespace-nowrap text-fg shadow-raised',
          side === 'top' ? 'bottom-full mb-1' : 'top-full mt-1',
        )}
      >
        {text}
      </span>
    </span>
  );
}
