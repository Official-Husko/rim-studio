import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { BalanceControl } from './BalanceControl';
import { catalogFixture } from './wizardTestSupport';

describe('BalanceControl', () => {
  it('offers weaker, typical and stronger and explains the chosen one', () => {
    const onChange = vi.fn();
    renderWithProviders(
      <BalanceControl catalog={catalogFixture()} value="typical" onChange={onChange} />,
    );
    expect(screen.getByText('Typical for its class')).toBeTruthy();
    fireEvent.click(screen.getByRole('radio', { name: 'Stronger' }));
    expect(onChange).toHaveBeenCalledWith('stronger');
  });
});
