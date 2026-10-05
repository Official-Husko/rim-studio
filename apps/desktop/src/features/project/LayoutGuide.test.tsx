import { screen, within } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { LayoutGuide } from './LayoutGuide';

describe('LayoutGuide', () => {
  it('lists every role of the layout with where it lives', () => {
    renderWithProviders(<LayoutGuide />);
    const list = screen.getByRole('list', { name: 'Folder roles' });
    expect(within(list).getAllByRole('listitem')).toHaveLength(13);
    expect(screen.getByText('Compat/CombatExtended/Patches/')).toBeTruthy();
    expect(screen.getByText('Defs/ThingDefs_Misc/Weapons/<Category>/<DefName>.xml')).toBeTruthy();
  });

  it('states the rules that matter for the designer', () => {
    renderWithProviders(<LayoutGuide />);
    expect(screen.getByText(/Definitions are always vanilla/)).toBeTruthy();
    expect(screen.getByText(/A new weapon is a new file/)).toBeTruthy();
  });
});
