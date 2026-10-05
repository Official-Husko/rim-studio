import { fireEvent, screen } from '@testing-library/preact';
import type { ArchetypeCatalogDto, CeCalibreDto } from 'rimstudio-ipc-types';
import { fixture } from '../../testSupport';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { CalibreControl, type CalibreControlProps } from './CalibreControl';
import { findArchetype } from './wizard-model';
import { catalogFixture } from './wizardTestSupport';

function props(over: Partial<CalibreControlProps> = {}): CalibreControlProps {
  const catalog = catalogFixture();
  const archetype = findArchetype(catalog, 'rifle/assault');
  if (!archetype) throw new Error('missing');
  return {
    archetype,
    catalog,
    calibre: 'medium',
    ammoSet: undefined,
    ceCalibre: false,
    ceCalibres: undefined,
    onCalibre: vi.fn(),
    onAmmoSet: vi.fn(),
    onUseCe: vi.fn(),
    ...over,
  };
}

describe('CalibreControl', () => {
  it('offers the calibre classes of the type and explains the chosen one', () => {
    const p = props();
    renderWithProviders(<CalibreControl {...p} />);
    expect(screen.getByText(/intermediate rifle rounds/)).toBeTruthy();
    fireEvent.click(screen.getByRole('radio', { name: 'Large' }));
    expect(p.onCalibre).toHaveBeenCalledWith('large');
  });

  it('disables the Combat Extended calibre when Combat Extended is not loaded', () => {
    const catalog: ArchetypeCatalogDto = {
      ...catalogFixture(),
      ce: {
        available: false,
        reason: 'No Combat Extended mod found.',
        calibres: [],
        aiClassTags: [],
      },
    };
    renderWithProviders(<CalibreControl {...props({ catalog })} />);
    expect((screen.getByRole('switch') as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText('No Combat Extended mod found.')).toBeTruthy();
  });

  it('lists the real ammo sets, the ones that suit the type first, and picks one', () => {
    const ceCalibres = fixture<ArchetypeCatalogDto>('designer-wizard-catalog-ce').ce
      .calibres as CeCalibreDto[];
    const p = props({ ceCalibre: true, ceCalibres });
    renderWithProviders(<CalibreControl {...p} />);
    const box = screen.getByRole('combobox', { name: 'Combat Extended ammo set' });
    fireEvent.focus(box);
    fireEvent.input(box, { target: { value: 'a' } });
    expect(screen.getAllByRole('option').length).toBeGreaterThan(100);
    fireEvent.input(box, { target: { value: '5.56' } });
    fireEvent.mouseDown(screen.getByRole('option', { name: /AmmoSet_556x45mmNATO, damage 14/ }));
    expect(p.onAmmoSet).toHaveBeenCalledWith('AmmoSet_556x45mmNATO');
    expect(screen.getByText(/the Combat Extended patch stays off/)).toBeTruthy();
  });

  it('asks for an ammo set before going on', () => {
    const ceCalibres = fixture<ArchetypeCatalogDto>('designer-wizard-catalog-ce').ce
      .calibres as CeCalibreDto[];
    renderWithProviders(<CalibreControl {...props({ ceCalibre: true, ceCalibres })} />);
    expect(
      screen.getByText('Pick an ammo set to go on, or switch back to a calibre class.'),
    ).toBeTruthy();
  });

  it('shows a loading state while the ammo sets load', () => {
    renderWithProviders(<CalibreControl {...props({ ceCalibre: true })} />);
    expect(screen.getAllByText('Loading the weapon types').length).toBeGreaterThan(0);
  });
});
