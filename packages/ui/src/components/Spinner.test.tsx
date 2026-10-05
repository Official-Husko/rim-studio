import { render, screen } from '@testing-library/preact';
import { describe, expect, it } from 'vitest';
import { Spinner } from './Spinner';

describe('Spinner', () => {
  it('has a status role and a name', () => {
    render(<Spinner label="Scanning" />);
    expect(screen.getByRole('status', { name: 'Scanning' })).toBeTruthy();
  });

  it('defaults the name to Loading', () => {
    render(<Spinner size="md" />);
    expect(screen.getByRole('status', { name: 'Loading' })).toBeTruthy();
  });
});
