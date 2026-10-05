import { screen } from '@testing-library/preact';
import type { WritePlanDto } from 'rimstudio-ipc-types';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { isDerived } from '../../output-model';
import { fixture } from '../../testSupport';
import { DerivedValues } from './DerivedValues';

describe('DerivedValues', () => {
  it('lists each derived number with where it came from', () => {
    const plan = fixture<WritePlanDto>('designer-output-plan-ce-ready');
    const derived = plan.diagnostics.filter(isDerived);
    renderWithProviders(<DerivedValues derived={derived} />);
    expect(screen.getByText(`${derived.length} values`)).toBeTruthy();
    expect(screen.getByText('/ce/bulk = 9.506', { exact: false })).toBeTruthy();
  });

  it('shows nothing when no number was derived', () => {
    const { container } = renderWithProviders(<DerivedValues derived={[]} />);
    expect(container.textContent).toBe('');
  });
});
