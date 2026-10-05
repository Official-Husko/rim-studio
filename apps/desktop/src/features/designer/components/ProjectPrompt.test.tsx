import { fireEvent, screen, waitFor } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { clearProject, currentProject } from '../project-source';
import { installTransport } from '../testSupport';
import { ProjectPrompt } from './ProjectPrompt';

afterEach(() => clearProject());

describe('ProjectPrompt', () => {
  it('opens the project folder the user types', async () => {
    const transport = installTransport({
      project_open: () => ({
        projectId: 'p-9',
        name: 'Test Mod',
        path: '/mods/Test',
        supportedVersions: [],
        hasAbout: true,
        hasLoadFolders: false,
        hasCeGate: false,
        defFiles: 0,
        diagnostics: [],
      }),
    });
    const onOpened = vi.fn();
    renderWithProviders(<ProjectPrompt onOpened={onOpened} />);
    fireEvent.input(screen.getByLabelText('Project folder'), { target: { value: '/mods/Test' } });
    fireEvent.click(screen.getByRole('button', { name: 'Open project' }));
    await waitFor(() => expect(onOpened).toHaveBeenCalled());
    expect(transport.calls[0]).toMatchObject({
      name: 'project_open',
      request: { path: '/mods/Test' },
    });
    expect(currentProject()?.projectId).toBe('p-9');
  });

  it('shows the error when the folder is no project', async () => {
    installTransport({
      project_open: () => {
        throw { code: 'project.not-a-mod', message: 'No About file here', errorId: 'e-1' };
      },
    });
    renderWithProviders(<ProjectPrompt onOpened={() => undefined} />);
    fireEvent.input(screen.getByLabelText('Project folder'), { target: { value: '/x' } });
    fireEvent.click(screen.getByRole('button', { name: 'Open project' }));
    await waitFor(() => expect(screen.getByText('No About file here')).toBeTruthy());
    expect(currentProject()).toBeUndefined();
  });
});
