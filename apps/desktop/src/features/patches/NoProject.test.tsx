import { render, screen } from '@testing-library/preact';
import { describe, expect, it } from 'vitest';
import { NoProject } from './NoProject';

describe('NoProject', () => {
  it('asks for a folder', () => {
    render(<NoProject />);
    expect(screen.getByText('No mod is open')).toBeTruthy();
    expect(screen.getByRole('link', { name: 'Open the Project page' }).getAttribute('href')).toBe(
      '#/project',
    );
  });

  it('names the folder that could not be found', () => {
    render(<NoProject missing={{ path: '/mods/Gone', reason: 'does not exist' }} />);
    expect(screen.getByText('The mod folder was not found')).toBeTruthy();
    expect(screen.getByText(/\/mods\/Gone/)).toBeTruthy();
    expect(screen.getByText('does not exist')).toBeTruthy();
  });
});
