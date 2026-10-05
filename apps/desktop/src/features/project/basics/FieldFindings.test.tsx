import { screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { FieldFindings } from './FieldFindings';

describe('FieldFindings', () => {
  it('shows each finding with its severity in words', () => {
    renderWithProviders(
      <FieldFindings
        items={[
          { code: 'a', severity: 'error', message: 'bad id' },
          { code: 'b', severity: 'hint', message: 'use lower case' },
        ]}
      />,
    );
    expect(screen.getByText('Error')).toBeTruthy();
    expect(screen.getByText('bad id')).toBeTruthy();
    expect(screen.getByText('Hint')).toBeTruthy();
  });

  it('renders nothing without findings', () => {
    const { container } = renderWithProviders(<FieldFindings items={[]} />);
    expect(container.innerHTML).toBe('');
  });
});
