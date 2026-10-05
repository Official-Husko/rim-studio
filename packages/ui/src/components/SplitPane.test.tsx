import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { press } from 'rimstudio-testkit';
import { SplitPane } from './SplitPane';

describe('SplitPane', () => {
  it('renders both panes and a divider with its value', () => {
    render(
      <SplitPane label="Resize list" first={<p>Items</p>} second={<p>Form</p>} defaultSize={300} />,
    );
    expect(screen.getByText('Items')).toBeTruthy();
    expect(screen.getByText('Form')).toBeTruthy();
    const sep = screen.getByRole('separator', { name: 'Resize list' });
    expect(sep.getAttribute('aria-valuenow')).toBe('300');
    expect(sep.getAttribute('aria-orientation')).toBe('vertical');
    expect(sep.getAttribute('tabindex')).toBe('0');
  });

  it('resizes with the arrow keys and larger steps with Shift', () => {
    const onSizeChange = vi.fn();
    render(
      <SplitPane
        label="Resize"
        first="a"
        second="b"
        defaultSize={300}
        onSizeChange={onSizeChange}
      />,
    );
    const sep = screen.getByRole('separator');
    press(sep, 'ArrowRight');
    expect(sep.getAttribute('aria-valuenow')).toBe('316');
    press(sep, 'ArrowLeft', { shiftKey: true });
    expect(sep.getAttribute('aria-valuenow')).toBe('252');
    expect(onSizeChange).toHaveBeenLastCalledWith(252);
  });

  it('respects the limits and Home and End', () => {
    render(<SplitPane label="Resize" first="a" second="b" defaultSize={300} min={200} max={400} />);
    const sep = screen.getByRole('separator');
    press(sep, 'Home');
    expect(sep.getAttribute('aria-valuenow')).toBe('200');
    press(sep, 'ArrowLeft');
    expect(sep.getAttribute('aria-valuenow')).toBe('200');
    press(sep, 'End');
    expect(sep.getAttribute('aria-valuenow')).toBe('400');
  });

  it('resizes by dragging', () => {
    render(<SplitPane label="Resize" first="a" second="b" defaultSize={300} />);
    const sep = screen.getByRole('separator');
    fireEvent.pointerDown(sep, { clientX: 300, pointerId: 1 });
    fireEvent.pointerMove(sep, { clientX: 340, pointerId: 1 });
    expect(sep.getAttribute('aria-valuenow')).toBe('340');
    fireEvent.pointerUp(sep, { pointerId: 1 });
    fireEvent.pointerMove(sep, { clientX: 500, pointerId: 1 });
    expect(sep.getAttribute('aria-valuenow')).toBe('340');
  });

  it('uses the horizontal separator orientation when stacked and follows the vertical keys', () => {
    render(
      <SplitPane direction="vertical" label="Resize" first="a" second="b" defaultSize={200} />,
    );
    const sep = screen.getByRole('separator');
    expect(sep.getAttribute('aria-orientation')).toBe('horizontal');
    press(sep, 'ArrowDown');
    expect(sep.getAttribute('aria-valuenow')).toBe('216');
  });
});
