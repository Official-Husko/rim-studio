import { Banner, Button, Dialog, ProgressBar, Spinner } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import { jobs } from '~/shared/ipc';
import { FixConfirm } from './FixConfirm';
import { FixPlanView } from './FixPlanView';
import { FixResultView } from './FixResultView';
import {
  backToReview,
  closeFix,
  fixFlow,
  goConfirm,
  reviewAgain,
  runApply,
  selectAllSafe,
  selectNone,
  toggleItem,
  toggleRename,
} from './fixStore';
import { view } from './store';

const STALE_CODE = 'designer.plan-stale';

function ApplyProgress() {
  const job = jobs.value.find(
    (j) => j.command === 'project_layout_fix_apply' && j.state === 'running',
  );
  const known = job?.total !== null && job?.total !== undefined && job.total > 0;
  return (
    <div class="flex flex-col gap-2">
      <p class="m-0 text-body">{t('project.fix.running')}</p>
      <ProgressBar
        label={t('project.fix.progress')}
        {...(known && job ? { value: job.done / (job.total ?? 1) } : {})}
        {...(known && job
          ? { caption: t('project.fix.progressCaption', { done: job.done, total: job.total ?? 0 }) }
          : {})}
      />
    </div>
  );
}

/** The whole fix flow in one dialog: planning, review, confirmation, the run and the result. */
export function FixDialog() {
  const flow = fixFlow.value;
  if (flow.phase === 'closed') return null;
  const plan = flow.plan;
  const picked = plan ? plan.items.filter((i) => flow.selected.includes(i.id)) : [];
  const busy = flow.phase === 'loading' || flow.phase === 'running';
  const stale = flow.error?.code === STALE_CODE;
  let footer;
  if (flow.phase === 'review') {
    footer = (
      <>
        <Button variant="secondary" onClick={closeFix}>
          {t('project.fix.cancel')}
        </Button>
        <Button variant="primary" disabled={picked.length === 0} onClick={goConfirm}>
          {tn('project.fix.continue', picked.length)}
        </Button>
      </>
    );
  } else if (flow.phase === 'confirm') {
    footer = (
      <>
        <Button variant="secondary" onClick={backToReview}>
          {t('project.fix.back')}
        </Button>
        <Button variant="primary" onClick={() => void runApply()}>
          {t('project.fix.apply')}
        </Button>
      </>
    );
  } else if (flow.phase === 'result' || flow.phase === 'error') {
    footer = (
      <>
        {flow.phase === 'error' ? (
          <Button variant="secondary" onClick={() => void reviewAgain()}>
            {t('project.fix.reviewAgain')}
          </Button>
        ) : null}
        <Button variant="primary" onClick={closeFix}>
          {t('project.fix.close')}
        </Button>
      </>
    );
  }
  return (
    <Dialog
      open
      size="lg"
      title={t('project.fix.title')}
      closeLabel={t('project.dialog.close')}
      onClose={() => {
        if (!busy) closeFix();
      }}
      footer={footer}
    >
      {flow.phase === 'loading' ? <Spinner label={t('project.fix.loading')} /> : null}
      {flow.phase === 'review' && plan ? (
        <FixPlanView
          plan={plan}
          selected={flow.selected}
          rename={flow.rename}
          onToggle={toggleItem}
          onRename={toggleRename}
          onTickSafe={selectAllSafe}
          onTickNone={selectNone}
        />
      ) : null}
      {flow.phase === 'confirm' ? (
        <FixConfirm items={picked} rename={flow.rename} folder={view.value?.summary.path ?? ''} />
      ) : null}
      {flow.phase === 'running' ? <ApplyProgress /> : null}
      {flow.phase === 'result' && flow.result ? <FixResultView result={flow.result} /> : null}
      {flow.phase === 'error' && flow.error ? (
        <Banner
          tone={stale ? 'warning' : 'error'}
          title={
            stale
              ? t('project.fix.stale.title')
              : plan
                ? t('project.fix.error.title')
                : t('project.fix.plan.error')
          }
        >
          {stale ? t('project.fix.stale.hint') : flow.error.message}
        </Banner>
      ) : null}
    </Dialog>
  );
}
