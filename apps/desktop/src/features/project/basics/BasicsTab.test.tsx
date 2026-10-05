import { fireEvent, screen, waitFor, within } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import type { ProjectAboutDto } from 'rimstudio-ipc-types';
import { describe, expect, it, vi } from 'vitest';
import * as platform from '~/shared/platform';
import { installTransport } from '../testSupport';
import { BasicsTab } from './BasicsTab';

const ID = 'p-c36c596a';

async function open(extra = {}) {
  const transport = installTransport(extra);
  renderWithProviders(<BasicsTab projectId={ID} />);
  await screen.findByRole('textbox', { name: /Mod name/ });
  return transport;
}

describe('BasicsTab sections', () => {
  it('shows every section with the values of the file', async () => {
    await open();
    for (const name of [
      'Identity',
      'Description',
      'Game versions',
      'Dependencies',
      'Load order and conflicts',
      'Images',
      'Advanced',
    ]) {
      expect(screen.getByRole('region', { name })).toBeTruthy();
    }
    expect(screen.getByRole<HTMLInputElement>('textbox', { name: /Package id/ }).value).toBe(
      'oh.weapons.gewehr41',
    );
    expect(screen.getByText('108 characters')).toBeTruthy();
    expect(screen.getByRole<HTMLInputElement>('checkbox', { name: '1.4' }).checked).toBe(true);
    expect(screen.getByRole('button', { name: 'Remove Ludeon.RimWorld' })).toBeTruthy();
  });

  it('shows the findings of the backend next to the field and in the totals', async () => {
    await open();
    expect(screen.getByText(/Findings: 0 errors, 1 warning, 1 note/)).toBeTruthy();
    expect(
      screen.getByText(/the installed game is 1.6 and the list does not name it/),
    ).toBeTruthy();
    expect(screen.getByText(/the preview image is 1920 by 1080 pixels/)).toBeTruthy();
  });

  it('shows the raw file and the advanced blocks read only', async () => {
    await open();
    fireEvent.click(screen.getByRole('button', { name: 'Advanced' }));
    expect(screen.getByRole('region', { name: 'The text of About.xml' })).toBeTruthy();
    expect(screen.getByText('This mod has no settings by game version.')).toBeTruthy();
  });

  it('says so when the file cannot be edited and disables the fields', async () => {
    const locked = {
      ...loadFixture<ProjectAboutDto>('about-get-gewehr'),
      editable: false,
      notEditableReason: 'The file is not UTF-8.',
    };
    await open({ project_about_get: () => locked });
    expect(screen.getByText('The file is not UTF-8.')).toBeTruthy();
    expect(screen.getByRole<HTMLInputElement>('textbox', { name: /Mod name/ }).disabled).toBe(true);
  });

  it('offers a retry when the file cannot be read', async () => {
    installTransport({
      project_about_get: () => {
        throw { code: 'io.not-found', message: 'no About.xml here', errorId: 'e-1' };
      },
    });
    renderWithProviders(<BasicsTab projectId={ID} />);
    expect(await screen.findByText('About.xml could not be read')).toBeTruthy();
    expect(screen.getByText('no About.xml here')).toBeTruthy();
  });
});

describe('BasicsTab editing', () => {
  it('counts an edit, previews it, shows the diff and saves it', async () => {
    const transport = await open();
    fireEvent.input(screen.getByRole('textbox', { name: /Mod name/ }), {
      target: { value: 'Gewehr 41 Remastered' },
    });
    expect(await screen.findByText('1 unsaved change')).toBeTruthy();
    await waitFor(() =>
      expect(transport.calls.some((c) => c.name === 'project_about_preview')).toBe(true),
    );
    const review = screen.getByRole<HTMLButtonElement>('button', { name: 'Review changes' });
    await waitFor(() => expect(review.disabled).toBe(false));
    fireEvent.click(review);
    const dialog = await screen.findByRole('dialog', { name: 'Review changes to About.xml' });
    expect(within(dialog).getByRole('region', { name: 'Changes to About.xml' })).toBeTruthy();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(screen.getByText('No unsaved changes')).toBeTruthy());
    expect(transport.calls.find((c) => c.name === 'project_about_update')).toBeDefined();
    expect(screen.getByText(/^Saved\. The old file is in /)).toBeTruthy();
  });

  it('discards the edits', async () => {
    await open();
    const name = screen.getByRole<HTMLInputElement>('textbox', { name: /Mod name/ });
    const original = name.value;
    fireEvent.input(name, { target: { value: 'Something else' } });
    fireEvent.click(await screen.findByRole('button', { name: 'Discard' }));
    expect(name.value).toBe(original);
    expect(screen.getByText('No unsaved changes')).toBeTruthy();
  });

  it('ticks game versions', async () => {
    await open();
    fireEvent.click(screen.getByRole('checkbox', { name: '1.6' }));
    expect(await screen.findByText('1 unsaved change')).toBeTruthy();
    expect(screen.getByRole<HTMLInputElement>('checkbox', { name: '1.6' }).checked).toBe(true);
  });

  it('adds and removes a person in the authors list and an id in load after', async () => {
    await open();
    const authors = screen.getByRole('textbox', { name: 'Several authors' });
    fireEvent.input(authors, { target: { value: 'Second' } });
    fireEvent.keyDown(authors, { key: 'Enter' });
    expect(
      within(screen.getByRole('list', { name: 'Several authors' })).getByText('Second'),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Remove Ludeon.RimWorld' }));
    expect(await screen.findByText('2 unsaved changes')).toBeTruthy();
  });

  it('adds a dependency from the library search and Combat Extended in one click', async () => {
    const transport = await open();
    fireEvent.click(screen.getByRole('button', { name: 'Add from library' }));
    fireEvent.input(
      screen.getByRole('searchbox', { name: 'Search the library for a dependency' }),
      {
        target: { value: 'combat' },
      },
    );
    fireEvent.click(await screen.findByRole('button', { name: 'Add Combat Extended Armors' }));
    const group = await screen.findByRole('group', { name: 'Dependency Combat Extended Armors' });
    expect(within(group).getByRole<HTMLInputElement>('textbox', { name: /Package id/ }).value).toBe(
      'CETeam.CombatExtendedArmors',
    );
    expect(transport.calls.some((c) => c.name === 'library_mod_search')).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: 'Add from library' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Add Combat Extended' }));
    expect(await screen.findByRole('group', { name: 'Dependency Combat Extended' })).toBeTruthy();
  });

  it('adds a dependency by hand and waits for its package id', async () => {
    await open();
    fireEvent.click(screen.getByRole('button', { name: 'Add by hand' }));
    expect(
      await screen.findByText('Type a package id to include this dependency in the change.'),
    ).toBeTruthy();
    expect(screen.getByText('No unsaved changes')).toBeTruthy();
  });

  it('chooses a preview image with the picker', async () => {
    const transport = await open();
    vi.spyOn(platform, 'pickFile').mockResolvedValue('/home/user/pic.png');
    fireEvent.click(screen.getByRole('button', { name: 'Replace the image' }));
    await waitFor(() =>
      expect(transport.calls.find((c) => c.name === 'project_about_set_preview')?.request).toEqual({
        projectId: ID,
        sourcePath: '/home/user/pic.png',
      }),
    );
    expect(await screen.findByText(/The preview image was added/)).toBeTruthy();
  });

  it('asks before removing the preview image', async () => {
    const transport = await open();
    fireEvent.click(screen.getByRole('button', { name: 'Remove the image' }));
    expect(transport.calls.some((c) => c.name === 'project_about_remove_preview')).toBe(false);
    fireEvent.click(screen.getByRole('button', { name: 'Remove the preview image' }));
    await waitFor(() =>
      expect(transport.calls.some((c) => c.name === 'project_about_remove_preview')).toBe(true),
    );
  });

  it('explains a stale file and reloads on request', async () => {
    const stale = loadFixture<{ error: unknown }>('about-update-stale').error;
    const transport = await open({
      project_about_update: () => {
        throw stale;
      },
    });
    fireEvent.input(screen.getByRole('textbox', { name: /Mod name/ }), {
      target: { value: 'Mine' },
    });
    const save = await screen.findByRole<HTMLButtonElement>('button', { name: 'Save' });
    await waitFor(() => expect(save.disabled).toBe(false));
    fireEvent.click(save);
    expect(await screen.findByText('About.xml changed on disk')).toBeTruthy();
    const reads = transport.calls.filter((c) => c.name === 'project_about_get').length;
    fireEvent.click(screen.getByRole('button', { name: 'Reload' }));
    await waitFor(() =>
      expect(transport.calls.filter((c) => c.name === 'project_about_get').length).toBe(reads + 1),
    );
    expect(screen.getByText('1 unsaved change')).toBeTruthy();
  });
});
