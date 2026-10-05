import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { StringListField } from './StringListField';

describe('StringListField', () => {
  it('adds a trimmed name and ignores a name already present', () => {
    const onChange = vi.fn();
    render(<StringListField label="Tags" values={['A']} onChange={onChange} />);
    const box = screen.getByRole('textbox', { name: 'Tags' });
    fireEvent.input(box, { target: { value: '  B  ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add' }));
    expect(onChange).toHaveBeenLastCalledWith(['A', 'B']);
    fireEvent.input(box, { target: { value: 'A' } });
    fireEvent.keyDown(box, { key: 'Enter' });
    expect(onChange).toHaveBeenCalledTimes(1);
  });

  it('removes a name through its chip', () => {
    const onChange = vi.fn();
    render(<StringListField label="Tags" values={['A', 'B']} onChange={onChange} />);
    fireEvent.click(screen.getByRole('button', { name: 'Remove A' }));
    expect(onChange).toHaveBeenLastCalledWith(['B']);
  });
});
