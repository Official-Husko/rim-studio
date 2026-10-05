import { fireEvent, screen, waitFor, within } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import type { ConvertScanDto, WritePlanDto } from 'rimstudio-ipc-types';
import { beforeEach, describe, expect, it } from 'vitest';
import PatchesPage from './PatchesPage';
import { effectiveBlock, ownAnswers, setAnswer, setBlock, setSkipUnderBarrel } from './answerStore';
import { QuestionsForm } from './QuestionsForm';
import { buildRequest } from './model';
import { installTransport, openFixtureProject } from './testSupport';

beforeEach(() => {
  window.location.hash = '#/patches';
});

const bows = loadFixture<ConvertScanDto>('patches-ce-scan-bows');

function bow(name: string) {
  const found = bows.candidates.find((c) => c.defName === name);
  if (!found) throw new Error(`fixture ${name}`);
  return found;
}

describe('Patches page with bows', () => {
  it('lists the bows as convertible with their asks and the converted one as already converted', async () => {
    installTransport({ designer_convert_scan: () => bows });
    openFixtureProject();
    renderWithProviders(<PatchesPage />);
    const table = await screen.findByRole('grid', { name: 'Weapons of the mod' });
    expect(within(table).getAllByText('Not converted')).toHaveLength(2);
    expect(within(table).getAllByText('Already CE')).toHaveLength(1);
    expect(within(table).queryByText(/not available|unsupported/i)).toBeNull();
    expect(screen.getByText('2 not converted')).toBeTruthy();
  });

  it('asks for an arrow set first and keeps the options tab beside the questions', async () => {
    installTransport({
      designer_convert_scan: () => bows,
      designer_export_plan: () => loadFixture<WritePlanDto>('patches-ce-plan-bow-convert'),
    });
    openFixtureProject();
    renderWithProviders(<PatchesPage />);
    const table = await screen.findByRole('grid', { name: 'Weapons of the mod' });
    fireEvent.click(within(table).getByText('UC_RecurveBow'));
    expect(await screen.findByRole('tab', { name: 'Options' })).toBeTruthy();
    expect(screen.getByRole('tab', { name: /Questions/ })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Browse all ammo' })).toBeTruthy();
    expect(screen.getByRole('group', { name: 'Quick picks, best fit first' })).toBeTruthy();
  });

  it('writes the options of the weapon into the overrides of its request', async () => {
    installTransport({
      designer_convert_scan: () => bows,
      designer_export_plan: () => loadFixture<WritePlanDto>('patches-ce-plan-bow-convert'),
    });
    openFixtureProject();
    renderWithProviders(<PatchesPage />);
    const table = await screen.findByRole('grid', { name: 'Weapons of the mod' });
    fireEvent.click(within(table).getByText('UC_RecurveBow'));
    fireEvent.click(await screen.findByRole('tab', { name: 'Options' }));
    const kind = screen.getByRole('radiogroup', { name: 'Treat as a bow' });
    fireEvent.click(kind.querySelectorAll('[role="radio"]')[1] as HTMLElement);
    await waitFor(() => expect(ownAnswers.value['UC_RecurveBow']?.block?.bow).toBe(true));
    const request = buildRequest(
      bow('UC_RecurveBow'),
      ownAnswers.value['UC_RecurveBow'],
      undefined,
    );
    expect(request.answers.overrides.bow).toBe(true);
  });
});

describe('answers beyond the plain asks', () => {
  const candidate = bow('UC_RecurveBow');

  it('keeps the block of a weapon apart from its family and lets the weapon win', () => {
    installTransport();
    const family = candidate.family ?? '';
    expect(family).not.toBe('');
    setBlock(candidate, 'family', { recoilPattern: 'Regular', extraTags: ['A'] });
    setBlock(candidate, 'weapon', { recoilPattern: 'Mounted' });
    expect(effectiveBlock(candidate)).toEqual({ recoilPattern: 'Mounted', extraTags: ['A'] });
    setBlock(candidate, 'weapon', { recoilPattern: undefined });
    expect(effectiveBlock(candidate).recoilPattern).toBe('Regular');
  });

  it('sends the nested under barrel answers and the skip choice to the backend', () => {
    installTransport();
    const ammo = {
      field: '/ce/underBarrel/ammoSet',
      label: 'ammo',
      kind: 'choice' as const,
      options: ['AmmoSet_Grenade'],
    };
    const range = {
      field: '/ce/underBarrel/range',
      label: 'range',
      kind: 'number' as const,
      options: [],
    };
    setAnswer(candidate, 'weapon', ammo, 'AmmoSet_Grenade');
    setAnswer(candidate, 'weapon', range, 25);
    setSkipUnderBarrel(candidate, 'weapon', true);
    const request = buildRequest(candidate, ownAnswers.value[candidate.defName], undefined);
    expect(request.answers.skipUnderBarrel).toBe(true);
    expect(request.answers.overrides).toMatchObject({
      underBarrel: { ammoSet: 'AmmoSet_Grenade', range: { value: 25, source: 'answered' } },
    });
    const withoutSkip = ownAnswers.value[candidate.defName];
    setSkipUnderBarrel(candidate, 'weapon', false);
    expect(
      buildRequest(candidate, ownAnswers.value[candidate.defName], undefined).answers
        .skipUnderBarrel,
    ).toBeUndefined();
    expect(withoutSkip?.skipUnderBarrel).toBe(true);
  });

  it('shows the skip choice on the questions of a weapon that has under barrel asks', () => {
    installTransport();
    const withUnit = {
      ...candidate,
      asks: [
        {
          field: '/ce/underBarrel/range',
          label: 'Unit range',
          kind: 'number' as const,
          options: [],
          reason: 'no other unit to learn from',
        },
      ],
    };
    renderWithProviders(<QuestionsForm candidate={withUnit} familySize={1} />);
    expect(screen.getByText('no other unit to learn from')).toBeTruthy();
    fireEvent.click(
      screen.getByRole('switch', { name: 'Leave the under barrel unit out of the conversion' }),
    );
    expect(ownAnswers.value[candidate.defName]?.skipUnderBarrel).toBe(true);
    const range = screen.getByRole('spinbutton', { name: 'Unit range' });
    fireEvent.input(range, { target: { value: '30' } });
    fireEvent.blur(range);
    expect(ownAnswers.value[candidate.defName]?.numbers['/ce/underBarrel/range']).toBe(30);
  });
});
