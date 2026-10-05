import { fireEvent, screen, waitFor } from '@testing-library/preact';
import type { DraftDto } from 'rimstudio-ipc-types';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { createDesigner } from '../../stores';
import { fixture, manualScheduler, settle } from '../../testSupport';
import { WeaponWizard } from './WeaponWizard';
import { installWizardTransport } from './wizardTestSupport';

function setup(props: Partial<Parameters<typeof WeaponWizard>[0]> = {}) {
  const transport = installWizardTransport({
    designer_draft_save: () => ({ id: 'd-new', savedAtMs: 5 }),
  });
  const timers = manualScheduler();
  const stores = createDesigner(() => 'p-1', timers.scheduler);
  const onClose = vi.fn();
  renderWithProviders(
    <WeaponWizard
      stores={stores}
      projectId={() => 'p-1'}
      scheduler={timers.scheduler}
      onClose={onClose}
      {...props}
    />,
  );
  return { transport, timers, stores, onClose };
}

async function pickSniper(timers: ReturnType<typeof manualScheduler>) {
  fireEvent.click(await screen.findByRole('button', { name: 'Rifle' }));
  fireEvent.click(await screen.findByRole('button', { name: 'Sniper rifle' }));
  fireEvent.click(screen.getByRole('button', { name: 'Next' }));
  timers.runTimers();
  await settle();
}

describe('WeaponWizard', () => {
  it('starts on the type step and cannot go on before a type is chosen', async () => {
    setup();
    expect(await screen.findByRole('button', { name: 'Sniper rifle' })).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Next' }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByRole('button', { name: /^Describe/ }).hasAttribute('disabled')).toBe(true);
  });

  it('walks through the steps, creates the draft and opens it in the editor', async () => {
    const { timers, stores, onClose, transport } = setup();
    await pickSniper(timers);
    expect(screen.getByRole('region', { name: 'Rate of fire' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    expect(await screen.findByRole('list', { name: 'Damage and penetration' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    fireEvent.input(screen.getByRole('textbox', { name: 'Label' }), {
      target: { value: 'Test rifle' },
    });
    fireEvent.input(screen.getByRole('textbox', { name: /Def name$/ }), {
      target: { value: 'RS_Test' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Create' }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(transport.calls.some((c) => c.name === 'designer_draft_save')).toBe(true);
    expect(stores.editor.entryId.value).toBe('d-new');
    expect(stores.editor.draft.value?.archetype?.archetype).toBe('rifle/sniper');
    expect(stores.drafts.entries.value[0]?.id).toBe('d-new');
  });

  it('keeps Create off until the weapon has a def name', async () => {
    const { timers } = setup();
    await pickSniper(timers);
    fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Next' }));
    expect((screen.getByRole('button', { name: 'Create' }) as HTMLButtonElement).disabled).toBe(
      true,
    );
  });

  it('goes back a step and offers a blank draft on the first step', async () => {
    const onBlank = vi.fn();
    const { timers } = setup({ onBlank });
    fireEvent.click(
      await screen.findByRole('button', { name: 'Start with a blank draft instead' }),
    );
    expect(onBlank).toHaveBeenCalled();
    await pickSniper(timers);
    fireEvent.click(screen.getByRole('button', { name: 'Back' }));
    expect(await screen.findByRole('button', { name: 'Sniper rifle' })).toBeTruthy();
  });

  it('closes on Cancel', async () => {
    const { onClose } = setup();
    fireEvent.click(await screen.findByRole('button', { name: 'Cancel' }));
    expect(onClose).toHaveBeenCalled();
  });

  it('retunes an open draft: two steps, applied to the draft that is open', async () => {
    const draft = fixture<{ draft: DraftDto }>('designer-wizard-apply-sniper').draft;
    const { timers, stores, onClose, transport } = setup({ retune: draft });
    stores.editor.open({
      id: 'd-1',
      defName: 'RS_Test',
      label: 'Test rifle',
      kind: 'ranged',
      updatedAtMs: 1,
      draft,
    });
    expect(await screen.findByRole('region', { name: 'Describe' })).toBeTruthy();
    timers.runTimers();
    await settle();
    expect(screen.queryByRole('button', { name: /^Name/ })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Apply to draft' }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    const request = transport.calls.find((c) => c.name === 'designer_archetype_apply')?.request as {
      draft: DraftDto;
    };
    expect(request.draft.spec.identity.defName).toBe('RS_Test');
  });
});
