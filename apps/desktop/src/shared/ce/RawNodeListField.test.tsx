import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { RawNodeListField } from './RawNodeListField';

describe('RawNodeListField', () => {
  it('adds a simple element by tag and text', () => {
    const onChange = vi.fn();
    render(<RawNodeListField label="Extras" nodes={[]} onChange={onChange} />);
    const add = screen.getByRole('button', { name: 'Add element' }) as HTMLButtonElement;
    expect(add.disabled).toBe(true);
    fireEvent.input(screen.getByRole('textbox', { name: 'Element name' }), {
      target: { value: 'li' },
    });
    fireEvent.input(screen.getByRole('textbox', { name: 'Text' }), { target: { value: 'x' } });
    fireEvent.click(add);
    expect(onChange).toHaveBeenLastCalledWith([{ tag: 'li', attrs: [], children: ['x'] }]);
  });

  it('names each element by its tag and text and removes one', () => {
    const onChange = vi.fn();
    render(
      <RawNodeListField
        label="Extras"
        nodes={[{ tag: 'li', attrs: [], children: ['x'] }]}
        onChange={onChange}
      />,
    );
    expect(screen.getByRole('textbox', { name: 'Element li: x' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Remove element' }));
    expect(onChange).toHaveBeenLastCalledWith([]);
  });
});
