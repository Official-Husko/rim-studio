import { screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import type { PreviewDto } from 'rimstudio-ipc-types';
import { fixture } from '../../testSupport';
import { ReadoutsPanel } from './ReadoutsPanel';

describe('ReadoutsPanel', () => {
  it('shows the real readouts of the edited clone', () => {
    const preview = fixture<PreviewDto>('designer_preview');
    renderWithProviders(<ReadoutsPanel readouts={preview.readouts} busy={false} />);
    expect(screen.getByText('Cycle time')).toBeTruthy();
    expect(screen.getAllByText('6.875').length).toBeGreaterThan(0);
    expect(screen.getByText('Hit adjusted DPS at 12 tiles')).toBeTruthy();
    expect(screen.getAllByText('How it is computed').length).toBeGreaterThan(0);
  });

  it('shows a dash for a readout that cannot be computed yet', () => {
    const preview = fixture<PreviewDto>('designer-preview-melee');
    const blank = {
      ...preview,
      readouts: [{ key: 'melee-dps', group: 'melee' as const, unit: 'damage-per-second' as const }],
    };
    renderWithProviders(<ReadoutsPanel readouts={blank.readouts} busy />);
    expect(screen.getByText('-')).toBeTruthy();
    expect(screen.getByText('Panel DPS')).toBeTruthy();
  });

  it('waits for the first preview', () => {
    renderWithProviders(<ReadoutsPanel readouts={undefined} busy={false} />);
    expect(screen.getByText(/appear after the first preview/)).toBeTruthy();
  });
});
