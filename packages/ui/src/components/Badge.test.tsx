import { render, screen } from '@testing-library/preact';
import { describe, expect, it } from 'vitest';
import { Badge } from './Badge';

describe('Badge', () => {
  it('shows its text', () => {
    render(<Badge tone="warning">Unusual</Badge>);
    expect(screen.getByText('Unusual')).toBeTruthy();
  });

  it('maps each tone to a token class', () => {
    const { container } = render(<Badge tone="danger">Error</Badge>);
    expect(container.firstElementChild?.getAttribute('class')).toContain('text-danger');
  });
});
