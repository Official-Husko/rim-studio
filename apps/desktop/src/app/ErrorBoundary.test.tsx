import { fireEvent, render, screen, waitFor } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { ErrorBoundary } from './ErrorBoundary';

let shouldThrow = true;
function Bomb() {
  if (shouldThrow) throw new Error('render failed');
  return <p>fine now</p>;
}

describe('ErrorBoundary', () => {
  it('shows an error card with the message and an id, and keeps siblings alive', () => {
    shouldThrow = true;
    render(
      <div>
        <p>sibling</p>
        <ErrorBoundary region="Weapons">
          <Bomb />
        </ErrorBoundary>
      </div>,
    );
    expect(screen.getByText('sibling')).toBeTruthy();
    expect(screen.getByRole('alert').textContent).toContain('render failed');
    expect(screen.getByRole('alert').textContent).toMatch(/Error id c-/);
  });

  it('retries and renders the children again', () => {
    shouldThrow = true;
    render(
      <ErrorBoundary>
        <Bomb />
      </ErrorBoundary>,
    );
    shouldThrow = false;
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(screen.getByText('fine now')).toBeTruthy();
  });

  it('copies diagnostics', async () => {
    shouldThrow = true;
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    render(
      <ErrorBoundary region="Patches">
        <Bomb />
      </ErrorBoundary>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Copy diagnostics' }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Copied' })).toBeTruthy());
    expect(writeText.mock.calls[0]?.[0]).toContain('region: Patches');
    expect(writeText.mock.calls[0]?.[0]).toContain('render failed');
  });
});
