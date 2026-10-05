import { fireEvent, screen, waitFor, within } from '@testing-library/preact';
import type { DraftDto, WritePlanDto } from 'rimstudio-ipc-types';
import { renderWithProviders } from 'rimstudio-testkit';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createAssetStore } from '../../asset-store';
import { fixture, importsDraft, installAssetTransport } from '../../testSupport';
import { makeEnv, WithEnv } from './fieldEnvTestkit';
import { SoundsPanel } from './SoundsPanel';

const pickFile = vi.hoisted(() => vi.fn());
vi.mock('~/shared/platform', () => ({ pickFile }));

const plan = () => fixture<WritePlanDto>('designer-assets-plan-imports');
const store = () => createAssetStore({ projectId: () => 'p-1' });
const plain = () => fixture<DraftDto>('designer-output-draft-vanilla').spec;
const GLOCK = '/home/user/Art/TLWWP_Glock_17_Shot.wav';
const AK = '/home/user/Art/TLWWP_AK_47_Shot.wav';

describe('SoundsPanel', () => {
  beforeEach(() => pickFile.mockReset());

  it('lists the clips of a custom shot with their facts, and what will be written', async () => {
    installAssetTransport();
    renderWithProviders(
      <WithEnv env={makeEnv({ spec: importsDraft().spec })}>
        <SoundsPanel assets={store()} plan={plan()} />
      </WithEnv>,
    );
    const clips = within(screen.getByRole('list', { name: 'Clips of the shot sound' }));
    expect(clips.getByText('TLWWP_Glock_17_Shot.wav')).toBeTruthy();
    await waitFor(() => expect(clips.getAllByText('WAV sound')).toHaveLength(2));
    expect(clips.getByText('1.3 s')).toBeTruthy();
    expect(clips.getAllByText('design.sound-stereo').length).toBeGreaterThan(0);
    expect(screen.getByText('DM_Carbine_Shot')).toBeTruthy();
    expect(screen.getByText('Defs/SoundDefs/World_Oneshots_Weapons.xml')).toBeTruthy();
    expect(screen.getByText('2 clips are copied:')).toBeTruthy();
    expect(
      within(screen.getByRole('list', { name: 'Clips that will be copied' })).getAllByRole(
        'listitem',
      ),
    ).toHaveLength(2);
  });

  it('adds a clip through the file dialog with a WAV and Ogg filter', async () => {
    installAssetTransport();
    const spec = { ...importsDraft().spec, sounds: { shot: { clips: [GLOCK] } } };
    const env = makeEnv({ spec });
    pickFile.mockResolvedValueOnce(AK);
    renderWithProviders(
      <WithEnv env={env}>
        <SoundsPanel assets={store()} plan={undefined} />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Add clip' }));
    await waitFor(() =>
      expect(env.setField).toHaveBeenCalledWith('/sounds', { shot: { clips: [GLOCK, AK] } }),
    );
    expect(pickFile).toHaveBeenCalledWith({
      filters: [{ name: 'WAV or Ogg sound', extensions: ['wav', 'ogg'] }],
    });
  });

  it('does not add the same clip twice', async () => {
    installAssetTransport();
    const spec = { ...importsDraft().spec, sounds: { shot: { clips: [GLOCK] } } };
    const env = makeEnv({ spec });
    pickFile.mockResolvedValueOnce(GLOCK);
    renderWithProviders(
      <WithEnv env={env}>
        <SoundsPanel assets={store()} plan={undefined} />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Add clip' }));
    await waitFor(() => expect(pickFile).toHaveBeenCalled());
    await Promise.resolve();
    expect(env.setField).not.toHaveBeenCalled();
  });

  it('removes a clip, and the whole custom sound when it was the last thing in it', () => {
    installAssetTransport();
    const two = makeEnv({ spec: importsDraft().spec });
    const { unmount } = renderWithProviders(
      <WithEnv env={two}>
        <SoundsPanel assets={store()} plan={undefined} />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Remove TLWWP_AK_47_Shot.wav' }));
    expect(two.setField).toHaveBeenCalledWith(
      '/sounds',
      expect.objectContaining({ shot: expect.objectContaining({ clips: [GLOCK] }) }),
    );
    unmount();
    const one = makeEnv({ spec: { ...importsDraft().spec, sounds: { shot: { clips: [GLOCK] } } } });
    renderWithProviders(
      <WithEnv env={one}>
        <SoundsPanel assets={store()} plan={undefined} />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Remove TLWWP_Glock_17_Shot.wav' }));
    expect(one.setField).toHaveBeenCalledWith('/sounds', undefined);
  });

  it('writes a range once both ends are there and asks for the other end before', () => {
    installAssetTransport();
    const env = makeEnv({ spec: { ...importsDraft().spec, sounds: { shot: { clips: [GLOCK] } } } });
    renderWithProviders(
      <WithEnv env={env}>
        <SoundsPanel assets={store()} plan={undefined} />
      </WithEnv>,
    );
    fireEvent.input(screen.getByRole('spinbutton', { name: 'Distance heard, lowest' }), {
      target: { value: '30' },
    });
    expect(screen.getByText('Enter both numbers to write the range.')).toBeTruthy();
    expect(env.setField).not.toHaveBeenCalled();
    fireEvent.input(screen.getByRole('spinbutton', { name: 'Distance heard, highest' }), {
      target: { value: '50' },
    });
    expect(env.setField).toHaveBeenCalledWith('/sounds', {
      shot: { clips: [GLOCK], distance: { min: 30, max: 50 } },
    });
  });

  it('chooses a game sound from the searchable list and writes its name', async () => {
    installAssetTransport();
    const env = makeEnv({ spec: plain() });
    renderWithProviders(
      <WithEnv env={env}>
        <SoundsPanel assets={store()} plan={undefined} />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Browse game sounds for Cast sound' }));
    const list = await screen.findByRole('list', { name: 'Game sounds for Cast sound' });
    await waitFor(() => expect(within(list).getAllByRole('button').length).toBeGreaterThan(5));
    expect(screen.getByText(/First 40 of 1231 sounds/)).toBeTruthy();
    fireEvent.input(screen.getByRole('searchbox', { name: 'Search game sounds for Cast sound' }), {
      target: { value: 'Shot_Bolt' },
    });
    const found = await screen.findByRole(
      'button',
      { name: /Shot_BoltActionRifle/ },
      { timeout: 2000 },
    );
    fireEvent.click(found);
    expect(env.setField).toHaveBeenCalledWith('/ranged/soundCast', 'Shot_BoltActionRifle');
    expect(screen.queryByRole('list', { name: 'Game sounds for Cast sound' })).toBeNull();
  });

  it('switches between a game sound and own clips, and drops the custom sound on the way back', () => {
    installAssetTransport();
    const env = makeEnv({ spec: importsDraft().spec });
    renderWithProviders(
      <WithEnv env={env}>
        <SoundsPanel assets={store()} plan={undefined} />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('radio', { name: 'Game sound' }));
    expect(env.setField).toHaveBeenCalledWith('/sounds', undefined);
  });

  it('shows the empty custom form without changing the draft', () => {
    installAssetTransport();
    const env = makeEnv({ spec: plain() });
    renderWithProviders(
      <WithEnv env={env}>
        <SoundsPanel assets={store()} plan={undefined} />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('radio', { name: 'Own clips' }));
    expect(screen.getByText('No clips yet. Add a WAV or Ogg file.')).toBeTruthy();
    expect(screen.getByText(/Nothing is written for the shot sound yet/)).toBeTruthy();
    expect(env.setField).not.toHaveBeenCalled();
  });

  it('offers only the interact sound for a melee weapon', () => {
    installAssetTransport();
    renderWithProviders(
      <WithEnv env={makeEnv({ spec: fixture<DraftDto>('designer-draft-melee').spec })}>
        <SoundsPanel assets={store()} plan={undefined} />
      </WithEnv>,
    );
    expect(screen.queryByText('Shot sound')).toBeNull();
    expect(screen.getByLabelText('Interact sound')).toBeTruthy();
  });

  it('shows the error of a failed sound search', async () => {
    installAssetTransport({
      defs_search: () => {
        throw { code: 'defs.no-session', message: 'No reference loaded', errorId: 'e' };
      },
    });
    renderWithProviders(
      <WithEnv env={makeEnv({ spec: plain() })}>
        <SoundsPanel assets={store()} plan={undefined} />
      </WithEnv>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Browse game sounds for Cast sound' }));
    expect(await screen.findByText('No reference loaded')).toBeTruthy();
  });
});
