import { fireEvent, screen, waitFor } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { installTransport } from '../testSupport';
import { ModPicker } from './ModPicker';

function type(text: string): void {
  fireEvent.input(screen.getByRole('searchbox', { name: 'Find' }), { target: { value: text } });
}

describe('ModPicker', () => {
  it('searches after a pause, lists names with package ids and picks one', async () => {
    const transport = installTransport();
    const onPick = vi.fn();
    renderWithProviders(<ModPicker label="Find" onPick={onPick} />);
    type('combat extended');
    const list = await screen.findByRole('list', { name: 'Library results' });
    expect(list.textContent).toContain('CETeam.CombatExtended');
    expect(transport.calls.find((c) => c.name === 'library_mod_search')?.request).toMatchObject({
      query: 'combat extended',
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add Combat Extended' }));
    expect(onPick).toHaveBeenCalledWith(
      expect.objectContaining({ packageId: 'CETeam.CombatExtended' }),
    );
  });

  it('marks mods that are already taken', async () => {
    installTransport();
    renderWithProviders(
      <ModPicker label="Find" onPick={() => undefined} taken={['ceteam.combatextended']} />,
    );
    type('combat');
    const button = await screen.findByRole<HTMLButtonElement>('button', {
      name: 'Add Combat Extended',
    });
    expect(button.disabled).toBe(true);
    expect(button.textContent).toBe('Added');
  });

  it('says when the library has not been scanned and when nothing matches', async () => {
    installTransport({ library_mod_search: () => loadFixture('library-search-unscanned') });
    renderWithProviders(<ModPicker label="Find" onPick={() => undefined} />);
    type('x');
    expect(await screen.findByText(/has not been scanned yet/)).toBeTruthy();
  });

  it('says when nothing matches', async () => {
    installTransport({ library_mod_search: () => loadFixture('library-search-none') });
    renderWithProviders(<ModPicker label="Find" onPick={() => undefined} />);
    type('zzzz');
    await waitFor(() => expect(screen.getByText('No mod matches zzzz')).toBeTruthy());
  });
});
