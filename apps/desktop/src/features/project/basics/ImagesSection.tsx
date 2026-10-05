import { useState } from 'preact/hooks';
import { Banner, Button, EmptyState, Panel } from 'rimstudio-ui';
import { formatBytes } from '~/shared/format';
import { t } from '~/shared/i18n';
import { pickFile } from '~/shared/platform';
import { FieldFindings } from './FieldFindings';
import { TextRow } from './TextRow';
import {
  aboutModel,
  findings,
  imageBusy,
  imageError,
  imageNotice,
  removePreviewImage,
  setPreviewImage,
} from './aboutStore';

const NOTICE = {
  added: 'project.basics.images.added',
  replaced: 'project.basics.images.replaced',
  removed: 'project.basics.images.removed',
} as const;

/** About/Preview.png with a thumbnail and its size, and the path of the mod icon. */
export function ImagesSection() {
  const about = aboutModel.value;
  const [confirming, setConfirming] = useState(false);
  if (!about) return null;
  const preview = about.preview;
  const notice = imageNotice.value;
  const locked = !about.editable || imageBusy.value;

  const choose = async (): Promise<void> => {
    const picked = await pickFile({
      filters: [{ name: t('project.basics.images.filter'), extensions: ['png'] }],
    });
    if (picked) await setPreviewImage(picked);
  };

  return (
    <Panel title={t('project.basics.images')} framed>
      <div class="flex flex-col gap-4 p-3">
        <div class="flex flex-wrap items-start gap-4">
          <div class="flex h-36 w-64 shrink-0 items-center justify-center border border-line bg-raised">
            {preview.exists && preview.dataUrl ? (
              <img
                src={preview.dataUrl}
                alt={t('project.basics.images.alt')}
                class="max-h-full max-w-full object-contain"
              />
            ) : (
              <EmptyState
                compact
                icon="image"
                title={
                  preview.exists
                    ? t('project.basics.images.nothumb')
                    : t('project.basics.images.none')
                }
              />
            )}
          </div>
          <div class="flex min-w-0 flex-1 flex-col gap-2">
            <p class="m-0 text-small text-muted">{t('project.basics.images.help')}</p>
            {preview.exists && preview.width && preview.height ? (
              <p class="m-0 text-small">
                {t('project.basics.images.size', {
                  width: preview.width,
                  height: preview.height,
                  bytes: formatBytes(preview.bytes ?? 0),
                })}
              </p>
            ) : null}
            <div class="flex flex-wrap gap-2">
              <Button
                icon="image"
                size="sm"
                loading={imageBusy.value}
                disabled={!about.editable}
                onClick={() => void choose()}
              >
                {preview.exists
                  ? t('project.basics.images.replace')
                  : t('project.basics.images.choose')}
              </Button>
              {preview.exists && !confirming ? (
                <Button
                  size="sm"
                  variant="ghost"
                  icon="trash"
                  disabled={locked}
                  onClick={() => setConfirming(true)}
                >
                  {t('project.basics.images.remove')}
                </Button>
              ) : null}
              {confirming ? (
                <>
                  <Button
                    size="sm"
                    variant="danger"
                    onClick={() => {
                      setConfirming(false);
                      void removePreviewImage();
                    }}
                  >
                    {t('project.basics.images.removeConfirm')}
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => setConfirming(false)}>
                    {t('project.basics.cancel')}
                  </Button>
                </>
              ) : null}
            </div>
            <p class="m-0 text-small text-faint">{t('project.basics.images.backup')}</p>
            {notice ? (
              <Banner tone="success">{t(NOTICE[notice as keyof typeof NOTICE])}</Banner>
            ) : null}
            {imageError.value ? (
              <Banner tone="error" title={t('project.basics.images.error')}>
                {imageError.value.message}
              </Banner>
            ) : null}
            <FieldFindings
              items={findings.value.filter((d) => d.code.startsWith('about.preview'))}
            />
          </div>
        </div>
        <TextRow
          field="modIconPath"
          label={t('project.basics.icon')}
          help={t('project.basics.icon.help')}
        />
      </div>
    </Panel>
  );
}
