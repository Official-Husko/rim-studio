import { fireEvent, screen, waitFor } from '@testing-library/preact';
import type { DiagnosticDto, DraftDto, WritePlanDto } from 'rimstudio-ipc-types';
import { renderWithProviders } from 'rimstudio-testkit';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createAssetStore } from '../../asset-store';
import { cloneEntry, fixture, importsDraft, installAssetTransport } from '../../testSupport';
import { makeEnv, WithEnv } from './fieldEnvTestkit';
import { TexturePanel } from './TexturePanel';

const pickFile = vi.hoisted(() => vi.fn());
vi.mock('~/shared/platform', () => ({ pickFile }));

const plan = () => fixture<WritePlanDto>('designer-assets-plan-imports');
const store = () => createAssetStore({ projectId: () => 'p-1' });

describe('TexturePanel', () => {
  beforeEach(() => pickFile.mockReset());

  it('shows the imported texture with its thumbnail, facts and target', async () => {
    installAssetTransport();
    renderWithProviders(
      <WithEnv env={makeEnv({ spec: importsDraft().spec })}>
        <TexturePanel assets={store()} plan={plan()} />
      </WithEnv>,
    );
    expect(screen.getByText(/never copied/)).toBeTruthy();
    expect(
      screen.getByText(/copied to Things\/Item\/Equipment\/WeaponRanged\/DM_Carbine in your mod/),
    ).toBeTruthy();
    await waitFor(() => expect(screen.getAllByText('PNG image')).toHaveLength(2));
    const img = screen.getByAltText('Thumbnail of TLWWP_Eagle_Carbine.png') as HTMLImageElement;
    expect(img.src.startsWith('data:image/png;base64,')).toBe(true);
    expect(screen.getAllByText('512 by 512 pixels')).toHaveLength(2);
    expect(screen.getByText('6865c3284585')).toBeTruthy();
    expect(screen.getByText('Projectile texture')).toBeTruthy();
  });

  it('imports a chosen PNG into the draft', async () => {
    installAssetTransport();
    const env = makeEnv({ spec: fixture<DraftDto>('designer-fields-draft-rifle-own-edited').spec });
    pickFile.mockResolvedValueOnce('/home/user/Art/TLWWP_Eagle_Carbine.png');
    renderWithProviders(
      <WithEnv env={env}>
        <TexturePanel assets={store()} plan={undefined} />
      </WithEnv>,
    );
    expect(
      screen.getByText(/Shared texture Things\/Item\/Equipment\/WeaponRanged\/BoltActionRifle/),
    ).toBeTruthy();
    fireEvent.click(screen.getAllByRole('button', { name: 'Import PNG' })[0] as HTMLElement);
    await waitFor(() =>
      expect(env.setField).toHaveBeenCalledWith('/assets', {
        texture: '/home/user/Art/TLWWP_Eagle_Carbine.png',
      }),
    );
    expect(pickFile).toHaveBeenCalledWith({
      filters: [{ name: 'PNG image', extensions: ['png'] }],
    });
  });

  it('removes an import and drops the member when it was the only one', async () => {
    installAssetTransport();
    const spec = {
      ...importsDraft().spec,
      assets: { texture: '/home/user/Art/TLWWP_Eagle_Carbine.png' },
    };
    const env = makeEnv({ spec });
    renderWithProviders(
      <WithEnv env={env}>
        <TexturePanel assets={store()} plan={undefined} />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Remove import' }));
    expect(env.setField).toHaveBeenCalledWith('/assets', undefined);
  });

  it('shows the problems of the plan and of the file under the field', async () => {
    installAssetTransport();
    const bad = fixture<DraftDto>('designer-assets-draft-problems').spec;
    const problems = fixture<WritePlanDto>('designer-assets-plan-problems').diagnostics;
    renderWithProviders(
      <WithEnv env={makeEnv({ spec: bad, diagnostics: problems })}>
        <TexturePanel assets={store()} plan={undefined} />
      </WithEnv>,
    );
    expect(await screen.findByText('design.texture-missing')).toBeTruthy();
    expect(screen.getByText('The file is not there.')).toBeTruthy();
    expect(document.querySelector('[data-field="/assets/texture"]')).toBeTruthy();
  });

  it('shows a diagnostic once when the file facts and the plan say the same', async () => {
    installAssetTransport();
    const same: DiagnosticDto[] = [];
    renderWithProviders(
      <WithEnv env={makeEnv({ spec: importsDraft().spec, diagnostics: same })}>
        <TexturePanel assets={store()} plan={plan()} />
      </WithEnv>,
    );
    await waitFor(() => expect(screen.getAllByText('PNG image').length).toBeGreaterThan(0));
    expect(screen.queryAllByText('design.texture-missing')).toHaveLength(0);
  });

  it('asks for an own projectile before a projectile texture, and offers no import for melee', () => {
    installAssetTransport();
    const shared = cloneEntry().draft.spec;
    const { unmount } = renderWithProviders(
      <WithEnv env={makeEnv({ spec: { ...shared, assets: undefined } })}>
        <TexturePanel assets={store()} plan={undefined} />
      </WithEnv>,
    );
    expect(screen.getByText(/needs an own projectile/)).toBeTruthy();
    expect(screen.queryByText('Projectile texture')).toBeNull();
    unmount();
    renderWithProviders(
      <WithEnv env={makeEnv({ spec: fixture<DraftDto>('designer-draft-melee').spec })}>
        <TexturePanel assets={store()} plan={undefined} />
      </WithEnv>,
    );
    expect(screen.queryByText(/needs an own projectile/)).toBeNull();
    expect(screen.getByText('Weapon texture')).toBeTruthy();
  });

  it('shows the error of a failed dialog', async () => {
    installAssetTransport();
    pickFile.mockRejectedValueOnce({
      code: 'platform.native-failed',
      message: 'no dialog',
      errorId: 'e',
    });
    renderWithProviders(
      <WithEnv env={makeEnv({ spec: fixture<DraftDto>('designer-output-draft-vanilla').spec })}>
        <TexturePanel assets={store()} plan={undefined} />
      </WithEnv>,
    );
    fireEvent.click(screen.getAllByRole('button', { name: 'Import PNG' })[0] as HTMLElement);
    expect(await screen.findByText('no dialog')).toBeTruthy();
  });
});
