import { computed, signal } from '@preact/signals';
import type { BridgeEvent, Transport } from './types';

export interface JobState {
  /** The bridge job id, or a local id until the first event binds it. */
  id: string;
  command: string;
  state: 'running' | 'done' | 'failed';
  message: string;
  done: number;
  total: number | null;
  startedAt: number;
  endedAt?: number;
}

let now = (): number => Date.now();
/** Replace the clock (tests). */
export function setJobClock(fn: () => number): void {
  now = fn;
}

const table = signal<ReadonlyMap<string, JobState>>(new Map());
/** Local pseudo job id to the bridge id it was bound to. */
const bound = new Map<string, string>();
let jobCommands = new Set<string>();
let localCounter = 0;

/** Every known job, newest first. */
export const jobs = computed<readonly JobState[]>(() =>
  [...table.value.values()].sort((a, b) => b.startedAt - a.startedAt),
);
/** Number of running jobs. */
export const runningCount = computed(() => jobs.value.filter((j) => j.state === 'running').length);

function put(job: JobState): void {
  const next = new Map(table.value);
  next.set(job.id, job);
  table.value = next;
}

/** Tell the store which commands are jobs (from GET /dev/commands). */
export function setJobCommands(names: Iterable<string>): void {
  jobCommands = new Set(names);
}

export function isJobCommand(name: string): boolean {
  return jobCommands.has(name);
}

function findLocal(command: string): JobState | undefined {
  for (const job of table.value.values()) {
    if (
      job.id.startsWith('local-') &&
      job.command === command &&
      job.state === 'running' &&
      !bound.has(job.id)
    )
      return job;
  }
  return undefined;
}

function resolveId(jobId: string, command: string): string {
  const local = findLocal(command);
  if (!table.value.has(jobId) && local) {
    // the first event of a call we started: move the local entry to the bridge id
    const next = new Map(table.value);
    next.delete(local.id);
    next.set(jobId, { ...local, id: jobId });
    table.value = next;
    bound.set(local.id, jobId);
  }
  return jobId;
}

/** Apply one bridge event to the store. */
export function applyEvent(event: BridgeEvent): void {
  const id = resolveId(event.jobId, event.command);
  const existing = table.value.get(id);
  if (event.type === 'job-progress') {
    put({
      id,
      command: event.command,
      state: 'running',
      message: event.message,
      done: event.done,
      total: event.total,
      startedAt: existing?.startedAt ?? now(),
    });
  } else {
    put({
      id,
      command: event.command,
      state: event.ok ? 'done' : 'failed',
      message: existing?.message ?? '',
      done: existing?.done ?? 0,
      total: existing?.total ?? null,
      startedAt: existing?.startedAt ?? now(),
      endedAt: now(),
    });
  }
}

/** Note the start of a job command call so the task centre shows it before any progress arrives. */
export function trackCall(command: string): { finish: (ok: boolean) => void } {
  localCounter += 1;
  const localId = `local-${localCounter}`;
  put({
    id: localId,
    command,
    state: 'running',
    message: '',
    done: 0,
    total: null,
    startedAt: now(),
  });
  return {
    finish(ok) {
      const id = bound.get(localId) ?? localId;
      bound.delete(localId);
      const job = table.value.get(id);
      if (job && job.state === 'running')
        put({ ...job, state: ok ? 'done' : 'failed', endedAt: now() });
    },
  };
}

/** Feed the store from a transport's event stream; returns the unsubscribe function. */
export function startJobStream(transport: Transport): () => void {
  return transport.events(applyEvent);
}

/** Remove one finished job from the list. */
export function dismissJob(id: string): void {
  const next = new Map(table.value);
  next.delete(id);
  table.value = next;
}

/** Remove every finished job. */
export function clearFinishedJobs(): void {
  const next = new Map([...table.value].filter(([, job]) => job.state === 'running'));
  table.value = next;
}

/** Forget everything (tests). */
export function resetJobs(): void {
  table.value = new Map();
  bound.clear();
  jobCommands = new Set();
  localCounter = 0;
}
