import { render, screen } from '@testing-library/preact';
import { describe, expect, it } from 'vitest';
import { Group } from './Group';

describe('Group', () => {
  it('keeps the title and state visible and holds the content', () => {
    render(
      <Group title="Under barrel" summary="2 entries">
        <p>content</p>
      </Group>,
    );
    expect(screen.getByText('Under barrel')).toBeTruthy();
    expect(screen.getByText('2 entries')).toBeTruthy();
    expect(screen.getByText('content')).toBeTruthy();
  });

  it('can start open', () => {
    const { container } = render(
      <Group title="Open one" open>
        <p>x</p>
      </Group>,
    );
    expect(container.querySelector('details')?.hasAttribute('open')).toBe(true);
  });
});
