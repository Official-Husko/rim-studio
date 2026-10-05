import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { TriState } from './TriState';

describe('TriState', () => {
  it('shows the undecided choice for an absent value and reports yes and no', () => {
    const onChange = vi.fn();
    render(<TriState label="Reload" value={undefined} onChange={onChange} />);
    expect(screen.getByRole('radio', { name: 'Decide for me' }).getAttribute('aria-checked')).toBe(
      'true',
    );
    fireEvent.click(screen.getByRole('radio', { name: 'Yes' }));
    expect(onChange).toHaveBeenLastCalledWith(true);
    fireEvent.click(screen.getByRole('radio', { name: 'No' }));
    expect(onChange).toHaveBeenLastCalledWith(false);
  });

  it('reports undefined when the undecided choice is picked again', () => {
    const onChange = vi.fn();
    render(<TriState label="Reload" value={false} onChange={onChange} autoLabel="Shape" />);
    fireEvent.click(screen.getByRole('radio', { name: 'Shape' }));
    expect(onChange).toHaveBeenLastCalledWith(undefined);
  });
});
