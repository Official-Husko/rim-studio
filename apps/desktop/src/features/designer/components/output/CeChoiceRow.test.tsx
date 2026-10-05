import { fireEvent, screen } from '@testing-library/preact';
import type { CeChoiceDto, CeSuggestionDto } from 'rimstudio-ipc-types';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { fixture } from '../../testSupport';
import { CeChoiceRow } from './CeChoiceRow';

const choice = (name: string, from = 'designer-output-suggest-on'): CeChoiceDto => {
  const found = fixture<CeSuggestionDto>(from).choices.find((c) => c.field === name);
  if (!found) throw new Error(name);
  return found;
};

describe('CeChoiceRow', () => {
  it('offers the ranked ammo sets and never picks one', () => {
    const onChoose = vi.fn();
    renderWithProviders(
      <CeChoiceRow
        choice={choice('/ce/ammoSet')}
        value={undefined}
        accepted={false}
        onAccept={() => {}}
        onChoose={onChoose}
      />,
    );
    const select = screen.getByRole('combobox', { name: /Which caliber/ }) as HTMLSelectElement;
    expect(select.value).toBe('');
    expect(screen.getByText(/never chosen for you/)).toBeTruthy();
    const names = Array.from(select.options).map((o) => o.value);
    expect(names.slice(1, 3)).toEqual(['AmmoSet_25x40mmGrenade', 'AmmoSet_280British']);
    fireEvent.change(select, { target: { value: 'AmmoSet_280British' } });
    expect(onChoose).toHaveBeenCalledWith('AmmoSet_280British');
  });

  it('names how many converted guns use each candidate', () => {
    renderWithProviders(
      <CeChoiceRow
        choice={choice('/ce/weaponTagClass')}
        value="CE_AI_BROOM"
        accepted={false}
        onAccept={() => {}}
        onChoose={() => {}}
      />,
    );
    expect(
      screen.getByRole('option', { name: 'CE_AI_BROOM (used by 6 converted guns)' }),
    ).toBeTruthy();
    expect(
      screen.getByRole('option', { name: 'CE_AI_LMG (used by 1 converted gun)' }),
    ).toBeTruthy();
  });

  it('offers the default projectile that follows the chosen ammo set', () => {
    const onAccept = vi.fn();
    renderWithProviders(
      <CeChoiceRow
        choice={choice('/ce/defaultProjectile', 'designer-output-suggest-answered')}
        value={undefined}
        accepted={false}
        onAccept={onAccept}
        onChoose={() => {}}
      />,
    );
    fireEvent.click(screen.getByRole('checkbox', { name: 'Use Bullet_303British_FMJ_SB' }));
    expect(onAccept).toHaveBeenCalledWith(true);
    expect(screen.getByText('first of the ammo set')).toBeTruthy();
  });

  it('writes a flag as a boolean', () => {
    const onChoose = vi.fn();
    renderWithProviders(
      <CeChoiceRow
        choice={choice('/ce/beltFed')}
        value={false}
        accepted={false}
        onAccept={() => {}}
        onChoose={onChoose}
      />,
    );
    fireEvent.click(screen.getByRole('checkbox', { name: 'Is the weapon belt fed?' }));
    expect(onChoose).toHaveBeenCalledWith(true);
  });
});
