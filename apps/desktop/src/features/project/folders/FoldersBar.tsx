import { useState } from 'preact/hooks';
import { Banner, Button, Dialog, DiffView } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import {
  discardFolders,
  foldersBusy,
  foldersEditError,
  foldersLastSave,
  foldersPending,
  foldersPreview,
  foldersSaving,
  foldersStale,
  loadFolders,
  saveFolders,
} from './folderStore';

export interface FoldersBarProps {
  projectId: string;
}

/** The bar at the bottom of the tab: how many edits are unsaved, review, save and discard. */
export function FoldersBar({ projectId }: FoldersBarProps) {
  const [reviewing, setReviewing] = useState(false);
  const count = foldersPending.value;
  const saved = foldersLastSave.value;
  const save = async (): Promise<void> => {
    if (await saveFolders()) setReviewing(false);
  };
  return (
    <div class="sticky bottom-0 z-10 flex flex-col gap-2 border-t border-line-strong bg-raised p-3">
      {foldersStale.value ? (
        <Banner
          tone="warning"
          title={t('project.folders.stale.title')}
          action={
            <Button
              size="sm"
              variant="secondary"
              icon="refresh"
              onClick={() => void loadFolders(projectId)}
            >
              {t('project.folders.stale.reload')}
            </Button>
          }
        >
          {t('project.folders.stale.body')}
        </Banner>
      ) : foldersEditError.value ? (
        <Banner tone="error">{foldersEditError.value.message}</Banner>
      ) : null}
      <div
        class="flex flex-wrap items-center gap-3"
        role="region"
        aria-label={t('project.folders.bar.label')}
      >
        <div class="min-w-0 flex-1">
          <p class="m-0 font-semibold" aria-live="polite">
            {count > 0 ? tn('project.folders.bar.unsaved', count) : t('project.folders.bar.clean')}
          </p>
          <p class="m-0 text-small text-muted">
            {saved && count === 0 && saved.backup
              ? t('project.basics.bar.saved', { path: saved.backup })
              : t('project.folders.bar.backup')}
          </p>
        </div>
        <Button
          variant="ghost"
          disabled={count === 0 || foldersSaving.value}
          onClick={discardFolders}
        >
          {t('project.basics.discard')}
        </Button>
        <Button
          variant="secondary"
          icon="xml"
          disabled={count === 0}
          onClick={() => setReviewing(true)}
        >
          {t('project.basics.review')}
        </Button>
        <Button
          disabled={count === 0 || foldersBusy.value}
          loading={foldersSaving.value}
          onClick={() => void save()}
        >
          {t('project.basics.save')}
        </Button>
      </div>
      <Dialog
        open={reviewing}
        size="lg"
        title={t('project.folders.review.title')}
        onClose={() => setReviewing(false)}
        closeLabel={t('project.basics.close')}
        footer={
          <>
            <Button variant="secondary" onClick={() => setReviewing(false)}>
              {t('project.basics.review.back')}
            </Button>
            <Button
              disabled={count === 0}
              loading={foldersSaving.value}
              onClick={() => void save()}
            >
              {t('project.basics.save')}
            </Button>
          </>
        }
      >
        <DiffView
          diff={foldersPreview.value?.diff ?? ''}
          label={t('project.folders.review.diff')}
          emptyText={t('project.basics.review.empty')}
          class="max-h-96"
        />
      </Dialog>
    </div>
  );
}
