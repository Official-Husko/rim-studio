import { Icon } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import type { PreviewEntry } from './scaffold';

export interface ScaffoldPreviewListProps {
  /** The folder that will be created, shown as the head of the list. */
  root: string;
  entries: readonly PreviewEntry[];
}

/** The folders and files that creating the mod will write, as an indented list. */
export function ScaffoldPreviewList({ root, entries }: ScaffoldPreviewListProps) {
  return (
    <div class="flex min-h-0 flex-col gap-2">
      <p class="m-0 font-mono text-mono break-all text-muted">{root}</p>
      <ul
        class="m-0 max-h-56 list-none overflow-auto border border-line bg-bg p-1 font-mono text-mono"
        aria-label={t('project.new.preview.label')}
        // a scrolling list must be reachable with the keyboard
        // oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex
        tabIndex={0}
      >
        {entries.map((entry) => {
          const parts = entry.path.split('/');
          const name = parts[parts.length - 1] ?? entry.path;
          return (
            <li
              key={entry.path}
              class="flex h-6 items-center gap-1.5"
              style={`padding-left:calc(${parts.length - 1} * 4 * var(--rs-space))`}
            >
              <span class="text-muted">
                <Icon name={entry.kind === 'folder' ? 'folder' : 'file'} />
              </span>
              <span class={entry.kind === 'file' ? 'text-fg' : 'text-muted'}>{name}</span>
            </li>
          );
        })}
      </ul>
      <p class="m-0 text-small text-faint">{tn('project.new.preview.count', entries.length)}</p>
    </div>
  );
}
