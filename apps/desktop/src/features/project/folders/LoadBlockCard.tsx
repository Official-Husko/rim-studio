import { useState } from 'preact/hooks';
import { Button, Panel, TextField } from 'rimstudio-ui';
import type { DiagnosticDto, LoadBlockDto } from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';
import { FieldFindings } from '../basics/FieldFindings';
import { findingsFor, pointerOf } from '../basics/aboutModel';
import { LoadEntryRow } from './LoadEntryRow';
import { applyFoldersOp } from './folderStore';

export interface LoadBlockCardProps {
  block: LoadBlockDto;
  findings: readonly DiagnosticDto[];
  disabled: boolean;
}

/** One version block of LoadFolders.xml: its folders in load order, and a field to add another. */
export function LoadBlockCard({ block, findings, disabled }: LoadBlockCardProps) {
  const [path, setPath] = useState('');
  const own = findings.filter((d) => pointerOf(d) === `blocks/${block.index}`);
  const title = block.key ? t('project.folders.block', { version: block.key }) : block.tag;
  const add = (): void => {
    const next = path.trim();
    if (next === '') return;
    setPath('');
    void applyFoldersOp({ op: 'add-entry', block: block.index, entry: { path: next } });
  };
  return (
    <Panel
      title={title}
      framed
      actions={
        <Button
          size="sm"
          variant="ghost"
          icon="trash"
          disabled={disabled}
          aria-label={t('project.folders.block.remove', { version: block.key || block.tag })}
          onClick={() => void applyFoldersOp({ op: 'remove-block', block: block.index })}
        >
          {t('project.folders.remove')}
        </Button>
      }
    >
      <div class="flex flex-col gap-3 p-3">
        <FieldFindings items={own} />
        {block.entries.length === 0 ? (
          <p class="m-0 text-small text-muted">{t('project.folders.block.empty')}</p>
        ) : (
          <ul
            class="m-0 flex list-none flex-col gap-2 p-0"
            aria-label={t('project.folders.block.list', { version: block.key || block.tag })}
          >
            {block.entries.map((entry) => (
              <LoadEntryRow
                key={entry.index}
                block={block.index}
                entry={entry}
                count={block.entries.length}
                disabled={disabled}
                findings={findingsFor(findings, `blocks/${block.index}/entries/${entry.index}`)}
              />
            ))}
          </ul>
        )}
        <div class="flex items-center gap-2">
          <div class="min-w-48 flex-1">
            <TextField
              aria-label={t('project.folders.add.path', { version: block.key || block.tag })}
              placeholder={t('project.folders.add.placeholder')}
              value={path}
              disabled={disabled}
              onValueChange={setPath}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  event.preventDefault();
                  add();
                }
              }}
            />
          </div>
          <Button size="sm" icon="plus" disabled={disabled || path.trim() === ''} onClick={add}>
            {t('project.folders.add.folder')}
          </Button>
        </div>
      </div>
    </Panel>
  );
}
