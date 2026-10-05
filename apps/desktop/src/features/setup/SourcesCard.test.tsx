import { fireEvent, screen, waitFor, within } from '@testing-library/preact';
import { loadFixture } from 'rimstudio-testkit';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { FolderPickerHost } from '~/shared/platform';
import { SourcesCard } from './SourcesCard';
import { loadSources } from './sourcesStore';
import { installTransport } from './testSupport';

async function renderWithCustom() {
  const transport = installTransport({
    sources_list: () => loadFixture('sources-list-with-custom'),
    settings_get: () => loadFixture('settings-get-with-custom'),
  });
  await loadSources();
  renderWithProviders(<SourcesCard />);
  return transport;
}

describe('SourcesCard', () => {
  it('lists the sources with kind, status and mod count', async () => {
    await renderWithCustom();
    const list = screen.getByRole('list', { name: 'Mod folders' });
    expect(within(list).getAllByRole('listitem')).toHaveLength(4);
    expect(within(list).getAllByText('Workshop').length).toBe(2);
    expect(within(list).getByText('Your folder')).toBeTruthy();
    expect(screen.getByText('691 mods')).toBeTruthy();
    expect(screen.getAllByText('Not scanned yet')).toHaveLength(1);
    expect(screen.getByText(/On drive project_drive/)).toBeTruthy();
  });

  it('offers rename and remove for custom folders only', async () => {
    await renderWithCustom();
    expect(screen.getAllByRole('button', { name: /^Remove / })).toHaveLength(1);
    expect(screen.getAllByRole('button', { name: /^Rename / })).toHaveLength(1);
    expect(screen.getAllByRole('switch')).toHaveLength(4);
  });

  it('disables the first Up and the last Down', async () => {
    await renderWithCustom();
    const up = screen.getAllByRole('button', { name: /^Move .* up$/ });
    const down = screen.getAllByRole('button', { name: /^Move .* down$/ });
    expect((up[0] as HTMLButtonElement).disabled).toBe(true);
    expect((down[3] as HTMLButtonElement).disabled).toBe(true);
  });

  it('moves a source to the position before it', async () => {
    const transport = await renderWithCustom();
    fireEvent.click(screen.getAllByRole('button', { name: /^Move .* up$/ })[3] as HTMLElement);
    await waitFor(() =>
      expect(transport.calls.some((c) => c.name === 'sources_update')).toBe(true),
    );
    expect(transport.calls.find((c) => c.name === 'sources_update')?.request).toEqual({
      id: 'cf_a4f98cb5',
      order: 2,
    });
  });

  it('turns a source off', async () => {
    const transport = await renderWithCustom();
    fireEvent.click(screen.getAllByRole('switch')[2] as HTMLElement);
    await waitFor(() =>
      expect(transport.calls.some((c) => c.name === 'sources_update')).toBe(true),
    );
    expect(transport.calls.find((c) => c.name === 'sources_update')?.request).toEqual({
      id: 'workshop-0',
      enabled: false,
    });
  });

  it('renames a custom folder from the keyboard', async () => {
    const transport = await renderWithCustom();
    fireEvent.click(screen.getByRole('button', { name: 'Rename My mods' }));
    const field = screen.getByRole('textbox', { name: 'Folder label' });
    fireEvent.input(field, { target: { value: 'Weapons' } });
    fireEvent.keyDown(field, { key: 'Enter' });
    await waitFor(() =>
      expect(transport.calls.some((c) => c.name === 'sources_update')).toBe(true),
    );
    expect(transport.calls.find((c) => c.name === 'sources_update')?.request).toEqual({
      id: 'cf_a4f98cb5',
      label: 'Weapons',
    });
  });

  it('asks before removing and says no files are touched', async () => {
    const transport = await renderWithCustom();
    fireEvent.click(screen.getByRole('button', { name: 'Remove My mods' }));
    const dialog = await screen.findByRole('dialog', { hidden: true });
    expect(within(dialog).getByText(/No files are touched/)).toBeTruthy();
    expect(transport.calls.some((c) => c.name === 'sources_remove')).toBe(false);
    fireEvent.click(within(dialog).getByRole('button', { name: 'Remove from list', hidden: true }));
    await waitFor(() =>
      expect(transport.calls.some((c) => c.name === 'sources_remove')).toBe(true),
    );
    expect(transport.calls.find((c) => c.name === 'sources_remove')?.request).toEqual({
      id: 'cf_a4f98cb5',
    });
  });

  it('shows an empty state before anything is loaded and listed', async () => {
    installTransport({ sources_list: () => ({ sources: [] }) });
    await loadSources();
    renderWithProviders(<SourcesCard />);
    expect(screen.getByText('No mod folders yet')).toBeTruthy();
  });

  it('runs the add folder flow: pick, probe result, confirm', async () => {
    const transport = installTransport();
    await loadSources();
    renderWithProviders(
      <>
        <SourcesCard />
        <FolderPickerHost />
      </>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Add folder' }));
    const picker = await screen.findByRole('dialog', { hidden: true });
    await waitFor(() =>
      expect(
        (
          within(picker).getByRole('button', {
            name: 'Choose this folder',
            hidden: true,
          }) as HTMLButtonElement
        ).disabled,
      ).toBe(false),
    );
    fireEvent.click(
      within(picker).getByRole('button', { name: 'Choose this folder', hidden: true }),
    );
    await screen.findByText('Looks like a mods folder');
    expect(screen.getByText('19 mods found')).toBeTruthy();
    expect(transport.calls.some((c) => c.name === 'sources_add_folder')).toBe(false);
  });
});
