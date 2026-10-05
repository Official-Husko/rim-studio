import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { StatEntryList } from './StatEntryList';

describe('StatEntryList', () => {
  it('adds an empty entry', () => {
    const onChange = vi.fn();
    render(<StatEntryList label="Stat offsets" entries={[]} onChange={onChange} />);
    fireEvent.click(screen.getByRole('button', { name: 'Add a stat' }));
    expect(onChange).toHaveBeenLastCalledWith([{ stat: '', value: 0 }]);
  });

  it('edits the name and the number of an entry and removes it', () => {
    const onChange = vi.fn();
    render(
      <StatEntryList
        label="Stat offsets"
        entries={[{ stat: 'SightsEfficiency', value: 0.5 }]}
        onChange={onChange}
      />,
    );
    fireEvent.input(screen.getByRole('textbox', { name: 'Stat offsets stat' }), {
      target: { value: 'Bulk' },
    });
    expect(onChange).toHaveBeenLastCalledWith([{ stat: 'Bulk', value: 0.5 }]);
    const number = screen.getByRole('spinbutton', { name: 'Stat offsets number' });
    fireEvent.input(number, { target: { value: '2' } });
    fireEvent.blur(number);
    expect(onChange).toHaveBeenLastCalledWith([{ stat: 'SightsEfficiency', value: 2 }]);
    fireEvent.click(screen.getByRole('button', { name: 'Remove a stat from Stat offsets' }));
    expect(onChange).toHaveBeenLastCalledWith([]);
  });
});
