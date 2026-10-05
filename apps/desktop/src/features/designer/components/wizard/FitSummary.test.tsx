import { screen } from '@testing-library/preact';
import type { ArchetypeProposalDto } from 'rimstudio-ipc-types';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { fixture } from '../../testSupport';
import { FitSummary } from './FitSummary';

describe('FitSummary', () => {
  it('shows the verdict, where it lands and the class it was compared with', () => {
    renderWithProviders(
      <FitSummary proposal={fixture<ArchetypeProposalDto>('designer-wizard-propose-sniper')} />,
    );
    expect(screen.getByRole('progressbar', { name: 'Typicality' })).toBeTruthy();
    expect(screen.getByText(/50 out of 100, where 100 is the strongest/)).toBeTruthy();
    expect(screen.getByText(/within what your install has/)).toBeTruthy();
    expect(screen.getByText(/Compared with: based on 12 items at Industrial/)).toBeTruthy();
    expect(screen.getByText(/typical, .* plausible, .* unusual/)).toBeTruthy();
  });

  it('says plainly when few reference weapons stand behind the comparison', () => {
    renderWithProviders(
      <FitSummary proposal={fixture<ArchetypeProposalDto>('designer-wizard-propose-thin')} />,
    );
    expect(screen.getByText('Rough comparison')).toBeTruthy();
    expect(screen.getByText(/3 weapons of your install stand behind it/)).toBeTruthy();
    expect(screen.getByText(/widened past the tier/)).toBeTruthy();
  });

  it('warns when the target was out of reach', () => {
    const proposal = fixture<ArchetypeProposalDto>('designer-wizard-propose-sniper');
    renderWithProviders(
      <FitSummary proposal={{ ...proposal, strength: { ...proposal.strength, clamped: true } }} />,
    );
    expect(screen.getByText('Out of reach')).toBeTruthy();
  });
});
