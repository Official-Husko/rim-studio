import { useState } from 'preact/hooks';
import { Banner, Button } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import { ReviewDialog } from './ReviewDialog';
import {
  aboutSaving,
  aboutStale,
  discardAbout,
  lastSave,
  loadAbout,
  pendingChanges,
  previewError,
  previewing,
  saveAbout,
  saveError,
} from './aboutStore';

export interface ChangesBarProps {
  projectId: string;
}

/** The bar at the bottom of the Basics tab: how many edits are unsaved, review, save and discard. */
export function ChangesBar({ projectId }: ChangesBarProps) {
  const [reviewing, setReviewing] = useState(false);
  const count = pendingChanges.value.length;
  const saved = lastSave.value;
  const error = saveError.value ?? previewError.value;
  return (
    <div class="sticky bottom-0 z-10 flex flex-col gap-2 border-t border-line-strong bg-raised p-3">
      {aboutStale.value ? (
        <Banner
          tone="warning"
          title={t('project.basics.stale.title')}
          action={
            <Button
              size="sm"
              variant="secondary"
              icon="refresh"
              onClick={() => void loadAbout(projectId, true)}
            >
              {t('project.basics.stale.reload')}
            </Button>
          }
        >
          {t('project.basics.stale.body')}
        </Banner>
      ) : error ? (
        <Banner tone="error">{error.message}</Banner>
      ) : null}
      <div
        class="flex flex-wrap items-center gap-3"
        role="region"
        aria-label={t('project.basics.bar.label')}
      >
        <div class="min-w-0 flex-1">
          <p class="m-0 font-semibold" aria-live="polite">
            {count > 0 ? tn('project.basics.bar.unsaved', count) : t('project.basics.bar.clean')}
          </p>
          <p class="m-0 text-small text-muted">
            {saved && count === 0 && saved.backup
              ? t('project.basics.bar.saved', { path: saved.backup })
              : t('project.basics.bar.backup')}
          </p>
        </div>
        <Button variant="ghost" disabled={count === 0 || aboutSaving.value} onClick={discardAbout}>
          {t('project.basics.discard')}
        </Button>
        <Button
          variant="secondary"
          disabled={count === 0}
          icon="xml"
          onClick={() => setReviewing(true)}
        >
          {t('project.basics.review')}
        </Button>
        <Button
          disabled={count === 0 || previewing.value || Boolean(previewError.value)}
          loading={aboutSaving.value}
          onClick={() => void saveAbout()}
        >
          {t('project.basics.save')}
        </Button>
      </div>
      <ReviewDialog open={reviewing} onClose={() => setReviewing(false)} />
    </div>
  );
}
