import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { LinkConfirmDialog } from './LinkConfirmDialog';
import { LINK_PROJECT_PATH, statusFixture } from './testSupport';

function show(
  patch = {},
  handlers: { onCancel?: () => void; onConfirm?: (...a: unknown[]) => void } = {},
) {
  const onCancel = handlers.onCancel ?? vi.fn();
  const onConfirm = handlers.onConfirm ?? vi.fn();
  renderWithProviders(
    <LinkConfirmDialog
      open
      status={statusFixture('link-status-not-linked', patch)}
      projectPath={LINK_PROJECT_PATH}
      onCancel={onCancel}
      onConfirm={onConfirm}
    />,
  );
  return { onCancel, onConfirm };
}

describe('LinkConfirmDialog', () => {
  it('says what is created and where', () => {
    show();
    expect(screen.getByRole('dialog', { name: 'Link into the game' })).toBeTruthy();
    expect(screen.getByText(statusFixture('link-status-not-linked').entryPath ?? '')).toBeTruthy();
    expect(screen.getByText(LINK_PROJECT_PATH)).toBeTruthy();
    expect(screen.getByText(/Nothing is copied and nothing in your project changes/)).toBeTruthy();
    expect(screen.getByText(/has to be restarted to see a new mod/)).toBeTruthy();
  });

  it('creates a symbolic link by default', () => {
    const { onConfirm } = show();
    fireEvent.click(screen.getByRole('button', { name: 'Create link' }));
    expect(onConfirm).toHaveBeenCalledWith('symlink', false);
  });

  it('warns that a copy does not follow edits and asks to create a copy', () => {
    const { onConfirm } = show();
    fireEvent.click(screen.getByRole('radio', { name: 'Copy' }));
    expect(screen.getByText(/A copy does not follow your edits/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Create copy' }));
    expect(onConfirm).toHaveBeenCalledWith('copy', false);
  });

  it('offers a junction only where the system can make one', () => {
    show({ support: { symlink: false, junction: true, needsPrivilege: true } });
    expect(screen.queryByRole('radio', { name: 'Link' })).toBeNull();
    expect(screen.getByRole('radio', { name: 'Junction' })).toBeTruthy();
  });

  it('needs a confirmation while the game is running', () => {
    const { onConfirm } = show({ gameRunning: 'running' });
    const create = screen.getByRole('button', { name: 'Create link' }) as HTMLButtonElement;
    expect(create.disabled).toBe(true);
    fireEvent.click(
      screen.getByRole('checkbox', { name: 'I will restart RimWorld to see the mod' }),
    );
    expect(create.disabled).toBe(false);
    fireEvent.click(create);
    expect(onConfirm).toHaveBeenCalledWith('symlink', true);
  });

  it('cancels', () => {
    const { onCancel, onConfirm } = show();
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onCancel).toHaveBeenCalled();
    expect(onConfirm).not.toHaveBeenCalled();
  });
});
