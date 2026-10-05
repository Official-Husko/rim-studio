import type { ComponentChildren } from 'preact';

export interface MoreFieldsProps {
  title: string;
  /** How many entries the group holds; a group with entries starts open. */
  count: number;
  children: ComponentChildren;
}

/** A group of rarely used lists that stays folded until it has entries or the user opens it. */
export function MoreFields({ title, count, children }: MoreFieldsProps) {
  return (
    <details open={count > 0 ? true : undefined} class="border-t border-line-subtle pt-2">
      <summary class="cursor-pointer text-small font-semibold text-muted focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus">
        {title} ({count})
      </summary>
      <div class="flex flex-col gap-3 pt-3">{children}</div>
    </details>
  );
}
