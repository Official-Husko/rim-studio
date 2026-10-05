import { render, screen } from '@testing-library/preact';
import { describe, expect, it } from 'vitest';
import { ProgressBar } from './ProgressBar';

describe('ProgressBar', () => {
  it('reports a determinate value as a percentage', () => {
    render(<ProgressBar label="Scan" value={0.42} caption="42 of 100" />);
    const bar = screen.getByRole('progressbar', { name: 'Scan' });
    expect(bar.getAttribute('aria-valuenow')).toBe('42');
    expect(screen.getByText('42 of 100')).toBeTruthy();
  });

  it('omits the value when indeterminate', () => {
    render(<ProgressBar label="Working" />);
    expect(screen.getByRole('progressbar').hasAttribute('aria-valuenow')).toBe(false);
  });

  it('clamps out of range values', () => {
    render(<ProgressBar label="Over" value={3} />);
    expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('100');
  });
});
