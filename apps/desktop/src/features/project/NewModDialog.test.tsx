import { fireEvent, screen, waitFor, within } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import type { ProjectCreateRequest } from 'rimstudio-ipc-types';
import { currentProject } from '~/shared/project';
import { NewModDialog } from './NewModDialog';
import { installTransport } from './testSupport';

function type(label: string, value: string): void {
  fireEvent.input(screen.getByLabelText(new RegExp(`^${label}`)), { target: { value } });
}

describe('NewModDialog', () => {
  it('suggests the package id and the folder name from what is typed', () => {
    installTransport();
    renderWithProviders(<NewModDialog open onClose={vi.fn()} startParent="/home/user/mods" />);
    type('Author', 'Pawbeans');
    type('Mod name', 'Pawbeans Arsenal');
    expect((screen.getByLabelText(/^Package id/) as HTMLInputElement).value).toBe(
      'pawbeans.pawbeansarsenal',
    );
    expect((screen.getByLabelText('Folder name') as HTMLInputElement).value).toBe(
      'Pawbeans Arsenal',
    );
    expect(screen.getByText('/home/user/mods/Pawbeans Arsenal')).toBeTruthy();
  });

  it('stops suggesting once the package id is typed by hand', () => {
    installTransport();
    renderWithProviders(<NewModDialog open onClose={vi.fn()} startParent="/m" />);
    type('Mod name', 'First');
    type('Package id', 'my.own.id');
    type('Mod name', 'Second');
    expect((screen.getByLabelText(/^Package id/) as HTMLInputElement).value).toBe('my.own.id');
  });

  it('shows the scaffold that will be written and follows the options', () => {
    installTransport();
    renderWithProviders(<NewModDialog open onClose={vi.fn()} startParent="/m" />);
    const list = screen.getByRole('list', { name: 'Files and folders that will be created' });
    expect(within(list).queryByText('LoadFolders.xml')).toBeNull();
    fireEvent.click(screen.getByLabelText('Combat Extended patch folder, gated'));
    expect(within(list).getByText('LoadFolders.xml')).toBeTruthy();
    expect(within(list).getByText('CombatExtended')).toBeTruthy();
  });

  it('keeps the create button off until the mod can be created', () => {
    installTransport();
    renderWithProviders(<NewModDialog open onClose={vi.fn()} startParent="/m" />);
    const create = screen.getByRole('button', { name: 'Create mod' }) as HTMLButtonElement;
    expect(create.disabled).toBe(true);
    type('Mod name', 'Arsenal');
    expect(create.disabled).toBe(false);
  });

  it('creates the mod with the typed values, makes it current and closes', async () => {
    const transport = installTransport();
    const onClose = vi.fn();
    renderWithProviders(<NewModDialog open onClose={onClose} startParent="/home/user/mods" />);
    type('Author', 'Pawbeans');
    type('Mod name', 'Arsenal');
    type('Supported versions', '1.5, 1.6');
    fireEvent.click(screen.getByLabelText('Combat Extended patch folder, gated'));
    fireEvent.click(screen.getByRole('button', { name: 'Create mod' }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    const call = transport.calls.find((c) => c.name === 'project_create');
    expect(call?.request).toMatchObject({
      path: '/home/user/mods/Arsenal',
      name: 'Arsenal',
      author: 'Pawbeans',
      packageId: 'pawbeans.arsenal',
      supportedVersions: ['1.5', '1.6'],
      cePatchFolder: true,
      patchesFolder: true,
    } satisfies Partial<ProjectCreateRequest>);
    expect(currentProject.value).toBeDefined();
  });

  it('keeps the dialog open and shows the error of the backend', async () => {
    installTransport({
      project_create: () => {
        throw {
          code: 'designer.apply-failed',
          message: 'the package id is not in the format of the game',
          errorId: 'e-1',
        };
      },
    });
    const onClose = vi.fn();
    renderWithProviders(<NewModDialog open onClose={onClose} startParent="/m" />);
    type('Mod name', 'Arsenal');
    fireEvent.click(screen.getByRole('button', { name: 'Create mod' }));
    expect(await screen.findByText('the package id is not in the format of the game')).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
  });
});
