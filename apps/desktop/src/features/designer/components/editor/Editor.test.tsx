import { fireEvent, screen, waitFor } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { createDesigner } from '../../stores';
import type { DraftDto } from 'rimstudio-ipc-types';
import { cloneEntry, fixture, installTransport, manualScheduler, settle } from '../../testSupport';
import { Editor } from './Editor';

async function open() {
  const transport = installTransport();
  const clock = manualScheduler();
  const stores = createDesigner(() => 'p-1', clock.scheduler);
  stores.editor.open(cloneEntry());
  clock.runFrames();
  clock.runTimers();
  await settle();
  return { transport, clock, stores };
}

describe('Editor', () => {
  it('shows the header, the live readouts, the fit and the clone diff of the real clone', async () => {
    const { stores } = await open();
    renderWithProviders(<Editor designer={stores} />);
    expect(screen.getByRole('heading', { name: 'fixture rifle' })).toBeTruthy();
    expect(screen.getByText('Live readouts')).toBeTruthy();
    expect(screen.getByRole('meter', { name: 'Damage' })).toBeTruthy();
    expect(screen.getByText('Changes from bolt-action rifle')).toBeTruthy();
    expect(screen.getByText('Saved')).toBeTruthy();
  });

  it('edits a field, marks the draft unsaved and asks for a new preview', async () => {
    const { stores, clock, transport } = await open();
    renderWithProviders(<Editor designer={stores} />);
    const before = transport.calls.filter((c) => c.name === 'designer_preview').length;
    fireEvent.input(screen.getByRole('spinbutton', { name: /^Damage/ }), {
      target: { value: '25' },
    });
    expect(stores.editor.saveState.value).toBe('dirty');
    expect(stores.editor.draft.value?.spec.ranged?.damage).toEqual({ value: 25, source: 'typed' });
    clock.runFrames();
    await settle();
    expect(transport.calls.filter((c) => c.name === 'designer_preview').length).toBe(before + 1);
  });

  it('saves when the focus leaves the form', async () => {
    const { stores, transport } = await open();
    renderWithProviders(<Editor designer={stores} />);
    const input = screen.getByRole('spinbutton', { name: /^Damage/ });
    fireEvent.input(input, { target: { value: '26' } });
    fireEvent.focusOut(input);
    await waitFor(() =>
      expect(transport.calls.some((c) => c.name === 'designer_draft_save')).toBe(true),
    );
  });

  it('focuses the field a diagnostic names', async () => {
    const { stores } = await open();
    renderWithProviders(<Editor designer={stores} />);
    const go = screen.getAllByRole('button', { name: 'Go to field' })[0];
    expect(go).toBeTruthy();
    fireEvent.click(go as HTMLElement);
    expect(document.activeElement?.tagName).toMatch(/INPUT|SELECT|TEXTAREA|BUTTON/);
  });

  it('opens the quiz from the calibrate mode', async () => {
    const { stores } = await open();
    const current = stores.editor.draft.value;
    if (current) stores.editor.edit({ ...current, calibration: 'quiz' });
    renderWithProviders(<Editor designer={stores} />);
    fireEvent.click(screen.getByRole('button', { name: /the quiz/ }));
    await waitFor(() => expect(stores.quiz.open.value).toBe(true));
  });

  it('shows the error of a failed preview without losing the form', async () => {
    const { stores } = await open();
    stores.editor.error.value = {
      code: 'designer.reference-unavailable',
      message: 'no game install',
      errorId: 'e-1',
    };
    renderWithProviders(<Editor designer={stores} />);
    expect(screen.getByText('no game install')).toBeTruthy();
    expect(screen.getByRole('spinbutton', { name: /^Damage/ })).toBeTruthy();
  });

  it('shows the notes of a fresh clone with that clone only, and they can be dismissed', async () => {
    const { stores } = await open();
    stores.drafts.notes.value = ['the clone shares a thing with its source'];
    stores.drafts.notesFor.value = stores.editor.entryId.value;
    renderWithProviders(<Editor designer={stores} />);
    expect(screen.getByText('the clone shares a thing with its source')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss the clone notes' }));
    expect(screen.queryByText('the clone shares a thing with its source')).toBeNull();
  });

  it('hides the notes of another draft', async () => {
    const { stores } = await open();
    stores.drafts.notes.value = ['an old note'];
    stores.drafts.notesFor.value = 'd-other';
    renderWithProviders(<Editor designer={stores} />);
    expect(screen.queryByText('an old note')).toBeNull();
  });

  it('turns the own projectile on through the backend and shows the own projectile fields', async () => {
    const own = fixture<{ draft: DraftDto; notes?: string[] }>('designer-fields-projectile-own');
    installTransport({ designer_projectile_own: () => own });
    const clock = manualScheduler();
    const stores = createDesigner(() => 'p-1', clock.scheduler);
    stores.editor.open(cloneEntry());
    renderWithProviders(<Editor designer={stores} />);
    fireEvent.click(screen.getByRole('switch', { name: 'Own projectile' }));
    await waitFor(() =>
      expect(screen.getByText(/Copied from the projectile Bullet_BoltActionRifle/)).toBeTruthy(),
    );
  });
});
