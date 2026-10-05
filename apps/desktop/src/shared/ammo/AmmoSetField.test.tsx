import { fireEvent, render, screen, waitFor, within } from '@testing-library/preact';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AmmoSetField, type AmmoSetFieldProps } from './AmmoSetField';
import { catalogSlice, customAmmo, installAmmoTransport } from './testSupport';
import { clearAmmoSummaries } from './useAmmoSummary';

function field(props: Partial<AmmoSetFieldProps> = {}) {
  const onSelect = vi.fn();
  const onCustomChange = vi.fn();
  render(
    <AmmoSetField
      label="Which caliber (ammo set) does the weapon use?"
      value={undefined}
      onSelect={onSelect}
      onCustomChange={onCustomChange}
      {...props}
    />,
  );
  return { onSelect, onCustomChange };
}

describe('AmmoSetField', () => {
  beforeEach(() => {
    clearAmmoSummaries();
    installAmmoTransport();
  });

  it('says nothing is chosen and never chooses for the user', () => {
    const { onSelect } = field({ reason: 'it is never chosen for you' });
    expect(screen.getByText('it is never chosen for you')).toBeTruthy();
    expect(onSelect).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'Browse all ammo' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Create custom ammo' })).toBeTruthy();
  });

  it('shows the chosen set with its caliber and ammo types', async () => {
    const entry = catalogSlice().entries.find((e) => e.weaponCount > 0);
    field({ value: entry?.defName });
    const chosen = document.querySelector('[data-ammo-chosen]') as HTMLElement;
    expect(chosen.textContent).toContain(entry?.defName);
    await waitFor(() => expect(chosen.textContent).toContain(entry?.caliber));
    for (const type of entry?.types ?? []) {
      expect(within(chosen).getAllByText(type.ammoClassLabel).length).toBeGreaterThan(0);
    }
  });

  it('keeps the ranked suggestions as quick picks and marks the chosen one', () => {
    const { onSelect } = field({
      value: 'AmmoSet_280British',
      quickPicks: [
        { name: 'AmmoSet_280British', hint: 'used by 2 converted guns' },
        { name: 'AmmoSet_303British' },
      ],
    });
    const picks = screen.getByRole('group', { name: 'Quick picks, best fit first' });
    const buttons = within(picks).getAllByRole('button');
    expect(buttons[0]?.getAttribute('aria-pressed')).toBe('true');
    expect(buttons[0]?.textContent).toBe('AmmoSet_280British (used by 2 converted guns)');
    fireEvent.click(buttons[1] as HTMLElement);
    expect(onSelect).toHaveBeenCalledWith('AmmoSet_303British');
  });

  it('opens the browser, chooses a set from it and closes it', async () => {
    const { onSelect } = field();
    fireEvent.click(screen.getByRole('button', { name: 'Browse all ammo' }));
    const dialog = await screen.findByRole('dialog', { name: 'Combat Extended ammunition' });
    await within(dialog).findByText('22 ammo sets');
    fireEvent.click(within(dialog).getByRole('button', { name: 'Select this set' }));
    expect(onSelect).toHaveBeenCalledWith(catalogSlice().entries[0]?.defName);
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });

  it('opens the custom ammo window from the field and from the browser', async () => {
    field();
    fireEvent.click(screen.getByRole('button', { name: 'Create custom ammo' }));
    expect(await screen.findByRole('dialog', { name: 'Create custom ammunition' })).toBeTruthy();
  });

  it('opens the custom ammo window from the browser', async () => {
    field();
    fireEvent.click(screen.getByRole('button', { name: 'Browse all ammo' }));
    const browser = await screen.findByRole('dialog', { name: 'Combat Extended ammunition' });
    fireEvent.click(within(browser).getByRole('button', { name: 'Create custom ammo' }));
    expect(await screen.findByRole('dialog', { name: 'Create custom ammunition' })).toBeTruthy();
    expect(screen.queryByRole('dialog', { name: 'Combat Extended ammunition' })).toBeNull();
  });

  it('shows the custom ammo in place of a set, and edits or removes it', async () => {
    const { onCustomChange } = field({ custom: customAmmo(), value: 'AmmoSet_Ignored' });
    const card = document.querySelector('[data-ammo-custom]') as HTMLElement;
    expect(card.textContent).toContain(customAmmo().name);
    expect(document.querySelector('[data-ammo-chosen]')).toBeNull();
    expect(screen.queryByRole('group', { name: 'Quick picks, best fit first' })).toBeNull();
    fireEvent.click(within(card).getByRole('button', { name: 'Edit custom ammo' }));
    const edit = await screen.findByRole('dialog', { name: 'Edit custom ammunition' });
    expect(within(edit).getByRole('button', { name: 'Remove custom ammo' })).toBeTruthy();
    fireEvent.click(within(edit).getByRole('button', { name: 'Remove custom ammo' }));
    expect(onCustomChange).toHaveBeenCalledWith(undefined);
  });

  it('does not offer a new custom caliber when it is switched off and says why in the browser', async () => {
    field({ canCreate: false, createHint: 'Open a project to create it.' });
    expect(screen.queryByRole('button', { name: 'Create custom ammo' })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Browse all ammo' }));
    expect(await screen.findByText('Open a project to create it.')).toBeTruthy();
  });

  it('clears the choice', () => {
    const { onSelect } = field({ value: 'AmmoSet_280British' });
    fireEvent.click(screen.getByRole('button', { name: 'Clear the choice' }));
    expect(onSelect).toHaveBeenCalledWith(undefined);
  });
});
