import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { press } from 'rimstudio-testkit';
import { Combobox } from './Combobox';

const OPTIONS = [
  { value: 'Gun_Carbine', label: 'Carbine', hint: 'Gun_Carbine' },
  { value: 'Gun_Revolver', label: 'Revolver' },
  { value: 'Gun_Sniper', label: 'Sniper rifle' },
];

describe('Combobox', () => {
  it('has the combobox role and starts collapsed', () => {
    render(
      <Combobox
        aria-label="Anchor"
        options={OPTIONS}
        value="Gun_Revolver"
        onValueChange={() => {}}
      />,
    );
    const input = screen.getByRole('combobox', { name: 'Anchor' }) as HTMLInputElement;
    expect(input.value).toBe('Revolver');
    expect(input.getAttribute('aria-expanded')).toBe('false');
  });

  it('filters by typed text', () => {
    render(
      <Combobox aria-label="Anchor" options={OPTIONS} value={undefined} onValueChange={() => {}} />,
    );
    const input = screen.getByRole('combobox');
    fireEvent.focus(input);
    fireEvent.input(input, { target: { value: 'rifle' } });
    expect(screen.getAllByRole('option')).toHaveLength(1);
    expect(screen.getByRole('option', { name: /Sniper rifle/ })).toBeTruthy();
  });

  it('selects with the keyboard', () => {
    const onValueChange = vi.fn();
    render(
      <Combobox
        aria-label="Anchor"
        options={OPTIONS}
        value={undefined}
        onValueChange={onValueChange}
      />,
    );
    const input = screen.getByRole('combobox');
    fireEvent.focus(input);
    press(input, 'ArrowDown');
    press(input, 'Enter');
    expect(onValueChange).toHaveBeenCalledWith('Gun_Revolver');
  });

  it('closes on Escape without selecting', () => {
    const onValueChange = vi.fn();
    render(
      <Combobox
        aria-label="Anchor"
        options={OPTIONS}
        value={undefined}
        onValueChange={onValueChange}
      />,
    );
    const input = screen.getByRole('combobox');
    fireEvent.focus(input);
    expect(input.getAttribute('aria-expanded')).toBe('true');
    press(input, 'Escape');
    expect(input.getAttribute('aria-expanded')).toBe('false');
    expect(onValueChange).not.toHaveBeenCalled();
  });

  it('selects with the pointer and shows an empty message', () => {
    const onValueChange = vi.fn();
    render(
      <Combobox
        aria-label="Anchor"
        options={OPTIONS}
        value={undefined}
        emptyText="Nothing found"
        onValueChange={onValueChange}
      />,
    );
    const input = screen.getByRole('combobox');
    fireEvent.focus(input);
    fireEvent.mouseDown(screen.getByRole('option', { name: /Carbine/ }));
    expect(onValueChange).toHaveBeenCalledWith('Gun_Carbine');
    fireEvent.focus(input);
    fireEvent.input(input, { target: { value: 'zzz' } });
    expect(screen.getByText('Nothing found')).toBeTruthy();
  });
});
