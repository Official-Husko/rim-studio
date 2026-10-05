import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import ProjectPage from '../ProjectPage';
import { installWithProject } from '../testSupport';

describe('ProjectPage with the link card', () => {
  it('shows Test in RimWorld on its own tab and asks the backend for the link status', async () => {
    const transport = installWithProject();
    renderWithProviders(<ProjectPage />);
    fireEvent.click(await screen.findByRole('tab', { name: 'Test in game' }));
    expect(await screen.findByRole('region', { name: 'Test in RimWorld' })).toBeTruthy();
    expect(await screen.findByText('Not visible to the game')).toBeTruthy();
    expect(transport.calls.some((c) => c.name === 'project_link_status')).toBe(true);
  });
});
