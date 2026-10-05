import { render, screen } from '@testing-library/preact';
import { describe, expect, it } from 'vitest';
import PatchesPage from './PatchesPage';

describe('PatchesPage', () => {
  it('shows the placeholder heading', () => {
    render(<PatchesPage />);
    expect(screen.getByRole('heading', { name: 'Patches' })).toBeTruthy();
  });
});
