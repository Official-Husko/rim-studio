import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { Select } from './Select';

const OPTIONS = [
  { value: 'ranged', label: 'Ranged weapon' },
  { value: 'melee', label: 'Melee weapon' },
  { value: 'apparel', label: 'Apparel', group: 'Wearable' },
];

describe('Select', () => {
  it('is a combobox named by its label with the chosen value', () => {
    render(
      <Select aria-label="Item kind" value="melee" options={OPTIONS} onValueChange={() => {}} />,
    );
    const select = screen.getByRole('combobox', { name: 'Item kind' }) as HTMLSelectElement;
    expect(select.value).toBe('melee');
  });

  it('reports a change', () => {
    const onValueChange = vi.fn();
    render(
      <Select
        aria-label="Item kind"
        value="ranged"
        options={OPTIONS}
        onValueChange={onValueChange}
      />,
    );
    fireEvent.change(screen.getByRole('combobox'), { target: { value: 'melee' } });
    expect(onValueChange).toHaveBeenCalledWith('melee');
  });

  it('shows a placeholder and groups', () => {
    render(
      <Select
        aria-label="Kind"
        value={undefined}
        placeholder="Choose a kind"
        options={OPTIONS}
        onValueChange={() => {}}
      />,
    );
    expect(screen.getByRole('option', { name: 'Choose a kind' })).toBeTruthy();
    expect(screen.getByRole('group', { name: 'Wearable' })).toBeTruthy();
  });
});
