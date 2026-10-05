import { fireEvent, screen, waitFor, within } from '@testing-library/preact';
import type { DraftDto, DraftEntryDto } from 'rimstudio-ipc-types';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { createAssetStore } from '../../asset-store';
import { setupOutput } from '../../output-testSupport';
import { fixture, importsDraft, recordedAssetInfo } from '../../testSupport';
import { ApplyDialog } from './ApplyDialog';
import { OutputPanel } from './OutputPanel';

function entry(): DraftEntryDto {
  const draft: DraftDto = importsDraft();
  return {
    id: 'd-imp',
    defName: draft.spec.identity.defName,
    label: draft.spec.identity.label,
    kind: 'ranged',
    updatedAtMs: 1,
    draft,
  };
}

function setup() {
  const s = setupOutput({
    designer_export_plan: () => fixture('designer-assets-plan-imports'),
    designer_asset_info: recordedAssetInfo,
    designer_apply_plan: () => fixture('designer-assets-apply-imports'),
  });
  const assets = createAssetStore({ projectId: () => 'p-out' });
  s.editor.open(entry());
  return { ...s, assets };
}

describe('OutputPanel with imported files', () => {
  it('lists the copies with a thumbnail of the texture and shows the facts of the selected copy', async () => {
    const s = setup();
    renderWithProviders(
      <OutputPanel
        draftId="d-imp"
        projectPath="/mods/Demo"
        store={s.output}
        editor={s.editor}
        assets={s.assets}
      />,
    );
    await s.flush();
    expect(screen.getByText('6 files')).toBeTruthy();
    const list = screen.getByRole('list', { name: 'Files of the plan' });
    expect(within(list).getAllByText('Sound clip')).toHaveLength(2);
    expect(within(list).getAllByText('Texture')).toHaveLength(2);
    expect(within(list).getByText('Sound definitions')).toBeTruthy();
    await waitFor(() => expect(within(list).getAllByRole('img')).toHaveLength(2));
    // the first file of the plan is a clip: its facts are shown instead of text
    expect(screen.getByText('Target in the project')).toBeTruthy();
    fireEvent.click(
      screen.getByRole('button', {
        name: /^Preview Textures\/Things\/Item\/Equipment\/WeaponRanged\/DM_Carbine\.png/,
      }),
    );
    expect(await screen.findByText('512 by 512 pixels')).toBeTruthy();
    expect(screen.getAllByAltText('Thumbnail of TLWWP_Eagle_Carbine.png')).toHaveLength(1);
  });

  it('lists the copies in the apply dialog, writes them and shows their hashes in the result', async () => {
    const s = setup();
    renderWithProviders(<ApplyDialog store={s.output} projectPath="/mods/Demo" />);
    s.output.start();
    await s.flush();
    s.output.openApply();
    const list = await screen.findByRole('list', { name: 'Files to write', hidden: true });
    expect(list.querySelectorAll('li')).toHaveLength(6);
    expect(list.textContent).toContain('Sounds/Weapons/DM_Carbine_Shot/TLWWP_AK_47_Shot.wav');
    expect(screen.getByText('4 of these files are copied from your computer.')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Write 6 files', hidden: true }));
    expect(await screen.findByText('Wrote 6 files')).toBeTruthy();
    expect(screen.getByText('0987afca4973')).toBeTruthy();
    expect(screen.getAllByText('Sound clip').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Texture').length).toBeGreaterThan(0);
  });
});
