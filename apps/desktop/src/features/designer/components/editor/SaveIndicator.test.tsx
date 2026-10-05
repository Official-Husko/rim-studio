import { screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { SaveIndicator } from './SaveIndicator';

describe('SaveIndicator', () => {
  it.each([
    ['dirty', 'Unsaved changes'],
    ['saved', 'Saved'],
    ['error', 'Save failed'],
  ] as const)('says %s in words', (state, text) => {
    renderWithProviders(<SaveIndicator state={state} />);
    expect(screen.getByText(text)).toBeTruthy();
  });

  it('announces saving as a status', () => {
    renderWithProviders(<SaveIndicator state="saving" />);
    expect(screen.getAllByText('Saving').length).toBeGreaterThan(0);
  });

  it('shows nothing when idle', () => {
    const { container } = renderWithProviders(<SaveIndicator state="idle" />);
    expect(container.textContent).toBe('');
  });
});
