import { render, screen } from '@testing-library/preact';
import { describe, expect, it } from 'vitest';
import WeaponsPage from './WeaponsPage';

describe('WeaponsPage', () => {
  it('shows the placeholder heading', () => {
    render(<WeaponsPage />);
    expect(screen.getByRole('heading', { name: 'Weapons' })).toBeTruthy();
  });
});
