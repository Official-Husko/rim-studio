import { Button, Checkbox, FormField, TextField } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { pickFolder } from '~/shared/platform';
import { FieldFindings } from '../basics/FieldFindings';
import { findingsFor } from '../basics/aboutModel';
import {
  changeForm,
  changeParent,
  createFolders,
  createForm,
  createParent,
  createPreview,
  createVersionChoices,
  toggleVersion,
} from './createStore';

/** Step 1: who and what the mod is, and where it is created. The backend's findings show as you type. */
export function IdentityStep() {
  const form = createForm.value;
  const parent = createParent.value;
  const diagnostics = createPreview.value?.diagnostics ?? [];
  const idFindings = findingsFor(diagnostics, 'packageId');
  const idError = idFindings.find((d) => d.severity === 'error');
  const rest = diagnostics.filter(
    (d) => !['name', 'packageId'].some((f) => findingsFor([d], f).length > 0),
  );

  const browse = async (): Promise<void> => {
    const picked = await pickFolder(parent ? { start: parent } : {});
    if (picked) changeParent(picked);
  };

  return (
    <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
      <div class="flex flex-col gap-3 md:col-span-2">
        <FormField label={t('project.new.parent')} help={t('project.new.parent.hint')} required>
          <TextField
            value={parent}
            onValueChange={changeParent}
            suffix={
              <Button size="sm" variant="ghost" icon="folder" onClick={() => void browse()}>
                {t('project.new.browse')}
              </Button>
            }
          />
        </FormField>
        {createFolders.value.length > 0 ? (
          <div class="flex flex-wrap items-center gap-2">
            <span class="text-small text-muted">{t('project.create.quick')}</span>
            {createFolders.value.map((source) => (
              <Button
                key={source.id}
                size="sm"
                variant="secondary"
                aria-label={t('project.create.quick.use', { name: source.label })}
                onClick={() => changeParent(source.path)}
              >
                {source.label}
              </Button>
            ))}
          </div>
        ) : null}
      </div>
      <div class="flex flex-col gap-1">
        <FormField
          label={t('project.new.name')}
          required
          {...(findingsFor(diagnostics, 'name')[0]
            ? { error: findingsFor(diagnostics, 'name')[0]?.message ?? '' }
            : {})}
        >
          <TextField value={form.name} onValueChange={(name) => changeForm({ name })} />
        </FormField>
      </div>
      <FormField label={t('project.new.author')}>
        <TextField value={form.author} onValueChange={(author) => changeForm({ author })} />
      </FormField>
      <div class="flex flex-col gap-1 md:col-span-2">
        <FormField
          label={t('project.new.packageId')}
          help={t('project.new.packageId.hint')}
          required
          {...(idError ? { error: idError.message } : {})}
        >
          <TextField
            value={form.packageId}
            onValueChange={(packageId) => changeForm({ packageId })}
          />
        </FormField>
        <FieldFindings items={idFindings.filter((d) => d !== idError)} />
      </div>
      <fieldset class="m-0 min-w-0 border-0 p-0 md:col-span-2">
        <legend class="mb-1 p-0 text-small font-semibold text-muted">
          {t('project.new.versions')}
        </legend>
        <div class="flex flex-wrap gap-x-6 gap-y-1">
          {createVersionChoices.value.map((version) => (
            <Checkbox
              key={version}
              checked={form.versions.includes(version)}
              onCheckedChange={(on) => toggleVersion(version, on)}
            >
              {version}
            </Checkbox>
          ))}
        </div>
        <p class="m-0 mt-1 text-small text-faint">{t('project.new.versions.hint')}</p>
      </fieldset>
      <FormField label={t('project.new.folder')} help={t('project.create.folder.help')}>
        <TextField
          value={form.folderName}
          onValueChange={(folderName) => changeForm({ folderName })}
        />
      </FormField>
      <div class="md:col-span-2">
        <FormField label={t('project.new.description')} help={t('project.new.description.hint')}>
          <TextField
            multiline
            rows={3}
            value={form.description}
            onValueChange={(description) => changeForm({ description })}
          />
        </FormField>
      </div>
      <div class="md:col-span-2">
        <FieldFindings items={rest} />
      </div>
    </div>
  );
}
