import { useEffect, useState } from 'preact/hooks';
import { Banner, Button, CodeView, EmptyState, Panel, Spinner } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { FieldFindings } from '../basics/FieldFindings';
import { pointerOf } from '../basics/aboutModel';
import { FoldersBar } from './FoldersBar';
import { LoadBlockCard } from './LoadBlockCard';
import { VersionAddDialog } from './VersionAddDialog';
import { VersionOverview } from './VersionOverview';
import {
  foldersCreate,
  foldersError,
  foldersModel,
  foldersShown,
  foldersState,
  loadFolders,
  startFoldersCreate,
} from './folderStore';

export interface FoldersTabProps {
  projectId: string;
}

/** Versions and folders: the version folders of the mod and LoadFolders.xml as a structured editor. */
export function FoldersTab({ projectId }: FoldersTabProps) {
  const [adding, setAdding] = useState(false);
  useEffect(() => {
    void loadFolders(projectId, true);
  }, [projectId]);
  const data = foldersShown.value;

  if (foldersState.value === 'error' && !foldersModel.value) {
    return (
      <div class="p-4">
        <Banner
          tone="error"
          title={t('project.folders.error')}
          action={
            <Button size="sm" variant="secondary" onClick={() => void loadFolders(projectId)}>
              {t('project.basics.retry')}
            </Button>
          }
        >
          {foldersError.value?.message}
        </Banner>
      </div>
    );
  }
  if (!data) {
    return (
      <div class="p-4">
        <Spinner label={t('project.folders.loading')} />
      </div>
    );
  }
  const locked = !data.editable;
  const fileLevel = data.diagnostics.filter((d) => !pointerOf(d).startsWith('blocks/'));
  const blockLevel = data.diagnostics.filter((d) => pointerOf(d).startsWith('blocks/'));
  const showEditor = data.exists || foldersCreate.value;

  return (
    <div class="flex min-h-0 flex-col">
      <div class="flex flex-col gap-4 p-4">
        {locked && data.exists ? (
          <Banner tone="warning" title={t('project.folders.locked')}>
            {data.notEditableReason ?? t('project.basics.locked.body')}
          </Banner>
        ) : null}
        <VersionOverview data={data} disabled={locked} onAddFolder={() => setAdding(true)} />
        <FieldFindings items={fileLevel} />
        {showEditor ? (
          <>
            {!data.exists ? <Banner tone="info">{t('project.folders.willCreate')}</Banner> : null}
            {data.blocks.length === 0 ? (
              <p class="m-0 text-small text-muted">{t('project.folders.noBlocks')}</p>
            ) : null}
            {data.blocks.map((block) => (
              <LoadBlockCard
                key={block.index}
                block={block}
                findings={blockLevel}
                disabled={locked}
              />
            ))}
          </>
        ) : (
          <Panel title={t('project.folders.file')} framed>
            <EmptyState
              compact
              icon="file"
              title={t('project.folders.absent.title')}
              description={t('project.folders.absent.body')}
              action={
                <Button icon="plus" onClick={() => void startFoldersCreate()}>
                  {t('project.folders.absent.create')}
                </Button>
              }
            />
          </Panel>
        )}
        {data.exists || data.rawText ? (
          <Panel title={t('project.folders.raw')} framed collapsible defaultCollapsed>
            <div class="p-3">
              <CodeView
                code={data.rawText}
                label={t('project.folders.raw.label')}
                heightClass="max-h-96"
                copyLabel={t('project.basics.copy')}
                copiedLabel={t('project.basics.copied')}
              />
            </div>
          </Panel>
        ) : null}
      </div>
      <FoldersBar projectId={projectId} />
      <VersionAddDialog open={adding} projectId={projectId} onClose={() => setAdding(false)} />
    </div>
  );
}
