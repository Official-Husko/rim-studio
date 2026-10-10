import { fireEvent, screen, waitFor, within } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { afterEach, describe, expect, it } from 'vitest';
import { bumpProjectRevision } from '~/shared/project';
import DesignerPage from './DesignerPage';
import { clearProject, setProject } from './project-source';
import { createDesigner } from './stores';
import { cloneEntry, fixture, installTransport, manualScheduler, settle } from './testSupport';

afterEach(() => clearProject());

function setup(withProject: boolean) {
  const transport = installTransport({
    designer_draft_list: () => ({ drafts: [cloneEntry()] }),
    defs_get_resolved: () => fixture('defs-resolved-bolt-action-rifle'),
  });
  if (withProject) setProject({ projectId: 'p-1', path: '/mods/Test', name: 'Test' });
  const clock = manualScheduler();
  const stores = createDesigner(() => 'p-1', clock.scheduler);
  renderWithProviders(<DesignerPage stores={stores} />);
  return { transport, stores, clock };
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

  it('lists the weapons of the mod between the drafts and the reference weapons', async () => {
    setup(true);
    const aside = screen.getByLabelText('Drafts and reference weapons');
    await waitFor(() => expect(within(aside).getByText('ranger rifle')).toBeTruthy());
    expect(within(aside).getByText('3 weapons')).toBeTruthy();
    const titles = within(aside)
      .getAllByRole('heading', { level: 2 })
      .map((h) => h.textContent);
    const at = (title: string) => titles.indexOf(title);
    expect(at('Drafts')).toBeGreaterThanOrEqual(0);
    expect(at("This mod's weapons")).toBeGreaterThan(at('Drafts'));
    expect(at('Reference weapons')).toBeGreaterThan(at("This mod's weapons"));
  });

  it('has no weapons of the mod without a project', async () => {
    const { transport } = setup(false);
    await waitFor(() => expect(screen.getByText('20 of 20 weapons')).toBeTruthy());
    expect(screen.queryByText("This mod's weapons")).toBeNull();
    expect(transport.calls.some((c) => c.name === 'designer_project_weapons')).toBe(false);
  });

  it('loads a weapon of the mod into the editor and edits it where it stands', async () => {
    const { transport } = setup(true);
    const load = await screen.findByRole('button', { name: 'Load ranger rifle into the editor' });
    fireEvent.click(load);
    const centre = screen.getByLabelText('Editor');
    await waitFor(() =>
      expect(within(centre).getByRole('heading', { name: 'ranger rifle' })).toBeTruthy(),
    );
    expect(within(centre).getByText('This weapon is edited where it stands')).toBeTruthy();
    expect(within(centre).getByText('Defs/ThingDefs_Weapons/RS_Weapons.xml')).toBeTruthy();
    expect(within(centre).getByText(/is defined next to the weapon/)).toBeTruthy();
    expect(within(centre).getByRole('button', { name: 'Dismiss the weapon notes' })).toBeTruthy();
    const call = transport.calls.find((c) => c.name === 'designer_open_weapon');
    expect(call?.request).toEqual({ projectId: 'p-1', defName: 'RS_Gun_Ranger' });
    // the weapon is a draft of the project now and its row continues it
    expect(await screen.findByRole('button', { name: 'ranger rifle' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Continue the draft of ranger rifle' })).toBeTruthy();
  });

  it('shows no editing bar for a draft that is not an edit of a weapon', async () => {
    setup(true);
    fireEvent.click(await screen.findByRole('button', { name: 'fixture rifle' }));
    await waitFor(() =>
      expect(screen.getByRole('heading', { name: 'fixture rifle' })).toBeTruthy(),
    );
    expect(screen.queryByText('This weapon is edited where it stands')).toBeNull();
  });

  it('reads the file of the open weapon again after a confirmation and drops unsaved edits', async () => {
    const { transport, stores, clock } = setup(true);
    fireEvent.click(
      await screen.findByRole('button', { name: 'Load ranger rifle into the editor' }),
    );
    await screen.findByText('This weapon is edited where it stands');
    stores.editor.setField('/ranged/damage', { value: 30, source: 'typed' });
    expect(stores.editor.saveState.value).toBe('dirty');

    fireEvent.click(screen.getByRole('button', { name: 'Read the file again' }));
    // nothing happens until the question is answered
    expect(transport.calls.filter((c) => c.name === 'designer_open_weapon')).toHaveLength(1);
    fireEvent.click(screen.getByRole('button', { name: 'Replace the draft' }));

    await waitFor(() =>
      expect(transport.calls.filter((c) => c.name === 'designer_open_weapon')).toHaveLength(2),
    );
    const again = transport.calls.filter((c) => c.name === 'designer_open_weapon')[1];
    expect(again?.request).toEqual({ projectId: 'p-1', defName: 'RS_Gun_Ranger', reload: true });
    await waitFor(() => expect(stores.editor.draft.value?.spec.ranged?.damage?.value).toBe(16));
    clock.runTimers();
    await settle();
    expect(transport.calls.some((c) => c.name === 'designer_draft_save')).toBe(false);
  });

  it('shows why a file could not be read again, next to the bar', async () => {
    const { transport } = setup(true);
    fireEvent.click(
      await screen.findByRole('button', { name: 'Load ranger rifle into the editor' }),
    );
    await screen.findByText('This weapon is edited where it stands');
    installTransport({
      designer_open_weapon: () => {
        throw { code: 'designer.invalid-draft', message: 'the file is gone', errorId: 'e-9' };
      },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Read the file again' }));
    fireEvent.click(screen.getByRole('button', { name: 'Replace the draft' }));
    const centre = screen.getByLabelText('Editor');
    await waitFor(() => expect(within(centre).getByText('the file is gone')).toBeTruthy());
    expect(transport.calls.some((c) => c.name === 'designer_open_weapon')).toBe(true);
  });

  it('reads the weapons of the mod again after files of the project were written', async () => {
    const { transport } = setup(true);
    await screen.findByText('ranger rifle');
    const reads = () => transport.calls.filter((c) => c.name === 'designer_project_weapons');
    expect(reads()).toHaveLength(1);
    bumpProjectRevision();
    await waitFor(() => expect(reads()).toHaveLength(2));
  });

  it('reads the weapons again for another project and forgets the first', async () => {
    const { transport } = setup(true);
    await screen.findByText('ranger rifle');
    setProject({ projectId: 'p-2', path: '/mods/Other', name: 'Other' });
    await waitFor(() =>
      expect(transport.calls.filter((c) => c.name === 'designer_project_weapons')).toHaveLength(2),
    );
  });
});
