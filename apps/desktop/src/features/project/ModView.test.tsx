import { fireEvent, screen, waitFor } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { ModView } from './ModView';
import { loadProject, view } from './store';
import { gewehrRef, installTransport } from './testSupport';

async function show() {
  const transport = installTransport();
  await loadProject(gewehrRef());
  const loaded = view.value;
  if (!loaded) throw new Error('not loaded');
  const handlers = { onRefresh: vi.fn(), onOpen: vi.fn(), onNew: vi.fn(), onClose: vi.fn() };
  renderWithProviders(<ModView view={loaded} refreshing={false} {...handlers} />);
  return { transport, handlers };
}

describe('ModView', () => {
  it('opens on Basics and loads only what the shown tab needs', async () => {
    const { transport } = await show();
    expect(screen.getByRole('tab', { name: 'Basics', selected: true })).toBeTruthy();
    await screen.findByRole('textbox', { name: /Mod name/ });
    expect(transport.calls.some((c) => c.name === 'project_load_folders_get')).toBe(false);
  });

  it('switches to the folders tab and to the files tab', async () => {
    const { transport } = await show();
    fireEvent.click(screen.getByRole('tab', { name: 'Versions and folders' }));
    await screen.findByRole('region', { name: 'Versions' });
    expect(transport.calls.some((c) => c.name === 'project_load_folders_get')).toBe(true);
    fireEvent.click(screen.getByRole('tab', { name: 'Files' }));
    expect(screen.getByRole('tree', { name: 'Project folders' })).toBeTruthy();
  });

  it('shows the number of unsaved edits on the Basics tab while another tab is open', async () => {
    await show();
    fireEvent.input(await screen.findByRole('textbox', { name: /Mod name/ }), {
      target: { value: 'Edited' },
    });
    fireEvent.click(screen.getByRole('tab', { name: /^Files/ }));
    await waitFor(() => expect(screen.getByRole('tab', { name: /^Basics 1/ })).toBeTruthy());
    // the edit is still there when the tab is shown again
    fireEvent.click(screen.getByRole('tab', { name: /^Basics/ }));
    expect(screen.getByRole<HTMLInputElement>('textbox', { name: /Mod name/ }).value).toBe(
      'Edited',
    );
  });

  it('jumps to the Files tab from a layout finding', async () => {
    await show();
    fireEvent.click(screen.getByRole('tab', { name: /^Layout/ }));
    fireEvent.click(await screen.findByRole('button', { name: 'Patches/ce_patch.xml' }));
    expect(await screen.findByRole('tab', { name: 'Files', selected: true })).toBeTruthy();
  });

  it('passes the header actions on', async () => {
    const { handlers } = await show();
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(handlers.onClose).toHaveBeenCalled();
  });
});
