import { fireEvent, render, screen } from '@testing-library/preact';
import { useState } from 'preact/hooks';
import { describe, expect, it } from 'vitest';
import { press } from 'rimstudio-testkit';
import { Tabs } from './Tabs';

const TABS = [
  { id: 'def', label: 'Definition' },
  { id: 'patch', label: 'CE patch', badge: '3' },
  { id: 'files', label: 'Files', disabled: true },
  { id: 'about', label: 'About' },
];

function Harness() {
  const [value, setValue] = useState('def');
  return (
    <Tabs label="Output" tabs={TABS} value={value} onValueChange={setValue}>
      {(id) => <p>Panel {id}</p>}
    </Tabs>
  );
}

describe('Tabs', () => {
  it('has a tab list, tabs and a labelled panel', () => {
    render(<Harness />);
    expect(screen.getByRole('tablist', { name: 'Output' })).toBeTruthy();
    expect(screen.getAllByRole('tab')).toHaveLength(4);
    const panel = screen.getByRole('tabpanel');
    expect(panel.getAttribute('aria-labelledby')).toBe(
      screen.getByRole('tab', { name: 'Definition' }).id,
    );
    expect(screen.getByText('Panel def')).toBeTruthy();
  });

  it('selects on click', () => {
    render(<Harness />);
    fireEvent.click(screen.getByRole('tab', { name: /CE patch/ }));
    expect(screen.getByText('Panel patch')).toBeTruthy();
  });

  it('moves with arrows, skipping disabled tabs, and wraps', () => {
    render(<Harness />);
    press(screen.getByRole('tab', { name: 'Definition' }), 'ArrowRight');
    expect(screen.getByText('Panel patch')).toBeTruthy();
    press(screen.getByRole('tab', { name: /CE patch/ }), 'ArrowRight');
    expect(screen.getByText('Panel about')).toBeTruthy();
    press(screen.getByRole('tab', { name: 'About' }), 'ArrowRight');
    expect(screen.getByText('Panel def')).toBeTruthy();
  });

  it('jumps with Home and End and keeps one tab stop', () => {
    render(<Harness />);
    press(screen.getByRole('tab', { name: 'Definition' }), 'End');
    expect(screen.getByText('Panel about')).toBeTruthy();
    const stops = screen.getAllByRole('tab').filter((t) => t.getAttribute('tabindex') === '0');
    expect(stops).toHaveLength(1);
    press(screen.getByRole('tab', { name: 'About' }), 'Home');
    expect(screen.getByText('Panel def')).toBeTruthy();
  });
});
