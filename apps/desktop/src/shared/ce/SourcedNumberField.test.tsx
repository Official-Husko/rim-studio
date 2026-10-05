import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { SourcedNumberField } from './SourcedNumberField';

describe('SourcedNumberField', () => {
  it('shows the held value and reports a typed number', () => {
    const onChange = vi.fn();
    render(
      <SourcedNumberField
        label="Mass"
        value={{ value: 1.5, source: 'suggested' }}
        onChange={onChange}
      />,
    );
    const box = screen.getByRole('spinbutton', { name: 'Mass' }) as HTMLInputElement;
    expect(box.value).toBe('1.5');
    fireEvent.input(box, { target: { value: '2.25' } });
    fireEvent.blur(box);
    expect(onChange).toHaveBeenLastCalledWith({ value: 2.25, source: 'typed' });
  });

  it('rounds a whole number field', () => {
    const onChange = vi.fn();
    render(<SourcedNumberField label="Per magazine" whole value={undefined} onChange={onChange} />);
    const box = screen.getByRole('spinbutton', { name: 'Per magazine' });
    fireEvent.input(box, { target: { value: '29.6' } });
    fireEvent.blur(box);
    expect(onChange).toHaveBeenLastCalledWith({ value: 30, source: 'typed' });
  });

  it('clears to undefined', () => {
    const onChange = vi.fn();
    render(
      <SourcedNumberField
        label="Per magazine"
        whole
        value={{ value: 30, source: 'typed' }}
        onChange={onChange}
      />,
    );
    const box = screen.getByRole('spinbutton', { name: 'Per magazine' });
    fireEvent.input(box, { target: { value: '' } });
    fireEvent.blur(box);
    expect(onChange).toHaveBeenLastCalledWith(undefined);
  });
});
