import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { IconButton } from './IconButton';

describe('IconButton', () => {
  it('is named by its label', () => {
    render(<IconButton icon="trash" label="Delete draft" />);
    expect(screen.getByRole('button', { name: 'Delete draft' })).toBeTruthy();
  });

  it('shows the label as a tooltip unless suppressed', () => {
    const { rerender } = render(<IconButton icon="copy" label="Copy" />);
    expect(screen.getByRole('tooltip', { hidden: true }).textContent).toBe('Copy');
    rerender(<IconButton icon="copy" label="Copy" noTooltip />);
    expect(screen.queryByRole('tooltip', { hidden: true })).toBeNull();
  });

  it('reports the pressed state', () => {
    render(<IconButton icon="lock" label="Lock" pressed />);
    expect(screen.getByRole('button', { name: 'Lock' }).getAttribute('aria-pressed')).toBe('true');
  });

  it('does not fire when disabled', () => {
    const onClick = vi.fn();
    render(<IconButton icon="plus" label="Add" disabled onClick={onClick} />);
    fireEvent.click(screen.getByRole('button', { name: 'Add' }));
    expect(onClick).not.toHaveBeenCalled();
  });
});
