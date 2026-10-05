import { fireEvent, screen, waitFor } from '@testing-library/preact';
import type { DraftDto, DraftEntryDto } from 'rimstudio-ipc-types';
import { renderWithProviders } from 'rimstudio-testkit';
import { afterEach, describe, expect, it } from 'vitest';
import DesignerPage from '../../DesignerPage';
import { clearProject, setProject } from '../../project-source';
import { createDesigner } from '../../stores';
import { fixture, manualScheduler } from '../../testSupport';
import { installWizardTransport } from './wizardTestSupport';

afterEach(() => clearProject());

function entryWithArchetype(): DraftEntryDto {
  const draft = fixture<{ draft: DraftDto }>('designer-wizard-apply-sniper').draft;
  return {
    id: 'd-w',
    defName: 'RS_Test',
    label: 'Test rifle',
    kind: 'ranged',
    updatedAtMs: 1,
    draft,
  };
}

function setup(drafts: DraftEntryDto[]) {
  installWizardTransport({
    designer_draft_list: () => ({ drafts }),
  });
  setProject({ projectId: 'p-1', path: '/mods/Test', name: 'Test' });
  const stores = createDesigner(() => 'p-1', manualScheduler().scheduler);
  renderWithProviders(<DesignerPage stores={stores} />);
  return stores;
}

describe('the wizard on the Weapons page', () => {
  it('opens from the drafts panel and closes again', async () => {
    setup([]);
    fireEvent.click(await screen.findByRole('button', { name: 'New weapon wizard' }));
    expect(await screen.findByRole('dialog', { name: 'New weapon' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'New weapon' })).toBeNull());
  });

  it('switches to the blank draft dialog', async () => {
    setup([]);
    fireEvent.click(await screen.findByRole('button', { name: 'New weapon wizard' }));
    fireEvent.click(
      await screen.findByRole('button', { name: 'Start with a blank draft instead' }),
    );
    expect(await screen.findByRole('dialog', { name: 'New ranged weapon' })).toBeTruthy();
  });

  it('offers to propose again for a draft the wizard made', async () => {
    const stores = setup([entryWithArchetype()]);
    await stores.drafts.load();
    await stores.select(entryWithArchetype());
    fireEvent.click(await screen.findByRole('button', { name: 'Propose again' }));
    expect(await screen.findByRole('dialog', { name: 'Propose the numbers again' })).toBeTruthy();
  });
});
