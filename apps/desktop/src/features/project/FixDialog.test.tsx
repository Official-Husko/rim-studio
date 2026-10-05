import { fireEvent, screen, waitFor, within } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { beforeEach, describe, expect, it } from 'vitest';
import { applyEvent } from '~/shared/ipc/jobs';
import { FixDialog } from './FixDialog';
import { fixFlow, openFix, resetFixFlow } from './fixStore';
import { gewehrRef, installWithProject } from './testSupport';
import { loadProject } from './store';

async function start(extra = {}) {
  installWithProject(extra);
  resetFixFlow();
  await loadProject(gewehrRef());
  renderWithProviders(<FixDialog />);
  await openFix();
}

describe('FixDialog', () => {
  beforeEach(() => resetFixFlow());

  it('walks from the review through the confirmation to the result and the undo', async () => {
    await start();
    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).getByText('Fix the layout')).toBeTruthy();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Continue with 4 changes' }));
    expect(within(dialog).getByText(/undo journal to its data folder/)).toBeTruthy();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Apply' }));
    expect(await within(dialog).findByText('4 changes carried out')).toBeTruthy();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Undo this fix' }));
    expect(await within(dialog).findByText(/moved back/)).toBeTruthy();
    fireEvent.click(within(dialog).getAllByRole('button', { name: 'Close' }).pop() as HTMLElement);
    await waitFor(() => expect(fixFlow.value.phase).toBe('closed'));
  });

  it('shows the progress of the job while it runs', async () => {
    let release: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    await start({
      project_layout_fix_apply: async () => {
        await gate;
        throw { code: 'ipc.transport', message: 'stopped', errorId: 'e' };
      },
    });
    const dialog = await screen.findByRole('dialog');
    fireEvent.click(within(dialog).getByRole('button', { name: 'Continue with 4 changes' }));
    fireEvent.click(within(dialog).getByRole('button', { name: 'Apply' }));
    expect(await within(dialog).findByRole('progressbar', { name: 'Fix progress' })).toBeTruthy();
    applyEvent({
      type: 'job-progress',
      jobId: 'j-1',
      command: 'project_layout_fix_apply',
      message: 'moving',
      done: 2,
      total: 4,
    });
    expect(await within(dialog).findByText('2 of 4')).toBeTruthy();
    release();
    expect(await within(dialog).findByText('stopped')).toBeTruthy();
  });

  it('explains a stale plan and offers to review again', async () => {
    await start({
      project_layout_fix_apply: () => {
        throw { code: 'designer.plan-stale', message: 'the plan changed', errorId: 'e-1' };
      },
    });
    const dialog = await screen.findByRole('dialog');
    fireEvent.click(within(dialog).getByRole('button', { name: 'Continue with 4 changes' }));
    fireEvent.click(within(dialog).getByRole('button', { name: 'Apply' }));
    expect(
      await within(dialog).findByText('The project changed since the plan was made'),
    ).toBeTruthy();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Review again' }));
    expect(
      await within(dialog).findByText('Continue with 4 changes', { exact: false }),
    ).toBeTruthy();
  });

  it('renders nothing while closed', () => {
    installWithProject();
    resetFixFlow();
    const { container } = renderWithProviders(<FixDialog />);
    expect(container.textContent).toBe('');
  });
});
