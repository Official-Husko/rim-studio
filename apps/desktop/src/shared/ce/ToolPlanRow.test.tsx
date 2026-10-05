import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { ToolPlanRow } from './ToolPlanRow';

function setup(tool = { label: 'stock' }, index = 1, count = 3) {
  const onChange = vi.fn();
  const onRemove = vi.fn();
  const onMove = vi.fn();
  render(
    <ToolPlanRow
      tool={tool}
      index={index}
      count={count}
      onChange={onChange}
      onRemove={onRemove}
      onMove={onMove}
    />,
  );
  return { onChange, onRemove, onMove };
}

describe('ToolPlanRow', () => {
  it('sets a number and drops it when cleared', () => {
    const { onChange } = setup({ label: 'stock', power: 8 } as never);
    const box = screen.getByRole('spinbutton', { name: 'Power' });
    fireEvent.input(box, { target: { value: '' } });
    fireEvent.blur(box);
    expect(onChange).toHaveBeenLastCalledWith({ label: 'stock' });
  });

  it('adds a capacity and sets the vanilla tool it comes from', () => {
    const { onChange } = setup();
    fireEvent.input(screen.getByRole('textbox', { name: 'Capacities' }), {
      target: { value: 'Blunt' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add' }));
    expect(onChange).toHaveBeenLastCalledWith({ label: 'stock', capacities: ['Blunt'] });
    fireEvent.input(screen.getByRole('textbox', { name: 'Vanilla tool' }), {
      target: { value: 'handle' },
    });
    expect(onChange).toHaveBeenLastCalledWith({ label: 'stock', from: 'handle' });
  });

  it('moves and removes the tool, and cannot move past the ends', () => {
    const { onMove, onRemove } = setup();
    fireEvent.click(screen.getByRole('button', { name: 'Move stock up' }));
    expect(onMove).toHaveBeenLastCalledWith(-1);
    fireEvent.click(screen.getByRole('button', { name: 'Move stock down' }));
    expect(onMove).toHaveBeenLastCalledWith(1);
    fireEvent.click(screen.getByRole('button', { name: 'Remove stock' }));
    expect(onRemove).toHaveBeenCalled();
  });

  it('disables the move that would leave the list', () => {
    setup({ label: 'stock' }, 0, 1);
    expect(
      (screen.getByRole('button', { name: 'Move stock up' }) as HTMLButtonElement).disabled,
    ).toBe(true);
    expect(
      (screen.getByRole('button', { name: 'Move stock down' }) as HTMLButtonElement).disabled,
    ).toBe(true);
  });
});
