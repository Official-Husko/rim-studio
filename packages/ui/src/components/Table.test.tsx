import { fireEvent, render, screen, within } from '@testing-library/preact';
import { useState } from 'preact/hooks';
import { describe, expect, it, vi } from 'vitest';
import { press } from 'rimstudio-testkit';
import { Table, type TableColumn } from './Table';

interface Row {
  id: string;
  name: string;
  dps: number;
}
const ROWS: Row[] = [
  { id: 'a', name: 'Carbine', dps: 8.4 },
  { id: 'b', name: 'Revolver', dps: 6.1 },
  { id: 'c', name: 'Sniper rifle', dps: 9.9 },
];
const COLUMNS: TableColumn<Row>[] = [
  { key: 'name', header: 'Weapon', render: (r) => r.name, sortable: true },
  { key: 'dps', header: 'DPS', render: (r) => r.dps, sortable: true, align: 'right', mono: true },
];

function Harness(props: { onActivate?: (r: Row) => void; selection?: 'single' | 'multiple' }) {
  const [keys, setKeys] = useState<Set<string>>(new Set());
  return (
    <Table
      label="Weapons"
      columns={COLUMNS}
      rows={ROWS}
      getKey={(r) => r.id}
      selection={props.selection ?? 'multiple'}
      selectedKeys={keys}
      onSelectionChange={setKeys}
      onRowActivate={props.onActivate}
    />
  );
}

describe('Table', () => {
  it('renders a grid with column headers and rows', () => {
    render(<Harness />);
    expect(screen.getByRole('grid', { name: 'Weapons' })).toBeTruthy();
    expect(screen.getAllByRole('columnheader')).toHaveLength(2);
    expect(screen.getAllByRole('row')).toHaveLength(4);
  });

  it('asks for a sort and reflects aria-sort', () => {
    const onSortChange = vi.fn();
    const { rerender } = render(
      <Table
        label="W"
        columns={COLUMNS}
        rows={ROWS}
        getKey={(r) => r.id}
        sort={{ key: 'dps', direction: 'asc' }}
        onSortChange={onSortChange}
      />,
    );
    expect(screen.getByRole('columnheader', { name: /DPS/ }).getAttribute('aria-sort')).toBe(
      'ascending',
    );
    fireEvent.click(screen.getByRole('button', { name: /DPS/ }));
    expect(onSortChange).toHaveBeenCalledWith('dps', 'desc');
    fireEvent.click(screen.getByRole('button', { name: 'Weapon' }));
    expect(onSortChange).toHaveBeenLastCalledWith('name', 'asc');
    rerender(
      <Table
        label="W"
        columns={COLUMNS}
        rows={ROWS}
        getKey={(r) => r.id}
        sort={{ key: 'dps', direction: 'desc' }}
        onSortChange={onSortChange}
      />,
    );
    expect(screen.getByRole('columnheader', { name: /DPS/ }).getAttribute('aria-sort')).toBe(
      'descending',
    );
  });

  it('selects rows by click and Space', () => {
    render(<Harness />);
    const rows = screen.getAllByRole('row').slice(1);
    fireEvent.click(rows[0] as HTMLElement);
    expect(rows[0]?.getAttribute('aria-selected')).toBe('true');
    fireEvent.click(rows[1] as HTMLElement, { ctrlKey: true });
    expect(rows[1]?.getAttribute('aria-selected')).toBe('true');
    press(rows[2] as HTMLElement, ' ');
    expect(rows[2]?.getAttribute('aria-selected')).toBe('true');
  });

  it('extends a range with Shift click', () => {
    render(<Harness />);
    const rows = screen.getAllByRole('row').slice(1);
    fireEvent.click(rows[0] as HTMLElement);
    fireEvent.click(rows[2] as HTMLElement, { shiftKey: true });
    expect(rows.map((r) => r.getAttribute('aria-selected'))).toEqual(['true', 'true', 'true']);
  });

  it('moves focus with the arrow keys and activates with Enter', () => {
    const onActivate = vi.fn();
    render(<Harness onActivate={onActivate} />);
    const rows = screen.getAllByRole('row').slice(1);
    (rows[0] as HTMLElement).focus();
    press(rows[0] as HTMLElement, 'ArrowDown');
    expect(document.activeElement).toBe(rows[1]);
    press(rows[1] as HTMLElement, 'Enter');
    expect(onActivate).toHaveBeenCalledWith(ROWS[1]);
    press(rows[1] as HTMLElement, 'End');
    expect(document.activeElement).toBe(rows[2]);
  });

  it('shows an empty message', () => {
    render(
      <Table
        label="E"
        columns={COLUMNS}
        rows={[]}
        getKey={(r) => r.id}
        emptyText="No weapons yet"
      />,
    );
    expect(within(screen.getByRole('grid')).getByText('No weapons yet')).toBeTruthy();
  });
});
