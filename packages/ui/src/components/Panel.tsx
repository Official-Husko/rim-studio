import type { ComponentChildren } from 'preact';
import { useId, useState } from 'preact/hooks';
import { cx } from '../cx';
import { Icon } from './Icon';

export interface PanelProps {
  /** Section header text; rendered in the condensed label style. */
  title?: string;
  /** Adds a header button that collapses the body. */
  collapsible?: boolean;
  defaultCollapsed?: boolean;
  /** Controls placed at the right of the header. */
  actions?: ComponentChildren;
  /** Draws registration ticks at the corners. */
  framed?: boolean;
  children?: ComponentChildren;
}

/** A hairline panel with an optional header. Panels are square, separated by lines, not gaps. */
export function Panel({
  title,
  collapsible,
  defaultCollapsed = false,
  actions,
  framed,
  children,
}: PanelProps) {
  const [collapsed, setCollapsed] = useState(defaultCollapsed);
  const bodyId = useId();
  const open = !collapsible || !collapsed;
  return (
    <section class={cx('border border-line bg-surface', framed && 'bp-ticks')} aria-label={title}>
      {title ? (
        <header class="flex h-8 items-center justify-between gap-2 border-b border-line-subtle px-3">
          {collapsible ? (
            <button
              type="button"
              aria-expanded={open}
              aria-controls={bodyId}
              onClick={() => setCollapsed(!collapsed)}
              class="flex min-w-0 items-center gap-1 font-display text-label font-semibold tracking-label text-muted uppercase"
            >
              <Icon name="chevron" turn={open ? 1 : 0} />
              {title}
            </button>
          ) : (
            <h2 class="font-display text-label font-semibold tracking-label text-muted uppercase">
              {title}
            </h2>
          )}
          {actions ? <div class="flex items-center gap-1">{actions}</div> : null}
        </header>
      ) : null}
      <div id={bodyId} hidden={!open} class="p-3">
        {children}
      </div>
    </section>
  );
}
