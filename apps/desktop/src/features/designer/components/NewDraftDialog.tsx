import { useState } from 'preact/hooks';
import type { ApiError } from 'rimstudio-ipc-types';
import { Banner, Button, Dialog, FormField, Switch, TextField } from 'rimstudio-ui';
import { t } from '~/shared/i18n';

export interface NewDraftDialogProps {
  open: boolean;
  /** Dialog title, for example "New ranged weapon" or "Clone bolt-action rifle". */
  title: string;
  /** Initial def name. */
  defName: string;
  /** Initial label. */
  label: string;
  busy: boolean;
  /** The last error of the create or clone call, shown in the dialog. */
  error?: ApiError | undefined;
  onClose: () => void;
  /** Offer the own projectile choice (a clone of a gun). */
  offerOwnProjectile?: boolean;
  onSubmit: (defName: string, label: string, ownProjectile: boolean) => void;
}

/** Asks for the def name and the label of a new draft or a clone. The backend checks the name. */
export function NewDraftDialog({
  open,
  title,
  defName,
  label,
  busy,
  error,
  offerOwnProjectile = false,
  onClose,
  onSubmit,
}: NewDraftDialogProps) {
  const [name, setName] = useState(defName);
  const [text, setText] = useState(label);
  const [own, setOwn] = useState(true);
  return (
    <Dialog
      open={open}
      title={title}
      onClose={onClose}
      closeLabel={t('designer.dialog.close')}
      footer={
        <>
          <Button onClick={onClose}>{t('designer.dialog.cancel')}</Button>
          <Button
            variant="primary"
            loading={busy}
            disabled={name.trim() === ''}
            onClick={() => onSubmit(name.trim(), text.trim(), own)}
          >
            {t('designer.dialog.create')}
          </Button>
        </>
      }
    >
      {error ? (
        <Banner tone="error" title={error.code}>
          {error.message}
        </Banner>
      ) : null}
      <form
        class="flex flex-col gap-3"
        onSubmit={(event) => {
          event.preventDefault();
          if (name.trim() !== '') onSubmit(name.trim(), text.trim(), own);
        }}
      >
        <FormField label={t('designer.field.defName')} help={t('designer.help.defName')} required>
          <TextField value={name} onValueChange={setName} />
        </FormField>
        <FormField label={t('designer.field.label')}>
          <TextField value={text} onValueChange={setText} />
        </FormField>
        {offerOwnProjectile ? (
          <div class="flex flex-col gap-1">
            <Switch checked={own} onCheckedChange={setOwn}>
              {t('designer.dialog.ownProjectile')}
            </Switch>
            <p class="text-small text-faint">{t('designer.dialog.ownProjectileHelp')}</p>
          </div>
        ) : null}
      </form>
    </Dialog>
  );
}
