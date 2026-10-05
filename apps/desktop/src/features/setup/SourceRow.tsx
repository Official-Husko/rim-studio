import { useState } from 'preact/hooks';
import { Badge, Button, Switch, TextField } from 'rimstudio-ui';
import type { SourceDto, VolumeHintDto } from 'rimstudio-ipc-types';
import { t, tn } from '~/shared/i18n';
import { SOURCE_KIND_KEYS, SOURCE_STATUS_KEYS } from './model';

export interface SourceRowProps {
  source: SourceDto;
  index: number;
  count: number;
  drive?: VolumeHintDto;
  onToggle: (enabled: boolean) => void;
  onRename: (label: string) => void;
  onMove: (order: number) => void;
  onRemove: () => void;
}

/** One mod source: status, path, mod count and its controls. Built in sources only toggle. */
export function SourceRow({
  source,
  index,
  count,
  drive,
  onToggle,
  onRename,
  onMove,
  onRemove,
}: SourceRowProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(source.label);
  const name = source.label || source.path;
  const custom = source.kind === 'custom';
  const commit = (): void => {
    setEditing(false);
    if (draft.trim() && draft.trim() !== source.label) onRename(draft.trim());
  };
  return (
    <li class="flex flex-col gap-2 py-3" aria-label={name}>
      <div class="flex flex-wrap items-center gap-2">
        {editing ? (
          <>
            <div class="w-64">
              <TextField
                aria-label={t('setup.source.label.field')}
                value={draft}
                onValueChange={setDraft}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') commit();
                  if (e.key === 'Escape') setEditing(false);
                }}
              />
            </div>
            <Button size="sm" onClick={commit}>
              {t('setup.source.label.save')}
            </Button>
          </>
        ) : (
          <span class="text-body font-semibold">{name}</span>
        )}
        <Badge>{t(SOURCE_KIND_KEYS[source.kind])}</Badge>
        <Badge tone={source.status === 'ready' ? 'success' : 'warning'}>
          {t(SOURCE_STATUS_KEYS[source.status])}
        </Badge>
        {source.modCount === undefined ? (
          <span class="text-small text-muted">{t('setup.source.count.unknown')}</span>
        ) : (
          <span class="text-small text-muted">{tn('setup.source.count', source.modCount)}</span>
        )}
      </div>
      <div class="font-mono text-mono break-all text-muted">{source.path}</div>
      {drive ? (
        <div class="text-small text-muted">
          {t('setup.source.drive', { name: drive.label ?? drive.mount })}
        </div>
      ) : null}
      <div class="flex flex-wrap items-center gap-2">
        <Switch checked={source.enabled} onCheckedChange={onToggle}>
          {t('setup.source.enabled', { name })}
        </Switch>
        <Button
          size="sm"
          variant="ghost"
          disabled={index === 0}
          onClick={() => onMove(index - 1)}
          aria-label={t('setup.source.up', { name })}
        >
          {t('setup.source.up.short')}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          disabled={index >= count - 1}
          onClick={() => onMove(index + 1)}
          aria-label={t('setup.source.down', { name })}
        >
          {t('setup.source.down.short')}
        </Button>
        {custom ? (
          <>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => {
                setDraft(source.label);
                setEditing(true);
              }}
              aria-label={t('setup.source.rename', { name })}
            >
              {t('setup.source.rename.short')}
            </Button>
            <Button
              size="sm"
              variant="danger"
              icon="trash"
              onClick={onRemove}
              aria-label={t('setup.source.remove', { name })}
            >
              {t('setup.source.remove.short')}
            </Button>
          </>
        ) : null}
      </div>
    </li>
  );
}
