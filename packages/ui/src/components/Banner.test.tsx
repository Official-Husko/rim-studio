import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { Banner } from './Banner';

describe('Banner', () => {
  it('uses the alert role for warnings and errors', () => {
    render(
      <Banner tone="error" title="Save failed">
        The file is read only.
      </Banner>,
    );
    expect(screen.getByRole('alert').textContent).toContain('The file is read only.');
  });

  it('uses the status role for information', () => {
    render(<Banner tone="info">Datasets are fresh.</Banner>);
    expect(screen.getByRole('status')).toBeTruthy();
  });

  it('renders an action and a dismiss button', () => {
    const onDismiss = vi.fn();
    render(
      <Banner
        tone="warning"
        action={<button type="button">Retry</button>}
        dismissLabel="Close banner"
        onDismiss={onDismiss}
      >
        Stale
      </Banner>,
    );
    expect(screen.getByRole('button', { name: 'Retry' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Close banner' }));
    expect(onDismiss).toHaveBeenCalled();
  });
});
