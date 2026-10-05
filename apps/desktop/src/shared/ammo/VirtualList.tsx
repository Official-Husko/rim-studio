import type { ComponentChildren } from 'preact';
import { useEffect, useRef, useState } from 'preact/hooks';

export interface VirtualListProps<T> {
  items: readonly T[];
  /** Height of every row in pixels; rows of a windowed list must all be the same height. */
  rowHeight: number;
  /** Accessible name of the list. */
  label: string;
  getKey: (item: T) => string;
  renderRow: (item: T, index: number) => ComponentChildren;
  selectedKey: string | undefined;
  onSelect: (key: string) => void;
  /** Rows drawn above and below the visible ones. */
  overscan?: number;
  /** Viewport height assumed until the element has been measured. */
  fallbackHeight?: number;
}

/**
 * A windowed list: only the rows in view (and a few more) are in the page, so hundreds of rows scroll
 * smoothly. It is a single selection listbox: the arrow keys, Home and End move the selection and keep it in view.
 */
export function VirtualList<T>({
  items,
  rowHeight,
  label,
  getKey,
  renderRow,
  selectedKey,
  onSelect,
  overscan = 6,
  fallbackHeight = 480,
}: VirtualListProps<T>) {
  const ref = useRef<HTMLDivElement>(null);
  const [top, setTop] = useState(0);
  const [height, setHeight] = useState(fallbackHeight);

  useEffect(() => {
    const el = ref.current;
    if (!el) return undefined;
    const measure = (): void => {
      if (el.clientHeight > 0) setHeight(el.clientHeight);
    };
    measure();
    if (typeof ResizeObserver === 'undefined') return undefined;
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const first = Math.max(0, Math.floor(top / rowHeight) - overscan);
  const last = Math.min(items.length, Math.ceil((top + height) / rowHeight) + overscan);
  const slice = items.slice(first, last);
  const selectedIndex = items.findIndex((item) => getKey(item) === selectedKey);

  const reveal = (index: number): void => {
    const el = ref.current;
    if (!el) return;
    const rowTop = index * rowHeight;
    if (rowTop < el.scrollTop) el.scrollTop = rowTop;
    else if (rowTop + rowHeight > el.scrollTop + height) el.scrollTop = rowTop + rowHeight - height;
  };

  const move = (to: number): void => {
    const item = items[Math.max(0, Math.min(items.length - 1, to))];
    if (!item) return;
    onSelect(getKey(item));
    reveal(Math.max(0, Math.min(items.length - 1, to)));
  };

  const onKeyDown = (event: KeyboardEvent): void => {
    const page = Math.max(1, Math.floor(height / rowHeight) - 1);
    if (event.key === 'ArrowDown') move(selectedIndex + 1);
    else if (event.key === 'ArrowUp') move(selectedIndex < 0 ? 0 : selectedIndex - 1);
    else if (event.key === 'PageDown') move(selectedIndex + page);
    else if (event.key === 'PageUp') move(selectedIndex - page);
    else if (event.key === 'Home') move(0);
    else if (event.key === 'End') move(items.length - 1);
    else return;
    event.preventDefault();
  };

  const active = selectedKey === undefined ? undefined : `${label}-${selectedKey}`;
  return (
    // The scroll area is the listbox: it owns the keyboard model of the options it draws.
    // oxlint-disable-next-line jsx-a11y/no-noninteractive-element-interactions
    <div
      ref={ref}
      role="listbox"
      aria-label={label}
      aria-activedescendant={active}
      tabIndex={0}
      onScroll={(e) => setTop(e.currentTarget.scrollTop)}
      onKeyDown={onKeyDown}
      class="relative min-h-0 flex-1 overflow-auto border border-line bg-surface"
    >
      <div style={{ height: items.length * rowHeight }} class="relative">
        {slice.map((item, offset) => {
          const index = first + offset;
          const key = getKey(item);
          const selected = key === selectedKey;
          return (
            // The option is selected by pointer or by the listbox keys above.
            // oxlint-disable-next-line jsx-a11y/click-events-have-key-events
            <div
              key={key}
              id={`${label}-${key}`}
              role="option"
              aria-selected={selected}
              aria-posinset={index + 1}
              aria-setsize={items.length}
              data-key={key}
              tabIndex={-1}
              onClick={() => onSelect(key)}
              style={{ top: index * rowHeight, height: rowHeight }}
              class={
                selected
                  ? 'absolute inset-x-0 cursor-pointer border-b border-line bg-accent-tint'
                  : 'absolute inset-x-0 cursor-pointer border-b border-line hover:bg-hover'
              }
            >
              {renderRow(item, index)}
            </div>
          );
        })}
      </div>
    </div>
  );
}
