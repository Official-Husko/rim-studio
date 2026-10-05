import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { CeToolPlanFields } from './CeToolPlanFields';

const BASE = { oneHanded: false, beltFed: false };

describe('CeToolPlanFields', () => {
  it('adds a deliberate muzzle tool to an empty plan', () => {
    const onChange = vi.fn();
    render(<CeToolPlanFields block={BASE} onChange={onChange} />);
    expect(screen.getByText('Vanilla tools are carried over')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Add a muzzle tool' }));
    expect(onChange).toHaveBeenLastCalledWith({ toolPlan: [{ label: 'muzzle' }] });
  });

  it('reorders the tools and removes the plan when its last tool goes', () => {
    const onChange = vi.fn();
    render(
      <CeToolPlanFields
        block={{ ...BASE, toolPlan: [{ label: 'a' }, { label: 'b' }] }}
        onChange={onChange}
      />,
    );
    expect(screen.getByText('2 tools')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Move b up' }));
    expect(onChange).toHaveBeenLastCalledWith({ toolPlan: [{ label: 'b' }, { label: 'a' }] });
    fireEvent.click(screen.getByRole('button', { name: 'Remove b' }));
    expect(onChange).toHaveBeenLastCalledWith({ toolPlan: [{ label: 'a' }] });
  });

  it('keeps a list of vanilla tool fields', () => {
    const onChange = vi.fn();
    render(<CeToolPlanFields block={BASE} onChange={onChange} />);
    fireEvent.input(screen.getByRole('textbox', { name: 'Vanilla tool fields to keep' }), {
      target: { value: 'power' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add' }));
    expect(onChange).toHaveBeenLastCalledWith({ keepToolFields: ['power'] });
  });
});
