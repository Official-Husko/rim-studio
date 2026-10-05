import { fireEvent, screen, within } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { settle } from '../../testSupport';
import { ResultStep } from './ResultStep';
import { installWizardTransport, loadedWizard } from './wizardTestSupport';

async function setup(id: string) {
  installWizardTransport();
  const wizard = await loadedWizard();
  wizard.store.chooseArchetype(id);
  wizard.timers.runTimers();
  await settle();
  wizard.store.go('result');
  renderWithProviders(<ResultStep store={wizard.store} />);
  return wizard;
}

describe('ResultStep', () => {
  it('groups the numbers and shows the verdict and the comparison', async () => {
    await setup('rifle/sniper');
    for (const name of [
      'Damage and penetration',
      'Range and accuracy',
      'Firing',
      'Weight and cost',
    ]) {
      expect(screen.getByRole('list', { name })).toBeTruthy();
    }
    expect(screen.getByRole('progressbar', { name: 'Typicality' })).toBeTruthy();
    expect(screen.getByRole('grid', { name: 'Compared with your install' })).toBeTruthy();
    const damage = screen.getByRole('list', { name: 'Damage and penetration' });
    expect(within(damage).getAllByText('Derived').length).toBe(2);
  });

  it('moves the numbers when the target changes and shows what they were', async () => {
    const { store, timers } = await setup('rifle/sniper');
    fireEvent.click(screen.getByRole('radio', { name: 'Stronger' }));
    timers.runTimers();
    await settle();
    expect(store.proposal.value?.choice.balance).toBe('stronger');
    expect(screen.getAllByText(/^was /).length).toBeGreaterThan(3);
  });

  it('shows the attacks of a melee weapon', async () => {
    await setup('sword/long');
    expect(screen.getByRole('list', { name: 'Attacks' })).toBeTruthy();
    expect(screen.getByText('Edge damage')).toBeTruthy();
  });

  it('shows the error of a failed proposal', async () => {
    installWizardTransport({
      designer_archetype_propose: () => {
        throw { code: 'designer.archetype-failed', message: 'no pool', errorId: 'e-3' };
      },
    });
    const { store, timers } = await loadedWizard();
    store.chooseArchetype('rifle/sniper');
    timers.runTimers();
    await settle();
    renderWithProviders(<ResultStep store={store} />);
    expect(screen.getByText('no pool')).toBeTruthy();
  });

  it('shows a waiting state before the first proposal', async () => {
    installWizardTransport();
    const { store } = await loadedWizard();
    renderWithProviders(<ResultStep store={store} />);
    expect(screen.getAllByText('Working out the numbers').length).toBeGreaterThan(0);
  });
});
