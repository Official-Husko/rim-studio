import { fireEvent, screen, waitFor, within } from '@testing-library/preact';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockTransport, press, renderWithProviders } from 'rimstudio-testkit';
import { setTransport } from '~/shared/ipc';
import { FolderPickerHost, openUrl, pickFile, pickFolder } from './index';
import { pickerRequest } from './dialogs';

beforeEach(() => {
  setTransport(createMockTransport());
  pickerRequest.value = null;
});
afterEach(() => {
  vi.restoreAllMocks();
});

async function opened(): Promise<void> {
  await waitFor(() => expect(screen.getByRole('dialog', { hidden: true })).toBeTruthy());
  await waitFor(() => expect(screen.getByRole('row', { name: /Mods/ })).toBeTruthy());
}

describe('folder picker', () => {
  it('lists the home folder and the places of the bridge', async () => {
    renderWithProviders(<FolderPickerHost />);
    void pickFolder();
    await opened();
    const dialog = screen.getByRole('dialog', { hidden: true });
    expect(within(dialog).getByRole('button', { name: 'Custom mods' })).toBeTruthy();
    expect(within(dialog).getByRole('row', { name: /Mods/ })).toBeTruthy();
    expect((within(dialog).getByRole('textbox', { name: 'Path' }) as HTMLInputElement).value).toBe(
      '/home/mock',
    );
  });

  it('navigates into a folder and resolves with the chosen path', async () => {
    renderWithProviders(<FolderPickerHost />);
    const result = pickFolder();
    await opened();
    fireEvent.dblClick(screen.getByRole('row', { name: /^Mods/ }));
    await waitFor(() => expect(screen.getByRole('row', { name: /Plasma Carbine/ })).toBeTruthy());
    expect(screen.getByRole('row', { name: /Plasma Carbine/ }).textContent).toContain('Mod folder');
    fireEvent.click(screen.getByRole('button', { name: 'Choose this folder' }));
    expect(await result).toBe('/home/mock/Mods');
    expect(pickerRequest.value).toBeNull();
  });

  it('goes up and uses the places', async () => {
    renderWithProviders(<FolderPickerHost />);
    void pickFolder();
    await opened();
    fireEvent.click(screen.getByRole('button', { name: 'Custom mods' }));
    await waitFor(() => expect(screen.getByRole('row', { name: /Plasma Carbine/ })).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: 'Up one folder' }));
    await waitFor(() =>
      expect((screen.getByRole('textbox', { name: 'Path' }) as HTMLInputElement).value).toBe(
        '/home/mock',
      ),
    );
  });

  it('opens a typed path on Enter', async () => {
    renderWithProviders(<FolderPickerHost />);
    void pickFolder({ start: '/home/mock/Mods' });
    await waitFor(() => expect(screen.getByRole('row', { name: /Field Rations/ })).toBeTruthy());
    const input = screen.getByRole('textbox', { name: 'Path' });
    fireEvent.input(input, { target: { value: '/home/mock/Documents' } });
    press(input, 'Enter');
    await waitFor(() => expect(screen.getByText('This folder is empty.')).toBeTruthy());
  });

  it('shows an error for a path that is not a folder', async () => {
    renderWithProviders(<FolderPickerHost />);
    void pickFolder({ start: '/does/not/exist' });
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('Not a folder'));
  });

  it('resolves null when cancelled and when replaced by a second request', async () => {
    renderWithProviders(<FolderPickerHost />);
    const first = pickFolder();
    await opened();
    const second = pickFolder();
    expect(await first).toBeNull();
    await waitFor(() => expect(screen.getByRole('button', { name: 'Cancel' })).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(await second).toBeNull();
  });

  it('picks a file in file mode', async () => {
    renderWithProviders(<FolderPickerHost />);
    const result = pickFile({ start: '/home/mock' });
    await waitFor(() => expect(screen.getByRole('row', { name: /notes.txt/ })).toBeTruthy());
    expect(
      (screen.getByRole('button', { name: 'Choose this file' }) as HTMLButtonElement).disabled,
    ).toBe(true);
    fireEvent.click(screen.getByRole('row', { name: /notes.txt/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Choose this file' }));
    expect(await result).toBe('/home/mock/notes.txt');
  });
});

describe('openUrl', () => {
  it('opens https links only', () => {
    const open = vi.spyOn(window, 'open').mockReturnValue(null);
    expect(openUrl('https://steamcommunity.com')).toBe(true);
    expect(openUrl('http://example.com')).toBe(false);
    expect(openUrl('javascript:alert(1)')).toBe(false);
    expect(open).toHaveBeenCalledTimes(1);
  });
});
