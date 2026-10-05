import type { ComponentChildren } from 'preact';
import { useRef, useState } from 'preact/hooks';
import { cx } from '../cx';
import { Icon } from './Icon';

export type SortDirection = 'asc' | 'desc';

export interface TableColumn<T> {
  key: string;
  header: string;
  render: (row: T) => ComponentChildren;
  /** Shows a sort button in the header; sorting itself is done by the caller (the backend). */
  sortable?: boolean;
  align?: 'left' | 'right';
  /** A static width utility class such as w-32; leave empty for a flexible column. */
  widthClass?: string;
  /** Machine values (numbers, ids) use the mono face. */
  mono?: boolean;
}

export interface TableProps<T> {
  columns: TableColumn<T>[];
  rows: T[];
  getKey: (row: T) => string;
  /** Accessible name of the table. */
  label: string;
  sort?: { key: string; direction: SortDirection };
  onSortChange?: (key: string, direction: SortDirection) => void;
  selection?: 'none' | 'single' | 'multiple';
  selectedKeys?: ReadonlySet<string>;
  onSelectionChange?: (keys: Set<string>) => void;
  /** Called on Enter or double click of a row. */
  onRowActivate?: (row: T) => void;
  dense?: boolean;
  emptyText?: string;
}

/** A dense table with sortable headers and keyboard selectable rows. */
export function Table<T>(props: TableProps<T>) {
  const {
    columns,
    rows,
    getKey,
    label,
    sort,
    selection = 'none',
    onRowActivate,
    dense,
    emptyText,
  } = props;
  const selected = props.selectedKeys ?? new Set<string>();
  const [focusIndex, setFocusIndex] = useState(0);
  const rowRefs = useRef<Array<HTMLTableRowElement | null>>([]);
  const lastAnchor = useRef<number>(0);

  const select = (
    index: number,
    event?: { ctrlKey?: boolean; metaKey?: boolean; shiftKey?: boolean },
  ): void => {
    if (selection === 'none' || !props.onSelectionChange) return;
    const row = rows[index];
    if (!row) return;
    const key = getKey(row);
    if (selection === 'single') {
      props.onSelectionChange(new Set([key]));
    } else if (event?.shiftKey) {
      const [from, to] = [Math.min(lastAnchor.current, index), Math.max(lastAnchor.current, index)];
      props.onSelectionChange(new Set(rows.slice(from, to + 1).map(getKey)));
      return;
    } else if (event?.ctrlKey || event?.metaKey || selected.has(key)) {
      const next = new Set(selected);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      props.onSelectionChange(next);
    } else {
      props.onSelectionChange(new Set([key]));
    }
    lastAnchor.current = index;
  };

  const focusRow = (index: number): void => {
    const clamped = Math.max(0, Math.min(rows.length - 1, index));
    setFocusIndex(clamped);
    rowRefs.current[clamped]?.focus();
  };

  const onKeyDown = (event: KeyboardEvent, index: number, row: T): void => {
    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault();
        focusRow(index + 1);
        break;
      case 'ArrowUp':
        event.preventDefault();
        focusRow(index - 1);
        break;
      case 'Home':
        event.preventDefault();
        focusRow(0);
        break;
      case 'End':
        event.preventDefault();
        focusRow(rows.length - 1);
        break;
      case ' ':
        event.preventDefault();
        select(index, event);
        break;
      case 'Enter':
        event.preventDefault();
        onRowActivate?.(row);
        break;
      default:
    }
  };

  const toggleSort = (column: TableColumn<T>): void => {
    if (!column.sortable || !props.onSortChange) return;
    const direction: SortDirection =
      sort?.key === column.key && sort.direction === 'asc' ? 'desc' : 'asc';
    props.onSortChange(column.key, direction);
  };

  return (
    <table
      role="grid"
      aria-label={label}
      aria-multiselectable={selection === 'multiple' ? 'true' : undefined}
      class="w-full border-collapse text-body"
    >
      <thead>
        <tr class="border-b border-line bg-raised">
          {columns.map((c) => {
            const sorted = sort?.key === c.key ? sort.direction : undefined;
            return (
              <th
                key={c.key}
                scope="col"
                aria-sort={
                  sorted
                    ? sorted === 'asc'
                      ? 'ascending'
                      : 'descending'
                    : c.sortable
                      ? 'none'
                      : undefined
                }
                class={cx(
                  'h-control-sm px-3 font-display text-label font-semibold tracking-label text-muted uppercase',
                  c.align === 'right' ? 'text-right' : 'text-left',
                  c.widthClass,
                )}
              >
                {c.sortable ? (
                  <button
                    type="button"
                    onClick={() => toggleSort(c)}
                    class={cx(
                      'inline-flex items-center gap-1 uppercase hover:text-fg',
                      c.align === 'right' && 'flex-row-reverse',
                    )}
                  >
                    {c.header}
                    {sorted ? <Icon name={sorted === 'asc' ? 'arrow-up' : 'arrow-down'} /> : null}
                  </button>
                ) : (
                  c.header
                )}
              </th>
            );
          })}
        </tr>
      </thead>
      <tbody>
        {rows.length === 0 ? (
          <tr>
            <td colSpan={columns.length} class="px-3 py-6 text-center text-muted">
              {emptyText ?? 'No rows'}
            </td>
          </tr>
        ) : null}
        {rows.map((row, index) => {
          const key = getKey(row);
          const isSelected = selected.has(key);
          return (
            <tr
              key={key}
              ref={(el) => {
                rowRefs.current[index] = el;
              }}
              tabIndex={index === focusIndex ? 0 : -1}
              aria-selected={selection === 'none' ? undefined : isSelected ? 'true' : 'false'}
              data-selected={isSelected ? '' : undefined}
              onFocus={() => setFocusIndex(index)}
              onClick={(e) => select(index, e)}
              onDblClick={() => onRowActivate?.(row)}
              onKeyDown={(e) => onKeyDown(e, index, row)}
              class={cx(
                'border-b border-line-subtle hover:bg-hover data-selected:bg-accent-tint data-selected:font-semibold',
                dense ? 'h-control-sm' : 'h-row',
              )}
            >
              {columns.map((c) => (
                <td
                  key={c.key}
                  class={cx(
                    'truncate px-3',
                    c.align === 'right' && 'text-right',
                    c.mono && 'font-mono text-mono tabular-nums',
                  )}
                >
                  {c.render(row)}
                </td>
              ))}
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
