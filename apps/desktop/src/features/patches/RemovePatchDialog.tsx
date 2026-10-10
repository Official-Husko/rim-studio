import { Banner, Button, Dialog, Spinner } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import type { ProjectRef } from '~/shared/project';
import { RemovalReport } from './RemovalReport';
import { RemovalReview } from './RemovalReview';
import { closeRemoval, confirmRemoval, isRemovable, openRemoval, removal } from './removeStore';

export interface RemovePatchDialogProps {
  project: ProjectRef;
}

/** A spinner with its words beside it; the spinner carries the accessible name, the text is for the eye. */
function Working({ label }: { label: string }) {
  return (
    <div class="flex items-center gap-2 text-muted">
      <Spinner label={label} />
      <span aria-hidden="true">{label}</span>
    </div>
  );
}

/**
 * The removal of the generated patch of one weapon in one dialog: planning, the review with the
 * confirmation, the run and the report. "Replace" is the same removal with the next step spelled out.
 */
export function RemovePatchDialog({ project }: RemovePatchDialogProps) {
  const state = removal.value;
  if (state.phase === 'closed') return null;
  const { target } = state;
  const replace = target.mode === 'replace';
  const busy = state.phase === 'planning' || state.phase === 'running';
  return (
    <Dialog
      open
      size="lg"
      title={t(replace ? 'patches.replace.title' : 'patches.remove.title', { name: target.name })}
      closeLabel={t('patches.close')}
      onClose={() => {
        if (!busy) closeRemoval();
      }}
      footer={
        state.phase === 'review' ? (
          <>
            <Button variant="secondary" onClick={closeRemoval}>
              {t('patches.cancel')}
            </Button>
            <Button
              variant={replace ? 'primary' : 'danger'}
              disabled={!isRemovable(state.plan)}
              onClick={() => void confirmRemoval(project)}
            >
              {t(replace ? 'patches.remove.confirm-replace' : 'patches.remove.confirm')}
            </Button>
          </>
        ) : state.phase === 'done' ? (
          <Button variant="primary" onClick={closeRemoval}>
            {t('patches.close')}
          </Button>
        ) : state.phase === 'failed' ? (
          <>
            <Button variant="secondary" onClick={closeRemoval}>
              {t('patches.close')}
            </Button>
            <Button variant="primary" onClick={() => void openRemoval(project, state.target)}>
              {t('patches.retry')}
            </Button>
          </>
        ) : null
      }
    >
      {state.phase === 'planning' ? <Working label={t('patches.remove.planning')} /> : null}
      {state.phase === 'review' ? <RemovalReview target={target} plan={state.plan} /> : null}
      {state.phase === 'running' ? <Working label={t('patches.remove.running')} /> : null}
      {state.phase === 'done' ? (
        <RemovalReport target={target} plan={state.plan} report={state.report} />
      ) : null}
      {state.phase === 'failed' ? (
        <Banner tone="error" title={t('patches.remove.failed')}>
          <code class="font-mono text-mono">{state.error.code}</code> {state.error.message}
        </Banner>
      ) : null}
    </Dialog>
  );
}
