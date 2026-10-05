import { screen } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import type { ProjectLayoutFixPlanDto } from 'rimstudio-ipc-types';
import { FixConfirm } from './FixConfirm';

const plan = loadFixture<ProjectLayoutFixPlanDto>('layout-fix-plan-conflict');

describe('FixConfirm', () => {
  it('names what moves where and where the undo journal goes', () => {
    renderWithProviders(<FixConfirm items={plan.items} rename={[]} folder="/mods/Gewehr" />);
    expect(screen.getByText('This changes 4 things in /mods/Gewehr:')).toBeTruthy();
    expect(
      screen.getByText('Patches/ce_patch.xml moves to Compat/CombatExtended/Patches/ce_patch.xml'),
    ).toBeTruthy();
    expect(screen.getByText('LoadFolders.xml is created')).toBeTruthy();
    expect(screen.getByText(/undo journal to its data folder/)).toBeTruthy();
  });

  it('shows the numbered name of a renamed move', () => {
    const move = plan.items.find((i) => i.conflict?.destinationExists);
    if (!move) throw new Error('fixture has a conflict');
    renderWithProviders(<FixConfirm items={[move]} rename={[move.id]} folder="/m" />);
    expect(screen.getByText(/ce_patch_2\.xml/)).toBeTruthy();
    expect(screen.getByText('(under a numbered name)')).toBeTruthy();
  });
});
