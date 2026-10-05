import type { ComponentChildren } from 'preact';
import { useId, useRef, useState } from 'preact/hooks';
import { cx } from '../cx';

export interface SplitPaneProps {
  /** horizontal puts the panes side by side; vertical stacks them. */
  direction?: 'horizontal' | 'vertical';
  first: ComponentChildren;
  second: ComponentChildren;
  /** Initial size of the first pane in pixels (uncontrolled). */
  defaultSize?: number;
  /** Controlled size of the first pane in pixels. */
  size?: number;
  onSizeChange?: (size: number) => void;
  min?: number;
  max?: number;
  /** Accessible name of the divider. */
  label: string;
}

const STEP = 16;

/** Two panes with a draggable and keyboard operable divider (arrows, Shift for bigger steps, Home, End). */
export function SplitPane({
  direction = 'horizontal',
  first,
  second,
  defaultSize = 280,
  size,
  onSizeChange,
  min = 120,
  max = 720,
  label,
}: SplitPaneProps) {
  const [own, setOwn] = useState(defaultSize);
  const current = Math.min(max, Math.max(min, size ?? own));
  const firstId = useId();
  const drag = useRef<{ start: number; origin: number } | null>(null);
  const horizontal = direction === 'horizontal';

  const apply = (next: number): void => {
    const clamped = Math.min(max, Math.max(min, next));
    setOwn(clamped);
    onSizeChange?.(clamped);
  };

  const onKeyDown = (event: KeyboardEvent): void => {
    const big = event.shiftKey ? 4 : 1;
    const forward = horizontal ? 'ArrowRight' : 'ArrowDown';
    const back = horizontal ? 'ArrowLeft' : 'ArrowUp';
    if (event.key === forward) apply(current + STEP * big);
    else if (event.key === back) apply(current - STEP * big);
    else if (event.key === 'Home') apply(min);
    else if (event.key === 'End') apply(max);
    else return;
    event.preventDefault();
  };

  return (
    <div class={cx('flex size-full min-h-0 min-w-0', horizontal ? 'flex-row' : 'flex-col')}>
      <div
        id={firstId}
        class="min-h-0 min-w-0 shrink-0 overflow-auto"
        style={horizontal ? { width: `${current}px` } : { height: `${current}px` }}
      >
        {first}
      </div>
      <div
        role="separator"
        aria-label={label}
        aria-orientation={horizontal ? 'vertical' : 'horizontal'}
        aria-controls={firstId}
        aria-valuenow={current}
        aria-valuemin={min}
        aria-valuemax={max}
        tabIndex={0}
        data-focus-inset
        onKeyDown={onKeyDown}
        onPointerDown={(e) => {
          e.currentTarget.setPointerCapture?.(e.pointerId);
          drag.current = { start: horizontal ? e.clientX : e.clientY, origin: current };
        }}
        onPointerMove={(e) => {
          if (!drag.current) return;
          apply(drag.current.origin + (horizontal ? e.clientX : e.clientY) - drag.current.start);
        }}
        onPointerUp={() => {
          drag.current = null;
        }}
        class={cx(
          'shrink-0 touch-none bg-line hover:bg-accent',
          horizontal ? 'w-1 cursor-col-resize' : 'h-1 cursor-row-resize',
        )}
      />
      <div class="min-h-0 min-w-0 flex-1 overflow-auto">{second}</div>
    </div>
  );
}
