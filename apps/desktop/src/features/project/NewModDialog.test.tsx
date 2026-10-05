import { fireEvent, screen, waitFor, within } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import type { ProjectCreateRequest, ProjectScaffoldPreviewDto } from 'rimstudio-ipc-types';
import { currentProject } from '~/shared/project';
import { NewModDialog } from './NewModDialog';
import { installTransport } from './testSupport';

/** A preview that follows the request: invalid without a dot in the id, else the recorded valid one. */
function previewFor(request: unknown): ProjectScaffoldPreviewDto {
  const req = request as ProjectCreateRequest;
  if (!req.packageId.includes('.') || req.name.trim() === '')
    return loadFixture('scaffold-preview-invalid');
  return {
    ...loadFixture<ProjectScaffoldPreviewDto>('scaffold-preview-versioned'),
    root: req.path,
  };
}

const handlers = { project_scaffold_preview: previewFor };

async function fillIdentity(): Promise<void> {
  const dialog = screen.getByRole('dialog', { name: 'New mod' });
  fireEvent.input(within(dialog).getByRole('textbox', { name: /Create in/ }), {
    target: { value: '/home/user/mods' },
  });
  fireEvent.input(within(dialog).getByRole('textbox', { name: /Mod name/ }), {
    target: { value: 'Pawbeans Arsenal' },
  });
  fireEvent.input(within(dialog).getByRole('textbox', { name: 'Author' }), {
    target: { value: 'Pawbeans' },
  });
}

describe('NewModDialog identity step', () => {
  it('suggests the package id and the folder name from what is typed', async () => {
    installTransport(handlers);
    renderWithProviders(<NewModDialog open onClose={() => undefined} />);
    await fillIdentity();
    expect(screen.getByRole<HTMLInputElement>('textbox', { name: /Package id/ }).value).toBe(
      'pawbeans.pawbeansarsenal',
    );
    expect(screen.getByRole<HTMLInputElement>('textbox', { name: 'Folder name' }).value).toBe(
      'Pawbeans Arsenal',
    );
  });

  it('keeps Next off until the backend accepts the values', async () => {
    installTransport(handlers);
    renderWithProviders(<NewModDialog open onClose={() => undefined} />);
    const next = screen.getByRole<HTMLButtonElement>('button', { name: 'Next' });
    expect(next.disabled).toBe(true);
    await fillIdentity();
    await waitFor(() => expect(next.disabled).toBe(false));
  });

  it('shows the backend finding about the package id while typing', async () => {
    installTransport(handlers);
    renderWithProviders(<NewModDialog open onClose={() => undefined} />);
    await fillIdentity();
    fireEvent.input(screen.getByRole('textbox', { name: /Package id/ }), {
      target: { value: 'nodots' },
    });
    expect(await screen.findByRole('alert')).toBeTruthy();
    expect(screen.getByRole<HTMLButtonElement>('button', { name: 'Next' }).disabled).toBe(true);
  });

  it('offers the mod folders of Setup as quick choices', async () => {
    installTransport(handlers);
    renderWithProviders(<NewModDialog open onClose={() => undefined} />);
    fireEvent.click(await screen.findByRole('button', { name: /^Create in Game mods/ }));
    const field = screen.getByRole<HTMLInputElement>('textbox', { name: /Create in/ });
    expect(field.value).toContain('Mods');
  });

  it('ticks the installed game version when detection knows it', async () => {
    installTransport({
      ...handlers,
      detect_get_report: () => ({
        report: { installs: [{ version: { raw: '1.5.4409', major: 1, minor: 5 } }] },
      }),
    });
    renderWithProviders(<NewModDialog open onClose={() => undefined} />);
    const box = await screen.findByRole<HTMLInputElement>('checkbox', { name: '1.5' });
    await waitFor(() => expect(box.checked).toBe(true));
    expect(screen.getByRole('checkbox', { name: '1.2' })).toBeTruthy();
  });
});

describe('NewModDialog structure and review', () => {
  async function toStructure(): Promise<void> {
    await fillIdentity();
    const next = screen.getByRole<HTMLButtonElement>('button', { name: 'Next' });
    await waitFor(() => expect(next.disabled).toBe(false));
    fireEvent.click(next);
  }

  it('shows the structure as a tree with a sentence about each folder and the options', async () => {
    const transport = installTransport(handlers);
    renderWithProviders(<NewModDialog open onClose={() => undefined} />);
    await toStructure();
    const tree = await screen.findByRole('list', { name: 'Structure of the new mod' });
    expect(
      within(tree).getByText('The About file. The game and the Workshop read it.'),
    ).toBeTruthy();
    expect(within(tree).getByText('Content that loads only on game version 1.6.')).toBeTruthy();
    fireEvent.click(screen.getByRole('checkbox', { name: 'README.md' }));
    await waitFor(() => {
      const last = [...transport.calls]
        .reverse()
        .find((c) => c.name === 'project_scaffold_preview');
      expect(last?.request).toMatchObject({ readme: true });
    });
  });

  it('reviews the files and creates the mod with the typed values', async () => {
    const onClose = vi.fn();
    const transport = installTransport({
      ...handlers,
      project_create: () => loadFixture('project-create-new'),
    });
    renderWithProviders(<NewModDialog open onClose={onClose} />);
    await toStructure();
    fireEvent.click(await screen.findByRole('button', { name: 'Next' }));
    const list = await screen.findByRole('list', {
      name: 'Files and folders that will be created',
    });
    expect(within(list).getByText('About/About.xml')).toBeTruthy();
    expect(screen.getByText(/^\d+ folders and \d+ files\.$/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Create mod' }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    const created = transport.calls.find((c) => c.name === 'project_create')
      ?.request as ProjectCreateRequest;
    expect(created).toMatchObject({
      path: '/home/user/mods/Pawbeans Arsenal',
      name: 'Pawbeans Arsenal',
      packageId: 'pawbeans.pawbeansarsenal',
      author: 'Pawbeans',
      supportedVersions: ['1.6'],
    });
    expect(currentProject.value).toBeDefined();
  });

  it('keeps the dialog open and shows the error of the backend', async () => {
    const onClose = vi.fn();
    installTransport({
      ...handlers,
      project_create: () => {
        throw {
          code: 'designer.apply-failed',
          message: 'About.xml already exists',
          errorId: 'e-9',
        };
      },
    });
    renderWithProviders(<NewModDialog open onClose={onClose} />);
    await toStructure();
    fireEvent.click(await screen.findByRole('button', { name: 'Next' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Create mod' }));
    expect(await screen.findByText('About.xml already exists')).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
  });

  it('goes back to the identity step with the values kept', async () => {
    installTransport(handlers);
    renderWithProviders(<NewModDialog open onClose={() => undefined} />);
    await toStructure();
    fireEvent.click(screen.getByRole('button', { name: 'Back' }));
    expect(screen.getByRole<HTMLInputElement>('textbox', { name: /Mod name/ }).value).toBe(
      'Pawbeans Arsenal',
    );
  });
});
