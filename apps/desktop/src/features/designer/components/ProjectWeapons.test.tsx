import { fireEvent, screen, waitFor, within } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import type { DesignerProjectWeaponsResponse } from 'rimstudio-ipc-types';
import { createDraftsStore } from '../drafts-store';
import { createProjectWeaponsStore } from '../project-weapons-store';
import { installTransport, openWeaponAnswer, projectWeaponsAnswer } from '../testSupport';
import { ProjectWeapons } from './ProjectWeapons';

const RANGER_DRAFT = 'd-rs-ranger';

interface Options {
  /** The answer of the list; the fixture by default. */
  list?: DesignerProjectWeaponsResponse;
  /** Give the first weapon a stored draft that the session holds. */
  withDraft?: boolean;
  openId?: string;
  /** Do not wait for the list: the call never answers. */
  neverAnswers?: boolean;
}

async function setup(options: Options = {}) {
  const list = options.list ?? projectWeaponsAnswer();
  const first = list.weapons[0];
  if (options.withDraft && first) first.draftId = RANGER_DRAFT;
  const transport = installTransport({
    designer_project_weapons: () => (options.neverAnswers ? new Promise(() => undefined) : list),
  });
  const drafts = createDraftsStore({ projectId: () => 'p-1' });
  if (options.withDraft) drafts.upsert(openWeaponAnswer().entry);
  const store = createProjectWeaponsStore({ projectId: () => 'p-1', drafts });
  if (options.neverAnswers) void store.load();
  else await store.load();
  const onOpened = vi.fn();
  renderWithProviders(<ProjectWeapons store={store} openId={options.openId} onOpened={onOpened} />);
  return { transport, drafts, store, onOpened };
}

function row(defName: string): HTMLElement {
  const found = document.querySelector<HTMLElement>(`li[data-def="${defName}"]`);
  if (!found) throw new Error(`no row for ${defName}`);
  return found;
}

describe('ProjectWeapons', () => {
  it('lists the weapons of the mod with their kind, definition name and key numbers', async () => {
    await setup();
    expect(screen.getByRole('heading', { name: "This mod's weapons" })).toBeTruthy();
    expect(screen.getByText('3 weapons')).toBeTruthy();
    const ranger = within(row('RS_Gun_Ranger'));
    expect(ranger.getByText('ranger rifle')).toBeTruthy();
    expect(ranger.getByText('Ranged')).toBeTruthy();
    expect(ranger.getByText('RS_Gun_Ranger')).toBeTruthy();
    const numbers = within(ranger.getByRole('list', { name: 'Key numbers of ranger rifle' }));
    expect(numbers.getAllByRole('listitem').map((li) => li.textContent)).toEqual([
      'Damage 16',
      'Range 32.9',
      'Cooldown 1.6',
    ]);
    const cleaver = within(row('RS_Melee_Cleaver'));
    expect(cleaver.getByText('Melee')).toBeTruthy();
    expect(cleaver.getByText('Damage 11')).toBeTruthy();
    expect(cleaver.getByText('Cooldown 2')).toBeTruthy();
    expect(cleaver.queryByText(/^Range/)).toBeNull();
  });

  it('says "1 weapon" for a single weapon', async () => {
    const list = projectWeaponsAnswer();
    list.weapons = list.weapons.slice(0, 1);
    await setup({ list });
    expect(screen.getByText('1 weapon')).toBeTruthy();
  });

  it('marks the weapon that has a generated Combat Extended patch', async () => {
    await setup();
    expect(within(row('RS_Gun_Scatter')).getByText('Combat Extended patch')).toBeTruthy();
    expect(within(row('RS_Gun_Ranger')).queryByText('Combat Extended patch')).toBeNull();
    expect(within(row('RS_Melee_Cleaver')).queryByText('Combat Extended patch')).toBeNull();
  });

  it('marks a weapon that has a draft and offers to continue it', async () => {
    await setup({ withDraft: true });
    const ranger = within(row('RS_Gun_Ranger'));
    expect(ranger.getByText('Draft')).toBeTruthy();
    expect(ranger.getByRole('button', { name: 'Continue the draft of ranger rifle' })).toBeTruthy();
    expect(ranger.getByText('Continue')).toBeTruthy();
    const scatter = within(row('RS_Gun_Scatter'));
    expect(scatter.queryByText('Draft')).toBeNull();
    expect(scatter.getByText('Load')).toBeTruthy();
  });

  it('drops the draft mark when the session no longer holds the draft', async () => {
    const { drafts } = await setup({ withDraft: true });
    drafts.entries.value = [];
    await waitFor(() => expect(within(row('RS_Gun_Ranger')).queryByText('Draft')).toBeNull());
    expect(within(row('RS_Gun_Ranger')).getByText('Load')).toBeTruthy();
  });

  it('marks the weapon whose draft is open in the editor', async () => {
    await setup({ withDraft: true, openId: RANGER_DRAFT });
    expect(row('RS_Gun_Ranger').getAttribute('aria-current')).toBe('true');
    expect(row('RS_Gun_Scatter').getAttribute('aria-current')).toBeNull();
  });

  it('shows the details of a weapon on demand and leaves out what the definition lacks', async () => {
    await setup();
    const ranger = within(row('RS_Gun_Ranger'));
    const toggle = ranger.getByRole('button', { name: 'Details of ranger rifle' });
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    expect(ranger.queryByText('Parent base')).toBeNull();
    fireEvent.click(toggle);
    expect(toggle.getAttribute('aria-expanded')).toBe('true');
    expect(ranger.getByText('Defs/ThingDefs_Weapons/RS_Weapons.xml')).toBeTruthy();
    expect(ranger.getByText('BaseHumanMakeableGun')).toBeTruthy();
    expect(ranger.getByText('Industrial')).toBeTruthy();
    expect(ranger.getByText('RS_Bullet_Ranger')).toBeTruthy();
    expect(ranger.getByText('RS_Ranger')).toBeTruthy();
    expect(ranger.getByText('Ranged, LongShots')).toBeTruthy();
    for (const label of ['Warmup', 'Burst count', 'Mass', 'Market value']) {
      expect(ranger.getByText(label)).toBeTruthy();
    }
    fireEvent.click(toggle);
    expect(ranger.queryByText('Parent base')).toBeNull();

    const cleaver = within(row('RS_Melee_Cleaver'));
    fireEvent.click(cleaver.getByRole('button', { name: 'Details of bone cleaver' }));
    expect(cleaver.getByText('Neolithic')).toBeTruthy();
    expect(cleaver.queryByText('Projectile def')).toBeNull();
    expect(cleaver.queryByText('Weapon tags')).toBeNull();
    expect(cleaver.queryByText('Warmup')).toBeNull();
  });

  it('loads a weapon into the editor', async () => {
    const { transport, onOpened } = await setup();
    fireEvent.click(
      within(row('RS_Gun_Ranger')).getByRole('button', {
        name: 'Load ranger rifle into the editor',
      }),
    );
    await waitFor(() => expect(onOpened).toHaveBeenCalledTimes(1));
    expect(onOpened.mock.calls[0]?.[0]).toMatchObject({
      id: RANGER_DRAFT,
      draft: { editsFile: 'Defs/ThingDefs_Weapons/RS_Weapons.xml' },
    });
    expect(transport.calls.at(-1)).toEqual({
      name: 'designer_open_weapon',
      request: { projectId: 'p-1', defName: 'RS_Gun_Ranger' },
    });
    await waitFor(() =>
      expect(
        within(row('RS_Gun_Ranger')).getByRole('button', {
          name: 'Continue the draft of ranger rifle',
        }),
      ).toBeTruthy(),
    );
  });

  it('continues a stored draft without asking the backend again', async () => {
    const { transport, onOpened } = await setup({ withDraft: true });
    fireEvent.click(
      within(row('RS_Gun_Ranger')).getByRole('button', {
        name: 'Continue the draft of ranger rifle',
      }),
    );
    await waitFor(() => expect(onOpened).toHaveBeenCalledTimes(1));
    expect(onOpened.mock.calls[0]?.[0].id).toBe(RANGER_DRAFT);
    expect(transport.calls.some((c) => c.name === 'designer_open_weapon')).toBe(false);
  });

  it('makes every row wait while one weapon is being opened', async () => {
    const { store } = await setup();
    store.opening.value = 'RS_Gun_Scatter';
    await waitFor(() => {
      const busy = within(row('RS_Gun_Scatter')).getByRole('button', {
        name: 'Load scatter gun into the editor',
      });
      expect(busy.getAttribute('aria-busy')).toBe('true');
    });
    const buttons = screen.getAllByRole('button', { name: /^Load .* into the editor$/ });
    expect(buttons.length).toBe(3);
    expect(buttons.every((b) => (b as HTMLButtonElement).disabled)).toBe(true);
  });

  it('shows what is being done before the first answer arrives', async () => {
    await setup({ neverAnswers: true });
    expect(screen.getByText('Reading the weapons of this mod')).toBeTruthy();
    expect(screen.getByRole('status', { name: 'Loading' })).toBeTruthy();
    expect(screen.queryByText('No weapon is defined in this mod yet')).toBeNull();
  });

  it('says so when the mod defines no weapon', async () => {
    await setup({ list: { weapons: [], diagnostics: [] } });
    expect(screen.getByText('No weapon is defined in this mod yet')).toBeTruthy();
    expect(screen.getByText('0 weapons')).toBeTruthy();
  });

  it('shows the error of a failed read and reads again on retry', async () => {
    let fail = true;
    installTransport({
      designer_project_weapons: () => {
        if (fail)
          throw { code: 'designer.reference-unavailable', message: 'no game', errorId: 'e-1' };
        return projectWeaponsAnswer();
      },
    });
    const drafts = createDraftsStore({ projectId: () => 'p-1' });
    const store = createProjectWeaponsStore({ projectId: () => 'p-1', drafts });
    await store.load();
    renderWithProviders(
      <ProjectWeapons store={store} openId={undefined} onOpened={() => undefined} />,
    );
    expect(screen.getByText('designer.reference-unavailable')).toBeTruthy();
    expect(screen.getByText('no game')).toBeTruthy();
    expect(screen.queryByText('No weapon is defined in this mod yet')).toBeNull();
    fail = false;
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    await waitFor(() => expect(screen.getByText('ranger rifle')).toBeTruthy());
    expect(screen.queryByText('no game')).toBeNull();
  });

  it('shows the error of a weapon that could not be opened above the list', async () => {
    const { store, onOpened } = await setup();
    installTransport({
      designer_open_weapon: () => {
        throw { code: 'designer.invalid-draft', message: 'not a weapon', errorId: 'e-2' };
      },
    });
    fireEvent.click(
      within(row('RS_Gun_Ranger')).getByRole('button', {
        name: 'Load ranger rifle into the editor',
      }),
    );
    await waitFor(() => expect(screen.getByText('not a weapon')).toBeTruthy());
    expect(onOpened).not.toHaveBeenCalled();
    expect(store.weapons.value).toHaveLength(3);
    expect(screen.getByText('ranger rifle')).toBeTruthy();
  });

  it('shows the problems found while reading the mod', async () => {
    const list = projectWeaponsAnswer();
    list.diagnostics = [
      {
        code: 'designer.def-file-unreadable',
        severity: 'warning',
        message: 'Defs/Broken.xml could not be read: not well formed',
      },
    ];
    await setup({ list });
    expect(screen.getByText('1 problem while reading this mod')).toBeTruthy();
    expect(screen.getByText(/Defs\/Broken\.xml could not be read/)).toBeTruthy();
    expect(screen.getByText('designer.def-file-unreadable')).toBeTruthy();
    expect(screen.getByText('ranger rifle')).toBeTruthy();
  });
});
