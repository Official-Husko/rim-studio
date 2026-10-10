import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import type { ApiError } from 'rimstudio-ipc-types';
import { EditsFileBar } from './EditsFileBar';

const FILE = 'Defs/ThingDefs_Weapons/RS_Weapons.xml';

function setup(props: { busy?: boolean; error?: ApiError } = {}) {
  const onReload = vi.fn();
  renderWithProviders(
    <EditsFileBar file={FILE} busy={props.busy ?? false} error={props.error} onReload={onReload} />,
  );
  return { onReload };
}

describe('EditsFileBar', () => {
  it('says the weapon is edited where it stands and names the file', () => {
    setup();
    expect(screen.getByText('This weapon is edited where it stands')).toBeTruthy();
    expect(screen.getByText(FILE)).toBeTruthy();
    expect(
      screen.getByText(
        'Applying replaces the definition of this weapon in this file and changes nothing else in it.',
      ),
    ).toBeTruthy();
  });

  it('asks before reading the file again, because the draft is replaced', () => {
    const { onReload } = setup();
    fireEvent.click(screen.getByRole('button', { name: 'Read the file again' }));
    expect(onReload).not.toHaveBeenCalled();
    const dialog = screen.getByRole('dialog');
    expect(dialog.textContent).toContain('Changes you have not applied are lost.');
    fireEvent.click(screen.getByRole('button', { name: 'Replace the draft' }));
    expect(onReload).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('keeps the draft when the question is cancelled', () => {
    const { onReload } = setup();
    fireEvent.click(screen.getByRole('button', { name: 'Read the file again' }));
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onReload).not.toHaveBeenCalled();
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('cannot be asked again while the file is being read', () => {
    setup({ busy: true });
    // a busy button carries the spinner's own name in front of its text
    const button = screen.getByRole('button', { name: /Read the file again/ }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    expect(button.getAttribute('aria-busy')).toBe('true');
  });

  it('shows why the file could not be read', () => {
    setup({
      error: {
        code: 'designer.invalid-draft',
        message: 'RS_Gun is not defined here',
        errorId: 'e-1',
      },
    });
    expect(screen.getByText('designer.invalid-draft')).toBeTruthy();
    expect(screen.getByText('RS_Gun is not defined here')).toBeTruthy();
  });
});
