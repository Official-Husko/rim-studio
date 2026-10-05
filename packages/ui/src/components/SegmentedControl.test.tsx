import { render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { press } from 'rimstudio-testkit';
import { SegmentedControl } from './SegmentedControl';

const OPTIONS = [
  { value: 'ranged', label: 'Ranged' },
  { value: 'melee', label: 'Melee' },
  { value: 'apparel', label: 'Apparel', disabled: true },
  { value: 'other', label: 'Other' },
];

describe('SegmentedControl', () => {
  it('is a radio group with one checked option', () => {
    render(
      <SegmentedControl label="Kind" options={OPTIONS} value="melee" onValueChange={() => {}} />,
    );
    expect(screen.getByRole('radiogroup', { name: 'Kind' })).toBeTruthy();
    expect(screen.getByRole('radio', { name: 'Melee' }).getAttribute('aria-checked')).toBe('true');
    expect(screen.getByRole('radio', { name: 'Ranged' }).getAttribute('aria-checked')).toBe(
      'false',
    );
  });

  it('uses a single tab stop', () => {
    render(
      <SegmentedControl label="Kind" options={OPTIONS} value="melee" onValueChange={() => {}} />,
    );
    const stops = screen.getAllByRole('radio').filter((r) => r.getAttribute('tabindex') === '0');
    expect(stops).toHaveLength(1);
    expect(stops[0]?.textContent).toBe('Melee');
  });

  it('moves with the arrow keys and skips disabled options', () => {
    const onValueChange = vi.fn();
    render(
      <SegmentedControl
        label="Kind"
        options={OPTIONS}
        value="melee"
        onValueChange={onValueChange}
      />,
    );
    press(screen.getByRole('radio', { name: 'Melee' }), 'ArrowRight');
    expect(onValueChange).toHaveBeenLastCalledWith('other');
    press(screen.getByRole('radio', { name: 'Melee' }), 'ArrowLeft');
    expect(onValueChange).toHaveBeenLastCalledWith('ranged');
  });

  it('wraps around at the ends', () => {
    const onValueChange = vi.fn();
    render(
      <SegmentedControl
        label="Kind"
        options={OPTIONS}
        value="other"
        onValueChange={onValueChange}
      />,
    );
    press(screen.getByRole('radio', { name: 'Other' }), 'ArrowRight');
    expect(onValueChange).toHaveBeenLastCalledWith('ranged');
  });
});
