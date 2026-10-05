import { screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { settle } from '../../testSupport';
import { LiveSummary } from './LiveSummary';
import { installWizardTransport, loadedWizard } from './wizardTestSupport';

describe('LiveSummary', () => {
  it('asks for a type before anything is chosen', async () => {
    installWizardTransport();
    const { store } = await loadedWizard();
    renderWithProviders(<LiveSummary store={store} />);
    expect(screen.getByText('Pick a weapon type to see its numbers here.')).toBeTruthy();
  });

  it('shows the type, the choices, the verdict and the headline numbers', async () => {
    installWizardTransport();
    const { store, timers } = await loadedWizard();
    store.chooseArchetype('rifle/sniper');
    timers.runTimers();
    await settle();
    renderWithProviders(<LiveSummary store={store} />);
    expect(screen.getByRole('heading', { name: 'Sniper rifle' })).toBeTruthy();
    expect(screen.getByText('plausible')).toBeTruthy();
    expect(screen.getByLabelText('Your choices').textContent).toContain('Calibre');
    expect(screen.getByText('Bolt action')).toBeTruthy();
    expect(screen.getByLabelText('Headline numbers').textContent).toContain('Range');
  });
});
