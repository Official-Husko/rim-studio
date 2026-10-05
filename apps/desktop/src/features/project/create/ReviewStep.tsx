import { Banner, KeyValueList } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import { FieldFindings } from '../basics/FieldFindings';
import { createForm, createPreview, createTarget } from './createStore';

/** Step 3: exactly what will be written, and what the backend found. */
export function ReviewStep() {
  const form = createForm.value;
  const preview = createPreview.value;
  const entries = preview?.entries ?? [];
  const folders = entries.filter((e) => e.kind === 'folder').length;
  const files = entries.length - folders;
  return (
    <div class="flex flex-col gap-4">
      <KeyValueList
        label={t('project.create.review.summary')}
        items={[
          { key: t('project.new.name'), value: form.name },
          { key: t('project.new.packageId'), value: form.packageId, mono: true },
          { key: t('project.new.author'), value: form.author || t('project.info.none') },
          { key: t('project.new.versions'), value: form.versions.join(', ') },
          { key: t('project.create.review.folder'), value: createTarget.value, mono: true },
        ]}
      />
      {preview && !preview.valid ? (
        <Banner tone="error">{t('project.create.review.invalid')}</Banner>
      ) : null}
      {preview?.targetExists ? (
        <Banner tone="info">{t('project.create.review.exists')}</Banner>
      ) : null}
      {preview && preview.conflicts.length > 0 ? (
        <Banner tone="error" title={t('project.create.review.conflicts')}>
          {preview.conflicts.join(', ')}
        </Banner>
      ) : null}
      <FieldFindings items={preview?.diagnostics ?? []} />
      <div class="flex flex-col gap-1">
        <h3 class="m-0 font-display text-small tracking-display text-muted uppercase">
          {t('project.new.preview.label')}
        </h3>
        <p class="m-0 text-small text-muted">
          {t('project.create.review.counts', {
            folders: tn('project.create.folders', folders),
            files: tn('project.create.files', files),
          })}
        </p>
        <ul
          class="m-0 max-h-72 list-none overflow-auto border border-line-subtle bg-surface p-2 font-mono text-mono-small"
          aria-label={t('project.new.preview.label')}
          // the list scrolls, so it must be reachable with the keyboard
          // oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex
          tabIndex={0}
        >
          {entries.map((entry) => (
            <li key={entry.path}>{entry.kind === 'folder' ? `${entry.path}/` : entry.path}</li>
          ))}
        </ul>
      </div>
    </div>
  );
}
