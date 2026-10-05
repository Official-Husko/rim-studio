import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { createDraftsStore } from '../drafts-store';
import { cloneEntry, installTransport } from '../testSupport';
import { DraftList } from './DraftList';

function setup(open?: string) {
  installTransport();
  const store = createDraftsStore({ projectId: () => 'p-1' });
  store.upsert(cloneEntry());
  const handlers = { onOpen: vi.fn(), onNew: vi.fn(), onDelete: vi.fn() };
  renderWithProviders(<DraftList store={store} openId={open} openState="saved" {...handlers} />);
  return handlers;
}

describe('DraftList', () => {
  it('lists the drafts with kind and status', () => {
    setup();
    expect(screen.getByText('fixture rifle')).toBeTruthy();
    expect(screen.getByText('Ranged')).toBeTruthy();
    expect(screen.getByText('Stored')).toBeTruthy();
  });

  it('shows the save state of the open draft', () => {
    setup('d-clone');
    expect(screen.getByText('Saved')).toBeTruthy();
  });

  it('opens a draft and starts new ones', () => {
    const h = setup();
    fireEvent.click(screen.getByRole('button', { name: 'fixture rifle' }));
    expect(h.onOpen).toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'New melee' }));
    expect(h.onNew).toHaveBeenCalledWith('melee');
  });

  it('asks before deleting a draft', () => {
    const h = setup();
    fireEvent.click(screen.getByRole('button', { name: 'Delete draft fixture rifle' }));
    expect(h.onDelete).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Delete draft' }));
    expect(h.onDelete).toHaveBeenCalledTimes(1);
  });

  it('explains the three ways to start when empty', () => {
    installTransport();
    const store = createDraftsStore({ projectId: () => 'p-1' });
    renderWithProviders(
      <DraftList
        store={store}
        openId={undefined}
        openState="idle"
        onOpen={() => undefined}
        onNew={() => undefined}
        onDelete={() => undefined}
      />,
    );
    expect(screen.getByText('No drafts yet')).toBeTruthy();
  });
});
