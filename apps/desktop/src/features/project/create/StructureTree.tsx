import { useMemo } from 'preact/hooks';
import { Icon } from 'rimstudio-ui';
import type { ScaffoldEntryDto } from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';
import { purposeOf } from './purposes';

export interface StructureTreeProps {
  /** The full path of the mod folder, the root of the tree. */
  root: string;
  entries: readonly ScaffoldEntryDto[];
}

const INDENT = [
  'pl-0',
  'pl-4',
  'pl-8',
  'pl-12',
  'pl-16',
  'pl-20',
  'pl-24',
  'pl-28',
  'pl-32',
] as const;

interface Row {
  path: string;
  name: string;
  depth: number;
  file: boolean;
}

function rowsOf(entries: readonly ScaffoldEntryDto[]): Row[] {
  return entries.map((entry) => {
    const parts = entry.path.split('/');
    return {
      path: entry.path,
      name: parts[parts.length - 1] ?? entry.path,
      depth: parts.length - 1,
      file: entry.kind === 'file',
    };
  });
}

/** The structure that will be written: every folder and file, indented, with a plain sentence about each. */
export function StructureTree({ root, entries }: StructureTreeProps) {
  const rows = useMemo(() => rowsOf(entries), [entries]);
  return (
    <div class="flex flex-col gap-2">
      <p class="m-0 break-all font-mono text-mono-small text-muted">{root}</p>
      <ul class="m-0 list-none p-0" aria-label={t('project.create.tree.label')}>
        {rows.map((row) => {
          const purpose = purposeOf(row.path);
          return (
            <li
              key={row.path}
              class={`${INDENT[Math.min(row.depth, INDENT.length - 1)] ?? 'pl-0'} flex items-start gap-2 py-0.5`}
            >
              <span class="mt-0.5 shrink-0 text-faint">
                <Icon name={row.file ? 'file' : 'folder'} />
              </span>
              <span class="min-w-0">
                <span class="font-mono text-mono">{row.name}</span>
                {purpose ? (
                  <span class="block text-small text-muted">
                    {t(purpose.key, { version: purpose.version ?? '' })}
                  </span>
                ) : null}
              </span>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
