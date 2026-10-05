import { fireEvent, screen, waitFor } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { afterEach, describe, expect, it } from 'vitest';
import DesignerPage from './DesignerPage';
import { clearProject, setProject } from './project-source';
import { createDesigner } from './stores';
import { cloneEntry, fixture, installTransport, manualScheduler } from './testSupport';

afterEach(() => clearProject());

function setup(withProject: boolean) {
  const transport = installTransport({
    designer_draft_list: () => ({ drafts: [cloneEntry()] }),
    defs_get_resolved: () => fixture('defs-resolved-bolt-action-rifle'),
  });
  if (withProject) setProject({ projectId: 'p-1', path: '/mods/Test', name: 'Test' });
  const stores = createDesigner(() => 'p-1', manualScheduler().scheduler);
  renderWithProviders(<DesignerPage stores={stores} />);
  return { transport, stores };
}

describe('DesignerPage', () => {
  it('shows the three regions and the reference weapons of the install', async () => {
    setup(true);
    expect(screen.getByLabelText('Drafts and reference weapons')).toBeTruthy();
    expect(screen.getByLabelText('Editor')).toBeTruthy();
    expect(screen.getByLabelText('Output slot')).toBeTruthy();
    await waitFor(() => expect(screen.getByText('20 of 20 weapons')).toBeTruthy());
    expect(screen.getByText('No draft open')).toBeTruthy();
  });

  it('asks for a project first when none is open, but still lists the reference weapons', async () => {
    setup(false);
    expect(screen.getByText(/No project is open/)).toBeTruthy();
    await waitFor(() => expect(screen.getByText('20 of 20 weapons')).toBeTruthy());
    const clones = await screen.findAllByRole('button', { name: 'Clone' });
    expect((clones[0] as HTMLButtonElement).disabled).toBe(true);
  });

  it('opens a stored draft in the editor', async () => {
    setup(true);
    const card = await screen.findByRole('button', { name: 'fixture rifle' });
    fireEvent.click(card);
    await waitFor(() =>
      expect(screen.getByRole('heading', { name: 'fixture rifle' })).toBeTruthy(),
    );
    expect(screen.getByText('Live readouts')).toBeTruthy();
  });

  it('clones a reference weapon through the dialog', async () => {
    const { transport } = setup(true);
    const clones = await screen.findAllByRole('button', { name: 'Clone' });
    fireEvent.click(clones[0] as HTMLElement);
    expect(await screen.findByText(/Clone flamebow/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Create' }));
    await waitFor(() =>
      expect(transport.calls.some((c) => c.name === 'designer_clone')).toBe(true),
    );
    const call = transport.calls.find((c) => c.name === 'designer_clone')?.request as {
      source: string;
      defName: string;
    };
    expect(call).toMatchObject({ source: 'Flamebow', defName: 'Flamebow_Copy' });
  });

  it('shows the real definition of a reference weapon in a dialog', async () => {
    setup(true);
    const buttons = await screen.findAllByRole('button', { name: 'Real definition' });
    fireEvent.click(buttons[0] as HTMLElement);
    await waitFor(() => expect(screen.getByRole('dialog')).toBeTruthy());
    await waitFor(() => expect(screen.getByText(/Ludeon.RimWorld/)).toBeTruthy());
  });

  it('starts a new melee weapon from the drafts panel', async () => {
    const { transport } = setup(true);
    fireEvent.click(await screen.findByRole('button', { name: 'New melee' }));
    fireEvent.input(screen.getByLabelText(/Def name/), { target: { value: 'TM_Axe' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create' }));
    await waitFor(() =>
      expect(transport.calls.some((c) => c.name === 'designer_draft_save')).toBe(true),
    );
    await waitFor(() => expect(screen.getByRole('heading', { name: 'TM_Axe' })).toBeTruthy());
  });
});
