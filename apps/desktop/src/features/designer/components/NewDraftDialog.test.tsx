import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { NewDraftDialog } from './NewDraftDialog';

const base = {
  open: true,
  title: 'Clone gladius',
  defName: 'Gladius_Copy',
  label: 'gladius copy',
  busy: false,
  onClose: () => undefined,
};

describe('NewDraftDialog', () => {
  it('submits the trimmed def name and label', () => {
    const onSubmit = vi.fn();
    renderWithProviders(<NewDraftDialog {...base} onSubmit={onSubmit} />);
    fireEvent.input(screen.getByLabelText(/Def name/), { target: { value: ' TM_Gladius ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create' }));
    expect(onSubmit).toHaveBeenCalledWith('TM_Gladius', 'gladius copy', true);
  });

  it('cannot submit an empty name', () => {
    renderWithProviders(<NewDraftDialog {...base} defName="" onSubmit={() => undefined} />);
    expect((screen.getByRole('button', { name: 'Create' }) as HTMLButtonElement).disabled).toBe(
      true,
    );
  });

  it('shows the error of the backend, such as a name already in use', () => {
    renderWithProviders(
      <NewDraftDialog
        {...base}
        error={{
          code: 'designer.invalid-draft',
          message: 'Gun_Revolver is already defined',
          errorId: 'e-1',
        }}
        onSubmit={() => undefined}
      />,
    );
    expect(screen.getByText('Gun_Revolver is already defined')).toBeTruthy();
  });

  it('offers the own projectile choice for a gun and passes it on', () => {
    const onSubmit = vi.fn();
    renderWithProviders(<NewDraftDialog {...base} offerOwnProjectile onSubmit={onSubmit} />);
    const choice = screen.getByRole('switch', { name: 'Give the clone its own projectile' });
    expect(choice.getAttribute('aria-checked')).toBe('true');
    fireEvent.click(choice);
    fireEvent.click(screen.getByRole('button', { name: 'Create' }));
    expect(onSubmit).toHaveBeenCalledWith('Gladius_Copy', 'gladius copy', false);
  });

  it('does not offer the choice for a melee weapon', () => {
    renderWithProviders(<NewDraftDialog {...base} onSubmit={() => undefined} />);
    expect(screen.queryByRole('switch')).toBeNull();
  });
});
