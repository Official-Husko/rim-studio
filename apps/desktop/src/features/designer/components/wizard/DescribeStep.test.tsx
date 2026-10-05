import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { settle } from '../../testSupport';
import { DescribeStep } from './DescribeStep';
import { installWizardTransport, loadedWizard } from './wizardTestSupport';

async function setup(id: string) {
  const transport = installWizardTransport();
  const wizard = await loadedWizard();
  wizard.store.chooseArchetype(id);
  wizard.store.go('describe');
  renderWithProviders(<DescribeStep store={wizard.store} />);
  return { ...wizard, transport };
}

describe('DescribeStep', () => {
  it('shows every descriptor of a bolt action rifle', async () => {
    await setup('rifle/sniper');
    for (const name of ['Action', 'Rate of fire', 'Calibre', 'Handling', 'Tier', 'Strength']) {
      expect(screen.getByRole('region', { name })).toBeTruthy();
    }
    expect(screen.getByText('One round per cycle, worked by hand between shots.')).toBeTruthy();
  });

  it('shows only what applies to a melee weapon', async () => {
    await setup('sword/long');
    expect(screen.queryByRole('region', { name: 'Calibre' })).toBeNull();
    expect(screen.queryByRole('region', { name: 'Action' })).toBeNull();
    expect(screen.getByRole('region', { name: 'Swing speed' })).toBeTruthy();
  });

  it('changes the choice and asks for new numbers', async () => {
    const { store, timers, transport } = await setup('rifle/sniper');
    fireEvent.click(screen.getByRole('radio', { name: 'Semi automatic' }));
    fireEvent.click(screen.getByRole('radio', { name: 'Large' }));
    fireEvent.change(screen.getByRole('combobox', { name: 'Tier' }), {
      target: { value: 'spacer' },
    });
    fireEvent.click(screen.getByRole('radio', { name: 'Stronger' }));
    expect(store.choice.value.descriptors).toMatchObject({
      action: 'semi',
      calibre: 'large',
      tier: 'spacer',
    });
    expect(store.choice.value.balance).toBe('stronger');
    timers.runTimers();
    await settle();
    expect(
      transport.calls.filter((c) => c.name === 'designer_archetype_propose').length,
    ).toBeGreaterThan(0);
  });

  it('counts the reference weapons of each tier', async () => {
    await setup('rifle/sniper');
    expect(screen.getByRole('option', { name: 'Industrial (12 reference weapons)' })).toBeTruthy();
    expect(screen.getByRole('option', { name: 'Medieval (1 reference weapon)' })).toBeTruthy();
    expect(screen.getByRole('option', { name: 'Archotech (0 reference weapons)' })).toBeTruthy();
  });
});
