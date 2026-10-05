import { useEffect, useState } from 'preact/hooks';
import { Button, EmptyState, IconButton, ProgressBar } from 'rimstudio-ui';
import { clearFinishedJobs, dismissJob, jobs, type JobState } from '~/shared/ipc';
import { t, type MessageKey } from '~/shared/i18n';
import { formatDuration, formatNumber } from '~/shared/format';

const STATE_TEXT: Record<JobState['state'], MessageKey> = {
  running: 'tasks.state.running',
  done: 'tasks.state.done',
  failed: 'tasks.state.failed',
};

function caption(job: JobState): string | undefined {
  if (job.total !== null)
    return `${formatNumber(job.done, 0)} / ${formatNumber(job.total, 0)}${job.message ? `  ${job.message}` : ''}`;
  return job.message || undefined;
}

function Row({ job, now }: { job: JobState; now: number }) {
  const elapsed = (job.endedAt ?? now) - job.startedAt;
  const fraction = job.state === 'done' ? 1 : job.total ? job.done / job.total : undefined;
  return (
    <li class="flex flex-col gap-1 border-b border-line-subtle px-3 py-2">
      <div class="flex items-center justify-between gap-2">
        <span class="font-mono text-mono">{job.command}</span>
        <span class="flex items-center gap-2 text-small text-muted">
          {t(STATE_TEXT[job.state])}
          <span class="font-mono text-mono-small text-faint">{formatDuration(elapsed)}</span>
          {job.state !== 'running' ? (
            <IconButton
              icon="close"
              label={t('tasks.dismiss', { command: job.command })}
              onClick={() => dismissJob(job.id)}
              noTooltip
            />
          ) : null}
        </span>
      </div>
      {job.state === 'running' ? (
        <ProgressBar label={job.command} value={fraction} caption={caption(job)} />
      ) : null}
      {job.state === 'failed' ? <ProgressBar label={job.command} value={1} tone="danger" /> : null}
    </li>
  );
}

export interface TaskCentreProps {
  onClose: () => void;
}

/** The task centre drawer: every job of the session with its phase, progress and elapsed time. */
export function TaskCentre({ onClose }: TaskCentreProps) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 500);
    return () => clearInterval(timer);
  }, []);
  const list = jobs.value;
  return (
    <aside
      aria-label={t('tasks.title')}
      class="flex h-full w-drawer flex-col border-l border-line-strong bg-raised"
    >
      <header class="flex h-8 items-center justify-between border-b border-line px-3">
        <h2 class="font-display text-label font-semibold tracking-label text-muted uppercase">
          {t('tasks.title')}
        </h2>
        <div class="flex items-center gap-1">
          <Button size="sm" variant="ghost" onClick={clearFinishedJobs}>
            {t('tasks.clear')}
          </Button>
          <IconButton icon="close" label={t('tasks.close')} onClick={onClose} noTooltip />
        </div>
      </header>
      {list.length === 0 ? (
        <EmptyState compact title={t('tasks.title')} description={t('tasks.empty')} />
      ) : (
        <ul class="min-h-0 flex-1 overflow-auto">
          {list.map((job) => (
            <Row key={job.id} job={job} now={now} />
          ))}
        </ul>
      )}
    </aside>
  );
}
