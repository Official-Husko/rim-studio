import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it } from 'vitest';
import { Panel } from './Panel';

describe('Panel', () => {
  it('is a labelled region with its title', () => {
    render(<Panel title="Readouts">Body</Panel>);
    expect(screen.getByRole('region', { name: 'Readouts' })).toBeTruthy();
    expect(screen.getByText('Body')).toBeTruthy();
  });

  it('collapses and expands from the header button', () => {
    render(
      <Panel title="Fit" collapsible>
        Hidden soon
      </Panel>,
    );
    const toggle = screen.getByRole('button', { name: 'Fit' });
    expect(toggle.getAttribute('aria-expanded')).toBe('true');
    fireEvent.click(toggle);
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    expect(screen.getByText('Hidden soon').closest('[hidden]')).not.toBeNull();
  });

  it('can start collapsed and shows actions', () => {
    render(
      <Panel title="Raw" collapsible defaultCollapsed actions={<button type="button">Copy</button>}>
        x
      </Panel>,
    );
    expect(screen.getByRole('button', { name: 'Raw' }).getAttribute('aria-expanded')).toBe('false');
    expect(screen.getByRole('button', { name: 'Copy' })).toBeTruthy();
  });
});
