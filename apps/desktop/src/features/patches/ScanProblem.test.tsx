import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { ScanProblem } from './ScanProblem';

describe('ScanProblem', () => {
  it('explains a missing Combat Extended in plain words', () => {
    render(
      <ScanProblem
        error={{ code: 'designer.reference-unavailable', message: 'no data', errorId: 'e' }}
        onRetry={() => {}}
      />,
    );
    expect(screen.getByText('Combat Extended is not available')).toBeTruthy();
    expect(screen.getByText(/numbers are read from them/)).toBeTruthy();
  });

  it('shows another error with its code and retries', () => {
    const onRetry = vi.fn();
    render(
      <ScanProblem
        error={{ code: 'io.failed', message: 'disk', errorId: 'e' }}
        onRetry={onRetry}
      />,
    );
    expect(screen.getByText('io.failed')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(onRetry).toHaveBeenCalled();
  });
});
