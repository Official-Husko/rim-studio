import type { ComponentChildren } from 'preact';
import { cx } from '../cx';

export interface KeyValueItem {
  key: string;
  value: ComponentChildren;
  /** Machine values (ids, paths, numbers) use the mono face. */
  mono?: boolean;
}

export interface KeyValueListProps {
  items: KeyValueItem[];
  /** Accessible name of the list. */
  label?: string;
}

/** A definition list laid out as two columns. */
export function KeyValueList({ items, label }: KeyValueListProps) {
  return (
    <dl aria-label={label} class="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1 text-body">
      {items.map((item) => (
        <div key={item.key} class="contents">
          <dt class="text-muted">{item.key}</dt>
          <dd class={cx('m-0 min-w-0 break-words text-fg', item.mono && 'font-mono text-mono')}>
            {item.value}
          </dd>
        </div>
      ))}
    </dl>
  );
}
