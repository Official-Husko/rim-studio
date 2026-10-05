import { useState } from 'preact/hooks';
import type { DraftEntryDto, ItemKindDto } from 'rimstudio-ipc-types';
import { Badge, Banner, Button, Card, Dialog, EmptyState, IconButton, Panel } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import type { DraftsStore } from '../drafts-store';
import type { SaveState } from '../editor-store';

export interface DraftListProps {
  store: DraftsStore;
  /** The id of the open draft and its save state, for the status line. */
  openId: string | undefined;
  openState: SaveState;
  onOpen: (entry: DraftEntryDto) => void;
  onNew: (kind: ItemKindDto) => void;
  onDelete: (entry: DraftEntryDto) => void;
}

function statusOf(entry: DraftEntryDto, openId: string | undefined, state: SaveState): string {
  if (entry.id !== openId) return t('designer.drafts.stored');
  switch (state) {
    case 'dirty':
      return t('designer.save.dirty');
    case 'saving':
      return t('designer.save.saving');
    case 'error':
      return t('designer.save.error');
    default:
      return t('designer.save.saved');
  }
}

/** The drafts of the project: open one, start a ranged or melee weapon, delete. */
export function DraftList({ store, openId, openState, onOpen, onNew, onDelete }: DraftListProps) {
  const [confirm, setConfirm] = useState<DraftEntryDto | undefined>(undefined);
  const entries = store.entries.value;
  const error = store.error.value;
  return (
    <Panel
      title={t('designer.panel.drafts')}
      actions={
        <>
          <Button size="sm" icon="plus" onClick={() => onNew('ranged')}>
            {t('designer.drafts.newRanged')}
          </Button>
          <Button size="sm" icon="plus" onClick={() => onNew('melee')}>
            {t('designer.drafts.newMelee')}
          </Button>
        </>
      }
    >
      <div class="flex flex-col gap-2">
        {error ? (
          <Banner tone="error" title={error.code}>
            {error.message}
          </Banner>
        ) : null}
        {entries.length === 0 ? (
          <EmptyState
            compact
            icon="crosshair"
            title={t('designer.drafts.emptyTitle')}
            description={t('designer.drafts.emptyBody')}
          />
        ) : (
          <ul class="flex flex-col gap-2">
            {entries.map((entry) => (
              <li key={entry.id} class="flex items-stretch gap-1">
                <div class="min-w-0 flex-1">
                  <Card
                    selected={entry.id === openId}
                    onSelect={() => onOpen(entry)}
                    label={entry.label || entry.defName}
                  >
                    <span class="flex flex-col gap-1">
                      <span class="truncate text-body font-semibold text-fg">
                        {entry.label || entry.defName}
                      </span>
                      <span class="flex flex-wrap items-center gap-2">
                        <span class="font-mono text-mono-small text-faint">{entry.defName}</span>
                        <Badge>
                          {entry.kind === 'ranged'
                            ? t('designer.kind.ranged')
                            : t('designer.kind.melee')}
                        </Badge>
                        <span class="text-small text-muted">
                          {statusOf(entry, openId, openState)}
                        </span>
                      </span>
                    </span>
                  </Card>
                </div>
                <IconButton
                  icon="trash"
                  variant="danger"
                  label={t('designer.drafts.delete', { name: entry.label || entry.defName })}
                  onClick={() => setConfirm(entry)}
                />
              </li>
            ))}
          </ul>
        )}
      </div>
      <Dialog
        open={confirm !== undefined}
        title={t('designer.drafts.deleteTitle')}
        onClose={() => setConfirm(undefined)}
        closeLabel={t('designer.dialog.close')}
        footer={
          <>
            <Button onClick={() => setConfirm(undefined)}>{t('designer.dialog.cancel')}</Button>
            <Button
              variant="danger"
              onClick={() => {
                if (confirm) onDelete(confirm);
                setConfirm(undefined);
              }}
            >
              {t('designer.drafts.confirmDelete')}
            </Button>
          </>
        }
      >
        <p>{t('designer.drafts.deleteBody', { name: confirm?.label || confirm?.defName || '' })}</p>
      </Dialog>
    </Panel>
  );
}
