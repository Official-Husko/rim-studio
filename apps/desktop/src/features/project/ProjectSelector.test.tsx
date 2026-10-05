import { fireEvent, screen, waitFor } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { beforeEach, describe, expect, it } from 'vitest';
import { transportKind } from '~/shared/ipc';
import { currentProject, resetProjectStore, setCurrentProject } from '~/shared/project';
import { ProjectSelector } from './ProjectSelector';
import { gewehrRef, installTransport, replaceTransport } from './testSupport';

beforeEach(() => {
  installTransport();
});

describe('ProjectSelector', () => {
  it('says that no project is open and links to the Project page', () => {
    renderWithProviders(<ProjectSelector />);
    expect(screen.getByText('No project open')).toBeTruthy();
    expect(screen.getByRole('link', { name: 'Manage' }).getAttribute('href')).toBe('#/project');
  });

  it('shows the current project in the list of recent ones', () => {
    setCurrentProject(gewehrRef());
    renderWithProviders(<ProjectSelector />);
    const select = screen.getByLabelText('Current project') as HTMLSelectElement;
    expect(select.value).toBe(gewehrRef().path);
    expect(screen.getByRole('option', { name: "Huskos's Gewehr 41" })).toBeTruthy();
  });

  it('switches to another recent project through the backend', async () => {
    setCurrentProject({ projectId: 'p-2', path: '/m/Lone Wolf', name: 'Lone Wolf' });
    setCurrentProject(gewehrRef());
    const transport = replaceTransport();
    renderWithProviders(<ProjectSelector />);
    fireEvent.change(screen.getByLabelText('Current project'), {
      target: { value: '/m/Lone Wolf' },
    });
    await waitFor(() => expect(transport.calls.some((c) => c.name === 'project_open')).toBe(true));
    expect(transport.calls.find((c) => c.name === 'project_open')?.request).toEqual({
      path: '/m/Lone Wolf',
    });
    await waitFor(() => expect(currentProject.value?.name).toBe('The Lone Wolf Weapon Package'));
  });

  it('opens the last project only after the transport is chosen', async () => {
    resetProjectStore();
    window.localStorage.setItem('rimstudio.project.current', '/m/Gewehr');
    const transport = replaceTransport();
    transportKind.value = 'none';
    renderWithProviders(<ProjectSelector />);
    expect(transport.calls.some((c) => c.name === 'project_open')).toBe(false);
    expect(window.localStorage.getItem('rimstudio.project.current')).toBe('/m/Gewehr');
    transportKind.value = 'mock';
    await waitFor(() => expect(transport.calls.some((c) => c.name === 'project_open')).toBe(true));
    expect(currentProject.value).toBeDefined();
  });

  it('shows the problem when the project cannot be opened and keeps the old one', async () => {
    setCurrentProject({ projectId: 'p-2', path: '/m/Gone', name: 'Gone' });
    setCurrentProject(gewehrRef());
    replaceTransport({
      project_open: () => {
        throw { code: 'io.not-found', message: '/m/Gone is not a mod project', errorId: 'e-1' };
      },
    });
    renderWithProviders(<ProjectSelector />);
    fireEvent.change(screen.getByLabelText('Current project'), { target: { value: '/m/Gone' } });
    expect(await screen.findByRole('alert')).toBeTruthy();
    expect(currentProject.value?.path).toBe(gewehrRef().path);
  });
});
