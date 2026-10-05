import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { GraphicPartRow } from './GraphicPartRow';

describe('GraphicPartRow', () => {
  it('adds a slot tag and removes the part', () => {
    const onChange = vi.fn();
    const onRemove = vi.fn();
    render(<GraphicPartRow part={{}} onChange={onChange} onRemove={onRemove} />);
    fireEvent.input(screen.getByRole('textbox', { name: 'Slot tags' }), {
      target: { value: 'Scope' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add' }));
    expect(onChange).toHaveBeenLastCalledWith({ slotTags: ['Scope'] });
    fireEvent.click(screen.getByRole('button', { name: 'Remove part' }));
    expect(onRemove).toHaveBeenCalled();
  });

  it('writes the part graphic as a tree and drops it when emptied', () => {
    const onChange = vi.fn();
    render(
      <GraphicPartRow
        part={{ partGraphic: { tag: 'partGraphic', attrs: [], children: [] } }}
        onChange={onChange}
        onRemove={() => undefined}
      />,
    );
    const box = screen.getByRole('textbox', { name: 'Part graphic' });
    fireEvent.input(box, { target: { value: '' } });
    fireEvent.blur(box);
    expect(onChange).toHaveBeenLastCalledWith({});
  });
});
