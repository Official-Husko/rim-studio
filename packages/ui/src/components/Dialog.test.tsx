import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { press } from 'rimstudio-testkit';
import { Dialog } from './Dialog';

function Sample(props: { open?: boolean; onClose?: () => void; closeOnBackdrop?: boolean }) {
  return (
    <>
      <button type="button">Opener</button>
      <Dialog
        open={props.open ?? true}
        title="Write files"
        onClose={props.onClose ?? (() => {})}
        closeOnBackdrop={props.closeOnBackdrop}
        footer={
          <>
            <button type="button">Cancel</button>
            <button type="button">Write</button>
          </>
        }
      >
        <p>Three files will change.</p>
        <input aria-label="Folder" />
      </Dialog>
    </>
  );
}

describe('Dialog', () => {
  it('renders nothing when closed', () => {
    render(<Sample open={false} />);
    expect(screen.queryByText('Three files will change.')).toBeNull();
  });

  it('is a dialog named by its title', () => {
    render(<Sample />);
    expect(screen.getByRole('dialog', { name: 'Write files', hidden: true })).toBeTruthy();
    expect(screen.getByText('Three files will change.')).toBeTruthy();
  });

  it('moves focus inside on open', () => {
    render(<Sample />);
    const dialog = screen.getByRole('dialog', { hidden: true });
    expect(dialog.contains(document.activeElement)).toBe(true);
  });

  it('closes on Escape', () => {
    const onClose = vi.fn();
    render(<Sample onClose={onClose} />);
    press(screen.getByLabelText('Folder'), 'Escape');
    expect(onClose).toHaveBeenCalled();
  });

  it('closes from the close button', () => {
    const onClose = vi.fn();
    render(<Sample onClose={onClose} />);
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(onClose).toHaveBeenCalled();
  });

  it('traps Tab inside from the last control to the first', () => {
    render(<Sample />);
    const write = screen.getByRole('button', { name: 'Write' });
    write.focus();
    press(write, 'Tab');
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Close' }));
    press(document.activeElement as HTMLElement, 'Tab', { shiftKey: true });
    expect(document.activeElement).toBe(write);
  });

  it('returns focus to the opener when it closes', () => {
    const { rerender } = render(<Sample open={false} />);
    const opener = screen.getByRole('button', { name: 'Opener' });
    opener.focus();
    rerender(<Sample open />);
    expect(document.activeElement).not.toBe(opener);
    rerender(<Sample open={false} />);
    expect(document.activeElement).toBe(opener);
  });

  it('closes on a backdrop click only when asked to', () => {
    const onClose = vi.fn();
    const { rerender } = render(<Sample onClose={onClose} />);
    const dialog = screen.getByRole('dialog', { hidden: true });
    fireEvent.click(dialog);
    expect(onClose).not.toHaveBeenCalled();
    rerender(<Sample onClose={onClose} closeOnBackdrop />);
    fireEvent.click(screen.getByRole('dialog', { hidden: true }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
