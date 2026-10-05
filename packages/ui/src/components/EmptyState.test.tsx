import { render, screen } from '@testing-library/preact';
import { describe, expect, it } from 'vitest';
import { EmptyState } from './EmptyState';

describe('EmptyState', () => {
  it('shows a heading, a description and an action', () => {
    render(
      <EmptyState
        title="No drafts yet"
        description="Start from a clone."
        action={<button type="button">New draft</button>}
      />,
    );
    expect(screen.getByRole('heading', { name: 'No drafts yet' })).toBeTruthy();
    expect(screen.getByText('Start from a clone.')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'New draft' })).toBeTruthy();
  });

  it('drops the grid canvas in compact mode', () => {
    const { container } = render(<EmptyState title="Nothing" compact />);
    expect(container.firstElementChild?.getAttribute('class')).not.toContain('bp-grid');
  });
});
