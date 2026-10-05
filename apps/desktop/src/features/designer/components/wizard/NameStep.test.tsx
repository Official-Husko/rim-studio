import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { settle } from '../../testSupport';
import { NameStep } from './NameStep';
import { installWizardTransport, loadedWizard } from './wizardTestSupport';

async function setup() {
  installWizardTransport();
  const wizard = await loadedWizard();
  wizard.store.chooseArchetype('rifle/sniper');
  wizard.timers.runTimers();
  await settle();
  const onSubmit = vi.fn();
  renderWithProviders(<NameStep store={wizard.store} onSubmit={onSubmit} />);
  return { ...wizard, onSubmit };
}

describe('NameStep', () => {
  it('follows the label with a def name until the user edits it', async () => {
    const { store } = await setup();
    fireEvent.input(screen.getByRole('textbox', { name: 'Def name prefix' }), {
      target: { value: 'RS_' },
    });
    fireEvent.input(screen.getByRole('textbox', { name: 'Label' }), {
      target: { value: 'Long rifle' },
    });
    expect(store.defName.value).toBe('RS_LongRifle');
    fireEvent.input(screen.getByRole('textbox', { name: /Def name$/ }), {
      target: { value: 'RS_Mine' },
    });
    fireEvent.input(screen.getByRole('textbox', { name: 'Label' }), { target: { value: 'Other' } });
    expect(store.defName.value).toBe('RS_Mine');
  });

  it('says what will be created and that Combat Extended stays off', async () => {
    await setup();
    expect(screen.getByText('Vanilla by default')).toBeTruthy();
    expect(screen.getByText(/The Combat Extended patch is off/)).toBeTruthy();
    expect(screen.getByLabelText('What will be created').textContent).toContain('Sniper rifle');
  });

  it('submits from the keyboard', async () => {
    const { onSubmit } = await setup();
    fireEvent.submit(
      screen.getByRole('textbox', { name: 'Label' }).closest('form') as HTMLFormElement,
    );
    expect(onSubmit).toHaveBeenCalled();
  });

  it('shows the error of a failed create', async () => {
    const { store } = await setup();
    store.error.value = { code: 'designer.save-failed', message: 'disk full', errorId: 'e-9' };
    fireEvent.input(screen.getByRole('textbox', { name: 'Label' }), { target: { value: 'x' } });
    expect(screen.getByText('disk full')).toBeTruthy();
  });
});
