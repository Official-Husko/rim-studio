import { beforeEach, describe, expect, it } from 'vitest';
import { createMockTransport } from 'rimstudio-testkit';
import {
  applyEvent,
  clearFinishedJobs,
  dismissJob,
  jobs,
  resetJobs,
  runningCount,
  setJobClock,
  startJobStream,
  trackCall,
} from './jobs';

let clock = 1000;
beforeEach(() => {
  resetJobs();
  clock = 1000;
  setJobClock(() => clock);
});

describe('job store', () => {
  it('creates a running job from a progress event and finishes it', () => {
    applyEvent({
      type: 'job-progress',
      jobId: 'j1',
      command: 'library_scan',
      message: 'Core',
      done: 3,
      total: 10,
    });
    expect(jobs.value[0]).toMatchObject({
      id: 'j1',
      state: 'running',
      done: 3,
      total: 10,
      message: 'Core',
    });
    expect(runningCount.value).toBe(1);
    clock = 2500;
    applyEvent({ type: 'job-finished', jobId: 'j1', command: 'library_scan', ok: true });
    expect(jobs.value[0]).toMatchObject({ state: 'done', endedAt: 2500, startedAt: 1000 });
    expect(runningCount.value).toBe(0);
  });

  it('records a failed job', () => {
    applyEvent({ type: 'job-finished', jobId: 'j2', command: 'designer_apply_plan', ok: false });
    expect(jobs.value[0]?.state).toBe('failed');
  });

  it('binds the local entry of a tracked call to the bridge job id', () => {
    const tracked = trackCall('library_scan');
    expect(jobs.value).toHaveLength(1);
    applyEvent({
      type: 'job-progress',
      jobId: 'j9',
      command: 'library_scan',
      message: '',
      done: 1,
      total: null,
    });
    expect(jobs.value).toHaveLength(1);
    expect(jobs.value[0]?.id).toBe('j9');
    tracked.finish(true);
    expect(jobs.value[0]?.state).toBe('done');
    expect(jobs.value).toHaveLength(1);
  });

  it('sorts newest first and clears finished jobs', () => {
    applyEvent({
      type: 'job-progress',
      jobId: 'a',
      command: 'x',
      message: '',
      done: 0,
      total: null,
    });
    clock = 2000;
    applyEvent({
      type: 'job-progress',
      jobId: 'b',
      command: 'x',
      message: '',
      done: 0,
      total: null,
    });
    expect(jobs.value.map((j) => j.id)).toEqual(['b', 'a']);
    applyEvent({ type: 'job-finished', jobId: 'a', command: 'x', ok: true });
    clearFinishedJobs();
    expect(jobs.value.map((j) => j.id)).toEqual(['b']);
    dismissJob('b');
    expect(jobs.value).toHaveLength(0);
  });

  it('is fed by a transport event stream until unsubscribed', () => {
    const transport = createMockTransport();
    const stop = startJobStream(transport);
    transport.emit({
      type: 'job-progress',
      jobId: 's1',
      command: 'c',
      message: '',
      done: 0,
      total: null,
    });
    expect(jobs.value).toHaveLength(1);
    stop();
    transport.emit({
      type: 'job-progress',
      jobId: 's2',
      command: 'c',
      message: '',
      done: 0,
      total: null,
    });
    expect(jobs.value).toHaveLength(1);
  });
});
