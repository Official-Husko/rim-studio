import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { MoveButtons } from './MoveButtons';

describe('MoveButtons', () => {
  it('moves up and down and disables the ends', () => {
    const onUp = vi.fn();
    const onDown = vi.fn();
    renderWithProviders(
      <MoveButtons name="Core" canUp={false} canDown onUp={onUp} onDown={onDown} />,
    );
    expect(screen.getByRole<HTMLButtonElement>('button', { name: 'Move Core up' }).disabled).toBe(
      true,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Move Core down' }));
    expect(onDown).toHaveBeenCalled();
    expect(onUp).not.toHaveBeenCalled();
  });
});
