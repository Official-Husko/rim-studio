import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { CeUnderBarrelFields } from './CeUnderBarrelFields';

const BASE = { oneHanded: false, beltFed: false };
const UNIT = { oneAmmoHolder: false, requiresReload: false, ammoSet: 'AmmoSet_Grenade' };

describe('CeUnderBarrelFields', () => {
  it('adds an empty unit when the switch is turned on and removes it when turned off', () => {
    const onChange = vi.fn();
    const { rerender } = render(<CeUnderBarrelFields block={BASE} onChange={onChange} />);
    expect(screen.getByText('Off')).toBeTruthy();
    fireEvent.click(screen.getByRole('switch', { name: /under barrel unit/i }));
    expect(onChange).toHaveBeenLastCalledWith({
      underBarrel: { oneAmmoHolder: false, requiresReload: false },
    });
    rerender(<CeUnderBarrelFields block={{ ...BASE, underBarrel: UNIT }} onChange={onChange} />);
    fireEvent.click(screen.getByRole('switch', { name: /under barrel unit/i }));
    expect(onChange).toHaveBeenLastCalledWith({ underBarrel: undefined });
  });

  it('writes a typed range into the unit and keeps what it held', () => {
    const onChange = vi.fn();
    render(<CeUnderBarrelFields block={{ ...BASE, underBarrel: UNIT }} onChange={onChange} />);
    const box = screen.getByRole('spinbutton', { name: 'Range' });
    fireEvent.input(box, { target: { value: '25' } });
    fireEvent.blur(box);
    expect(onChange).toHaveBeenLastCalledWith({
      underBarrel: { ...UNIT, range: { value: 25, source: 'typed' } },
    });
  });

  it('drops a text member when its box is emptied', () => {
    const onChange = vi.fn();
    render(
      <CeUnderBarrelFields
        block={{ ...BASE, underBarrel: { ...UNIT, soundCast: 'Shot_Grenade' } }}
        onChange={onChange}
      />,
    );
    fireEvent.input(screen.getByRole('textbox', { name: 'Shot sound' }), { target: { value: '' } });
    expect(onChange).toHaveBeenLastCalledWith({ underBarrel: UNIT });
  });

  it('offers the ammo sets it was given', () => {
    render(
      <CeUnderBarrelFields
        block={{ ...BASE, underBarrel: { ...UNIT, ammoSet: undefined as never } }}
        onChange={() => undefined}
        ammoSets={[{ value: 'AmmoSet_A', label: 'AmmoSet_A' }]}
      />,
    );
    expect(screen.getByRole('combobox', { name: 'Ammo set' })).toBeTruthy();
  });
});
