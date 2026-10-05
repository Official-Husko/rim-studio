import { render, screen, within } from '@testing-library/preact';
import { loadFixture } from 'rimstudio-testkit';
import type { WritePlanDto } from 'rimstudio-ipc-types';
import { describe, expect, it } from 'vitest';
import { DerivedTable } from './DerivedTable';
import { splitDiagnostics } from './model';

describe('DerivedTable', () => {
  it('lists each derived number with its predictor and rating', () => {
    const plan = loadFixture<WritePlanDto>('patches-plan-ready');
    render(<DerivedTable diagnostics={splitDiagnostics(plan.diagnostics).derived} />);
    const grid = screen.getByRole('grid', { name: 'Derived numbers' });
    const range = within(grid).getByText('range').closest('tr') as HTMLElement;
    expect(within(range).getByText('52.92')).toBeTruthy();
    expect(within(range).getByText('Ratio')).toBeTruthy();
    expect(within(range).getByText('rough')).toBeTruthy();
    const mass = within(grid).getByText('mass').closest('tr') as HTMLElement;
    expect(within(mass).getByText('reliable')).toBeTruthy();
  });

  it('shows an empty text', () => {
    render(<DerivedTable diagnostics={[]} />);
    expect(screen.getByText('No derived numbers')).toBeTruthy();
  });
});
