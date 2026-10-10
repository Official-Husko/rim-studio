import { useState } from 'preact/hooks';
import type { ApiError } from 'rimstudio-ipc-types';
import { Banner, Button, Dialog } from 'rimstudio-ui';
import { t } from '~/shared/i18n';

export interface EditsFileBarProps {
  /** The project file whose definition the open draft edits. */
  file: string;
  /** The file is being read again. */
  busy: boolean;
  /** Why reading the file again failed, when it did. */
  error: ApiError | undefined;
  /** Read the file again and replace the draft; called after the confirmation. */
  onReload: () => void;
}

/**
 * Above the editor of a weapon that is edited where it stands: names the file, says what Apply
 * does to it, and offers to read the file again after asking, because that replaces the draft.
 */
export function EditsFileBar({ file, busy, error, onReload }: EditsFileBarProps) {
  const [confirming, setConfirming] = useState(false);
  return (
    <div class="flex flex-col gap-2 px-3 pt-3">
      <Banner
        tone="info"
        title={t('designer.editsFile.title')}
        action={
          <Button size="sm" icon="refresh" loading={busy} onClick={() => setConfirming(true)}>
            {t('designer.editsFile.reload')}
          </Button>
        }
      >
        <span class="mt-1 block break-all font-mono text-mono-small text-fg">{file}</span>
        <span class="block text-small text-muted">{t('designer.editsFile.body')}</span>
      </Banner>
      {error ? (
        <Banner tone="error" title={error.code}>
          {error.message}
        </Banner>
      ) : null}
      <Dialog
        open={confirming}
        title={t('designer.editsFile.reloadTitle')}
        onClose={() => setConfirming(false)}
        closeLabel={t('designer.dialog.close')}
        footer={
          <>
            <Button onClick={() => setConfirming(false)}>{t('designer.dialog.cancel')}</Button>
            <Button
              variant="danger"
              onClick={() => {
                setConfirming(false);
                onReload();
              }}
            >
              {t('designer.editsFile.reloadConfirm')}
            </Button>
          </>
        }
      >
        <p>{t('designer.editsFile.reloadBody')}</p>
      </Dialog>
    </div>
  );
}
