import { fireEvent, render, screen } from '@testing-library/preact';
import { useState } from 'preact/hooks';
import { describe, expect, it } from 'vitest';
import { VirtualList } from './VirtualList';

const items = Array.from({ length: 500 }, (_, i) => ({ id: `k${i}`, text: `Row ${i}` }));

function Harness() {
  const [selected, setSelected] = useState<string | undefined>('k0');
  return (
    <VirtualList
      items={items}
      rowHeight={40}
      label="Rows"
      fallbackHeight={400}
      getKey={(item) => item.id}
      selectedKey={selected}
      onSelect={setSelected}
      renderRow={(item) => <span>{item.text}</span>}
    />
  );
}

describe('VirtualList', () => {
  it('draws only the rows in view and says how many there are', () => {
    render(<Harness />);
    const options = screen.getAllByRole('option');
    expect(options.length).toBeLessThan(30);
    expect(options[0]?.getAttribute('aria-setsize')).toBe('500');
    expect(options[0]?.getAttribute('aria-posinset')).toBe('1');
  });

  it('moves the selection with the arrow keys, Home and End', () => {
    render(<Harness />);
    const list = screen.getByRole('listbox', { name: 'Rows' });
    fireEvent.keyDown(list, { key: 'ArrowDown' });
    expect(screen.getByRole('option', { selected: true }).textContent).toBe('Row 1');
    fireEvent.keyDown(list, { key: 'End' });
    expect(list.getAttribute('aria-activedescendant')).toBe('Rows-k499');
    fireEvent.keyDown(list, { key: 'Home' });
    expect(screen.getByRole('option', { selected: true }).textContent).toBe('Row 0');
    fireEvent.keyDown(list, { key: 'PageDown' });
    expect(list.getAttribute('aria-activedescendant')).not.toBe('Rows-k0');
  });

  it('selects a row by a click and follows a scroll', () => {
    render(<Harness />);
    fireEvent.click(screen.getByText('Row 3'));
    expect(screen.getByRole('option', { selected: true }).textContent).toBe('Row 3');
    const list = screen.getByRole('listbox', { name: 'Rows' });
    list.scrollTop = 4000;
    fireEvent.scroll(list);
    expect(screen.queryByText('Row 3')).toBeNull();
    expect(screen.getByText('Row 100')).toBeTruthy();
  });
});
