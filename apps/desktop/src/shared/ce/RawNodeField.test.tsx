import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { RawNodeField } from './RawNodeField';

const NODE = { tag: 'partGraphic', attrs: [], children: [] as never[] };

describe('RawNodeField', () => {
  it('shows the element as a tree and commits a valid edit on blur', () => {
    const onChange = vi.fn();
    render(<RawNodeField label="Part" node={NODE} onChange={onChange} />);
    const box = screen.getByRole('textbox', { name: 'Part' }) as HTMLTextAreaElement;
    expect(JSON.parse(box.value)).toEqual(NODE);
    fireEvent.input(box, { target: { value: '{"tag":"outlineGraphic"}' } });
    fireEvent.blur(box);
    expect(onChange).toHaveBeenLastCalledWith({ tag: 'outlineGraphic', attrs: [], children: [] });
  });

  it('keeps an invalid text and names the problem instead of committing', () => {
    const onChange = vi.fn();
    render(<RawNodeField label="Part" node={undefined} onChange={onChange} />);
    const box = screen.getByRole('textbox', { name: 'Part' });
    fireEvent.input(box, { target: { value: '{"tag":' } });
    fireEvent.blur(box);
    expect(screen.getByText(/not valid JSON/)).toBeTruthy();
    fireEvent.input(box, { target: { value: '{"name":"x"}' } });
    fireEvent.blur(box);
    expect(screen.getByText(/needs a tag/)).toBeTruthy();
    expect(onChange).not.toHaveBeenCalled();
  });

  it('removes the element when the text is emptied', () => {
    const onChange = vi.fn();
    render(<RawNodeField label="Part" node={NODE} onChange={onChange} />);
    const box = screen.getByRole('textbox', { name: 'Part' });
    fireEvent.input(box, { target: { value: '' } });
    fireEvent.blur(box);
    expect(onChange).toHaveBeenLastCalledWith(undefined);
  });
});
