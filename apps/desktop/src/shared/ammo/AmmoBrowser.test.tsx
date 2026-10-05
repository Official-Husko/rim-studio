import { fireEvent, render, screen, waitFor, within } from '@testing-library/preact';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AmmoBrowser } from './AmmoBrowser';
import { createCatalogStore } from './catalogStore';
import { catalogSlice, installAmmoTransport, manyEntries, pagedCatalog } from './testSupport';
import { clearAmmoSummaries } from './useAmmoSummary';

function open(props: Partial<Parameters<typeof AmmoBrowser>[0]> = {}) {
  const store = createCatalogStore();
  const onSelect = vi.fn();
  const onClose = vi.fn();
  render(
    <AmmoBrowser
      open
      store={store}
      onSelect={onSelect}
      onClose={onClose}
      searchDelayMs={0}
      {...props}
    />,
  );
  return { store, onSelect, onClose };
}

describe('AmmoBrowser', () => {
  beforeEach(() => clearAmmoSummaries());

  it('lists every ammo set with its numbers and weapons, and selects one', async () => {
    installAmmoTransport();
    const { onSelect } = open({ onCreateCustom: vi.fn() });
    const list = await screen.findByRole('listbox', { name: 'Ammo sets' });
    expect(await within(list).findAllByRole('option')).not.toHaveLength(0);
    expect(screen.getByText('22 ammo sets')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Create custom ammo' })).toBeTruthy();
    const target = catalogSlice().entries.find((e) => e.weaponCount > 0);
    fireEvent.click(within(list).getByText(target?.label as string));
    const detail = document.querySelector(`[data-ammo-detail="${target?.defName}"]`) as HTMLElement;
    expect(within(detail).getByRole('table', { name: 'Ammo types' })).toBeTruthy();
    expect(within(detail).getByText(/converted weapons? use/)).toBeTruthy();
    fireEvent.click(within(detail).getByRole('button', { name: 'Select this set' }));
    expect(onSelect).toHaveBeenCalledWith(target?.defName);
  });

  it('marks the chosen set and does not select it again', async () => {
    installAmmoTransport();
    const first = catalogSlice().entries[0];
    open({ current: first?.defName });
    expect(await screen.findByRole('button', { name: 'Chosen' })).toHaveProperty('disabled', true);
  });

  it('stays light with hundreds of sets: only the rows in view are drawn', async () => {
    installAmmoTransport({ designer_ce_ammo_catalog: pagedCatalog(manyEntries(450)) });
    open();
    expect(await screen.findByText('450 ammo sets', undefined, { timeout: 3000 })).toBeTruthy();
    const options = within(screen.getByRole('listbox', { name: 'Ammo sets' })).getAllByRole(
      'option',
    );
    expect(options.length).toBeLessThan(40);
    expect(options[0]?.getAttribute('aria-setsize')).toBe('450');
  });

  it('asks again with the words typed and shows an empty state with a way back', async () => {
    const transport = installAmmoTransport({
      designer_ce_ammo_catalog: (request) =>
        (request as { query?: string }).query
          ? { ...catalogSlice(), matching: 0, entries: [] }
          : catalogSlice(),
    });
    open();
    await screen.findByText('22 ammo sets');
    fireEvent.input(screen.getByRole('searchbox', { name: 'Search ammo sets' }), {
      target: { value: 'zzz' },
    });
    expect(await screen.findByText('No ammo set matches')).toBeTruthy();
    expect(transport.calls.at(-1)?.request).toMatchObject({ query: 'zzz' });
    fireEvent.click(screen.getByRole('button', { name: 'Clear the filters' }));
    expect(await screen.findByText('22 ammo sets')).toBeTruthy();
  });

  it('filters by the weapons that use a set', async () => {
    installAmmoTransport();
    open();
    await screen.findByText('22 ammo sets');
    fireEvent.click(screen.getByRole('checkbox', { name: 'Used by weapons' }));
    const used = catalogSlice().entries.filter((e) => e.weaponCount > 0).length;
    await waitFor(() => expect(screen.getByText(`${used} of 22 ammo sets`)).toBeTruthy());
  });

  it('says plainly when Combat Extended is not installed', async () => {
    installAmmoTransport({
      designer_ce_ammo_catalog: () => ({
        ...catalogSlice(),
        available: false,
        reason: 'No Combat Extended data is loaded.',
        entries: [],
      }),
    });
    open();
    expect(await screen.findByText('Combat Extended is not loaded')).toBeTruthy();
    expect(screen.getByText('No Combat Extended data is loaded.')).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Create custom ammo' })).toBeNull();
  });

  it('shows the first read as loading and a failed read with a retry', async () => {
    let fail = true;
    installAmmoTransport({
      designer_ce_ammo_catalog: async () => {
        await new Promise((r) => setTimeout(r, 5));
        if (fail) throw { code: 'designer.failed', message: 'It broke.', errorId: 'e-1' };
        return catalogSlice();
      },
    });
    open();
    expect(await screen.findByText(/The first read can take a few seconds/)).toBeTruthy();
    expect(await screen.findByText('It broke.')).toBeTruthy();
    fail = false;
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(await screen.findByText('22 ammo sets')).toBeTruthy();
  });

  it('explains why custom ammunition is not offered', async () => {
    installAmmoTransport();
    open({ createHint: 'Open a project first.' });
    expect(await screen.findByText('Open a project first.')).toBeTruthy();
  });
});
