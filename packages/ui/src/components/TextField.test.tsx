import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { TextField } from './TextField';

describe('TextField', () => {
  it('reports typed text', () => {
    const onValueChange = vi.fn();
    render(<TextField aria-label="Label" value="" onValueChange={onValueChange} />);
    fireEvent.input(screen.getByRole('textbox', { name: 'Label' }), {
      target: { value: 'Carbine' },
    });
    expect(onValueChange).toHaveBeenCalledWith('Carbine');
  });

  it('supports multiple lines', () => {
    render(<TextField aria-label="Notes" multiline rows={3} value="a" onValueChange={() => {}} />);
    expect(screen.getByRole('textbox', { name: 'Notes' }).tagName).toBe('TEXTAREA');
  });

  it('marks invalid and read only states', () => {
    render(<TextField aria-label="Id" value="x" invalid readOnly onValueChange={() => {}} />);
    const input = screen.getByRole('textbox', { name: 'Id' }) as HTMLInputElement;
    expect(input.getAttribute('aria-invalid')).toBe('true');
    expect(input.readOnly).toBe(true);
  });

  it('shows prefix and suffix content', () => {
    render(
      <TextField aria-label="Mass" value="1" prefix="~" suffix="kg" onValueChange={() => {}} />,
    );
    expect(screen.getByText('kg')).toBeTruthy();
    expect(screen.getByText('~')).toBeTruthy();
  });
});
