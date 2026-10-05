import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { Checkbox } from './Checkbox';

describe('Checkbox', () => {
  it('is named by its label', () => {
    render(
      <Checkbox checked={false} onCheckedChange={() => {}}>
        Write a Combat Extended patch
      </Checkbox>,
    );
    expect(screen.getByRole('checkbox', { name: 'Write a Combat Extended patch' })).toBeTruthy();
  });

  it('reports the new state', () => {
    const onCheckedChange = vi.fn();
    render(
      <Checkbox checked={false} onCheckedChange={onCheckedChange}>
        Pin
      </Checkbox>,
    );
    fireEvent.click(screen.getByRole('checkbox'));
    expect(onCheckedChange).toHaveBeenCalledWith(true);
  });

  it('supports the indeterminate state', () => {
    render(
      <Checkbox checked={false} indeterminate onCheckedChange={() => {}}>
        All
      </Checkbox>,
    );
    expect((screen.getByRole('checkbox') as HTMLInputElement).indeterminate).toBe(true);
  });

  it('does not change when disabled', () => {
    const onCheckedChange = vi.fn();
    render(
      <Checkbox checked disabled onCheckedChange={onCheckedChange}>
        Locked
      </Checkbox>,
    );
    fireEvent.click(screen.getByRole('checkbox'));
    expect(onCheckedChange).not.toHaveBeenCalled();
  });
});
