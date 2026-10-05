import { render, screen } from '@testing-library/preact';
import { describe, expect, it } from 'vitest';
import { Tooltip } from './Tooltip';

describe('Tooltip', () => {
  it('renders the bubble with a tooltip role next to the trigger', () => {
    render(
      <Tooltip text="Copy the path">
        <button type="button">Copy</button>
      </Tooltip>,
    );
    expect(screen.getByRole('tooltip', { hidden: true }).textContent).toBe('Copy the path');
    expect(screen.getByRole('button', { name: 'Copy' })).toBeTruthy();
  });

  it('links the wrapper to the bubble for assistive tech', () => {
    const { container } = render(
      <Tooltip text="Hint">
        <button type="button">x</button>
      </Tooltip>,
    );
    const wrapper = container.querySelector('.rs-tip');
    const bubble = container.querySelector('[role="tooltip"]');
    expect(wrapper?.getAttribute('aria-describedby')).toBe(bubble?.id);
  });
});
