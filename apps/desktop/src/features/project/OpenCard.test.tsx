import { fireEvent, screen, waitFor } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { beforeEach, describe, expect, it } from 'vitest';
import { FolderPickerHost } from '~/shared/platform';
import { OpenCard } from './OpenCard';
import { openError } from './store';
import { installTransport } from './testSupport';

beforeEach(() => {
  installTransport();
});

describe('OpenCard', () => {
  it('offers the picker and a shortcut into each mod folder of the sources', async () => {
    renderWithProviders(<OpenCard />);
    expect(screen.getByRole('button', { name: 'Choose a mod folder' })).toBeTruthy();
    // the workshop and the game data folder are not places to keep a project
    expect(await screen.findByRole('button', { name: 'Browse in Game mods' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Browse in Workshop' })).toBeNull();
  });

  it('starts the picker inside the chosen source', async () => {
    renderWithProviders(
      <>
        <OpenCard />
        <FolderPickerHost />
      </>,
    );
    fireEvent.click(await screen.findByRole('button', { name: 'Browse in Game mods' }));
    expect(await screen.findByRole('dialog')).toBeTruthy();
    await waitFor(() => expect(screen.getAllByText(/Mods/).length).toBeGreaterThan(0));
  });

  it('explains a folder that is not a mod', () => {
    openError.value = { code: 'io.not-found', message: 'x is not a mod project', errorId: 'e-1' };
    renderWithProviders(<OpenCard />);
    expect(screen.getByText('That folder could not be opened')).toBeTruthy();
    expect(screen.getByText(/has no About\/About\.xml file/)).toBeTruthy();
  });
});
