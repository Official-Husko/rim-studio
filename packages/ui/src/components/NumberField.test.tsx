import { fireEvent, render, screen } from '@testing-library/preact';
import { useState } from 'preact/hooks';
import { describe, expect, it, vi } from 'vitest';
import { press } from 'rimstudio-testkit';
import { Chip } from './Chip';
import { NumberField } from './NumberField';

function Harness(props: {
  start?: number;
  step?: number;
  min?: number;
  max?: number;
  onChange?: (v: number | undefined) => void;
}) {
  const [value, setValue] = useState<number | undefined>(props.start);
  return (
    <NumberField
      aria-label="Damage"
      value={value}
      step={props.step}
      min={props.min}
      max={props.max}
      unit="dmg"
      chip={<Chip kind="suggested" />}
      onValueChange={(v) => {
        setValue(v);
        props.onChange?.(v);
      }}
    />
  );
}

describe('NumberField', () => {
  it('exposes a spinbutton with range values', () => {
    render(<Harness start={12} min={1} max={50} />);
    const input = screen.getByRole('spinbutton', { name: 'Damage' });
    expect(input.getAttribute('aria-valuenow')).toBe('12');
    expect(input.getAttribute('aria-valuemin')).toBe('1');
    expect(input.getAttribute('aria-valuemax')).toBe('50');
  });

  it('steps with the arrow keys and Shift', () => {
    render(<Harness start={10} step={0.5} />);
    const input = screen.getByRole('spinbutton') as HTMLInputElement;
    press(input, 'ArrowUp');
    expect(input.value).toBe('10.5');
    press(input, 'ArrowDown', { shiftKey: true });
    expect(input.value).toBe('5.5');
    press(input, 'PageUp');
    expect(input.value).toBe('10.5');
  });

  it('avoids floating point noise', () => {
    render(<Harness start={0.1} step={0.1} />);
    const input = screen.getByRole('spinbutton') as HTMLInputElement;
    press(input, 'ArrowUp');
    press(input, 'ArrowUp');
    expect(input.value).toBe('0.3');
  });

  it('clamps stepping to the range', () => {
    render(<Harness start={49} min={1} max={50} step={5} />);
    const input = screen.getByRole('spinbutton') as HTMLInputElement;
    press(input, 'ArrowUp');
    expect(input.value).toBe('50');
  });

  it('commits typed text on blur and clamps it', () => {
    const onChange = vi.fn();
    render(<Harness start={5} min={1} max={20} onChange={onChange} />);
    const input = screen.getByRole('spinbutton') as HTMLInputElement;
    fireEvent.input(input, { target: { value: '99' } });
    expect(input.getAttribute('aria-invalid')).toBe('true');
    fireEvent.blur(input);
    expect(input.value).toBe('20');
    expect(onChange).toHaveBeenLastCalledWith(20);
  });

  it('reports an empty field as undefined', () => {
    const onChange = vi.fn();
    render(<Harness start={5} onChange={onChange} />);
    const input = screen.getByRole('spinbutton');
    fireEvent.input(input, { target: { value: '' } });
    fireEvent.blur(input);
    expect(onChange).toHaveBeenLastCalledWith(undefined);
  });

  it('shows the unit and the source chip slot', () => {
    render(<Harness start={1} />);
    expect(screen.getByText('dmg')).toBeTruthy();
    expect(screen.getByText('Suggested')).toBeTruthy();
  });
});
