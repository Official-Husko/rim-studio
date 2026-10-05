import { screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { LinkSteps } from './LinkSteps';

describe('LinkSteps', () => {
  it('lists the steps with the mod name', () => {
    renderWithProviders(<LinkSteps name="RS Arms" hasCePatch={false} />);
    expect(screen.getByText('To see it in the game')).toBeTruthy();
    expect(screen.getByText('Open Mods in the main menu and enable RS Arms.')).toBeTruthy();
    expect(screen.queryByText(/Combat Extended/)).toBeNull();
  });

  it('adds the Combat Extended step and its load order note', () => {
    renderWithProviders(<LinkSteps name="RS Arms" hasCePatch />);
    expect(
      screen.getByText(/Enable Combat Extended as well and put RS Arms below it/),
    ).toBeTruthy();
    expect(screen.getByText(/right after Core and the DLCs/)).toBeTruthy();
  });
});
