import { useEffect, useState } from 'preact/hooks';
import { Banner, Button, Dialog, FormField, TextField } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { pickFolder } from '~/shared/platform';
import { NewModOptions } from './NewModOptions';
import { ScaffoldPreviewList } from './ScaffoldPreviewList';
import { folderNameOf, joinPath, suggestPackageId } from './model';
import { emptyForm, requestOf, scaffoldPreview, type NewModForm } from './scaffold';
import { createError, creating, createMod } from './store';

export interface NewModDialogProps {
  open: boolean;
  onClose: () => void;
  /** The parent folder to start with, when the user has one in mind. */
  startParent?: string;
}

/** The new mod dialog: the identity of the mod, the scaffold options and the list that will be written. */
export function NewModDialog({ open, onClose, startParent }: NewModDialogProps) {
  const [form, setForm] = useState<NewModForm>(emptyForm);
  const [parent, setParent] = useState(startParent ?? '');
  const [idTouched, setIdTouched] = useState(false);
  const [folderTouched, setFolderTouched] = useState(false);

  useEffect(() => {
    if (!open) return;
    setForm(emptyForm());
    setParent(startParent ?? '');
    setIdTouched(false);
    setFolderTouched(false);
    createError.value = undefined;
  }, [open, startParent]);

  const change = (patch: Partial<NewModForm>): void => {
    setForm((current) => {
      const next = { ...current, ...patch };
      if (!idTouched && ('name' in patch || 'author' in patch)) {
        next.packageId = suggestPackageId(next.author, next.name);
      }
      if (!folderTouched && 'name' in patch) next.folderName = folderNameOf(next.name);
      return next;
    });
  };

  const browse = async (): Promise<void> => {
    const picked = await pickFolder(parent ? { start: parent } : {});
    if (picked) setParent(picked);
  };

  const target = parent && form.folderName ? joinPath(parent, form.folderName) : '';
  const ready = form.name.trim() !== '' && form.packageId.trim() !== '' && target !== '';

  const submit = async (): Promise<void> => {
    if (!ready) return;
    if (await createMod(requestOf(form, target))) onClose();
  };

  return (
    <Dialog
      open={open}
      title={t('project.new.title')}
      size="lg"
      onClose={onClose}
      closeLabel={t('project.dialog.close')}
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>
            {t('project.dialog.cancel')}
          </Button>
          <Button disabled={!ready} loading={creating.value} onClick={() => void submit()}>
            {t('project.new.create')}
          </Button>
        </>
      }
    >
      <div class="grid grid-cols-1 gap-6 md:grid-cols-2">
        <div class="flex min-w-0 flex-col gap-3">
          <FormField label={t('project.new.parent')} help={t('project.new.parent.hint')} required>
            <TextField
              value={parent}
              onValueChange={setParent}
              suffix={
                <Button size="sm" variant="ghost" icon="folder" onClick={() => void browse()}>
                  {t('project.new.browse')}
                </Button>
              }
            />
          </FormField>
          <div class="grid grid-cols-2 gap-3">
            <FormField label={t('project.new.name')} required>
              <TextField value={form.name} onValueChange={(name) => change({ name })} />
            </FormField>
            <FormField label={t('project.new.author')}>
              <TextField value={form.author} onValueChange={(author) => change({ author })} />
            </FormField>
          </div>
          <FormField
            label={t('project.new.packageId')}
            help={t('project.new.packageId.hint')}
            required
          >
            <TextField
              value={form.packageId}
              onValueChange={(packageId) => {
                setIdTouched(true);
                change({ packageId });
              }}
            />
          </FormField>
          <div class="grid grid-cols-2 gap-3">
            <FormField label={t('project.new.versions')} help={t('project.new.versions.hint')}>
              <TextField value={form.versions} onValueChange={(versions) => change({ versions })} />
            </FormField>
            <FormField label={t('project.new.folder')}>
              <TextField
                value={form.folderName}
                onValueChange={(folderName) => {
                  setFolderTouched(true);
                  change({ folderName });
                }}
              />
            </FormField>
          </div>
          <FormField label={t('project.new.description')} help={t('project.new.description.hint')}>
            <TextField
              multiline
              rows={2}
              value={form.description}
              onValueChange={(description) => change({ description })}
            />
          </FormField>
        </div>
        <div class="flex min-w-0 flex-col gap-4">
          <NewModOptions form={form} onChange={change} />
          <ScaffoldPreviewList
            root={target || t('project.new.preview.empty')}
            entries={scaffoldPreview(form)}
          />
        </div>
      </div>
      {createError.value ? (
        <div class="mt-4">
          <Banner tone="error" title={t('project.new.error')}>
            {createError.value.message}
          </Banner>
        </div>
      ) : null}
    </Dialog>
  );
}
