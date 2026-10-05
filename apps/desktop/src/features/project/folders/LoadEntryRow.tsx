import { useEffect, useState } from 'preact/hooks';
import { Badge, Button, Chip, TextField } from 'rimstudio-ui';
import type { DiagnosticDto, LoadEntryDto } from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';
import { ChipListEditor } from '../basics/ChipListEditor';
import { FieldFindings } from '../basics/FieldFindings';
import { MoveButtons } from '../basics/MoveButtons';
import { applyFoldersOp } from './folderStore';

export interface LoadEntryRowProps {
  block: number;
  entry: LoadEntryDto;
  count: number;
  findings: readonly DiagnosticDto[];
  disabled: boolean;
}

type Condition = 'ifModActive' | 'ifModActiveAll' | 'ifModNotActive';

const CONDITIONS: { key: Condition; label: Parameters<typeof t>[0] }[] = [
  { key: 'ifModActive', label: 'project.folders.cond.active' },
  { key: 'ifModActiveAll', label: 'project.folders.cond.all' },
  { key: 'ifModNotActive', label: 'project.folders.cond.not' },
];

function shownPath(path: string): string {
  return path === '' ? '/' : path;
}

/** One folder of a version block: its path, whether it exists, its conditions, and move and remove. */
export function LoadEntryRow({ block, entry, count, findings, disabled }: LoadEntryRowProps) {
  const [path, setPath] = useState(shownPath(entry.path));
  const [open, setOpen] = useState(false);
  useEffect(() => setPath(shownPath(entry.path)), [entry.path]);
  const name = shownPath(entry.path);

  const commitPath = (): void => {
    const next = path.trim();
    if (next === '' || next === name) {
      setPath(name);
      return;
    }
    void applyFoldersOp({
      op: 'set-entry',
      block,
      entry: entry.index,
      change: { path: next, dropIgnoredAttributes: false },
    });
  };
  const setCondition = (key: Condition, items: string[]): void => {
    void applyFoldersOp({
      op: 'set-entry',
      block,
      entry: entry.index,
      change: { [key]: items, dropIgnoredAttributes: false },
    });
  };
  const summary = CONDITIONS.flatMap((c) => entry[c.key].map((id) => ({ c, id })));

  return (
    <li>
      <div
        role="group"
        aria-label={t('project.folders.entry', { name })}
        class="flex flex-col gap-2 border border-line-subtle p-2"
      >
        <div class="flex flex-wrap items-center gap-2">
          <div class="min-w-48 flex-1">
            <TextField
              aria-label={t('project.folders.entry.path', { name })}
              value={path}
              disabled={disabled}
              onValueChange={setPath}
              onBlur={commitPath}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  event.preventDefault();
                  commitPath();
                }
              }}
            />
          </div>
          {entry.folderExists ? (
            <Badge tone="success">{t('project.folders.exists')}</Badge>
          ) : (
            <Badge tone="warning">{t('project.folders.missing')}</Badge>
          )}
          <MoveButtons
            name={name}
            canUp={entry.index > 0}
            canDown={entry.index < count - 1}
            disabled={disabled}
            onUp={() =>
              void applyFoldersOp({
                op: 'move-entry',
                block,
                entry: entry.index,
                to: entry.index - 1,
              })
            }
            onDown={() =>
              void applyFoldersOp({
                op: 'move-entry',
                block,
                entry: entry.index,
                to: entry.index + 1,
              })
            }
          />
          <Button size="sm" variant="ghost" aria-expanded={open} onClick={() => setOpen(!open)}>
            {t('project.folders.conditions')}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            icon="trash"
            disabled={disabled}
            aria-label={t('project.folders.entry.remove', { name })}
            onClick={() => void applyFoldersOp({ op: 'remove-entry', block, entry: entry.index })}
          >
            {t('project.folders.remove')}
          </Button>
        </div>
        {summary.length > 0 && !open ? (
          <ul
            class="m-0 flex list-none flex-wrap gap-1 p-0"
            aria-label={t('project.folders.conditions')}
          >
            {summary.map(({ c, id }) => (
              <li key={`${c.key}:${id}`}>
                <Chip>{`${t(c.label)}: ${id}`}</Chip>
              </li>
            ))}
          </ul>
        ) : null}
        {entry.ignoredAttributes.length > 0 ? (
          <div class="flex flex-wrap items-center gap-2 text-small text-warning">
            <span>
              {t('project.folders.ignored', { names: entry.ignoredAttributes.join(', ') })}
            </span>
            <Button
              size="sm"
              variant="secondary"
              disabled={disabled}
              onClick={() =>
                void applyFoldersOp({
                  op: 'set-entry',
                  block,
                  entry: entry.index,
                  change: { dropIgnoredAttributes: true },
                })
              }
            >
              {t('project.folders.ignored.drop')}
            </Button>
          </div>
        ) : null}
        {open ? (
          <div class="grid grid-cols-1 gap-3 lg:grid-cols-3">
            {CONDITIONS.map((c) => (
              <ChipListEditor
                key={c.key}
                library
                label={`${t(c.label)} (${name})`}
                items={entry[c.key]}
                disabled={disabled}
                onChange={(items) => setCondition(c.key, items)}
              />
            ))}
          </div>
        ) : null}
        <FieldFindings items={findings} />
      </div>
    </li>
  );
}
