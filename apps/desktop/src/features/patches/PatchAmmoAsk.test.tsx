import { fireEvent, render, screen, waitFor, within } from '@testing-library/preact';
import { loadFixture } from 'rimstudio-testkit';
import type { ConvertRequestDto, ConvertScanDto } from 'rimstudio-ipc-types';
import { beforeEach, describe, expect, it } from 'vitest';
import { catalogSlice, customAmmo, suggestionFmj } from '~/shared/ammo/testSupport';
import { clearAmmoSummaries } from '~/shared/ammo/useAmmoSummary';
import { requestWithCustomAmmo } from './ammoRequest';
import { effectiveBlock, familyAnswers, ownAnswers, resetAnswers, setAnswer } from './answerStore';
import { QuestionsForm } from './QuestionsForm';
import { installTransport } from './testSupport';

const candidate = loadFixture<ConvertScanDto>('designer_convert_scan').candidates[0];
if (!candidate) throw new Error('fixture');
const ammoAsk = candidate.asks.find((a) => a.field === '/ce/ammoSet');
if (!ammoAsk) throw new Error('fixture has no ammo ask');
const project = { projectId: 'p-1', name: 'Mod', path: '/tmp/mod' };

function setup() {
  return installTransport({
    designer_ce_ammo_catalog: () => catalogSlice(),
    designer_ce_ammo_suggest: () => suggestionFmj(),
  });
}

describe('the ammo question of the Patches page', () => {
  beforeEach(() => {
    resetAnswers();
    clearAmmoSummaries();
  });

  it('is the field with the browser and the quick picks', () => {
    setup();
    render(<QuestionsForm candidate={candidate} familySize={1} project={project} />);
    expect(screen.getByRole('button', { name: 'Browse all ammo' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Create custom ammo' })).toBeTruthy();
  });

  it('answers the question with a set chosen in the browser', async () => {
    setup();
    render(<QuestionsForm candidate={candidate} familySize={1} project={project} />);
    fireEvent.click(screen.getByRole('button', { name: 'Browse all ammo' }));
    const dialog = await screen.findByRole('dialog', { name: 'Combat Extended ammunition' });
    await within(dialog).findByText('22 ammo sets');
    fireEvent.click(within(dialog).getByRole('button', { name: 'Select this set' }));
    const first = catalogSlice().entries[0]?.defName;
    await waitFor(() => expect(ownAnswers.value['OH_G41m']?.ammoSet).toBe(first));
    expect(await screen.findByText('1 of 9 answered')).toBeTruthy();
  });

  it('keeps custom ammunition in the answers, counts the question as answered and clears the set', async () => {
    setup();
    setAnswer(candidate, 'weapon', ammoAsk, 'AmmoSet_280British');
    render(<QuestionsForm candidate={candidate} familySize={1} project={project} />);
    fireEvent.click(screen.getByRole('button', { name: 'Create custom ammo' }));
    const win = await screen.findByRole('dialog', { name: 'Create custom ammunition' });
    fireEvent.input(within(win).getByRole('textbox', { name: /^Name/ }), {
      target: { value: 'Six' },
    });
    fireEvent.click(within(win).getByRole('tab', { name: /^Ammo types/ }));
    fireEvent.click(within(win).getByRole('button', { name: 'Add an ammo type' }));
    await waitFor(() =>
      expect(
        (within(win).getByRole('button', { name: 'Save custom ammo' }) as HTMLButtonElement)
          .disabled,
      ).toBe(false),
    );
    fireEvent.click(within(win).getByRole('button', { name: 'Save custom ammo' }));
    expect(effectiveBlock(candidate).customAmmo?.name).toBe('Six');
    expect(ownAnswers.value['OH_G41m']?.ammoSet).toBeUndefined();
    expect(await screen.findByText('1 of 9 answered')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Remove custom ammo' }));
    expect(effectiveBlock(candidate).customAmmo).toBeUndefined();
  });

  it('does not offer a new custom caliber without an open project', () => {
    setup();
    render(<QuestionsForm candidate={candidate} familySize={1} />);
    expect(screen.queryByRole('button', { name: 'Create custom ammo' })).toBeNull();
  });

  it('sends the custom ammunition in the overrides of the request that checks it', () => {
    setup();
    setAnswer(candidate, 'weapon', ammoAsk, 'AmmoSet_280British');
    const request: ConvertRequestDto = requestWithCustomAmmo(candidate, 'weapon', customAmmo());
    expect(request.defName).toBe(candidate.defName);
    expect(request.answers.overrides.customAmmo?.name).toBe(customAmmo().name);
    expect(request.answers.ammoSet).toBeUndefined();
    // the answers on the page are not touched
    expect(ownAnswers.value['OH_G41m']?.ammoSet).toBe('AmmoSet_280British');
  });

  it('puts the custom ammunition into the family group when the family scope is edited', () => {
    setup();
    const withFamily = { ...candidate, family: 'ranged/Gun/Bullet_X' };
    const request = requestWithCustomAmmo(withFamily, 'family', customAmmo());
    expect(request.groups?.[0]?.family).toBe('ranged/Gun/Bullet_X');
    expect(JSON.stringify(request.groups?.[0]?.answers)).toContain(customAmmo().name);
    expect(request.answers.overrides.customAmmo).toBeUndefined();
    expect(familyAnswers.value).toEqual({});
  });
});
