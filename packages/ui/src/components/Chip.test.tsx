import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { Chip } from './Chip';

describe('Chip', () => {
  it('uses the default text of its kind', () => {
    render(<Chip kind="suggested" />);
    expect(screen.getByText('Suggested')).toBeTruthy();
  });

  it('allows custom text', () => {
    render(<Chip kind="anchor">Anchor: Assault rifle</Chip>);
    expect(screen.getByText('Anchor: Assault rifle')).toBeTruthy();
  });

  it('shows a letter for the four source kinds', () => {
    const { container } = render(<Chip kind="derived" />);
    expect(container.textContent).toContain('D');
  });

  it('removes through an accessible button', () => {
    const onRemove = vi.fn();
    render(
      <Chip removeLabel="Remove tag melee" onRemove={onRemove}>
        melee
      </Chip>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Remove tag melee' }));
    expect(onRemove).toHaveBeenCalled();
  });
});
