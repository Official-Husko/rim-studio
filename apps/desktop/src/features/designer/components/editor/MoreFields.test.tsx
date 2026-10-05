import { screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { MoreFields } from './MoreFields';

describe('MoreFields', () => {
  it('starts folded without entries and shows the count', () => {
    const { container } = renderWithProviders(
      <MoreFields title="More" count={0}>
        <p>inside</p>
      </MoreFields>,
    );
    expect(screen.getByText('More (0)')).toBeTruthy();
    expect(container.querySelector('details')?.open).toBe(false);
  });

  it('starts open with entries', () => {
    const { container } = renderWithProviders(
      <MoreFields title="More" count={2}>
        <p>inside</p>
      </MoreFields>,
    );
    expect(container.querySelector('details')?.open).toBe(true);
  });
});
