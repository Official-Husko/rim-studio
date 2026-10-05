import { Banner, Button, Dialog, DiffView, Spinner } from 'rimstudio-ui';
import { tn, t } from '~/shared/i18n';
import {
  aboutPreviewResult,
  aboutSaving,
  pendingChanges,
  previewError,
  previewing,
  saveAbout,
} from './aboutStore';

export interface ReviewDialogProps {
  open: boolean;
  onClose: () => void;
}

/** The exact change to About.xml as a diff, with the choice to save it. */
export function ReviewDialog({ open, onClose }: ReviewDialogProps) {
  const count = pendingChanges.value.length;
  const preview = aboutPreviewResult.value;
  const save = async (): Promise<void> => {
    if (await saveAbout()) onClose();
  };
  return (
    <Dialog
      open={open}
      size="lg"
      title={t('project.basics.review.title')}
      onClose={onClose}
      closeLabel={t('project.basics.close')}
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>
            {t('project.basics.review.back')}
          </Button>
          <Button
            disabled={count === 0 || previewing.value || Boolean(previewError.value)}
            loading={aboutSaving.value}
            onClick={() => void save()}
          >
            {t('project.basics.save')}
          </Button>
        </>
      }
    >
      <div class="flex flex-col gap-3">
        <p class="m-0 text-small text-muted">
          {tn('project.basics.review.count', count)} {t('project.basics.review.help')}
        </p>
        {previewError.value ? (
          <Banner tone="error">{previewError.value.message}</Banner>
        ) : previewing.value && !preview ? (
          <Spinner label={t('project.basics.review.loading')} />
        ) : (
          <DiffView
            diff={preview?.diff ?? ''}
            label={t('project.basics.review.diff')}
            emptyText={t('project.basics.review.empty')}
            class="max-h-96"
          />
        )}
      </div>
    </Dialog>
  );
}
