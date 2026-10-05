import { useState } from 'preact/hooks';
import { Button, Dialog, ProgressBar, Spinner } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import type { ProjectRef } from '~/shared/project';
import { ApplyReport } from './ApplyReport';
import { ApplyReview } from './ApplyReview';
import { applyState, closeReview, isReady, runApply } from './applyStore';

export interface ApplyDialogProps {
  project: ProjectRef;
}

/** The apply flow in one dialog: planning, the review with the confirmation, the run and the report. */
export function ApplyDialog({ project }: ApplyDialogProps) {
  const [backup, setBackup] = useState(true);
  const [dryApply, setDryApply] = useState(true);
  const state = applyState.value;
  if (state.phase === 'closed') return null;
  const ready = state.phase === 'review' ? state.items.filter((i) => isReady(i.entry)) : [];
  const running = state.phase === 'running' || state.phase === 'planning';
  return (
    <Dialog
      open
      size="lg"
      title={t('patches.apply.title')}
      closeLabel={t('patches.close')}
      onClose={() => {
        if (!running) closeReview();
      }}
      footer={
        state.phase === 'review' ? (
          <>
            <Button variant="secondary" onClick={closeReview}>
              {t('patches.cancel')}
            </Button>
            <Button
              variant="primary"
              disabled={ready.length === 0}
              onClick={() => void runApply(project, state.items, { backup, dryApply })}
            >
              {tn('patches.apply.confirm', ready.length)}
            </Button>
          </>
        ) : state.phase === 'done' ? (
          <Button variant="primary" onClick={closeReview}>
            {t('patches.close')}
          </Button>
        ) : null
      }
    >
      {state.phase === 'planning' ? <Spinner label={t('patches.apply.planning')} /> : null}
      {state.phase === 'review' ? (
        <ApplyReview
          items={state.items}
          backup={backup}
          dryApply={dryApply}
          onBackup={setBackup}
          onDryApply={setDryApply}
        />
      ) : null}
      {state.phase === 'running' ? (
        <ProgressBar
          label={t('patches.apply.running')}
          value={state.total === 0 ? 0 : state.done / state.total}
          caption={t('patches.apply.progress', {
            done: state.done + 1,
            total: state.total,
            name: state.current,
          })}
        />
      ) : null}
      {state.phase === 'done' ? <ApplyReport results={state.results} /> : null}
    </Dialog>
  );
}
