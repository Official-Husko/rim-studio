import { fireEvent, screen, waitFor } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { AddFolderDialog } from './AddFolderDialog';
import { pending, startAdd } from './sourcesStore';
import { installTransport } from './testSupport';

describe('AddFolderDialog', () => {
  it('is closed until a folder was picked', () => {
    installTransport();
    renderWithProviders(<AddFolderDialog />);
    expect(screen.queryByRole('dialog', { hidden: true })).toBeNull();
  });

  it('shows the probe result of the owner mods folder and adds with a label', async () => {
    const transport = installTransport();
    renderWithProviders(<AddFolderDialog />);
    await startAdd('/run/media/pawbeans/project_drive/pawbeans/Projects/RimWorld Mods');
    await screen.findByText('Looks like a mods folder');
    expect(screen.getByText('19 mods found')).toBeTruthy();
    expect(screen.getByText('Scan depth 1')).toBeTruthy();
    fireEvent.input(screen.getByRole('textbox', { name: 'Label' }), {
      target: { value: 'My mods' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add folder' }));
    await waitFor(() => expect(pending.value).toBeUndefined());
    expect(transport.calls.find((c) => c.name === 'sources_add_folder')?.request).toMatchObject({
      label: 'My mods',
      scanDepth: 1,
    });
  });

  it('explains an overlap and disables the add button', async () => {
    installTransport({ sources_probe_folder: () => loadFixture('probe-overlap') });
    renderWithProviders(<AddFolderDialog />);
    await startAdd('/workshop');
    await screen.findByText(/overlaps a folder already in the list/);
    expect((screen.getByRole('button', { name: 'Add folder' }) as HTMLButtonElement).disabled).toBe(
      true,
    );
    expect(screen.getByText('This folder cannot be added.')).toBeTruthy();
  });

  it('shows the error when the probe fails and closes on cancel', async () => {
    installTransport({
      sources_probe_folder: () => {
        throw { code: 'io.denied', message: 'denied', errorId: 'e4' };
      },
    });
    renderWithProviders(<AddFolderDialog />);
    await startAdd('/secret');
    await screen.findByText('denied');
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { hidden: true })).toBeNull());
  });
});
