import { fireEvent, screen, waitFor, within } from '@testing-library/preact';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockTransport, press, renderWithProviders } from 'rimstudio-testkit';
import { setTransport } from '~/shared/ipc';
import { FolderPickerHost, isDesktop, openUrl, pickFile, pickFolder, revealPath } from './index';
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

  it('cannot choose the old folder while a typed path is still loading', async () => {
    renderWithProviders(<FolderPickerHost />);
    const result = pickFolder({ start: '/home/mock/Mods' });
    await waitFor(() => expect(screen.getByRole('row', { name: /Field Rations/ })).toBeTruthy());
    const input = screen.getByRole('textbox', { name: 'Path' });
    fireEvent.input(input, { target: { value: '/home/mock/Documents' } });
    press(input, 'Enter');
    const choose = screen.getByRole('button', { name: 'Choose this folder' }) as HTMLButtonElement;
    expect(choose.disabled).toBe(true);
    await waitFor(() => expect(screen.getByText('This folder is empty.')).toBeTruthy());
    expect(choose.disabled).toBe(false);
    fireEvent.click(choose);
    expect(await result).toBe('/home/mock/Documents');
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

describe('file filters in the browser picker', () => {
  it('shows only files of the wanted types', async () => {
    renderWithProviders(<FolderPickerHost />);
    void pickFile({ start: '/home/mock', filters: [{ name: 'Images', extensions: ['png'] }] });
    await waitFor(() => expect(screen.getByRole('row', { name: /Mods/ })).toBeTruthy());
    expect(screen.queryByRole('row', { name: /notes.txt/ })).toBeNull();
  });
});

describe('in the desktop shell', () => {
  const dialog = { open: vi.fn() };
  const opener = { revealItemInDir: vi.fn(), openUrl: vi.fn() };

  beforeEach(() => {
    dialog.open.mockReset();
    opener.revealItemInDir.mockReset();
    opener.openUrl.mockReset();
    Object.defineProperty(window, '__TAURI_INTERNALS__', { value: {}, configurable: true });
  });
  afterEach(() => {
    Reflect.deleteProperty(window, '__TAURI_INTERNALS__');
  });
  vi.doMock('@tauri-apps/plugin-dialog', () => dialog);
  vi.doMock('@tauri-apps/plugin-opener', () => opener);

  it('is detected by the capability probe', () => {
    expect(isDesktop()).toBe(true);
  });

  it('opens the native folder dialog and returns the chosen path', async () => {
    dialog.open.mockResolvedValue('/home/user/Mods');
    expect(await pickFolder({ start: '/home/user' })).toBe('/home/user/Mods');
    expect(dialog.open).toHaveBeenCalledWith({
      directory: true,
      multiple: false,
      defaultPath: '/home/user',
    });
    expect(pickerRequest.value).toBeNull();
  });

  it('passes the filters of a file dialog and maps a cancel to null', async () => {
    dialog.open.mockResolvedValue(null);
    const filters = [{ name: 'Images', extensions: ['png', 'jpg'] }];
    expect(await pickFile({ filters })).toBeNull();
    expect(dialog.open).toHaveBeenCalledWith({
      directory: false,
      multiple: false,
      defaultPath: undefined,
      filters,
    });
  });

  it('reports a failing dialog as an error object', async () => {
    dialog.open.mockRejectedValue(new Error('no display'));
    await expect(pickFolder()).rejects.toMatchObject({
      code: 'platform.native-failed',
      message: 'no display',
    });
  });

  it('reveals a path through the opener', async () => {
    opener.revealItemInDir.mockResolvedValue(undefined);
    expect(await revealPath('/home/user/Mods/a')).toBe(true);
    expect(opener.revealItemInDir).toHaveBeenCalledWith('/home/user/Mods/a');
  });

  it('opens https links through the opener and never opens a window', async () => {
    const open = vi.spyOn(window, 'open').mockReturnValue(null);
    expect(openUrl('https://steamcommunity.com')).toBe(true);
    await waitFor(() => expect(opener.openUrl).toHaveBeenCalledWith('https://steamcommunity.com'));
    expect(open).not.toHaveBeenCalled();
    expect(openUrl('http://example.com')).toBe(false);
  });
});

describe('revealPath in a browser', () => {
  it('has no file manager to open and says so', async () => {
    expect(isDesktop()).toBe(false);
    expect(await revealPath('/home/mock')).toBe(false);
  });
});
