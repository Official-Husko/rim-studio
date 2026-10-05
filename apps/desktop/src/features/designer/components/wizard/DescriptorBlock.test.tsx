import { screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { DescriptorBlock } from './DescriptorBlock';

describe('DescriptorBlock', () => {
  it('names its control and shows what the choice does', () => {
    renderWithProviders(
      <DescriptorBlock title="Action" help="One round per cycle.">
        <button type="button">Bolt</button>
      </DescriptorBlock>,
    );
    expect(screen.getByRole('region', { name: 'Action' })).toBeTruthy();
    expect(screen.getByText('One round per cycle.')).toBeTruthy();
  });
});
