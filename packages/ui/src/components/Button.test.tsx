import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { Button } from './Button';

describe('Button', () => {
  it('renders its name and handles clicks', () => {
    const onClick = vi.fn();
    render(<Button onClick={onClick}>Save</Button>);
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it('is a plain button by default so it never submits a form by accident', () => {
    render(<Button>Save</Button>);
    expect(screen.getByRole('button').getAttribute('type')).toBe('button');
  });

  it('disables itself and reports busy while loading', () => {
    const onClick = vi.fn();
    render(
      <Button loading onClick={onClick}>
        Apply
      </Button>,
    );
    const button = screen.getByRole('button', { name: /Apply/ }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    expect(button.getAttribute('aria-busy')).toBe('true');
    fireEvent.click(button);
    expect(onClick).not.toHaveBeenCalled();
  });

  it('applies the variant and size classes', () => {
    render(
      <Button variant="primary" size="sm">
        Go
      </Button>,
    );
    const cls = screen.getByRole('button').getAttribute('class') ?? '';
    expect(cls).toContain('bg-accent');
    expect(cls).toContain('h-control-sm');
  });

  it('draws icons without adding to the accessible name', () => {
    render(<Button icon="plus">Add weapon</Button>);
    expect(screen.getByRole('button', { name: 'Add weapon' })).toBeTruthy();
  });
});
