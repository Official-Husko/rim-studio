import { useState } from 'preact/hooks';
import { Badge, Banner, Button, Checkbox, Dialog, ProgressBar } from 'rimstudio-ui';
import { formatBytes } from '~/shared/format';
import { t, tn } from '~/shared/i18n';
import { jobs } from '~/shared/ipc';
import type { OutputStore } from '../../output-store';
import { ApplyResult } from './ApplyResult';
import { actionText, actionTone } from './labels';

export interface ApplyDialogProps {
  store: OutputStore;
  /** The folder of the project, shown so the user sees where the files go. */
  projectPath: string | undefined;
}

function WriteProgress() {
  const job = jobs.value.find((j) => j.command === 'designer_apply_plan' && j.state === 'running');
  const known = job?.total !== null && job?.total !== undefined && job.total > 0;
  return (
    <ProgressBar
      label={t('designer.output.apply.progress')}
      {...(known && job ? { value: job.done / (job.total ?? 1) } : {})}
      caption={
        known && job
          ? t('designer.output.apply.progressCaption', { done: job.done, total: job.total ?? 0 })
          : t('designer.output.apply.writing')
      }
    />
  );
}

/**
 * The confirmation before anything is written: it lists exactly the files that will change and says
 * where backups go. While it writes it shows the progress of the job, then the result.
 */
export function ApplyDialog({ store, projectPath }: ApplyDialogProps) {
  const [backup, setBackup] = useState(true);
  const [dry, setDry] = useState(true);
  const state = store.apply.value;
  const plan = store.plan.value;
  const writing = plan?.files.filter((f) => f.action !== 'unchanged') ?? [];
  const same = (plan?.files.length ?? 0) - writing.length;
  const hasPatch = plan?.files.some((f) => f.kind === 'ce-patch') ?? false;
  const open = state.phase !== 'idle';
  const confirm = state.phase === 'confirm';
  return (
    <Dialog
      open={open}
      size="lg"
      title={t('designer.output.apply.title')}
      closeLabel={t('designer.dialog.close')}
      onClose={store.closeApply}
      footer={
        confirm ? (
          <>
            <Button onClick={store.closeApply}>{t('designer.dialog.cancel')}</Button>
            <Button
              variant="primary"
              onClick={() => void store.confirmApply({ backup, dryApply: hasPatch && dry })}
            >
              {tn('designer.output.apply.confirm', writing.length)}
            </Button>
          </>
        ) : state.phase === 'running' ? null : (
          <Button variant="primary" onClick={store.closeApply}>
            {t('designer.output.apply.done')}
          </Button>
        )
      }
    >
      {confirm ? (
        <div class="flex flex-col gap-3">
          <p class="text-body">{t('designer.output.apply.intro', { folder: projectPath ?? '' })}</p>
          <ul aria-label={t('designer.output.apply.listLabel')} class="flex flex-col gap-2">
            {writing.map((file) => (
              <li key={file.path} class="flex flex-col gap-1 border border-line p-2">
                <span class="break-all font-mono text-mono text-fg">{file.path}</span>
                <span class="flex items-center gap-2">
                  <Badge tone={actionTone(file.action)}>{actionText(file.action)}</Badge>
                  <span class="font-mono text-mono-small text-muted">
                    {formatBytes(file.bytes)}
                  </span>
                </span>
              </li>
            ))}
          </ul>
          {same > 0 ? (
            <p class="text-small text-muted">{tn('designer.output.apply.unchanged', same)}</p>
          ) : null}
          <Checkbox checked={backup} onCheckedChange={setBackup}>
            {t('designer.output.apply.backup')}
          </Checkbox>
          <p class="text-small text-muted">{t('designer.output.apply.backupHelp')}</p>
          {hasPatch ? (
            <Checkbox checked={dry} onCheckedChange={setDry}>
              {t('designer.output.apply.dry')}
            </Checkbox>
          ) : null}
        </div>
      ) : null}
      {state.phase === 'running' ? <WriteProgress /> : null}
      {state.phase === 'done' && state.report ? <ApplyResult report={state.report} /> : null}
      {state.phase === 'failed' && state.error ? (
        <Banner tone="error" title={state.error.code}>
          {state.error.message}
        </Banner>
      ) : null}
    </Dialog>
  );
}
