import { render, screen } from '@testing-library/preact';
import { describe, expect, it } from 'vitest';
import ProjectPage from './ProjectPage';

describe('ProjectPage', () => {
  it('shows the placeholder heading', () => {
    render(<ProjectPage />);
    expect(screen.getByRole('heading', { name: 'Project' })).toBeTruthy();
  });
});
