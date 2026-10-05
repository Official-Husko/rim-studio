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

  it('uses the selected look without the surface look, so the two never compete', () => {
    render(
      <Card selected onSelect={() => undefined} label="On">
        On
      </Card>,
    );
    render(
      <Card onSelect={() => undefined} label="Off">
        Off
      </Card>,
    );
    const on = screen.getByRole('button', { name: 'On' }).className;
    const off = screen.getByRole('button', { name: 'Off' }).className;
    expect(on).toContain('bg-accent-tint');
    expect(on).not.toContain('bg-surface');
    expect(off).toContain('bg-surface');
    expect(off).not.toContain('bg-accent-tint');
  });
});
