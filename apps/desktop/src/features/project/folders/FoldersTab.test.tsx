import { fireEvent, screen, waitFor, within } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import type { ProjectLoadFoldersDto, ProjectLoadFoldersUpdateDto } from 'rimstudio-ipc-types';
import { describe, expect, it } from 'vitest';
import { installTransport } from '../testSupport';
import { FoldersTab } from './FoldersTab';

const ID = 'p-c36c596a';

async function open(file: 'load-folders-get-versioned' | 'load-folders-get-none', extra = {}) {
  const transport = installTransport({
    project_load_folders_get: () => loadFixture(file),
    ...extra,
  });
  renderWithProviders(<FoldersTab projectId={ID} />);
  await screen.findByRole('region', { name: 'Versions' });
  return transport;
}

describe('FoldersTab with a LoadFolders.xml', () => {
  it('shows the versions with their folders and blocks and the entries of each block', async () => {
    await open('load-folders-get-versioned');
    const table = screen.getByRole('table', { name: 'Versions' });
    expect(within(table).getAllByText('Has a block')).toHaveLength(2);
    const list = screen.getByRole('list', { name: 'Folders for 1.6' });
    expect(within(list).getByRole('textbox', { name: 'Folder path 1.6' })).toBeTruthy();
    const ce = within(list).getByRole('group', { name: 'Folder 1.6/Compat/CombatExtended' });
    expect(within(ce).getByText('Only if any is active: ceteam.combatextended')).toBeTruthy();
    expect(within(ce).getByText('Exists')).toBeTruthy();
  });

  it('adds a folder on Enter and checks it with a dry run', async () => {
    const transport = await open('load-folders-get-versioned');
    const field = screen.getByRole('textbox', { name: 'Add a folder to 1.5' });
    fireEvent.input(field, { target: { value: 'Extra' } });
    fireEvent.keyDown(field, { key: 'Enter' });
    await waitFor(() => expect(screen.getByText('1 unsaved change')).toBeTruthy());
    expect(
      transport.calls.some(
        (c) =>
          c.name === 'project_load_folders_update' &&
          JSON.stringify((c.request as { changes: unknown }).changes).includes('"add-entry"'),
      ),
    ).toBe(true);
  });

  it('shows a missing folder and the findings of a block', async () => {
    const data = loadFixture<ProjectLoadFoldersDto>('load-folders-get-versioned');
    const block = data.blocks[0];
    if (!block) throw new Error('fixture has no block');
    const entry = block.entries[1];
    if (!entry) throw new Error('fixture has no entry');
    entry.folderExists = false;
    data.diagnostics = [
      {
        code: 'loadfolders.empty-block',
        severity: 'warning',
        message: 'block 0 is empty',
        field: '/blocks/0',
      },
      {
        code: 'loadfolders.folder-missing',
        severity: 'warning',
        message: 'folder 1.5 is missing',
        field: '/blocks/0/entries/1/path',
      },
    ];
    await open('load-folders-get-versioned', { project_load_folders_get: () => data });
    expect(screen.getByText('Missing')).toBeTruthy();
    expect(screen.getByText('block 0 is empty')).toBeTruthy();
    expect(screen.getByText('folder 1.5 is missing')).toBeTruthy();
  });

  it('reviews the diff and saves', async () => {
    const transport = await open('load-folders-get-versioned');
    fireEvent.click(screen.getByRole('button', { name: 'Remove the block for 1.5' }));
    await waitFor(() => expect(screen.getByText('1 unsaved change')).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: 'Review changes' }));
    const dialog = await screen.findByRole('dialog', { name: 'Review changes to LoadFolders.xml' });
    expect(within(dialog).getByRole('region', { name: 'Changes to LoadFolders.xml' })).toBeTruthy();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(screen.getByText('No unsaved changes')).toBeTruthy());
    const writes = transport.calls.filter(
      (c) =>
        c.name === 'project_load_folders_update' &&
        (c.request as { dryRun: boolean }).dryRun === false,
    );
    expect(writes).toHaveLength(1);
  });

  it('shows the raw text read only', async () => {
    await open('load-folders-get-versioned');
    fireEvent.click(screen.getByRole('button', { name: 'LoadFolders.xml text' }));
    expect(screen.getByRole('region', { name: 'The text of LoadFolders.xml' })).toBeTruthy();
  });
});

describe('FoldersTab without a LoadFolders.xml', () => {
  it('offers to create the file and shows what saving would create', async () => {
    const created = loadFixture<ProjectLoadFoldersUpdateDto>('load-folders-update-dry');
    created.loadFolders = { ...created.loadFolders, exists: true, blocks: [], fileHash: undefined };
    const transport = await open('load-folders-get-none', {
      project_load_folders_update: () => created,
    });
    expect(screen.getByText('This mod has no LoadFolders.xml')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Create LoadFolders.xml' }));
    expect(await screen.findByText(/does not exist yet/)).toBeTruthy();
    expect(screen.getByText('1 unsaved change')).toBeTruthy();
    expect(
      transport.calls.find((c) => c.name === 'project_load_folders_update')?.request,
    ).toMatchObject({
      create: true,
    });
  });
});

describe('FoldersTab version folder', () => {
  it('previews and creates a version folder', async () => {
    const transport = await open('load-folders-get-versioned');
    fireEvent.click(screen.getByRole('button', { name: 'Add a version folder' }));
    const dialog = await screen.findByRole('dialog', { name: 'Add a version folder' });
    fireEvent.input(within(dialog).getByRole('textbox', { name: 'Game version' }), {
      target: { value: '1.7' },
    });
    const plan = await within(dialog).findByRole('region', { name: 'What will be created' });
    expect(within(plan).getByText(/folders will be created in/)).toBeTruthy();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Create the folder' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    const calls = transport.calls.filter((c) => c.name === 'project_version_add');
    expect(calls.at(-1)?.request).toMatchObject({
      version: '1.7',
      dryRun: false,
      standardFolders: true,
      addBlock: true,
    });
  });

  it('shows the refusal of the backend for a bad version', async () => {
    await open('load-folders-get-versioned', {
      project_version_add: () => {
        throw { code: 'project.edit-invalid', message: 'not a game version', errorId: 'e-3' };
      },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add a version folder' }));
    const dialog = await screen.findByRole('dialog', { name: 'Add a version folder' });
    fireEvent.input(within(dialog).getByRole('textbox', { name: 'Game version' }), {
      target: { value: 'x' },
    });
    expect(await within(dialog).findByText('not a game version')).toBeTruthy();
    expect(
      within(dialog).getByRole<HTMLButtonElement>('button', { name: 'Create the folder' }).disabled,
    ).toBe(true);
  });
});
