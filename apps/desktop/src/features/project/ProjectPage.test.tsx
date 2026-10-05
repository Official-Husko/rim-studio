import { fireEvent, screen, waitFor, within } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { FolderPickerHost } from '~/shared/platform';
import { currentProject, setCurrentProject } from '~/shared/project';
import ProjectPage from './ProjectPage';
import { installTransport, installWithProject } from './testSupport';

describe('ProjectPage without a project', () => {
  it('offers to open a mod, to create one and shows the recent list', async () => {
    installTransport();
    renderWithProviders(<ProjectPage />);
    expect(screen.getByRole('heading', { name: 'Project' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Choose a mod folder' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'New mod' })).toBeTruthy();
    expect(screen.getByText('No recent projects')).toBeTruthy();
    // the sources the app knows offer a shortcut into the picker
    expect(await screen.findByRole('button', { name: 'Browse in Game mods' })).toBeTruthy();
  });

  it('opens the folder picker from the choose button', async () => {
    installTransport();
    renderWithProviders(
      <>
        <ProjectPage />
        <FolderPickerHost />
      </>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Choose a mod folder' }));
    expect(await screen.findByRole('dialog')).toBeTruthy();
  });

  it('shows the recent projects and opens one', async () => {
    const transport = installTransport();
    setCurrentProject({ projectId: 'p-1', path: '/home/user/mods/Old', name: 'Old Mod' });
    setCurrentProject(undefined);
    renderWithProviders(<ProjectPage />);
    fireEvent.click(await screen.findByRole('button', { name: /^Old Mod/ }));
    await waitFor(() => expect(currentProject.value).toBeDefined());
    expect(transport.calls.some((c) => c.name === 'project_open')).toBe(true);
  });
});

describe('ProjectPage with a project', () => {
  it('shows the info card with names, versions and counts', async () => {
    installWithProject();
    renderWithProviders(<ProjectPage />);
    expect(await screen.findByText("Huskos's Gewehr 41")).toBeTruthy();
    expect(screen.getByText('oh.weapons.gewehr41')).toBeTruthy();
    expect(screen.getByText('1.4')).toBeTruthy();
    expect(screen.getByText('Game style')).toBeTruthy();
    expect(screen.getByText('Not used')).toBeTruthy();
    const counts = screen.getByRole('list', { name: 'Contents' });
    expect(within(counts).getByText('Weapon defs').previousSibling?.textContent).toBe('5');
  });

  it('shows the annotated tree with role badges and an issue marker', async () => {
    installWithProject();
    renderWithProviders(<ProjectPage />);
    const tree = await screen.findByRole('tree', { name: 'Project folders' });
    expect(within(tree).getByText('Content')).toBeTruthy();
    expect(within(tree).getAllByText('1 issue').length).toBeGreaterThan(0);
    expect(within(tree).getByText('Source')).toBeTruthy();
  });

  it('shows a file with xml colouring when it is chosen in the tree', async () => {
    const transport = installWithProject();
    renderWithProviders(<ProjectPage />);
    const tree = await screen.findByRole('tree', { name: 'Project folders' });
    fireEvent.click(within(tree).getByRole('treeitem', { name: /About\.xml/ }));
    expect(await screen.findByRole('region', { name: 'Contents of About/About.xml' })).toBeTruthy();
    const read = transport.calls.find((c) => c.name === 'project_read_file');
    expect(read?.request).toEqual({ projectId: 'p-c36c596a', path: 'About/About.xml' });
  });

  it('shows the layout issues and creates the missing folders', async () => {
    const transport = installWithProject();
    renderWithProviders(<ProjectPage />);
    await screen.findByRole('tree', { name: 'Project folders' });
    fireEvent.click(screen.getByRole('tab', { name: /^Layout \d/ }));
    expect(screen.getByText('Combat Extended content is not gated')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Create 2 missing folders' }));
    await waitFor(() =>
      expect(transport.calls.some((c) => c.name === 'project_scaffold_missing')).toBe(true),
    );
    expect(transport.calls.find((c) => c.name === 'project_scaffold_missing')?.request).toEqual({
      projectId: 'p-c36c596a',
      dryRun: false,
    });
    expect(await screen.findByText('Created 2 folders')).toBeTruthy();
  });

  it('closes the project and returns to the choice screen', async () => {
    installWithProject();
    renderWithProviders(<ProjectPage />);
    await screen.findByText("Huskos's Gewehr 41");
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(await screen.findByRole('button', { name: 'Choose a mod folder' })).toBeTruthy();
    expect(currentProject.value).toBeUndefined();
  });

  it('says so when the project cannot be read', async () => {
    installWithProject({
      project_open: () => {
        throw { code: 'io.not-found', message: 'gone', errorId: 'e-1' };
      },
    });
    renderWithProviders(<ProjectPage />);
    expect(await screen.findByText('The project could not be read')).toBeTruthy();
    expect(screen.getByText('gone')).toBeTruthy();
  });
});
