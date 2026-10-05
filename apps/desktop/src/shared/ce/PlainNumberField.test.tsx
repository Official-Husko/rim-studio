import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { PlainNumberField } from './PlainNumberField';

describe('PlainNumberField', () => {
  it('reports a number and rounds a whole one', () => {
    const onChange = vi.fn();
    render(<PlainNumberField label="Burst shots" whole value={undefined} onChange={onChange} />);
    const box = screen.getByRole('spinbutton', { name: 'Burst shots' });
    fireEvent.input(box, { target: { value: '2.6' } });
    fireEvent.blur(box);
    expect(onChange).toHaveBeenLastCalledWith(3);
  });

  it('clears to undefined', () => {
    const onChange = vi.fn();
    render(<PlainNumberField label="Burst shots" value={4} onChange={onChange} />);
    const box = screen.getByRole('spinbutton', { name: 'Burst shots' });
    fireEvent.input(box, { target: { value: '' } });
    fireEvent.blur(box);
    expect(onChange).toHaveBeenLastCalledWith(undefined);
  });
});
