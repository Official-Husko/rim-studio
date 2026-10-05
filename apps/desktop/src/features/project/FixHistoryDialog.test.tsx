import { fireEvent, screen, waitFor } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import { beforeEach, describe, expect, it } from 'vitest';
import { FixHistoryDialog } from './FixHistoryDialog';
import { historyFlow, openHistory, resetFixFlow } from './fixStore';
import { installWithProject } from './testSupport';
import { loadProject } from './store';
import { gewehrRef } from './testSupport';

async function open(extra = {}) {
  installWithProject(extra);
  resetFixFlow();
  await loadProject(gewehrRef());
  renderWithProviders(<FixHistoryDialog />);
  await openHistory();
}

beforeEach(() => resetFixFlow());

describe('FixHistoryDialog', () => {
  it('lists the applies with the reason an undo is not possible', async () => {
    await open();
    const list = await screen.findByRole('list', { name: 'Applied fixes' });
    expect(list.querySelectorAll('li').length).toBe(2);
    expect(screen.getByText('Cannot be undone')).toBeTruthy();
    expect(screen.getByText(/was changed after the fix/)).toBeTruthy();
    expect(screen.getByText('Undone')).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Undo' })).toBeNull();
  });

  it('offers Undo where it is still possible and runs it', async () => {
    const journal = loadFixture<{ journals: unknown[] }>('layout-fix-history-gewehr')
      .journals[0] as object;
    await open({
      project_layout_fix_history: () => ({
        projectId: 'p-c36c596a',
        journals: [{ ...journal, undoPossible: true, undoBlocker: undefined }],
      }),
    });
    fireEvent.click(await screen.findByRole('button', { name: 'Undo' }));
    expect(await screen.findByText(/moved back/)).toBeTruthy();
  });

  it('says when nothing was applied and closes', async () => {
    await open({ project_layout_fix_history: () => loadFixture('layout-fix-history-empty') });
    expect(await screen.findByText('No fixes applied yet')).toBeTruthy();
    fireEvent.click(screen.getAllByRole('button', { name: 'Close' })[0] as HTMLElement);
    await waitFor(() => expect(historyFlow.value.open).toBe(false));
  });

  it('shows an error when the history cannot be read', async () => {
    await open({
      project_layout_fix_history: () => {
        throw { code: 'project.fix-journal-damaged', message: 'journal damaged', errorId: 'e' };
      },
    });
    expect(await screen.findByText('journal damaged')).toBeTruthy();
  });
});
