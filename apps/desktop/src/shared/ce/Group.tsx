import type { ComponentChildren } from 'preact';

export interface GroupProps {
  title: string;
  /** A short state shown next to the title, for example how many entries the group holds. */
  summary?: string;
  /** Open at first. */
  open?: boolean;
  children: ComponentChildren;
}

/** A collapsible group of the block editor. The title and the state stay visible when it is closed. */
export function Group({ title, summary, open, children }: GroupProps) {
  return (
    <details class="rounded-sm border border-line bg-surface p-2" {...(open ? { open: true } : {})}>
      <summary class="flex cursor-pointer flex-wrap items-baseline gap-2 text-body font-medium">
        <span>{title}</span>
        {summary ? <span class="text-small font-normal text-muted">{summary}</span> : null}
      </summary>
      <div class="mt-3 flex flex-col gap-3">{children}</div>
    </details>
  );
}
