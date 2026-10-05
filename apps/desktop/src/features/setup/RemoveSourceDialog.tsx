import { Button, Dialog } from 'rimstudio-ui';
import type { SourceDto } from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';

export interface RemoveSourceDialogProps {
  source: SourceDto | undefined;
  onConfirm: () => void;
  onCancel: () => void;
}

/** Confirmation before a custom folder is forgotten; it says that no files are touched. */
export function RemoveSourceDialog({ source, onConfirm, onCancel }: RemoveSourceDialogProps) {
  return (
    <Dialog
      open={source !== undefined}
      title={t('setup.remove.title')}
      onClose={onCancel}
      closeLabel={t('setup.dialog.close')}
      footer={
        <>
          <Button variant="secondary" onClick={onCancel}>
            {t('setup.dialog.cancel')}
          </Button>
          <Button variant="danger" onClick={onConfirm}>
            {t('setup.remove.confirm')}
          </Button>
        </>
      }
    >
      <p class="m-0">{t('setup.remove.body', { name: source?.label || source?.path || '' })}</p>
      <p class="m-0 mt-2 font-mono text-mono break-all text-muted">{source?.path}</p>
      <p class="m-0 mt-2">{t('setup.remove.nofiles')}</p>
    </Dialog>
  );
}
