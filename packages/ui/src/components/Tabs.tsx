import type { ComponentChildren } from 'preact';
import { useId, useRef } from 'preact/hooks';
import { cx } from '../cx';

export interface TabItem {
  id: string;
  label: string;
  disabled?: boolean;
  /** A short count or status shown after the label. */
  badge?: string;
}

export interface TabsProps {
  tabs: TabItem[];
  value: string;
  onValueChange: (id: string) => void;
  /** Accessible name of the tab list. */
  label: string;
  /** Renders the panel of the selected tab. */
  children: (id: string) => ComponentChildren;
}

/** Tabs with a roving tab stop: arrows move, Home and End jump, the panel is labelled by its tab. */
export function Tabs({ tabs, value, onValueChange, label, children }: TabsProps) {
  const base = useId();
  const refs = useRef<Array<HTMLButtonElement | null>>([]);
  const enabled = tabs.map((t, i) => (t.disabled ? -1 : i)).filter((i) => i >= 0);

  const goTo = (index: number | undefined): void => {
    const tab = index === undefined ? undefined : tabs[index];
    if (tab && index !== undefined) {
      onValueChange(tab.id);
      refs.current[index]?.focus();
    }
  };

  const onKeyDown = (event: KeyboardEvent, from: number): void => {
    const pos = enabled.indexOf(from);
    if (event.key === 'ArrowRight') goTo(enabled[(pos + 1) % enabled.length]);
    else if (event.key === 'ArrowLeft') goTo(enabled[(pos - 1 + enabled.length) % enabled.length]);
    else if (event.key === 'Home') goTo(enabled[0]);
    else if (event.key === 'End') goTo(enabled[enabled.length - 1]);
    else return;
    event.preventDefault();
  };

  return (
    <div class="flex min-h-0 flex-col">
      <div role="tablist" aria-label={label} class="flex border-b border-line">
        {tabs.map((t, i) => {
          const selected = t.id === value;
          return (
            <button
              key={t.id}
              ref={(el) => {
                refs.current[i] = el;
              }}
              id={`${base}-tab-${t.id}`}
              type="button"
              role="tab"
              aria-selected={selected ? 'true' : 'false'}
              aria-controls={`${base}-panel-${t.id}`}
              disabled={t.disabled}
              tabIndex={selected ? 0 : -1}
              onClick={() => onValueChange(t.id)}
              onKeyDown={(e) => onKeyDown(e, i)}
              class={cx(
                'inline-flex h-control items-center gap-2 border-b-2 px-3 text-body whitespace-nowrap disabled:cursor-not-allowed disabled:opacity-50',
                selected
                  ? 'border-accent font-semibold text-fg'
                  : 'border-transparent text-muted hover:bg-hover hover:text-fg',
              )}
            >
              {t.label}
              {t.badge ? <span class="font-mono text-mono-small text-faint">{t.badge}</span> : null}
            </button>
          );
        })}
      </div>
      <div
        id={`${base}-panel-${value}`}
        role="tabpanel"
        aria-labelledby={`${base}-tab-${value}`}
        class="min-h-0 flex-1 pt-3"
      >
        {children(value)}
      </div>
    </div>
  );
}
