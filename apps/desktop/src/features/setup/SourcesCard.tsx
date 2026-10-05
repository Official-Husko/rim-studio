import { useState } from 'preact/hooks';
import { Banner, Button, EmptyState, Panel, Spinner } from 'rimstudio-ui';
import type { SourceDto } from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';
import { pickFolder } from '~/shared/platform';
import { AddFolderDialog } from './AddFolderDialog';
import { RemoveSourceDialog } from './RemoveSourceDialog';
import { settings } from './settingsStore';
import { SourceRow } from './SourceRow';
import {
  forgetSource,
  moveSource,
  renameSource,
  setSourceEnabled,
  sources,
  sourcesError,
  sourcesLoaded,
  startAdd,
} from './sourcesStore';

/** Card 2: the mod folders, in scan order, with add, rename, enable, move and remove. */
export function SourcesCard() {
  const [removing, setRemoving] = useState<SourceDto | undefined>(undefined);
  const list = sources.value;
  const hints = settings.value?.customFolders ?? [];

  const add = async (): Promise<void> => {
    const picked = await pickFolder();
    if (picked) await startAdd(picked);
  };

  return (
    <Panel
      title={t('setup.sources.title')}
      framed
      actions={
        <Button size="sm" icon="plus" onClick={() => void add()}>
          {t('setup.sources.add')}
        </Button>
      }
    >
      <p class="m-0 text-small text-muted">{t('setup.sources.hint')}</p>
      {sourcesError.value ? (
        <div class="mt-3">
          <Banner tone="error" title={t('setup.sources.error')}>
            {sourcesError.value.message}
          </Banner>
        </div>
      ) : null}
      {list.length > 0 ? (
        <ul
          class="m-0 mt-2 list-none divide-y divide-line-subtle p-0"
          aria-label={t('setup.sources.list')}
        >
          {list.map((source, index) => (
            <SourceRow
              key={source.id}
              source={source}
              index={index}
              count={list.length}
              drive={hints.find((h) => h.id === source.id)?.volumeHint}
              onToggle={(enabled) => void setSourceEnabled(source.id, enabled)}
              onRename={(label) => void renameSource(source.id, label)}
              onMove={(order) => void moveSource(source.id, order)}
              onRemove={() => setRemoving(source)}
            />
          ))}
        </ul>
      ) : sourcesLoaded.value ? (
        <EmptyState
          compact
          icon="folder"
          title={t('setup.sources.empty.title')}
          description={t('setup.sources.empty.hint')}
        />
      ) : (
        <Spinner label={t('app.loading')} />
      )}
      <AddFolderDialog />
      <RemoveSourceDialog
        source={removing}
        onCancel={() => setRemoving(undefined)}
        onConfirm={() => {
          const target = removing;
          setRemoving(undefined);
          if (target) void forgetSource(target.id);
        }}
      />
    </Panel>
  );
}
