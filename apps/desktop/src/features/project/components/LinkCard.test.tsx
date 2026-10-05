import { fireEvent, screen, waitFor } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { beforeEach, describe, expect, it } from 'vitest';
import { LinkCard } from './LinkCard';
import {
  installLinkTransport,
  LINK_PROJECT_ID,
  LINK_PROJECT_PATH,
  resultFixture,
  statusFixture,
} from './testSupport';

function show() {
  renderWithProviders(
    <LinkCard projectId={LINK_PROJECT_ID} projectPath={LINK_PROJECT_PATH} projectName="RS Arms" />,
  );
}

describe('LinkCard', () => {
  beforeEach(() => {
    installLinkTransport();
  });

  it('shows the state, the active list hints and the steps for a project that is not linked', async () => {
    installLinkTransport({
      project_link_status: () => statusFixture('link-status-not-linked'),
    });
    show();
    expect(await screen.findByText('Not visible to the game')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Link into the game' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Remove link' })).toBeNull();
    expect(screen.getByText("In the game's active mod list")).toBeTruthy();
    expect(screen.getByText('not yet')).toBeTruthy();
    // this recorded project has a Combat Extended patch, and Combat Extended is in the list
    expect(screen.getByText('Combat Extended in the active mod list')).toBeTruthy();
    expect(screen.getByText(/Enable Combat Extended as well/)).toBeTruthy();
    // nothing went wrong, so no manual command is offered
    expect(screen.queryByText('Or create the link yourself')).toBeNull();
  });

  it('asks for a confirmation before it links, then shows the result', async () => {
    const transport = installLinkTransport({
      project_link_status: () => statusFixture('link-status-not-linked'),
      project_link_create: () => resultFixture('link-create-done'),
    });
    show();
    fireEvent.click(await screen.findByRole('button', { name: 'Link into the game' }));
    expect(transport.calls.filter((c) => c.name === 'project_link_create')).toHaveLength(0);
    fireEvent.click(await screen.findByRole('button', { name: 'Create link' }));
    expect(await screen.findByText('Linked into the game')).toBeTruthy();
    expect(screen.getByText(/The link was created/)).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Remove link' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Link into the game' })).toBeNull();
  });

  it('removes the link', async () => {
    const transport = installLinkTransport({
      project_link_status: () => statusFixture('link-status-linked'),
      project_link_remove: () => resultFixture('link-remove-done'),
    });
    show();
    fireEvent.click(await screen.findByRole('button', { name: 'Remove link' }));
    expect(await screen.findByText(/The link was removed/)).toBeTruthy();
    expect(transport.calls.some((c) => c.name === 'project_link_remove')).toBe(true);
    expect(screen.getByText('Not visible to the game')).toBeTruthy();
  });

  it('shows the refusal and leaves the taken name alone', async () => {
    installLinkTransport({
      project_link_status: () => statusFixture('link-status-not-linked'),
      project_link_create: () => resultFixture('link-create-refused'),
    });
    show();
    fireEvent.click(await screen.findByRole('button', { name: 'Link into the game' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Create link' }));
    expect(await screen.findByText('Nothing was changed')).toBeTruthy();
    expect(screen.getByText(/already exists in the Mods folder/, { selector: 'div' })).toBeTruthy();
    expect(screen.getByText('Name taken by a folder')).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Link into the game' })).toBeNull();
  });

  it('offers the manual command when the Mods folder is missing', async () => {
    installLinkTransport({ project_link_status: () => statusFixture('link-status-no-mods') });
    show();
    expect(await screen.findByText('Cannot link')).toBeTruthy();
    expect(screen.getByText('Or create the link yourself')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Copy command' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Link into the game' })).toBeNull();
  });

  it('offers the manual command after a refusal because the folder is read only', async () => {
    installLinkTransport({
      project_link_status: () => statusFixture('link-status-not-linked'),
      project_link_create: () =>
        resultFixture('link-create-done', {
          state: 'not-linked',
          canCreate: true,
          canRemove: false,
          modsReadOnly: true,
        }),
    });
    show();
    fireEvent.click(await screen.findByRole('button', { name: 'Link into the game' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Create link' }));
    await waitFor(() => expect(screen.queryByText('Or create the link yourself')).not.toBeNull());
  });

  it('shows an error when the status cannot be read', async () => {
    installLinkTransport({
      project_link_status: () => {
        throw new Error('boom');
      },
    });
    show();
    expect(await screen.findByText('The link status could not be read')).toBeTruthy();
  });
});
