import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { Switch } from './Switch';

describe('Switch', () => {
  it('has a switch role, a name and a state', () => {
    render(
      <Switch checked onCheckedChange={() => {}}>
        Combat Extended patch
      </Switch>,
    );
    const sw = screen.getByRole('switch', { name: 'Combat Extended patch' });
    expect(sw.getAttribute('aria-checked')).toBe('true');
  });

  it('toggles on click', () => {
    const onCheckedChange = vi.fn();
    render(
      <Switch checked={false} onCheckedChange={onCheckedChange}>
        Dry run
      </Switch>,
    );
    fireEvent.click(screen.getByRole('switch'));
    expect(onCheckedChange).toHaveBeenCalledWith(true);
  });

  it('is inert when disabled', () => {
    const onCheckedChange = vi.fn();
    render(
      <Switch checked={false} disabled onCheckedChange={onCheckedChange}>
        Off
      </Switch>,
    );
    fireEvent.click(screen.getByRole('switch'));
    expect(onCheckedChange).not.toHaveBeenCalled();
  });
});
