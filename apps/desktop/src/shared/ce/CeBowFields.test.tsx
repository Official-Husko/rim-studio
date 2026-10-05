import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { CeBowFields } from './CeBowFields';

const BASE = { oneHanded: false, beltFed: false };

describe('CeBowFields', () => {
  it('forces the bow choice and returns to the shape on the first option', () => {
    const onChange = vi.fn();
    render(<CeBowFields block={{ ...BASE, bow: true }} onChange={onChange} />);
    const group = screen.getByRole('radiogroup', { name: 'Treat as a bow' });
    const radios = group.querySelectorAll('[role="radio"]');
    fireEvent.click(radios[0] as HTMLElement);
    expect(onChange).toHaveBeenLastCalledWith({ bow: undefined });
    fireEvent.click(radios[2] as HTMLElement);
    expect(onChange).toHaveBeenLastCalledWith({ bow: false });
  });

  it('writes the ammo per magazine as a typed whole number', () => {
    const onChange = vi.fn();
    render(<CeBowFields block={BASE} onChange={onChange} />);
    const box = screen.getByRole('spinbutton', { name: 'Ammo generated per magazine' });
    fireEvent.input(box, { target: { value: '40' } });
    fireEvent.blur(box);
    expect(onChange).toHaveBeenLastCalledWith({ ammoGenPerMag: { value: 40, source: 'typed' } });
  });

  it('sets and clears the recoil pattern', () => {
    const onChange = vi.fn();
    render(<CeBowFields block={{ ...BASE, recoilPattern: 'Regular' }} onChange={onChange} />);
    expect(screen.getByText('Regular')).toBeTruthy();
    fireEvent.input(screen.getByRole('textbox', { name: 'Recoil pattern' }), {
      target: { value: '' },
    });
    expect(onChange).toHaveBeenLastCalledWith({ recoilPattern: undefined });
  });
});
