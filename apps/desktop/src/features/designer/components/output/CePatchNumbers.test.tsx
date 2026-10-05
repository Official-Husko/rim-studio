import { screen } from '@testing-library/preact';
import type { CeSuggestionDto } from 'rimstudio-ipc-types';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { fixture } from '../../testSupport';
import { CePatchNumbers } from './CePatchNumbers';

describe('CePatchNumbers', () => {
  it('lists mass, range and timings the patch derives, with ratings', () => {
    const numbers = fixture<CeSuggestionDto>('designer-output-suggest-on').patchNumbers;
    renderWithProviders(<CePatchNumbers numbers={numbers} />);
    expect(screen.getByText('Mass in the patch')).toBeTruthy();
    expect(screen.getByText('3.5')).toBeTruthy();
    expect(screen.getAllByText('Reliable').length).toBeGreaterThan(0);
  });

  it('shows nothing without numbers', () => {
    const { container } = renderWithProviders(<CePatchNumbers numbers={[]} />);
    expect(container.textContent).toBe('');
  });
});
