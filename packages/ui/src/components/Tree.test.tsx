import { fireEvent, render, screen } from '@testing-library/preact';
import { useState } from 'preact/hooks';
import { describe, expect, it, vi } from 'vitest';
import { press } from 'rimstudio-testkit';
import { Tree, type TreeNode } from './Tree';

const NODES: TreeNode[] = [
  {
    id: 'mod',
    label: 'Plasma Carbine',
    icon: 'folder',
    children: [
      {
        id: 'defs',
        label: 'Defs',
        icon: 'folder',
        role: 'defs',
        children: [{ id: 'gun', label: 'Gun_Plasma.xml', icon: 'xml' }],
      },
      { id: 'about', label: 'About', icon: 'folder' },
    ],
  },
  { id: 'readme', label: 'README.txt', icon: 'file' },
];

function Harness(props: { onSelect?: (id: string) => void }) {
  const [sel, setSel] = useState<string>();
  return (
    <Tree
      label="Project files"
      nodes={NODES}
      selectedId={sel}
      onSelect={(id) => {
        setSel(id);
        props.onSelect?.(id);
      }}
    />
  );
}

describe('Tree', () => {
  it('renders a tree with levels and collapsed children hidden', () => {
    render(<Harness />);
    expect(screen.getByRole('tree', { name: 'Project files' })).toBeTruthy();
    expect(screen.getAllByRole('treeitem')).toHaveLength(2);
    expect(
      screen.getByRole('treeitem', { name: /Plasma Carbine/ }).getAttribute('aria-expanded'),
    ).toBe('false');
  });

  it('expands with the toggle and with ArrowRight, collapses with ArrowLeft', () => {
    render(<Harness />);
    const root = screen.getByRole('treeitem', { name: /Plasma Carbine/ });
    press(root, 'ArrowRight');
    expect(screen.getAllByRole('treeitem')).toHaveLength(4);
    expect(screen.getByRole('treeitem', { name: /Defs/ }).getAttribute('aria-level')).toBe('2');
    press(root, 'ArrowLeft');
    expect(screen.getAllByRole('treeitem')).toHaveLength(2);
  });

  it('moves focus with arrows, Home and End', () => {
    render(<Harness />);
    const root = screen.getByRole('treeitem', { name: /Plasma Carbine/ });
    root.focus();
    press(root, 'ArrowDown');
    expect(document.activeElement).toBe(screen.getByRole('treeitem', { name: /README/ }));
    press(document.activeElement as HTMLElement, 'Home');
    expect(document.activeElement).toBe(root);
    press(root, 'End');
    expect(document.activeElement).toBe(screen.getByRole('treeitem', { name: /README/ }));
  });

  it('selects on click and on Enter, and moves to the parent with ArrowLeft', () => {
    const onSelect = vi.fn();
    render(<Harness onSelect={onSelect} />);
    fireEvent.click(screen.getByRole('treeitem', { name: /README/ }));
    expect(onSelect).toHaveBeenLastCalledWith('readme');
    const root = screen.getByRole('treeitem', { name: /Plasma Carbine/ });
    press(root, 'ArrowRight');
    const defs = screen.getByRole('treeitem', { name: /Defs/ });
    defs.focus();
    press(defs, 'Enter');
    expect(onSelect).toHaveBeenLastCalledWith('defs');
    press(defs, 'ArrowLeft');
    expect(document.activeElement).toBe(root);
  });

  it('keeps one tab stop', () => {
    render(<Harness />);
    expect(
      screen.getAllByRole('treeitem').filter((n) => n.getAttribute('tabindex') === '0'),
    ).toHaveLength(1);
  });
});
