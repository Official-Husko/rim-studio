import { fireEvent, screen, within } from '@testing-library/preact';
import type { DraftDto, DraftEntryDto } from 'rimstudio-ipc-types';
import { renderWithProviders } from 'rimstudio-testkit';
import { afterEach, describe, expect, it } from 'vitest';
import { setupOutput, type OutputSetup } from '../../output-testSupport';
import { fixture } from '../../testSupport';
import { CeSection } from './CeSection';

let stop: (() => void) | undefined;
afterEach(() => {
  stop?.();
  stop = undefined;
});

function entryOf(name: string): DraftEntryDto {
  const draft = fixture<DraftDto>(name);
  return {
    id: 'd-ce',
    defName: draft.spec.identity.defName,
    label: draft.spec.identity.label,
    kind: 'ranged',
    updatedAtMs: 1,
    draft,
  };
}

function Harness({ setup }: { setup: OutputSetup }) {
  const draft = setup.editor.draft.value;
  if (!draft) return null;
  return <CeSection store={setup.output} spec={draft.spec} onGoTo={() => undefined} />;
}

async function show(draftName: string, suggestName: string, planName: string) {
  const setup = setupOutput({
    designer_ce_suggest: () => fixture(suggestName),
    designer_export_plan: () => fixture(planName),
  });
  stop = setup.output.start();
  setup.editor.open(entryOf(draftName));
  await setup.flush();
  renderWithProviders(<Harness setup={setup} />);
  return setup;
}

describe('Combat Extended extras in the output panel', () => {
  it('offers the tool list the conversions suggest and writes it only when taken', async () => {
    const setup = await show(
      'designer-ce-draft-gun-answered',
      'designer-ce-suggest-gun-answered',
      'designer-ce-plan-gun-extras',
    );
    expect(screen.getByText('More Combat Extended options')).toBeTruthy();
    const card = document.querySelector('[data-option="tool-plan"]') as HTMLElement;
    expect(within(card).getByText('3 of 3 weapons')).toBeTruthy();
    expect(setup.editor.draft.value?.spec.ce?.toolPlan).toBeUndefined();
    fireEvent.click(within(card).getByRole('button', { name: /^Take: / }));
    const plan = setup.editor.draft.value?.spec.ce?.toolPlan ?? [];
    expect(plan.map((tool) => tool.label)).toEqual(['stock', 'barrel', 'muzzle']);
    expect(plan[2]?.from).toBe('barrel');
  });

  it('turns the weapon platform on and adds an attachment link to the draft', async () => {
    const setup = await show(
      'designer-ce-draft-gun-answered',
      'designer-ce-suggest-gun-answered',
      'designer-ce-plan-gun-extras',
    );
    fireEvent.click(screen.getByRole('switch', { name: 'This weapon is a platform' }));
    expect(setup.editor.draft.value?.spec.ce?.isWeaponPlatform).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: 'Add an attachment' }));
    expect(setup.editor.draft.value?.spec.ce?.attachmentLinks).toEqual([{ attachment: '' }]);
    fireEvent.input(screen.getByRole('textbox', { name: 'Attachment definition' }), {
      target: { value: 'RS_Scope' },
    });
    expect(setup.editor.draft.value?.spec.ce?.attachmentLinks?.[0]?.attachment).toBe('RS_Scope');
  });

  it('adds an under barrel unit and removes it again, leaving the rest of the block alone', async () => {
    const setup = await show(
      'designer-ce-draft-gun-answered',
      'designer-ce-suggest-gun-answered',
      'designer-ce-plan-gun-extras',
    );
    const before = setup.editor.draft.value?.spec.ce?.ammoSet;
    fireEvent.click(screen.getByRole('switch', { name: 'This weapon has an under barrel unit' }));
    expect(setup.editor.draft.value?.spec.ce?.underBarrel).toEqual({
      oneAmmoHolder: false,
      requiresReload: false,
    });
    fireEvent.click(screen.getByRole('switch', { name: 'This weapon has an under barrel unit' }));
    expect(setup.editor.draft.value?.spec.ce?.underBarrel).toBeUndefined();
    expect(setup.editor.draft.value?.spec.ce?.ammoSet).toBe(before);
  });

  it('shows what a recorded draft with every extra holds', async () => {
    await show(
      'designer-ce-draft-gun-extras',
      'designer-ce-suggest-gun-answered',
      'designer-ce-plan-gun-extras',
    );
    expect(screen.getByText('1 attachment, 1 part')).toBeTruthy();
    expect(screen.getByText('3 tools')).toBeTruthy();
    expect(screen.getByText('1 element')).toBeTruthy();
    expect(screen.getAllByText('AmmoSet_25x40mmGrenade').length).toBeGreaterThan(0);
  });

  it('keeps the bow ammo set among the arrow sets with the bow numbers asked', async () => {
    await show(
      'designer-ce-draft-bow-on',
      'designer-ce-suggest-bow-on',
      'designer-ce-plan-bow-ready',
    );
    expect(screen.getAllByRole('combobox', { name: /ammo set/i }).length).toBeGreaterThan(0);
    expect(screen.getByRole('radiogroup', { name: 'Treat as a bow' })).toBeTruthy();
  });
});
