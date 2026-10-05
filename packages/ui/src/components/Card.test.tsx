import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { press } from 'rimstudio-testkit';
import { Card } from './Card';

describe('Card', () => {
  it('renders a static block', () => {
    render(<Card>Plain content</Card>);
    expect(screen.getByText('Plain content')).toBeTruthy();
    expect(screen.queryByRole('button')).toBeNull();
  });

  it('is a toggle button when selectable', () => {
    const onSelect = vi.fn();
    render(
      <Card selected onSelect={onSelect} label="Anchor card">
        Assault rifle
      </Card>,
    );
    const card = screen.getByRole('button', { name: 'Anchor card' });
    expect(card.getAttribute('aria-pressed')).toBe('true');
    fireEvent.click(card);
    expect(onSelect).toHaveBeenCalledTimes(1);
    press(card, 'Enter');
  });
});
