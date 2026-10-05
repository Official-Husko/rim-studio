import { fireEvent, render, screen } from '@testing-library/preact';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { applyEvent, resetJobs } from '~/shared/ipc/jobs';
import { TaskCentre } from './TaskCentre';

beforeEach(() => resetJobs());

describe('TaskCentre', () => {
  it('shows an empty state without jobs', () => {
    render(<TaskCentre onClose={() => {}} />);
    expect(screen.getByText('No tasks have run in this session.')).toBeTruthy();
  });

  it('lists a running job with progress and a finished one with its state', () => {
    applyEvent({
      type: 'job-progress',
      jobId: 'j1',
      command: 'library_scan',
      message: 'Core',
      done: 3,
      total: 10,
    });
    applyEvent({ type: 'job-finished', jobId: 'j2', command: 'designer_apply_plan', ok: false });
    render(<TaskCentre onClose={() => {}} />);
    expect(
      screen.getByRole('progressbar', { name: 'library_scan' }).getAttribute('aria-valuenow'),
    ).toBe('30');
    expect(screen.getByText('Running')).toBeTruthy();
    expect(screen.getByText('Failed')).toBeTruthy();
  });

  it('dismisses a finished job and clears finished jobs', () => {
    applyEvent({ type: 'job-finished', jobId: 'j2', command: 'designer_apply_plan', ok: true });
    render(<TaskCentre onClose={() => {}} />);
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss designer_apply_plan' }));
    expect(screen.queryByText('designer_apply_plan')).toBeNull();
  });

  it('closes from the header button', () => {
    const onClose = vi.fn();
    render(<TaskCentre onClose={onClose} />);
    fireEvent.click(screen.getByRole('button', { name: 'Close tasks' }));
    expect(onClose).toHaveBeenCalled();
  });
});
